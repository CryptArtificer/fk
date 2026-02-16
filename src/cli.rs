use std::env;
use std::process;

#[derive(Debug)]
pub struct Args {
    pub field_separator: Option<String>,
    pub assignments: Vec<(String, String)>,
    pub program: String,
    pub files: Vec<String>,
}

pub fn parse_args() -> Args {
    let args: Vec<String> = env::args().skip(1).collect();

    let mut field_separator: Option<String> = None;
    let mut assignments: Vec<(String, String)> = Vec::new();
    let mut program: Option<String> = None;
    let mut files: Vec<String> = Vec::new();

    let mut i = 0;
    while i < args.len() {
        let arg = &args[i];

        if arg == "--" {
            i += 1;
            // Everything after -- is a file operand
            while i < args.len() {
                files.push(args[i].clone());
                i += 1;
            }
            break;
        }

        if arg == "-F" {
            i += 1;
            if i >= args.len() {
                eprintln!("fk: -F requires an argument");
                process::exit(1);
            }
            field_separator = Some(args[i].clone());
        } else if arg.starts_with("-F") {
            field_separator = Some(arg[2..].to_string());
        } else if arg == "-v" {
            i += 1;
            if i >= args.len() {
                eprintln!("fk: -v requires an argument");
                process::exit(1);
            }
            match parse_assignment(&args[i]) {
                Some(pair) => assignments.push(pair),
                None => {
                    eprintln!("fk: invalid -v assignment: {}", args[i]);
                    process::exit(1);
                }
            }
        } else if arg.starts_with("-v") {
            match parse_assignment(&arg[2..]) {
                Some(pair) => assignments.push(pair),
                None => {
                    eprintln!("fk: invalid -v assignment: {}", &arg[2..]);
                    process::exit(1);
                }
            }
        } else if arg.starts_with('-') && arg.len() > 1 {
            eprintln!("fk: unknown option: {}", arg);
            process::exit(1);
        } else if program.is_none() {
            program = Some(arg.clone());
        } else {
            files.push(arg.clone());
        }

        i += 1;
    }

    let program = match program {
        Some(p) => p,
        None => {
            eprintln!("usage: fk [-F fs] [-v var=val] 'program' [file ...]");
            process::exit(1);
        }
    };

    Args {
        field_separator,
        assignments,
        program,
        files,
    }
}

fn parse_assignment(s: &str) -> Option<(String, String)> {
    let eq = s.find('=')?;
    if eq == 0 {
        return None;
    }
    let name = &s[..eq];
    if !is_valid_ident(name) {
        return None;
    }
    let value = &s[eq + 1..];
    Some((name.to_string(), value.to_string()))
}

fn is_valid_ident(s: &str) -> bool {
    let mut chars = s.chars();
    match chars.next() {
        Some(c) if c.is_ascii_alphabetic() || c == '_' => {}
        _ => return false,
    }
    chars.all(|c| c.is_ascii_alphanumeric() || c == '_')
}
