pub mod json;
pub mod math;
pub mod printf;
pub mod string;
pub mod time;

pub use self::printf::format_printf;
pub use self::string::string_replace;

/// Coerce a string to a number (awk semantics: leading numeric prefix is parsed,
/// non-numeric strings become 0).
pub fn to_number(s: &str) -> f64 {
    let s = s.trim();
    if s.is_empty() {
        return 0.0;
    }
    if let Ok(n) = s.parse::<f64>() {
        return n;
    }
    // Try leading numeric prefix (awk parses "123abc" as 123)
    let mut end = 0;
    let bytes = s.as_bytes();
    if end < bytes.len() && (bytes[end] == b'+' || bytes[end] == b'-') {
        end += 1;
    }
    let mut has_digit = false;
    while end < bytes.len() && bytes[end].is_ascii_digit() {
        end += 1;
        has_digit = true;
    }
    if end < bytes.len() && bytes[end] == b'.' {
        end += 1;
        while end < bytes.len() && bytes[end].is_ascii_digit() {
            end += 1;
            has_digit = true;
        }
    }
    if has_digit {
        s[..end].parse::<f64>().unwrap_or(0.0)
    } else {
        0.0
    }
}

/// Format a number using a printf-style format string (OFMT/CONVFMT).
/// Fast path for the default "%.6g".
pub fn format_number_fmt(n: f64, fmt: &str) -> String {
    if fmt == "%.6g" {
        return format_number(n);
    }
    printf::format_printf(fmt, &[format_number(n)])
}

/// Format a number for output (integer form when exact, otherwise up to 6 decimals).
pub fn format_number(n: f64) -> String {
    if n.is_nan() {
        return "nan".to_string();
    }
    if n.is_infinite() {
        return if n > 0.0 {
            "inf".to_string()
        } else {
            "-inf".to_string()
        };
    }
    if n == (n as i64) as f64 {
        (n as i64).to_string()
    } else {
        let s = format!("{:.6}", n);
        let s = s.trim_end_matches('0');
        let s = s.trim_end_matches('.');
        s.to_string()
    }
}

/// Dispatch pure built-in function calls (those that don't need runtime access).
pub fn call_builtin(name: &str, args: &[String]) -> String {
    match name {
        "length" | "substr" | "index" | "tolower" | "toupper" | "trim" | "ltrim" | "rtrim"
        | "startswith" | "endswith" | "repeat" | "reverse" | "chr" | "ord" | "hex" | "lpad"
        | "rpad" => string::call(name, args),
        "int" | "sin" | "cos" | "sqrt" | "log" | "exp" | "atan2" | "abs" | "ceil" | "floor"
        | "round" | "log2" | "log10" | "min" | "max" | "rand" | "srand" => math::call(name, args),
        "systime" | "strftime" | "mktime" | "parsedate" => time::call(name, args),
        "jpath" => json::call(args),
        _ => {
            eprintln!("fk: unknown function: {}", name);
            String::new()
        }
    }
}
