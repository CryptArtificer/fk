use std::io::{self, BufRead};

use super::{Record, RecordReader};

/// JSON Lines record reader.
///
/// Each input line is a JSON object. Top-level string/number values become
/// fields, keyed by insertion order.  The raw line is preserved as `$0`.
pub struct JsonReader;

impl RecordReader for JsonReader {
    fn next_record(&mut self, reader: &mut dyn BufRead) -> io::Result<Option<Record>> {
        let mut line = String::new();
        let bytes = reader.read_line(&mut line)?;
        if bytes == 0 {
            return Ok(None);
        }
        if line.ends_with('\n') {
            line.pop();
            if line.ends_with('\r') {
                line.pop();
            }
        }

        let fields = parse_json_fields(&line);
        Ok(Some(Record {
            text: line,
            fields: Some(fields),
        }))
    }
}

/// Minimal JSON object parser — extracts top-level key-value pairs as strings.
/// Does not attempt full JSON compliance; handles the common case of flat
/// objects with string and number values.
fn parse_json_fields(s: &str) -> Vec<String> {
    let trimmed = s.trim();
    if !trimmed.starts_with('{') || !trimmed.ends_with('}') {
        return vec![trimmed.to_string()];
    }

    let inner = &trimmed[1..trimmed.len() - 1];
    let pairs = split_top_level(inner);
    let mut fields = Vec::new();

    for pair in pairs {
        let pair = pair.trim();
        if let Some(colon) = find_colon(pair) {
            let value = pair[colon + 1..].trim();
            fields.push(unquote(value));
        }
    }

    fields
}

/// Split a string by commas at the top level (not inside strings/objects/arrays).
fn split_top_level(s: &str) -> Vec<String> {
    let mut parts = Vec::new();
    let mut current = String::new();
    let mut in_string = false;
    let mut depth = 0;
    let mut prev = '\0';

    for ch in s.chars() {
        if ch == '"' && prev != '\\' {
            in_string = !in_string;
        }
        if !in_string {
            match ch {
                '{' | '[' => depth += 1,
                '}' | ']' => depth -= 1,
                ',' if depth == 0 => {
                    parts.push(current);
                    current = String::new();
                    prev = ch;
                    continue;
                }
                _ => {}
            }
        }
        current.push(ch);
        prev = ch;
    }

    if !current.is_empty() {
        parts.push(current);
    }
    parts
}

/// Find the colon separating key from value, respecting quoted strings.
fn find_colon(s: &str) -> Option<usize> {
    let mut in_string = false;
    let mut prev = '\0';
    for (i, ch) in s.chars().enumerate() {
        if ch == '"' && prev != '\\' {
            in_string = !in_string;
        }
        if !in_string && ch == ':' {
            return Some(i);
        }
        prev = ch;
    }
    None
}

/// Remove surrounding double quotes and unescape basic JSON escapes.
fn unquote(s: &str) -> String {
    let s = s.trim();
    if s.starts_with('"') && s.ends_with('"') && s.len() >= 2 {
        let inner = &s[1..s.len() - 1];
        let mut result = String::new();
        let mut chars = inner.chars();
        while let Some(ch) = chars.next() {
            if ch == '\\' {
                match chars.next() {
                    Some('"') => result.push('"'),
                    Some('\\') => result.push('\\'),
                    Some('/') => result.push('/'),
                    Some('n') => result.push('\n'),
                    Some('t') => result.push('\t'),
                    Some('r') => result.push('\r'),
                    Some(other) => {
                        result.push('\\');
                        result.push(other);
                    }
                    None => result.push('\\'),
                }
            } else {
                result.push(ch);
            }
        }
        result
    } else {
        // Numbers, booleans, null — return as-is
        s.to_string()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn flat_object() {
        let fields = parse_json_fields(r#"{"name":"Alice","age":30}"#);
        assert_eq!(fields, vec!["Alice", "30"]);
    }

    #[test]
    fn string_with_escapes() {
        let fields = parse_json_fields(r#"{"msg":"hello \"world\""}"#);
        assert_eq!(fields, vec!["hello \"world\""]);
    }

    #[test]
    fn nested_value_preserved() {
        let fields = parse_json_fields(r#"{"a":"x","b":{"c":1}}"#);
        assert_eq!(fields, vec!["x", r#"{"c":1}"#]);
    }

    #[test]
    fn non_object_becomes_single_field() {
        let fields = parse_json_fields("just a string");
        assert_eq!(fields, vec!["just a string"]);
    }
}
