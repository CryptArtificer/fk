use std::io::{self, BufRead};

use super::{Record, RecordReader};

/// RFC 4180 compliant CSV record reader.
///
/// Handles:
/// - Comma-separated fields (configurable delimiter)
/// - Double-quoted fields (embedded commas, newlines, escaped `""`)
/// - CRLF and LF line endings
pub struct CsvReader {
    delimiter: u8,
}

impl CsvReader {
    pub fn new(delimiter: u8) -> Self {
        CsvReader { delimiter }
    }

    /// Convenience constructor for standard CSV (comma-delimited).
    pub fn comma() -> Self {
        Self::new(b',')
    }

    /// Convenience constructor for TSV (tab-delimited).
    pub fn tab() -> Self {
        Self::new(b'\t')
    }
}

impl RecordReader for CsvReader {
    fn next_record(&mut self, reader: &mut dyn BufRead) -> io::Result<Option<Record>> {
        let mut raw = String::new();
        let bytes = reader.read_line(&mut raw)?;
        if bytes == 0 {
            return Ok(None);
        }

        // Handle multi-line records: if we have an odd number of unescaped
        // quotes, the record spans multiple lines.
        while !quotes_balanced(&raw) {
            let mut cont = String::new();
            let n = reader.read_line(&mut cont)?;
            if n == 0 {
                break;
            }
            raw.push_str(&cont);
        }

        // Strip trailing newline
        if raw.ends_with('\n') {
            raw.pop();
            if raw.ends_with('\r') {
                raw.pop();
            }
        }

        let fields = parse_fields(&raw, self.delimiter);
        let text = raw;

        Ok(Some(Record {
            text,
            fields: Some(fields),
        }))
    }
}

/// Check if all double-quotes are balanced (even count outside of escaped pairs).
fn quotes_balanced(s: &str) -> bool {
    let mut in_quotes = false;
    let chars: Vec<char> = s.chars().collect();
    let mut i = 0;
    while i < chars.len() {
        if chars[i] == '"' {
            if in_quotes {
                if i + 1 < chars.len() && chars[i + 1] == '"' {
                    i += 2; // escaped quote
                    continue;
                }
                in_quotes = false;
            } else {
                in_quotes = true;
            }
        }
        i += 1;
    }
    !in_quotes
}

/// Parse a single CSV/TSV line into fields.
fn parse_fields(line: &str, delimiter: u8) -> Vec<String> {
    let delim = delimiter as char;
    let mut fields = Vec::new();
    let mut field = String::new();
    let chars: Vec<char> = line.chars().collect();
    let mut i = 0;

    while i <= chars.len() {
        if i == chars.len() {
            fields.push(field);
            break;
        }
        if chars[i] == delim && !is_in_quoted_field(&chars, i) {
            fields.push(field);
            field = String::new();
            i += 1;
            continue;
        }
        if chars[i] == '"' {
            // Start of quoted field — consume until closing quote
            i += 1;
            while i < chars.len() {
                if chars[i] == '"' {
                    if i + 1 < chars.len() && chars[i + 1] == '"' {
                        field.push('"');
                        i += 2;
                    } else {
                        i += 1; // closing quote
                        break;
                    }
                } else {
                    field.push(chars[i]);
                    i += 1;
                }
            }
            continue;
        }
        field.push(chars[i]);
        i += 1;
    }

    fields
}

/// Determine if position `pos` is inside a quoted field.
/// Used to distinguish delimiters inside vs outside quotes.
fn is_in_quoted_field(chars: &[char], pos: usize) -> bool {
    let mut in_quotes = false;
    let mut i = 0;
    while i < pos {
        if chars[i] == '"' {
            if in_quotes {
                if i + 1 < chars.len() && chars[i + 1] == '"' {
                    i += 2;
                    continue;
                }
                in_quotes = false;
            } else {
                in_quotes = true;
            }
        }
        i += 1;
    }
    in_quotes
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn simple_csv() {
        let fields = parse_fields("a,b,c", b',');
        assert_eq!(fields, vec!["a", "b", "c"]);
    }

    #[test]
    fn quoted_field_with_comma() {
        let fields = parse_fields("a,\"b,c\",d", b',');
        assert_eq!(fields, vec!["a", "b,c", "d"]);
    }

    #[test]
    fn escaped_quotes() {
        let fields = parse_fields("a,\"he said \"\"hi\"\"\",c", b',');
        assert_eq!(fields, vec!["a", "he said \"hi\"", "c"]);
    }

    #[test]
    fn tab_delimited() {
        let fields = parse_fields("x\ty\tz", b'\t');
        assert_eq!(fields, vec!["x", "y", "z"]);
    }

    #[test]
    fn empty_fields_csv() {
        let fields = parse_fields("a,,c", b',');
        assert_eq!(fields, vec!["a", "", "c"]);
    }

    #[test]
    fn multiline_record() {
        // Simulate a multi-line CSV field by testing quotes_balanced
        assert!(!quotes_balanced("\"hello"));
        assert!(quotes_balanced("\"hello\""));
        assert!(!quotes_balanced("a,\"hello\nworld"));
        assert!(quotes_balanced("a,\"hello\nworld\""));
    }
}
