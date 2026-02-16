mod action;
mod cli;
mod field;
mod input;
mod lexer;
mod parser;
mod runtime;

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

    // Execute
    let mut exec = action::Executor::new(&program, &mut rt);

    exec.run_begin();

    let mut input = input::Input::new(&args.files);
    loop {
        match input.next_line() {
            Ok(Some(line)) => exec.run_record(&line),
            Ok(None) => break,
            Err(e) => {
                eprintln!("{}", e);
                process::exit(1);
            }
        }
    }

    exec.run_end();
}
