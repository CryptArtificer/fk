use super::{to_number, format_number};

/// Dispatch string built-in functions.
pub fn call(name: &str, args: &[String]) -> String {
    match name {
        "length" => {
            let s = args.first().map(|s| s.as_str()).unwrap_or("");
            format_number(s.chars().count() as f64)
        }
        "substr" => {
            let s = args.first().map(|s| s.as_str()).unwrap_or("");
            let start = args.get(1).map(|s| to_number(s) as usize).unwrap_or(1);
            let start = if start > 0 { start - 1 } else { 0 };
            let char_count = s.chars().count();
            if start >= char_count {
                return String::new();
            }
            if let Some(len_str) = args.get(2) {
                let len = to_number(len_str) as usize;
                s.chars().skip(start).take(len).collect()
            } else {
                s.chars().skip(start).collect()
            }
        }
        "index" => {
            let s = args.first().map(|s| s.as_str()).unwrap_or("");
            let target = args.get(1).map(|s| s.as_str()).unwrap_or("");
            match s.find(target) {
                Some(byte_pos) => {
                    let char_pos = s[..byte_pos].chars().count() + 1;
                    format_number(char_pos as f64)
                }
                None => "0".to_string(),
            }
        }
        "tolower" => {
            let s = args.first().map(|s| s.as_str()).unwrap_or("");
            s.to_lowercase()
        }
        "toupper" => {
            let s = args.first().map(|s| s.as_str()).unwrap_or("");
            s.to_uppercase()
        }
        "trim" => {
            let s = args.first().map(|s| s.as_str()).unwrap_or("");
            s.trim().to_string()
        }
        "ltrim" => {
            let s = args.first().map(|s| s.as_str()).unwrap_or("");
            s.trim_start().to_string()
        }
        "rtrim" => {
            let s = args.first().map(|s| s.as_str()).unwrap_or("");
            s.trim_end().to_string()
        }
        "startswith" => {
            let s = args.first().map(|s| s.as_str()).unwrap_or("");
            let prefix = args.get(1).map(|s| s.as_str()).unwrap_or("");
            format_number(if s.starts_with(prefix) { 1.0 } else { 0.0 })
        }
        "endswith" => {
            let s = args.first().map(|s| s.as_str()).unwrap_or("");
            let suffix = args.get(1).map(|s| s.as_str()).unwrap_or("");
            format_number(if s.ends_with(suffix) { 1.0 } else { 0.0 })
        }
        "repeat" => {
            let s = args.first().map(|s| s.as_str()).unwrap_or("");
            let n = args.get(1).map(|s| to_number(s) as usize).unwrap_or(0);
            s.repeat(n)
        }
        "reverse" => {
            let s = args.first().map(|s| s.as_str()).unwrap_or("");
            s.chars().rev().collect()
        }
        "chr" => {
            let n = args.first().map(|s| to_number(s) as u32).unwrap_or(0);
            char::from_u32(n).map(|c| c.to_string()).unwrap_or_default()
        }
        "ord" => {
            let s = args.first().map(|s| s.as_str()).unwrap_or("");
            format_number(s.chars().next().map(|c| c as u32 as f64).unwrap_or(0.0))
        }
        "hex" => {
            let n = args.first().map(|s| to_number(s) as i64).unwrap_or(0);
            format!("{:#x}", n)
        }
        "lpad" => {
            let s = args.first().map(|s| s.as_str()).unwrap_or("");
            let width = args.get(1).map(|s| to_number(s) as usize).unwrap_or(0);
            let pad = args.get(2).and_then(|s| s.chars().next()).unwrap_or(' ');
            let chars: usize = s.chars().count();
            if chars >= width { s.to_string() } else {
                let mut out = String::with_capacity(width);
                for _ in 0..(width - chars) { out.push(pad); }
                out.push_str(s);
                out
            }
        }
        "rpad" => {
            let s = args.first().map(|s| s.as_str()).unwrap_or("");
            let width = args.get(1).map(|s| to_number(s) as usize).unwrap_or(0);
            let pad = args.get(2).and_then(|s| s.chars().next()).unwrap_or(' ');
            let chars: usize = s.chars().count();
            if chars >= width { s.to_string() } else {
                let mut out = String::with_capacity(width);
                out.push_str(s);
                for _ in 0..(width - chars) { out.push(pad); }
                out
            }
        }
        _ => String::new(),
    }
}

/// Replace first or all occurrences of a pattern in a string.
/// Returns (new_string, replacement_count).
pub fn string_replace(s: &str, pattern: &str, replacement: &str, global: bool) -> (String, usize) {
    if pattern.is_empty() {
        return (s.to_string(), 0);
    }
    if global {
        let count = s.matches(pattern).count();
        (s.replace(pattern, replacement), count)
    } else if let Some(pos) = s.find(pattern) {
        let mut result = String::with_capacity(s.len());
        result.push_str(&s[..pos]);
        result.push_str(replacement);
        result.push_str(&s[pos + pattern.len()..]);
        (result, 1)
    } else {
        (s.to_string(), 0)
    }
}
