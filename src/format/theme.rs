//! Styling: map token kinds to output styles (ANSI, or placeholder for HTML etc.).
//!
//! **Constructs highlighted (each has its own style):**
//! - **Keyword** — BEGIN, END, BEGINFILE, ENDFILE, print, printf, if, else, while, for, do, in,
//!   delete, function, return, getline, nextfile, next, break, continue, exit
//! - **LiteralString** — `"..."` double-quoted strings
//! - **LiteralNumber** — numeric literals (e.g. 42, 3.14, 0xFF)
//! - **Regex** — `/pattern/` regex literals
//! - **Identifier** — user-defined variable and function names
//! - **BuiltinVar** — built-in variables (NR, NF, FS, FILENAME, etc.)
//! - **Field** — `$0`, `$1`, `$name`, bare `$`
//! - **Comment** — `#` to end of line
//! - **Operator** — `+ - * / % ** = += -= ~ !~ && || ? : >> |` etc.
//! - **Delimiter** — `{ } ( ) [ ] ; ,` and newline

use crate::lexer::Token;

/// Semantic style for a segment of source (keyword, string, comment, etc.).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Style {
    Keyword,
    LiteralString,
    LiteralNumber,
    Regex,
    Identifier,
    BuiltinVar,
    Field,
    Comment,
    Operator,
    Delimiter,
}

/// Something that can map a style to a prefix/suffix (e.g. ANSI codes).
pub trait Theme {
    /// Prefix to emit before a segment with this style (e.g. ANSI bold cyan).
    fn prefix(&self, style: Style) -> &str;
    /// Suffix to emit after the segment (e.g. reset).
    fn suffix(&self, style: Style) -> &str;
}

/// Built-in variable names (constants) — get a distinct style from user identifiers.
const BUILTIN_VARS: &[&str] = &[
    "ARGC", "ARGV", "CONVFMT", "ENVIRON", "FILENAME", "FNR", "FS", "NF", "NR", "OFMT", "OFS",
    "ORS", "RS", "SUBSEP",
];

/// Default ANSI theme for terminal output.
#[derive(Debug)]
pub struct AnsiTheme {
    reset: String,
    keyword: String,
    string: String,
    number: String,
    regex: String,
    ident: String,
    builtin_var: String,
    field: String,
    comment: String,
    operator: String,
    delimiter: String,
}

impl AnsiTheme {
    /// Theme with richer colors (One Dark / Dracula–inspired) for dark backgrounds.
    #[must_use]
    pub fn dark() -> Self {
        Self {
            reset: "\x1b[0m".into(),
            keyword: "\x1b[38;5;208m".into(), // orange (BEGIN, if, print, etc.)
            string: "\x1b[38;5;113m".into(),  // green
            number: "\x1b[38;5;179m".into(),  // gold/amber
            regex: "\x1b[38;5;170m".into(),   // purple/magenta
            ident: "\x1b[38;5;223m".into(),   // light sand (variables)
            builtin_var: "\x1b[38;5;117m".into(), // light blue (NR, NF, FS, etc.)
            field: "\x1b[1;38;5;221m".into(), // bold yellow ($1, $name)
            comment: "\x1b[38;5;246m".into(), // gray
            operator: "\x1b[38;5;81m".into(), // cyan
            delimiter: "\x1b[38;5;102m".into(), // dim gray (braces, parens)
        }
    }

    /// No ANSI codes (plain text).
    #[must_use]
    pub fn none() -> Self {
        Self {
            reset: String::new(),
            keyword: String::new(),
            string: String::new(),
            number: String::new(),
            regex: String::new(),
            ident: String::new(),
            builtin_var: String::new(),
            field: String::new(),
            comment: String::new(),
            operator: String::new(),
            delimiter: String::new(),
        }
    }

    fn style(&self, style: Style) -> &str {
        match style {
            Style::Keyword => self.keyword.as_str(),
            Style::LiteralString => self.string.as_str(),
            Style::LiteralNumber => self.number.as_str(),
            Style::Regex => self.regex.as_str(),
            Style::Identifier => self.ident.as_str(),
            Style::BuiltinVar => self.builtin_var.as_str(),
            Style::Field => self.field.as_str(),
            Style::Comment => self.comment.as_str(),
            Style::Operator => self.operator.as_str(),
            Style::Delimiter => self.delimiter.as_str(),
        }
    }
}

impl Theme for AnsiTheme {
    fn prefix(&self, style: Style) -> &str {
        self.style(style)
    }

    fn suffix(&self, _style: Style) -> &str {
        self.reset.as_str()
    }
}

/// Map a lexer token to a semantic style (no literal value used).
#[must_use]
pub fn token_style(t: &Token) -> Style {
    match t {
        Token::Begin
        | Token::End
        | Token::Beginfile
        | Token::Endfile
        | Token::Print
        | Token::Printf
        | Token::If
        | Token::Else
        | Token::While
        | Token::For
        | Token::Do
        | Token::In
        | Token::Delete
        | Token::Function
        | Token::Return
        | Token::Getline
        | Token::Nextfile
        | Token::Next
        | Token::Break
        | Token::Continue
        | Token::Exit => Style::Keyword,

        Token::StringLit(..) => Style::LiteralString,
        Token::Number(..) => Style::LiteralNumber,
        Token::Regex(..) => Style::Regex,

        Token::Ident(..) => {
            if t.as_ident_str()
                .is_some_and(|s| BUILTIN_VARS.binary_search(&s).is_ok())
            {
                Style::BuiltinVar
            } else {
                Style::Identifier
            }
        }
        Token::Field(..) | Token::FieldVar(..) | Token::Dollar => Style::Field,

        Token::Plus
        | Token::Minus
        | Token::Star
        | Token::Power
        | Token::Slash
        | Token::Percent
        | Token::Assign
        | Token::PlusAssign
        | Token::MinusAssign
        | Token::StarAssign
        | Token::SlashAssign
        | Token::PercentAssign
        | Token::Increment
        | Token::Decrement
        | Token::Eq
        | Token::Ne
        | Token::Lt
        | Token::Le
        | Token::Gt
        | Token::Ge
        | Token::Match
        | Token::NotMatch
        | Token::Not
        | Token::And
        | Token::Or
        | Token::Question
        | Token::Colon
        | Token::Append
        | Token::Pipe => Style::Operator,

        Token::LBrace
        | Token::RBrace
        | Token::LParen
        | Token::RParen
        | Token::LBracket
        | Token::RBracket
        | Token::Semicolon
        | Token::Comma
        | Token::Newline => Style::Delimiter,

        Token::Eof => Style::Delimiter, // not emitted as segment
    }
}
