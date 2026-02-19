use std::io::{self, BufRead, Write};

use crate::action::Executor;
use crate::input::Record;
use crate::lexer::Lexer;
use crate::parser::Parser;
use crate::runtime::Runtime;

/// Run an interactive REPL session.
///
/// Each line is lexed, parsed, and executed as a standalone program.
/// Runtime state (variables, arrays) persists across lines.
///
/// Commands:
///   :q / :quit   — exit
///   :reset       — clear all variables and arrays
///   :vars        — show all variables
pub fn run(rt: &mut Runtime) {
    let stdin = io::stdin();
    let mut reader = stdin.lock();

    loop {
        print!("fk> ");
        let _ = io::stdout().flush();

        let mut line = String::new();
        match reader.read_line(&mut line) {
            Ok(0) => break,
            Ok(_) => {}
            Err(e) => {
                eprintln!("fk: read error: {}", e);
                break;
            }
        }

        let trimmed = line.trim();
        if trimmed.is_empty() {
            continue;
        }

        match trimmed {
            ":q" | ":quit" => break,
            ":reset" => {
                *rt = Runtime::new();
                println!("(state cleared)");
                continue;
            }
            ":vars" => {
                print_vars(rt);
                continue;
            }
            _ => {}
        }

        // Collect continuation lines for unclosed braces
        let mut source = line.trim_end().to_string();
        while brace_depth(&source) > 0 {
            print!("... ");
            let _ = io::stdout().flush();
            let mut cont = String::new();
            match reader.read_line(&mut cont) {
                Ok(0) => break,
                Ok(_) => {
                    source.push('\n');
                    source.push_str(cont.trim_end());
                }
                Err(_) => break,
            }
        }

        // Try to parse as a full program (rules / BEGIN / END)
        let result = try_run_program(&source, rt);

        match result {
            Ok(output) => {
                if !output.is_empty() {
                    print!("{}", output);
                }
            }
            Err(e) => eprintln!("error: {}", e),
        }
    }
}

fn try_run_program(source: &str, rt: &mut Runtime) -> Result<String, crate::error::FkError> {
    let mut lex = Lexer::new(source);
    let tokens = lex.tokenize()?;
    let mut par = Parser::new(tokens);
    let program = par.parse()?;

    let mut exec = Executor::new(&program, rt);
    exec.run_begin();
    // In REPL, execute main rules once with an empty record
    if !program.rules.is_empty() {
        let rec = Record {
            text: String::new(),
            fields: None,
        };
        exec.run_record(&rec);
    }
    exec.run_end();

    Ok(String::new())
}

fn brace_depth(s: &str) -> i32 {
    let mut depth: i32 = 0;
    let mut in_string = false;
    let mut prev = '\0';
    for ch in s.chars() {
        if ch == '"' && prev != '\\' {
            in_string = !in_string;
        }
        if !in_string {
            match ch {
                '{' => depth += 1,
                '}' => depth -= 1,
                _ => {}
            }
        }
        prev = ch;
    }
    depth
}

fn print_vars(rt: &Runtime) {
    let names = rt.all_var_names();
    if names.is_empty() {
        println!("(no variables)");
        return;
    }
    for name in &names {
        println!("  {} = \"{}\"", name, rt.get_var(name));
    }
}
