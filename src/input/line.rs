use std::io::{self, BufRead};

use super::{Record, RecordReader};

/// Default record reader: one record per line (awk's standard behaviour).
/// Reuses a read buffer across records to minimise allocations.
pub struct LineReader {
    buf: String,
}

impl Default for LineReader {
    fn default() -> Self {
        LineReader { buf: String::with_capacity(256) }
    }
}

impl LineReader {
    pub fn new() -> Self {
        Self::default()
    }
}

impl RecordReader for LineReader {
    fn next_record(&mut self, reader: &mut dyn BufRead) -> io::Result<Option<Record>> {
        self.buf.clear();
        let bytes = reader.read_line(&mut self.buf)?;
        if bytes == 0 {
            return Ok(None);
        }
        if self.buf.ends_with('\n') {
            self.buf.pop();
            if self.buf.ends_with('\r') {
                self.buf.pop();
            }
        }
        let text = self.buf.clone();
        Ok(Some(Record { text, fields: None }))
    }
}
