use std::env;
use std::process;

use fk::{action, cli, describe, input, lexer, parser, runtime, repl};
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
                break;
            }
        }
    }
}

fn main() {
    let args = cli::parse_args();

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
    if program.rules.is_empty() && args.files.is_empty() {
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
                        prev_filename = cur_filename;
                        exec.set_var("FILENAME", &prev_filename);
                        exec.reset_fnr();
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
                        exec.skip_input_source();
                        first_record = true;
                    }
                }
                Ok(None) => break,
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
