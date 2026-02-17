/// Source location: line and column (both 1-indexed).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Span {
    pub line: usize,
    pub col: usize,
}

impl Span {
    #[must_use]
    pub fn new(line: usize, col: usize) -> Self {
        Span { line, col }
    }
}

impl std::fmt::Display for Span {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}:{}", self.line, self.col)
    }
}

/// Structured error with source location.
#[derive(Debug)]
pub struct FkError {
    pub span: Span,
    pub message: String,
}

impl FkError {
    #[must_use]
    pub fn new(span: Span, message: impl Into<String>) -> Self {
        FkError { span, message: message.into() }
    }
}

impl std::fmt::Display for FkError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}: {}", self.span, self.message)
    }
}

impl std::error::Error for FkError {}
