use std::fs::File;
use std::io::{self, BufRead, BufReader};

/// A unified reader that iterates over lines from stdin or a sequence of files,
/// similar to awk's implicit concatenation of input sources.
pub struct Input {
    sources: Vec<Source>,
    current: usize,
    reader: Option<Box<dyn BufRead>>,
}

enum Source {
    Stdin,
    File(String),
}

impl Input {
    /// Create an Input from a list of file paths.
    /// If the list is empty, read from stdin.
    pub fn new(files: &[String]) -> Self {
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
        }
    }

    /// Read the next line. Returns None at end of all input.
    pub fn next_line(&mut self) -> io::Result<Option<String>> {
        loop {
            // Open the next source if we don't have an active reader
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
            let mut line = String::new();
            let bytes = reader.read_line(&mut line)?;
            if bytes == 0 {
                // End of this source — move to the next
                self.reader = None;
                self.current += 1;
                continue;
            }

            // Strip the trailing newline (record separator)
            if line.ends_with('\n') {
                line.pop();
                if line.ends_with('\r') {
                    line.pop();
                }
            }

            return Ok(Some(line));
        }
    }
}
