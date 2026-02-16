mod action;
mod builtins;
mod cli;
mod error;
mod field;
mod input;
mod lexer;
mod parser;
mod repl;
mod runtime;

#[cfg(test)]
mod tests;

use std::process;

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

    // REPL mode
    if args.repl {
        repl::run(&mut rt);
        return;
    }

    // Execute
    let mut exec = action::Executor::new(&program, &mut rt);

    exec.run_begin();

    // Select record reader based on input mode and RS (which may be set in BEGIN)
    let reader: Box<dyn input::RecordReader> = {
        let rs = exec.get_var("RS");
        if args.input_mode == cli::InputMode::Line && rs.len() > 1 {
            // Multi-char RS: treat as regex
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
    loop {
        match inp.next_record() {
            Ok(Some(record)) => {
                // Header mode: first record defines column names
                if args.header_mode && first_record {
                    first_record = false;
                    if let Some(fields) = &record.fields {
                        exec.set_header(fields);
                    } else {
                        exec.set_header_from_text(&record.text);
                    }
                    continue;
                }
                exec.run_record(&record);
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
}
