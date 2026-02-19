//! Syntax highlighting: tokenize source, build styled segments, emit ANSI (or plain).

use super::theme::{Style, Theme, token_style};
use crate::error::{FkError, Span};
use crate::lexer::{Lexer, Spanned, Token};
use std::io::{self, Write};

/// (byte_start, byte_end, style) for a contiguous segment of source.
type Segment = (usize, usize, Style);

/// Build line starts: line_starts[i] = character offset of first char of line (i+1).
fn line_starts_char(source: &str) -> Vec<usize> {
    let mut starts = vec![0];
    for (i, c) in source.chars().enumerate() {
        if c == '\n' {
            starts.push(i + 1);
        }
    }
    starts
}

/// Character offset (0-based) for (line, col); line and col are 1-based.
fn span_to_char_offset(line_starts: &[usize], line: usize, col: usize) -> usize {
    if line == 0 {
        return 0;
    }
    let line_start = line_starts
        .get(line.saturating_sub(1))
        .copied()
        .unwrap_or(0);
    line_start + col.saturating_sub(1)
}

/// Build a single char-index → byte-offset table so we don't O(n) scan per token.
/// Table length is n_chars + 1; table[i] = byte offset of i-th char, table[n_chars] = source.len().
fn char_to_byte_table(source: &str) -> Vec<usize> {
    let n_chars = source.chars().count();
    let mut table = Vec::with_capacity(n_chars + 1);
    for (b, _) in source.char_indices() {
        table.push(b);
    }
    table.push(source.len());
    table
}

/// Token segments from lexer output: (byte_start, byte_end, style) for each token except Eof.
fn token_segments(source: &str, tokens: &[Spanned]) -> Result<Vec<Segment>, FkError> {
    let line_starts = line_starts_char(source);
    let char_to_byte = char_to_byte_table(source);
    let n_chars = char_to_byte.len().saturating_sub(1);

    let mut segs = Vec::new();
    for (i, s) in tokens.iter().enumerate() {
        if matches!(s.token, Token::Eof) {
            continue;
        }
        let start_char = span_to_char_offset(&line_starts, s.span.line, s.span.col);
        let end_char = tokens
            .get(i + 1)
            .map(|n| span_to_char_offset(&line_starts, n.span.line, n.span.col))
            .unwrap_or(n_chars);
        let byte_start = char_to_byte
            .get(start_char)
            .copied()
            .unwrap_or(source.len());
        let byte_end = char_to_byte.get(end_char).copied().unwrap_or(source.len());
        let style = token_style(&s.token);
        segs.push((byte_start, byte_end, style));
    }

    Ok(segs)
}

/// Comment segments: in ranges not covered by string segments, find # ... \n.
fn comment_segments(source: &str, token_segments: &[Segment]) -> Vec<Segment> {
    let string_ranges: Vec<(usize, usize)> = token_segments
        .iter()
        .filter(|(_, _, s)| *s == Style::LiteralString)
        .map(|(a, b, _)| (*a, *b))
        .collect();

    let in_string = |byte_off: usize| -> bool {
        string_ranges
            .iter()
            .any(|(a, b)| *a <= byte_off && byte_off < *b)
    };

    let mut segs = Vec::new();
    let bytes = source.as_bytes();
    let mut i = 0;

    while i < bytes.len() {
        if bytes[i] != b'#' {
            i += 1;
            continue;
        }
        if in_string(i) {
            i += 1;
            continue;
        }
        let start = i;
        while i < bytes.len() && bytes[i] != b'\n' {
            i += 1;
        }
        segs.push((start, i, Style::Comment));
        if i < bytes.len() {
            i += 1; // consume \n
        }
    }

    segs
}

/// Merge token and comment segments, sort by start, then by end (tokens before overlapping comment).
fn merge_segments(mut token_segs: Vec<Segment>, comment_segs: Vec<Segment>) -> Vec<Segment> {
    token_segs.extend(comment_segs);
    token_segs.sort_by_key(|(a, b, _)| (*a, *b));
    token_segs
}

/// Emit highlighted source to a string using the given theme.
pub fn highlight(source: &str) -> Result<String, FkError> {
    highlight_with_theme(source, &super::theme::AnsiTheme::dark())
}

/// Emit highlighted source with a specific theme.
pub fn highlight_with_theme<T: Theme>(source: &str, theme: &T) -> Result<String, FkError> {
    if source.is_empty() {
        return Ok(String::new());
    }

    let mut lexer = Lexer::new(source);
    let tokens = lexer.tokenize()?;
    let token_segs = token_segments(source, &tokens)?;
    let comment_segs = comment_segments(source, &token_segs);
    let segments = merge_segments(token_segs, comment_segs);

    let mut out = String::with_capacity(source.len() + segments.len() * 16); // rough ANSI overhead
    let mut pos = 0;

    for (a, b, style) in segments {
        if a > pos {
            out.push_str(&source[pos..a]);
        }
        if a < b {
            out.push_str(theme.prefix(style));
            out.push_str(&source[a..b]);
            out.push_str(theme.suffix(style));
        }
        pos = b;
    }
    if pos < source.len() {
        out.push_str(&source[pos..]);
    }

    Ok(out)
}

/// Write highlighted source to stderr (e.g. for showcase scripts that run `fk` and want to print the program).
pub fn highlight_to_stderr(source: &str) -> Result<(), FkError> {
    let s = highlight(source)?;
    io::stderr()
        .lock()
        .write_all(s.as_bytes())
        .map_err(|e| FkError::new(Span::new(0, 0), e.to_string()))?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::format::theme::AnsiTheme;

    #[test]
    fn highlight_simple() {
        let src = "{ print $1 }";
        let out = highlight(src).unwrap();
        assert!(out.contains("print")); // content preserved
        assert!(out.contains("\x1b[")); // has ANSI
    }

    #[test]
    fn highlight_empty() {
        assert_eq!(highlight("").unwrap(), "");
    }

    #[test]
    fn highlight_with_comment() {
        let src = "{ # hello\n  print $0 }";
        let out = highlight(src).unwrap();
        assert!(out.contains("# hello"));
        assert!(out.contains("print"));
    }

    #[test]
    fn theme_none_no_ansi() {
        let src = "BEGIN { print 1 }";
        let out = highlight_with_theme(src, &AnsiTheme::none()).unwrap();
        assert_eq!(out, src);
    }
}
