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
    pub program_files: Vec<String>,
    pub describe: bool,
    pub suggest: bool,
    pub highlight: bool,
    pub format: bool,
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
    let mut program_files: Vec<String> = Vec::new();
    let mut describe = false;
    let mut suggest = false;
    let mut highlight = false;
    let mut format = false;

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
            program_files.push(args[i].clone());
        } else if arg == "--repl" {
            repl = true;
        } else if arg == "--describe" || arg == "-d" {
            describe = true;
        } else if arg == "--suggest" || arg == "-S" {
            describe = true;
            suggest = true;
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
        } else if arg == "-h" || arg == "--help" {
            print_usage();
            process::exit(0);
        } else if arg == "--version" {
            println!("fk {}", env!("CARGO_PKG_VERSION"));
            process::exit(0);
        } else if arg == "--highlight" {
            highlight = true;
        } else if arg == "--format" {
            format = true;
        } else if arg.starts_with('-') && arg.len() > 1 {
            eprintln!("fk: unknown option: {}", arg);
            eprintln!("Try 'fk --help' for usage.");
            process::exit(1);
        } else if program.is_none() {
            program = Some(arg.clone());
        } else {
            files.push(arg.clone());
        }

        i += 1;
    }

    if field_separator.is_some() && input_mode != InputMode::Line {
        eprintln!("fk: -F is ignored when -i {} is set", match input_mode {
            InputMode::Csv => "csv",
            InputMode::Tsv => "tsv",
            InputMode::Json => "json",
            InputMode::Parquet => "parquet",
            InputMode::Line => unreachable!(),
        });
        process::exit(1);
    }

    // -f takes priority; if both -f and inline program given, inline becomes a file arg
    if !program_files.is_empty() {
        if let Some(p) = program {
            files.insert(0, p);
        }
        let mut parts = Vec::new();
        for pf in &program_files {
            match std::fs::read_to_string(pf) {
                Ok(contents) => parts.push(contents),
                Err(e) => {
                    eprintln!("fk: cannot read program file '{}': {}", pf, e);
                    process::exit(2);
                }
            }
        }
        program = Some(parts.join("\n"));
    }

    // In describe mode, all positional args are files, not a program (unless --highlight / --format)
    if describe && !highlight && !format
        && let Some(p) = program.take() {
            files.insert(0, p);
    }

    // If the "program" looks like a file path, treat all positional args
    // as files and default to `{ print }`.
    // Guard: only trigger when the arg looks path-like (contains '/' or '.')
    // to avoid false positives on short programs like `1` or `NR>5`.
    if program_files.is_empty() && !describe && !repl
        && let Some(ref p) = program
        && (p.contains('/') || p.contains('.'))
        && std::path::Path::new(p).exists()
    {
        files.insert(0, p.clone());
        program = Some("{ print }".to_string());
    }

    let program = match program {
        Some(p) => p,
        None if repl => String::new(),
        None if describe => String::new(),
        None => {
            print_usage();
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
        program_files,
        describe,
        suggest,
        highlight,
        format,
    }
}

fn print_usage() {
    eprintln!("fk {} — filter-kernel, a fast awk for structured data", env!("CARGO_PKG_VERSION"));
    eprintln!();
    eprintln!("Usage: fk [options] 'program' [file ...]");
    eprintln!("       fk [options] file ...              # defaults to {{ print }}");
    eprintln!();
    eprintln!("Options:");
    eprintln!("  -F fs            Field separator (implies line mode)");
    eprintln!("  -v var=val       Set variable (e.g. -v 'OFS=\\t')");
    eprintln!("  -f progfile      Read program from file (repeatable)");
    eprintln!("  -i mode          Input mode: csv, tsv, json, parquet");
    eprintln!("  -H               Header mode (skip header, enable $name)");
    eprintln!("  -d / -S          Describe / suggest mode");
    eprintln!("  --repl           Interactive mode");
    eprintln!("  --highlight      Print syntax-highlighted program to stdout and exit");
    eprintln!("  --format         Pretty-print program (indent, line breaks) and exit");
    eprintln!("  -h, --help       Show this help (see also: man fk)");
    eprintln!();
    eprintln!("  Format auto-detected from .csv/.tsv/.json extensions (+compression).");
    eprintln!("  Files without a program default to '{{ print }}' (OFS-joined fields).");
    eprintln!();
    eprintln!("Examples:");
    eprintln!("  fk data.csv                              # view CSV as columns");
    eprintln!("  fk -H -v 'OFS=\\t' data.csv               # skip header, tab output");
    eprintln!("  fk -H '{{ print $name, $age }}' data.csv");
    eprintln!("  fk '/error/ {{ print $2 }}' server.log");
    eprintln!("  fk -F: '!/^#/{{ u[$1]++ }} END {{ print u }}' /etc/passwd");
    eprintln!("    # split on ':', skip comments, unique users sorted");
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
    Some((name.to_string(), interpret_escapes(value)))
}

/// Interpret C-style escape sequences in a string (POSIX awk semantics for -v values).
fn interpret_escapes(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    let chars: Vec<char> = s.chars().collect();
    let mut i = 0;
    while i < chars.len() {
        if chars[i] == '\\' && i + 1 < chars.len() {
            match chars[i + 1] {
                'n' => { out.push('\n'); i += 2; }
                't' => { out.push('\t'); i += 2; }
                'r' => { out.push('\r'); i += 2; }
                'a' => { out.push('\x07'); i += 2; }
                'b' => { out.push('\x08'); i += 2; }
                'f' => { out.push('\x0C'); i += 2; }
                'v' => { out.push('\x0B'); i += 2; }
                '\\' => { out.push('\\'); i += 2; }
                '"' => { out.push('"'); i += 2; }
                '/' => { out.push('/'); i += 2; }
                'x' => {
                    i += 2;
                    if let Some((ch, consumed)) = read_hex(&chars, i, 2) {
                        out.push(ch);
                        i += consumed;
                    } else {
                        out.push_str("\\x");
                    }
                }
                'u' => {
                    i += 2;
                    if let Some((ch, consumed)) = read_hex(&chars, i, 4) {
                        out.push(ch);
                        i += consumed;
                    } else {
                        out.push_str("\\u");
                    }
                }
                _ => {
                    out.push('\\');
                    out.push(chars[i + 1]);
                    i += 2;
                }
            }
        } else {
            out.push(chars[i]);
            i += 1;
        }
    }
    out
}

fn read_hex(chars: &[char], start: usize, count: usize) -> Option<(char, usize)> {
    if start + count > chars.len() {
        return None;
    }
    let hex: String = chars[start..start + count].iter().collect();
    let code = u32::from_str_radix(&hex, 16).ok()?;
    let ch = char::from_u32(code)?;
    Some((ch, count))
}

fn is_valid_ident(s: &str) -> bool {
    let mut chars = s.chars();
    match chars.next() {
        Some(c) if c.is_ascii_alphabetic() || c == '_' => {}
        _ => return false,
    }
    chars.all(|c| c.is_ascii_alphanumeric() || c == '_')
}
