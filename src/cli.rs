use std::env;
use std::process;

#[derive(Debug, Clone, PartialEq)]
pub enum InputMode {
    Line,
    Csv,
    Tsv,
    Json,
    Parquet,
}

#[derive(Debug)]
pub struct Args {
    pub field_separator: Option<String>,
    pub assignments: Vec<(String, String)>,
    pub program: String,
    pub files: Vec<String>,
    pub repl: bool,
    pub input_mode: InputMode,
    pub header_mode: bool,
    pub program_file: Option<String>,
    pub describe: bool,
}

pub fn parse_args() -> Args {
    let args: Vec<String> = env::args().skip(1).collect();

    let mut field_separator: Option<String> = None;
    let mut assignments: Vec<(String, String)> = Vec::new();
    let mut program: Option<String> = None;
    let mut files: Vec<String> = Vec::new();
    let mut repl = false;
    let mut input_mode = InputMode::Line;
    let mut header_mode = false;
    let mut program_file: Option<String> = None;
    let mut describe = false;

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
        } else if let Some(fs) = arg.strip_prefix("-F") {
            field_separator = Some(fs.to_string());
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
        } else if let Some(rest) = arg.strip_prefix("-v") {
            match parse_assignment(rest) {
                Some(pair) => assignments.push(pair),
                None => {
                    eprintln!("fk: invalid -v assignment: {}", rest);
                    process::exit(1);
                }
            }
        } else if arg == "-f" {
            i += 1;
            if i >= args.len() {
                eprintln!("fk: -f requires an argument");
                process::exit(1);
            }
            program_file = Some(args[i].clone());
        } else if arg == "--repl" {
            repl = true;
        } else if arg == "--describe" || arg == "-d" {
            describe = true;
        } else if arg == "-H" || arg == "--header" {
            header_mode = true;
        } else if arg == "-i" {
            i += 1;
            if i >= args.len() {
                eprintln!("fk: -i requires an argument (csv, tsv, json, parquet)");
                process::exit(1);
            }
            input_mode = parse_input_mode(&args[i]);
        } else if arg.starts_with("-i") && arg.len() > 2 {
            input_mode = parse_input_mode(&arg[2..]);
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

    // -f takes priority; if both -f and inline program given, inline becomes a file arg
    if let Some(ref pf) = program_file {
        if let Some(p) = program {
            files.insert(0, p);
        }
        match std::fs::read_to_string(pf) {
            Ok(contents) => program = Some(contents),
            Err(e) => {
                eprintln!("fk: cannot read program file '{}': {}", pf, e);
                process::exit(2);
            }
        }
    }

    // In describe mode, all positional args are files, not a program
    if describe
        && let Some(p) = program.take() {
            files.insert(0, p);
    }

    let program = match program {
        Some(p) => p,
        None if repl => String::new(),
        None if describe => String::new(),
        None => {
            eprintln!("usage: fk [-F fs] [-v var=val] [-f progfile] 'program' [file ...]");
            eprintln!("       fk --describe [file ...]");
            eprintln!("       fk --repl");
            process::exit(1);
        }
    };

    Args {
        field_separator,
        assignments,
        program,
        files,
        repl,
        input_mode,
        header_mode,
        program_file,
        describe,
    }
}

fn parse_input_mode(s: &str) -> InputMode {
    match s {
        "csv" => InputMode::Csv,
        "tsv" => InputMode::Tsv,
        "json" => InputMode::Json,
        "parquet" => InputMode::Parquet,
        other => {
            eprintln!("fk: unknown input mode: {}", other);
            process::exit(1);
        }
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
