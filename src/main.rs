use std::env;
use std::process;
use std::io::Write;

use fk::{action, cli, describe, format, input, lexer, parser, runtime, repl};
use fk::builtins::format_number;

#[cfg(feature = "parquet")]
fn run_parquet(args: &cli::Args, exec: &mut action::Executor) {
    if args.files.is_empty() {
        eprintln!("fk: parquet mode requires file arguments (not stdin)");
        process::exit(2);
    }
    for path in &args.files {
        exec.set_var("FILENAME", path);
        exec.reset_fnr();
        exec.run_beginfile();
        if exec.should_exit().is_some() { return; }

        let (columns, rows) = match input::parquet_reader::read_parquet_file(path) {
            Ok(v) => v,
            Err(e) => {
                eprintln!("{}", e);
                process::exit(1);
            }
        };

        exec.set_header(&columns);
        exec.increment_fnr();

        for fields in rows {
            let text = fields.join(exec.get_var("OFS").as_str());
            let rec = input::Record { text, fields: Some(fields) };
            exec.increment_fnr();
            exec.run_record(&rec);
            if exec.should_exit().is_some() {
                return;
            }
            if exec.take_next_file() {
                exec.run_endfile();
                break;
            }
        }
        exec.run_endfile();
        if exec.should_exit().is_some() { return; }
    }
}

fn main() {
    let args = cli::parse_args();

    // Highlight mode: print syntax-highlighted program and exit
    if args.highlight {
        match format::highlight(&args.program) {
            Ok(s) => {
                print!("{}", s);
                return;
            }
            Err(e) => {
                eprintln!("fk: {}", e);
                process::exit(2);
            }
        }
    }

    // Format mode: pretty-print program and exit
    if args.format {
        match format::format_program(&args.program) {
            Ok(s) => {
                println!("{}", s);
                return;
            }
            Err(e) => {
                eprintln!("fk: {}", e);
                process::exit(2);
            }
        }
    }

    // Describe / suggest mode
    if args.describe {
        describe::run_describe(&args.files, args.suggest);
        return;
    }

    // Lex
    let mut lex = lexer::Lexer::new(&args.program);
    let tokens = match lex.tokenize() {
        Ok(t) => t,
        Err(e) => {
            eprintln!("fk: syntax error: {}", e);
            process::exit(2);
        }
    };

    // Parse
    let mut par = parser::Parser::new(tokens);
    let program = match par.parse() {
        Ok(p) => p,
        Err(e) => {
            eprintln!("fk: parse error: {}", e);
            process::exit(2);
        }
    };

    // Set up runtime
    let mut rt = runtime::Runtime::new();

    // Apply -F
    if let Some(ref fs) = args.field_separator {
        rt.set_var("FS", fs);
    }

    // Apply -v assignments
    for (name, value) in &args.assignments {
        rt.set_var(name, value);
    }

    // Populate ENVIRON array from process environment
    for (key, val) in env::vars() {
        rt.set_array("ENVIRON", &key, &val);
    }

    // Populate ARGC / ARGV from command-line args
    let raw_args: Vec<String> = env::args().collect();
    rt.set_var("ARGC", &format_number(raw_args.len() as f64));
    for (i, arg) in raw_args.iter().enumerate() {
        rt.set_array("ARGV", &i.to_string(), arg);
    }

    // REPL mode
    if args.repl {
        repl::run(&mut rt);
        return;
    }

    // Execute
    let mut exec = action::Executor::new(&program, &mut rt);

    exec.run_begin();

    // Early exit from BEGIN
    if let Some(code) = exec.should_exit() {
        exec.run_end();
        process::exit(code);
    }

    // BEGIN/END-only programs with no files: skip stdin (gawk behaviour)
    if program.rules.is_empty() && program.beginfile.is_none() && program.endfile.is_none()
        && args.files.is_empty() {
        exec.run_end();
        if let Some(code) = exec.should_exit() {
            process::exit(code);
        }
        return;
    }

    // Auto-detect input mode from first file extension when user didn't
    // specify -i *and* didn't specify -F (explicit -F implies line mode).
    let effective_mode = if args.input_mode == cli::InputMode::Line
        && args.field_separator.is_none()
        && !args.files.is_empty()
    {
        if let Some(fmt) = describe::format_from_extension(&args.files[0]) {
            match fmt {
                describe::Format::Csv => cli::InputMode::Csv,
                describe::Format::Tsv => cli::InputMode::Tsv,
                describe::Format::Json => cli::InputMode::Json,
                describe::Format::Space => cli::InputMode::Line,
                describe::Format::Parquet => cli::InputMode::Parquet,
            }
        } else {
            args.input_mode.clone()
        }
    } else {
        args.input_mode.clone()
    };

    // Fast path: END { print NR } with no rules (line counting).
    let fast_count_nr = program.begin.is_none()
        && program.beginfile.is_none()
        && program.endfile.is_none()
        && program.rules.is_empty()
        && program.functions.is_empty()
        && !args.header_mode
        && is_end_print_nr_only(&program);
    // Fast path: head-style NR>limit { exit } 1
    let fast_head_limit = if program.begin.is_none()
        && program.end.is_none()
        && program.beginfile.is_none()
        && program.endfile.is_none()
        && program.functions.is_empty()
        && !args.header_mode
    {
        head_print_limit(&program)
    } else {
        None
    };

    // Parquet mode: reads entire file upfront (not streaming)
    if effective_mode == cli::InputMode::Parquet {
        #[cfg(feature = "parquet")]
        {
            run_parquet(&args, &mut exec);
        }
        #[cfg(not(feature = "parquet"))]
        {
            eprintln!("fk: parquet support not compiled in. Rebuild with: cargo build --features parquet");
            process::exit(2);
        }
    } else if fast_count_nr {
        // Simple line/record count: avoid per-record runtime setup.
        let reader: Box<dyn input::RecordReader> = {
            let rs = exec.get_var("RS");
            if effective_mode == cli::InputMode::Line && rs.len() > 1 {
                match input::regex_rs::RegexReader::new(&rs) {
                    Ok(r) => Box::new(r),
                    Err(e) => {
                        eprintln!("fk: {}", e);
                        process::exit(2);
                    }
                }
            } else {
                match effective_mode {
                    cli::InputMode::Csv  => Box::new(input::csv::CsvReader::comma()),
                    cli::InputMode::Tsv  => Box::new(input::csv::CsvReader::tab()),
                    cli::InputMode::Json => Box::new(input::json::JsonReader),
                    cli::InputMode::Line => Box::new(input::line::LineReader::new()),
                    cli::InputMode::Parquet => unreachable!(),
                }
            }
        };

        let mut inp = input::Input::with_reader(&args.files, reader);
        loop {
            match inp.next_record() {
                Ok(Some(_)) => exec.increment_nr(),
                Ok(None) => break,
                Err(e) => {
                    eprintln!("{}", e);
                    process::exit(1);
                }
            }
        }
    } else if let Some(limit) = fast_head_limit {
        // Head-like program: print first N records and exit.
        let rs = exec.get_var("RS");
        let ors = exec.get_var("ORS");
        let mut out = std::io::BufWriter::new(std::io::stdout());
        if effective_mode == cli::InputMode::Line && rs.len() == 1 {
            let mut nr: u64 = 0;
            let sources = if args.files.is_empty() {
                vec!["-".to_string()]
            } else {
                args.files.clone()
            };
            for src in sources {
                let mut reader: Box<dyn std::io::BufRead> = if src == "-" {
                    Box::new(std::io::BufReader::new(std::io::stdin()))
                } else {
                    let r = describe::open_maybe_compressed(&src).map_err(|e| {
                        std::io::Error::new(e.kind(), format!("fk: {}: {}", src, e))
                    });
                    match r {
                        Ok(r) => Box::new(std::io::BufReader::new(r)),
                        Err(e) => {
                            eprintln!("{}", e);
                            process::exit(1);
                        }
                    }
                };

                let mut buf = String::with_capacity(256);
                loop {
                    buf.clear();
                    let bytes = match reader.read_line(&mut buf) {
                        Ok(n) => n,
                        Err(e) => {
                            eprintln!("{}", e);
                            process::exit(1);
                        }
                    };
                    if bytes == 0 { break; }
                    if buf.ends_with('\n') {
                        buf.pop();
                        if buf.ends_with('\r') {
                            buf.pop();
                        }
                    }
                    nr += 1;
                    if nr > limit { break; }
                    let _ = out.write_all(buf.as_bytes());
                    let _ = out.write_all(ors.as_bytes());
                }
                if nr >= limit { break; }
            }
        } else {
            let reader: Box<dyn input::RecordReader> = {
                if effective_mode == cli::InputMode::Line && rs.len() > 1 {
                    match input::regex_rs::RegexReader::new(&rs) {
                        Ok(r) => Box::new(r),
                        Err(e) => {
                            eprintln!("fk: {}", e);
                            process::exit(2);
                        }
                    }
                } else {
                    match effective_mode {
                        cli::InputMode::Csv  => Box::new(input::csv::CsvReader::comma()),
                        cli::InputMode::Tsv  => Box::new(input::csv::CsvReader::tab()),
                        cli::InputMode::Json => Box::new(input::json::JsonReader),
                        cli::InputMode::Line => Box::new(input::line::LineReader::new()),
                        cli::InputMode::Parquet => unreachable!(),
                    }
                }
            };

            let mut inp = input::Input::with_reader(&args.files, reader);
            let mut nr: u64 = 0;
            loop {
                match inp.next_record() {
                    Ok(Some(record)) => {
                        nr += 1;
                        if nr > limit { break; }
                        if !record.text.is_empty() {
                            let _ = out.write_all(record.text.as_bytes());
                        }
                        let _ = out.write_all(ors.as_bytes());
                    }
                    Ok(None) => break,
                    Err(e) => {
                        eprintln!("{}", e);
                        process::exit(1);
                    }
                }
            }
        }
        let _ = out.flush();
    } else {
        // Select record reader based on input mode and RS (which may be set in BEGIN)
        let reader: Box<dyn input::RecordReader> = {
            let rs = exec.get_var("RS");
            if effective_mode == cli::InputMode::Line && rs.len() > 1 {
                match input::regex_rs::RegexReader::new(&rs) {
                    Ok(r) => Box::new(r),
                    Err(e) => {
                        eprintln!("fk: {}", e);
                        process::exit(2);
                    }
                }
            } else {
                match effective_mode {
                    cli::InputMode::Csv  => Box::new(input::csv::CsvReader::comma()),
                    cli::InputMode::Tsv  => Box::new(input::csv::CsvReader::tab()),
                    cli::InputMode::Json => Box::new(input::json::JsonReader),
                    cli::InputMode::Line => Box::new(input::line::LineReader::new()),
                    cli::InputMode::Parquet => unreachable!(),
                }
            }
        };

        let inp = input::Input::with_reader(&args.files, reader);
        exec.set_input(inp);
        let mut first_record = true;
        let mut prev_filename = String::new();
        loop {
            match exec.next_record() {
                Ok(Some(record)) => {
                    let cur_filename = exec.current_filename().to_owned();
                    if cur_filename != prev_filename {
                        if !prev_filename.is_empty() {
                            exec.run_endfile();
                            if exec.should_exit().is_some() { break; }
                        }
                        prev_filename = cur_filename;
                        exec.set_var("FILENAME", &prev_filename);
                        exec.reset_fnr();
                        exec.run_beginfile();
                        if exec.should_exit().is_some() { break; }
                    }

                    if args.header_mode && first_record {
                        first_record = false;
                        if let Some(fields) = &record.fields {
                            exec.set_header(fields);
                        } else {
                            exec.set_header_from_text(&record.text);
                        }
                        exec.increment_fnr();
                        continue;
                    }
                    exec.increment_fnr();
                    exec.run_record(&record);
                    if exec.should_exit().is_some() {
                        break;
                    }
                    if exec.take_next_file() {
                        exec.run_endfile();
                        exec.skip_input_source();
                        first_record = true;
                        prev_filename.clear();
                        if exec.should_exit().is_some() { break; }
                    }
                }
                Ok(None) => {
                    if !prev_filename.is_empty() {
                        exec.run_endfile();
                    }
                    break;
                }
                Err(e) => {
                    eprintln!("{}", e);
                    process::exit(1);
                }
            }
        }
    }

    exec.run_end();
    if let Some(code) = exec.should_exit() {
        process::exit(code);
    }
}

fn is_end_print_nr_only(program: &parser::Program) -> bool {
    let end = match &program.end {
        Some(block) => block,
        None => return false,
    };
    if end.len() != 1 {
        return false;
    }
    match &end[0] {
        parser::Statement::Print(exprs, None) if exprs.len() == 1 => match &exprs[0] {
            parser::Expr::Var(name) => name == "NR",
            _ => false,
        },
        _ => false,
    }
}

fn head_print_limit(program: &parser::Program) -> Option<u64> {
    if program.rules.len() != 2 {
        return None;
    }
    let (first, second) = (&program.rules[0], &program.rules[1]);
    let limit = match &first.pattern {
        Some(parser::Pattern::Expression(parser::Expr::BinOp(left, op, right))) => {
            match (left.as_ref(), op, right.as_ref()) {
                (parser::Expr::Var(name), parser::BinOp::Gt, parser::Expr::NumberLit(n))
                    if name == "NR" && *n >= 0.0 && n.fract() == 0.0 => *n as u64,
                _ => return None,
            }
        }
        _ => return None,
    };
    let exit_only = matches!(
        first.action.as_slice(),
        [parser::Statement::Exit(None)]
    );
    if !exit_only {
        return None;
    }
    let print_default = match (&second.pattern, second.action.as_slice()) {
        (Some(parser::Pattern::Expression(expr)), [parser::Statement::Print(exprs, None)]) => {
            matches!(expr, parser::Expr::NumberLit(n) if *n != 0.0)
                && matches!(
                    exprs.as_slice(),
                    [parser::Expr::Field(inner)]
                        if matches!(inner.as_ref(), parser::Expr::NumberLit(n) if *n == 0.0)
                )
        }
        _ => false,
    };
    if print_default { Some(limit) } else { None }
}
