pub mod line;

use std::fs::File;
use std::io::{self, BufRead, BufReader};

/// Strategy for reading one record from a byte stream.
/// The default (`LineReader`) reads one line per record.
/// Future implementations (CSV, JSON, …) will override this.
pub trait RecordReader {
    fn next_record(&mut self, reader: &mut dyn BufRead) -> io::Result<Option<String>>;
}

/// A unified reader that iterates over records from stdin or a sequence of
/// files, similar to awk's implicit concatenation of input sources.
pub struct Input {
    sources: Vec<Source>,
    current: usize,
    reader: Option<Box<dyn BufRead>>,
    record_reader: Box<dyn RecordReader>,
}

enum Source {
    Stdin,
    File(String),
}

impl Input {
    /// Create an Input from a list of file paths using the default line reader.
    /// If the list is empty, read from stdin.
    pub fn new(files: &[String]) -> Self {
        Self::with_reader(files, Box::new(line::LineReader))
    }

    /// Create an Input with a custom record reader.
    pub fn with_reader(files: &[String], record_reader: Box<dyn RecordReader>) -> Self {
        let sources = if files.is_empty() {
            vec![Source::Stdin]
        } else {
            files
                .iter()
                .map(|f| {
                    if f == "-" {
                        Source::Stdin
                    } else {
                        Source::File(f.clone())
                    }
                })
                .collect()
        };

        Input {
            sources,
            current: 0,
            reader: None,
            record_reader,
        }
    }

    /// Read the next record. Returns None at end of all input.
    pub fn next_record(&mut self) -> io::Result<Option<String>> {
        loop {
            if self.reader.is_none() {
                if self.current >= self.sources.len() {
                    return Ok(None);
                }
                let reader: Box<dyn BufRead> = match &self.sources[self.current] {
                    Source::Stdin => Box::new(BufReader::new(io::stdin())),
                    Source::File(path) => {
                        let file = File::open(path).map_err(|e| {
                            io::Error::new(e.kind(), format!("fk: {}: {}", path, e))
                        })?;
                        Box::new(BufReader::new(file))
                    }
                };
                self.reader = Some(reader);
            }

            let reader = self.reader.as_mut().unwrap();
            match self.record_reader.next_record(reader.as_mut())? {
                Some(record) => return Ok(Some(record)),
                None => {
                    self.reader = None;
                    self.current += 1;
                }
            }
        }
    }
}
