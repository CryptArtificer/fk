pub mod csv;
pub mod json;
pub mod line;
pub mod regex_rs;
#[cfg(feature = "parquet")]
pub mod parquet_reader;

use std::io::{self, BufRead, BufReader};

/// A record returned by a `RecordReader`.
///
/// `text` is the raw record text (becomes `$0`).
/// `fields` is optionally pre-split fields — when `Some`, the runtime
/// uses these directly instead of FS-based splitting.
pub struct Record {
    pub text: String,
    pub fields: Option<Vec<String>>,
}

/// Strategy for reading one record from a byte stream.
/// The default (`LineReader`) reads one line per record.
/// CSV, TSV, and JSON readers override this.
pub trait RecordReader {
    fn next_record(&mut self, reader: &mut dyn BufRead) -> io::Result<Option<Record>>;
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

    /// Skip the rest of the current source and advance to the next one.
    pub fn skip_source(&mut self) {
        self.reader = None;
        self.current += 1;
    }

    /// Return the name of the current input source.
    pub fn current_filename(&self) -> &str {
        if self.current < self.sources.len() {
            match &self.sources[self.current] {
                Source::Stdin => "",
                Source::File(path) => path,
            }
        } else {
            ""
        }
    }

    /// Read the next record. Returns None at end of all input.
    pub fn next_record(&mut self) -> io::Result<Option<Record>> {
        loop {
            if self.reader.is_none() {
                if self.current >= self.sources.len() {
                    return Ok(None);
                }
                let reader: Box<dyn BufRead> = match &self.sources[self.current] {
                    Source::Stdin => Box::new(BufReader::new(io::stdin())),
                    Source::File(path) => {
                        let reader = crate::describe::open_maybe_compressed(path)
                            .map_err(|e| {
                                io::Error::new(e.kind(), format!("fk: {}: {}", path, e))
                            })?;
                        Box::new(BufReader::new(reader))
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
