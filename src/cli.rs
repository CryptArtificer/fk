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
    pub explain: bool,
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
    let mut explain = false;

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
            field_separator = Some(interpret_escapes(&args[i]));
        } else if let Some(fs) = arg.strip_prefix("-F") {
            field_separator = Some(interpret_escapes(fs));
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
        } else if arg == "--hierarchical-menu" {
            print_logo();
            process::exit(0);
        } else if arg == "--highlight" {
            highlight = true;
        } else if arg == "--format" {
            format = true;
        } else if arg == "--explain" {
            explain = true;
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
        eprintln!(
            "fk: -F is ignored when -i {} is set",
            match input_mode {
                InputMode::Csv => "csv",
                InputMode::Tsv => "tsv",
                InputMode::Json => "json",
                InputMode::Parquet => "parquet",
                InputMode::Line => unreachable!(),
            }
        );
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
    if describe
        && !highlight
        && !format
        && let Some(p) = program.take()
    {
        files.insert(0, p);
    }

    // If the "program" looks like a file path, treat all positional args
    // as files and default to `{ print }`.
    // Guard: only trigger when the arg looks path-like (contains '/' or '.')
    // to avoid false positives on short programs like `1` or `NR>5`.
    if program_files.is_empty()
        && !describe
        && !repl
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
        explain,
    }
}

fn print_usage() {
    eprintln!(
        "fk {} — filter-kernel, a fast awk for structured data",
        env!("CARGO_PKG_VERSION")
    );
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
    eprintln!("  --explain        Print a terse description of the program and exit");
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

fn print_logo() {
    // RLE-encoded Great Auk silhouette: s=space, b=body, w=white. Rows separated by '|'.
    const RLE: &str = "5s6w5s13w|3s3w4b7w11b4w|2s2w27b3w|s2w20b2w8b2w|2w21b3w8bw|w22b3w8b2w|5w30bw|4s3w28bw|6s7w3b2w5b6w6bw|12s3wb14w5bw|14sw2b13w5bw|14s2w2b12w5bw|15s2w2b7w2bw6b2w|16s2w2b6w10bw|17s2w2b6w9b3w|18s2wb6w11b2w|19s2wb5w12bw|20s2wb5w11b2w|21s2wb4w12b3w|22swb5w13b3w|22swb6w14b2w|22s2wb5w15b4w|22s2wb6w17b2w|22sw2b7w17b3w|22swb11w16b2w|22swb14w14b2w|22swb16w13b2w|22swb18w12b2w|22swb19w12b2w|22swb20w12b2w|22sw2b20w12bw|22s2wb21w11b2w|23swb22w11b2w|23sw2b21w12bw|23sw2b21w12b2w|23s2w2b20w13bw|24sw2b20w13b2w|24s2wb20w14b2w|25swb21w14bw|25s2wb7wb12w14b2w|26swb7wb12w15bw|26s2wb6w2b12w14bw|27sw3b4w2b12w14b2w|27s2w3b4wb13w14bw|28s2w3b3w2b12w14b2w|29sw4b3wb13w14bw|29s2w5b2wb13w13b2w|30s2w7b14w13bw|31s2w9b12w12b2w|32s2w10b11w12b2w|33s2w10b11w12b2w|34s2w10b11w12b2w|35sw11b11w12b2w|35s2w11b10w13b2w|36s2w11b10w13b2w|37sw12b9w14bw|37s2w12b8w14b2w|38sw13b8w14b2w|38s2w13b7w15b2w|39s2w13b7w12bw2b2w|40s2w12b7w13b2wbw|41s2w13b6w8b3wb4w|42s3w11b7w8b3wbw|38s8w11b6w8b5w|37s2w4b5w11b5w9bw|37sw8b3w11b5w8b2w|36s2w23b4w9b3w|36swb3w13b2w8b2w10b2w|36s5wb5w3b2w2b7w16b2w|39swb11w2b13w11bw|39s5w10bw11s13w|41sw12bw|40s2w12bw|40sw12b2w|40s3w10bw|41s2w9b2w|41sw10bw|41swb4w4b2w|41swb5w2b2w|41s3w2swb3w|46swbw|46swbw";

    fn decode_rle(rle: &str) -> Vec<Vec<u8>> {
        rle.split('|').map(|row| {
            let mut out = Vec::new();
            let bytes = row.as_bytes();
            let mut i = 0;
            while i < bytes.len() {
                let mut n: usize = 0;
                while i < bytes.len() && bytes[i].is_ascii_digit() {
                    n = n * 10 + (bytes[i] - b'0') as usize;
                    i += 1;
                }
                if n == 0 { n = 1; }
                if i < bytes.len() {
                    let v = match bytes[i] { b'b' => 2, b'w' => 1, _ => 0 };
                    out.extend(std::iter::repeat_n(v, n));
                    i += 1;
                }
            }
            out
        }).collect()
    }

    let rows = decode_rle(RLE);
    let blk = "\x1b[30m";
    let wht = "\x1b[97m";
    let rst = "\x1b[0m";
    println!("\n");
    for pair in rows.chunks(2) {
        let top = &pair[0];
        let bot = if pair.len() > 1 { &pair[1] } else { &vec![] };
        let w = top.len().max(bot.len());
        let mut line = String::new();
        for x in 0..w {
            let t = top.get(x).copied().unwrap_or(0);
            let b = bot.get(x).copied().unwrap_or(0);
            match (t, b) {
                (0, 0) => { line.push(' '); continue; }
                (2, 2) => { line.push_str(blk); line.push('█'); }
                (1, 1) => { line.push_str(wht); line.push('█'); }
                (2, 1) => { line.push_str("\x1b[30;107m▀"); }
                (1, 2) => { line.push_str("\x1b[97;40m▀"); }
                (2, 0) => { line.push_str(blk); line.push('▀'); }
                (0, 2) => { line.push_str(blk); line.push('▄'); }
                (1, 0) => { line.push_str(wht); line.push('▀'); }
                (0, 1) => { line.push_str(wht); line.push('▄'); }
                _ => { line.push(' '); continue; }
            }
            line.push_str(rst);
        }
        println!(" {}", line.trim_end());
    }
    let v = env!("CARGO_PKG_VERSION");
    println!(
        "\n\x1b[2m  fk {v} — filter-kernel\n  in memory of the Great Auk\x1b[0m\n"
    );
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
                'n' => {
                    out.push('\n');
                    i += 2;
                }
                't' => {
                    out.push('\t');
                    i += 2;
                }
                'r' => {
                    out.push('\r');
                    i += 2;
                }
                'a' => {
                    out.push('\x07');
                    i += 2;
                }
                'b' => {
                    out.push('\x08');
                    i += 2;
                }
                'f' => {
                    out.push('\x0C');
                    i += 2;
                }
                'v' => {
                    out.push('\x0B');
                    i += 2;
                }
                '\\' => {
                    out.push('\\');
                    i += 2;
                }
                '"' => {
                    out.push('"');
                    i += 2;
                }
                '/' => {
                    out.push('/');
                    i += 2;
                }
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
