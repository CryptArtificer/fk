use std::env;
use std::process;

use fk::{action, cli, input, lexer, parser, runtime, repl};
use fk::builtins::format_number;

fn main() {
    let args = cli::parse_args();

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

    // Select record reader based on input mode and RS (which may be set in BEGIN)
    let reader: Box<dyn input::RecordReader> = {
        let rs = exec.get_var("RS");
        if args.input_mode == cli::InputMode::Line && rs.len() > 1 {
            match input::regex_rs::RegexReader::new(&rs) {
                Ok(r) => Box::new(r),
                Err(e) => {
                    eprintln!("fk: {}", e);
                    process::exit(2);
                }
            }
        } else {
            match args.input_mode {
                cli::InputMode::Csv  => Box::new(input::csv::CsvReader::comma()),
                cli::InputMode::Tsv  => Box::new(input::csv::CsvReader::tab()),
                cli::InputMode::Json => Box::new(input::json::JsonReader),
                cli::InputMode::Line => Box::new(input::line::LineReader),
            }
        }
    };

    let mut inp = input::Input::with_reader(&args.files, reader);
    let mut first_record = true;
    let mut prev_filename = String::new();
    loop {
        match inp.next_record() {
            Ok(Some(record)) => {
                // Track FILENAME and FNR after reading (current points to actual source)
                let cur_filename = inp.current_filename().to_owned();
                if cur_filename != prev_filename {
                    exec.set_var("FILENAME", &cur_filename);
                    exec.reset_fnr();
                    prev_filename = cur_filename;
                }

                // Header mode: first record defines column names
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
                    inp.skip_source();
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

    exec.run_end();
    if let Some(code) = exec.should_exit() {
        process::exit(code);
    }
}
