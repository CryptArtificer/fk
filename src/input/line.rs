use std::io::{self, BufRead};

use super::RecordReader;

/// Default record reader: one record per line (awk's standard behaviour).
pub struct LineReader;

impl RecordReader for LineReader {
    fn next_record(&mut self, reader: &mut dyn BufRead) -> io::Result<Option<String>> {
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
        Ok(Some(line))
    }
}
