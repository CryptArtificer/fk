use std::io::{self, BufRead};

use regex::Regex;

use super::{Record, RecordReader};

/// Record reader that splits input on a regex record separator (RS).
///
/// On first call per source, buffers the entire source and splits by the
/// RS pattern. Subsequent calls yield one record at a time.
pub struct RegexReader {
    pattern: Regex,
    buffer: Vec<String>,
    pos: usize,
}

impl RegexReader {
    pub fn new(pattern: &str) -> Result<Self, String> {
        let re = Regex::new(pattern).map_err(|e| format!("invalid RS regex: {}", e))?;
        Ok(RegexReader {
            pattern: re,
            buffer: Vec::new(),
            pos: 0,
        })
    }
}

impl RecordReader for RegexReader {
    fn next_record(&mut self, reader: &mut dyn BufRead) -> io::Result<Option<Record>> {
        // Buffer the source on first access (or after exhausting previous source)
        if self.buffer.is_empty() {
            let mut all = String::new();
            reader.read_to_string(&mut all)?;
            if all.is_empty() {
                return Ok(None);
            }
            // Trim a single trailing newline so it doesn't produce an empty final record
            if all.ends_with('\n') {
                all.pop();
                if all.ends_with('\r') {
                    all.pop();
                }
            }
            self.buffer = self.pattern.split(&all).map(|s| s.to_string()).collect();
            self.pos = 0;
        }

        if self.pos >= self.buffer.len() {
            self.buffer.clear();
            return Ok(None);
        }

        let text = self.buffer[self.pos].clone();
        self.pos += 1;
        Ok(Some(Record { text, fields: None }))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Cursor;

    #[test]
    fn split_on_double_newline() {
        let mut reader = RegexReader::new(r"\n\n").unwrap();
        let data = "a\nb\n\nc\nd\n";
        let mut cursor = Cursor::new(data);

        let r1 = reader.next_record(&mut cursor).unwrap().unwrap();
        assert_eq!(r1.text, "a\nb");
        let r2 = reader.next_record(&mut cursor).unwrap().unwrap();
        assert_eq!(r2.text, "c\nd");
        assert!(reader.next_record(&mut cursor).unwrap().is_none());
    }

    #[test]
    fn split_on_pipe_pattern() {
        let mut reader = RegexReader::new(r"\s*\|\s*").unwrap();
        let data = "alice | bob | carol\n";
        let mut cursor = Cursor::new(data);

        let r1 = reader.next_record(&mut cursor).unwrap().unwrap();
        assert_eq!(r1.text, "alice");
        let r2 = reader.next_record(&mut cursor).unwrap().unwrap();
        assert_eq!(r2.text, "bob");
        let r3 = reader.next_record(&mut cursor).unwrap().unwrap();
        assert_eq!(r3.text, "carol");
        assert!(reader.next_record(&mut cursor).unwrap().is_none());
    }
}
