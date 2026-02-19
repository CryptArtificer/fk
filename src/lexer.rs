use crate::error::{FkError, Span};

/// Token types for the fk language.
#[derive(Debug, Clone, PartialEq)]
pub enum Token {
    // Literals
    Number(f64),
    StringLit(String),
    Regex(String),

    // Identifiers and fields
    Ident(String),
    Field(u32),         // $0, $1, ...
    FieldVar(String),   // $variable (resolved at runtime)
    Dollar,             // bare $ (followed by expression)

    // Keywords
    Begin,
    End,
    Beginfile,
    Endfile,
    Print,
    Printf,
    If,
    Else,
    While,
    For,
    Do,
    In,
    Delete,
    Function,
    Return,
    Getline,
    Nextfile,
    Next,
    Break,
    Continue,
    Exit,

    // Operators
    Plus,
    Minus,
    Star,
    Power,          // **
    Slash,
    Percent,
    Assign,
    PlusAssign,     // +=
    MinusAssign,    // -=
    StarAssign,     // *=
    SlashAssign,    // /=
    PercentAssign,  // %=
    Increment,      // ++
    Decrement,      // --
    Eq,
    Ne,
    Lt,
    Le,
    Gt,
    Ge,
    Match,       // ~
    NotMatch,    // !~
    Not,         // !
    And,         // &&
    Or,          // ||
    Question,    // ?
    Colon,       // :
    Append,      // >>
    Pipe,        // | (single, for output redirection)

    // Delimiters
    LBrace,
    RBrace,
    LParen,
    RParen,
    LBracket,
    RBracket,
    Semicolon,
    Comma,
    Newline,

    // Special
    Eof,
}

impl Token {
    /// Reference to the identifier string if this token is `Ident(_)`.
    #[must_use]
    pub fn as_ident_str(&self) -> Option<&str> {
        match self {
            Token::Ident(s) => Some(s.as_str()),
            _ => None,
        }
    }
}

/// A token with its source location.
#[derive(Debug, Clone)]
pub struct Spanned {
    pub token: Token,
    pub span: Span,
}

pub struct Lexer {
    input: Vec<char>,
    pos: usize,
    line: usize,
    col: usize,
}

impl Lexer {
    pub fn new(input: &str) -> Self {
        Lexer {
            input: input.chars().collect(),
            pos: 0,
            line: 1,
            col: 1,
        }
    }

    /// Current source location — O(1).
    fn span(&self) -> Span {
        Span::new(self.line, self.col)
    }

    /// Advance position by one character, updating line/col tracking.
    fn advance_char(&mut self) {
        if self.pos < self.input.len() {
            if self.input[self.pos] == '\n' {
                self.line += 1;
                self.col = 1;
            } else {
                self.col += 1;
            }
            self.pos += 1;
        }
    }

    pub fn tokenize(&mut self) -> Result<Vec<Spanned>, FkError> {
        let mut tokens = Vec::new();

        loop {
            self.skip_whitespace();
            if self.pos >= self.input.len() {
                let span = self.span();
                tokens.push(Spanned { token: Token::Eof, span });
                break;
            }

            let span = self.span();
            let ch = self.input[self.pos];

            // Comments
            if ch == '#' {
                while self.pos < self.input.len() && self.input[self.pos] != '\n' {
                    self.advance_char();
                }
                continue;
            }

            let token = match ch {
                '\n' => { self.advance_char(); Token::Newline }
                '{' => { self.advance_char(); Token::LBrace }
                '}' => { self.advance_char(); Token::RBrace }
                '(' => { self.advance_char(); Token::LParen }
                ')' => { self.advance_char(); Token::RParen }
                '[' => { self.advance_char(); Token::LBracket }
                ']' => { self.advance_char(); Token::RBracket }
                ';' => { self.advance_char(); Token::Semicolon }
                ',' => { self.advance_char(); Token::Comma }
                '+' => {
                    self.advance_char();
                    if self.peek() == Some('=') {
                        self.advance_char();
                        Token::PlusAssign
                    } else if self.peek() == Some('+') {
                        self.advance_char();
                        Token::Increment
                    } else {
                        Token::Plus
                    }
                }
                '-' => {
                    self.advance_char();
                    if self.peek() == Some('=') {
                        self.advance_char();
                        Token::MinusAssign
                    } else if self.peek() == Some('-') {
                        self.advance_char();
                        Token::Decrement
                    } else {
                        Token::Minus
                    }
                }
                '*' => {
                    self.advance_char();
                    if self.peek() == Some('*') {
                        self.advance_char();
                        Token::Power
                    } else if self.peek() == Some('=') {
                        self.advance_char();
                        Token::StarAssign
                    } else {
                        Token::Star
                    }
                }
                '%' => {
                    self.advance_char();
                    if self.peek() == Some('=') {
                        self.advance_char();
                        Token::PercentAssign
                    } else {
                        Token::Percent
                    }
                }
                '/' => {
                    if self.is_regex_context(&tokens) {
                        self.read_regex()?
                    } else {
                        self.advance_char();
                        if self.peek() == Some('=') {
                            self.advance_char();
                            Token::SlashAssign
                        } else {
                            Token::Slash
                        }
                    }
                }
                '=' => {
                    self.advance_char();
                    if self.peek() == Some('=') {
                        self.advance_char();
                        Token::Eq
                    } else {
                        Token::Assign
                    }
                }
                '!' => {
                    self.advance_char();
                    if self.peek() == Some('=') {
                        self.advance_char();
                        Token::Ne
                    } else if self.peek() == Some('~') {
                        self.advance_char();
                        Token::NotMatch
                    } else {
                        Token::Not
                    }
                }
                '<' => {
                    self.advance_char();
                    if self.peek() == Some('=') {
                        self.advance_char();
                        Token::Le
                    } else {
                        Token::Lt
                    }
                }
                '>' => {
                    self.advance_char();
                    if self.peek() == Some('>') {
                        self.advance_char();
                        Token::Append
                    } else if self.peek() == Some('=') {
                        self.advance_char();
                        Token::Ge
                    } else {
                        Token::Gt
                    }
                }
                '~' => { self.advance_char(); Token::Match }
                '?' => { self.advance_char(); Token::Question }
                ':' => { self.advance_char(); Token::Colon }
                '&' => {
                    self.advance_char();
                    if self.peek() == Some('&') {
                        self.advance_char();
                        Token::And
                    } else {
                        return Err(FkError::new(span, "unexpected character '&'"));
                    }
                }
                '|' => {
                    self.advance_char();
                    if self.peek() == Some('|') {
                        self.advance_char();
                        Token::Or
                    } else {
                        Token::Pipe
                    }
                }
                '"' => self.read_string()?,
                '$' => self.read_field()?,
                _ if ch.is_ascii_digit() || ch == '.' => self.read_number()?,
                _ if ch.is_ascii_alphabetic() || ch == '_' => self.read_ident(),
                _ => {
                    return Err(FkError::new(span, format!("unexpected character '{}'", ch)));
                }
            };

            tokens.push(Spanned { token, span });
        }

        Ok(tokens)
    }

    fn peek(&self) -> Option<char> {
        self.input.get(self.pos).copied()
    }

    fn skip_whitespace(&mut self) {
        while self.pos < self.input.len() {
            let ch = self.input[self.pos];
            if ch == ' ' || ch == '\t' || ch == '\r' {
                self.advance_char();
            } else {
                break;
            }
        }
    }

    fn is_regex_context(&self, tokens: &[Spanned]) -> bool {
        matches!(
            tokens.last().map(|s| &s.token),
            None | Some(Token::LBrace) | Some(Token::Semicolon) | Some(Token::Newline)
            | Some(Token::And) | Some(Token::Or) | Some(Token::Not)
            | Some(Token::LParen) | Some(Token::Comma)
            | Some(Token::Match) | Some(Token::NotMatch)
        )
    }

    fn read_regex(&mut self) -> Result<Token, FkError> {
        let span = self.span();
        self.advance_char(); // skip opening /
        let mut pattern = String::new();
        loop {
            if self.pos >= self.input.len() {
                return Err(FkError::new(span, "unterminated regex"));
            }
            let ch = self.input[self.pos];
            if ch == '/' {
                self.advance_char();
                return Ok(Token::Regex(pattern));
            }
            if ch == '\\' && self.pos + 1 < self.input.len() {
                pattern.push('\\');
                self.advance_char();
                pattern.push(self.input[self.pos]);
                self.advance_char();
            } else {
                pattern.push(ch);
                self.advance_char();
            }
        }
    }

    fn read_string(&mut self) -> Result<Token, FkError> {
        let span = self.span();
        self.advance_char(); // skip opening "
        let mut s = String::new();
        loop {
            if self.pos >= self.input.len() {
                return Err(FkError::new(span, "unterminated string"));
            }
            let ch = self.input[self.pos];
            if ch == '"' {
                self.advance_char();
                return Ok(Token::StringLit(s));
            }
            if ch == '\\' && self.pos + 1 < self.input.len() {
                self.advance_char();
                let escaped = self.input[self.pos];
                match escaped {
                    'n' => { s.push('\n'); self.advance_char(); }
                    't' => { s.push('\t'); self.advance_char(); }
                    '\\' => { s.push('\\'); self.advance_char(); }
                    '"' => { s.push('"'); self.advance_char(); }
                    '/' => { s.push('/'); self.advance_char(); }
                    'x' => {
                        self.advance_char(); // skip 'x'
                        if let Some(ch) = self.read_hex_escape(2) {
                            s.push(ch);
                        } else {
                            s.push_str("\\x");
                        }
                    }
                    'u' => {
                        self.advance_char(); // skip 'u'
                        if let Some(ch) = self.read_hex_escape(4) {
                            s.push(ch);
                        } else {
                            s.push_str("\\u");
                        }
                    }
                    _ => {
                        s.push('\\');
                        s.push(escaped);
                        self.advance_char();
                    }
                }
            } else {
                s.push(ch);
                self.advance_char();
            }
        }
    }

    fn read_field(&mut self) -> Result<Token, FkError> {
        let span = self.span();
        self.advance_char(); // skip $
        if self.pos < self.input.len() && self.input[self.pos].is_ascii_digit() {
            let start = self.pos;
            while self.pos < self.input.len() && self.input[self.pos].is_ascii_digit() {
                self.advance_char();
            }
            let num: u32 = self.input[start..self.pos]
                .iter()
                .collect::<String>()
                .parse()
                .map_err(|_| FkError::new(span, "invalid field number"))?;
            Ok(Token::Field(num))
        } else if self.pos < self.input.len()
            && (self.input[self.pos].is_ascii_alphabetic() || self.input[self.pos] == '_')
        {
            let ident = self.read_ident_str();
            Ok(Token::FieldVar(ident))
        } else {
            Ok(Token::Dollar)
        }
    }

    fn read_number(&mut self) -> Result<Token, FkError> {
        let span = self.span();
        let start = self.pos;

        // Hex literal: 0x...
        if self.input[self.pos] == '0'
            && self.pos + 1 < self.input.len()
            && (self.input[self.pos + 1] == 'x' || self.input[self.pos + 1] == 'X')
        {
            self.advance_char(); // '0'
            self.advance_char(); // 'x'
            let hex_start = self.pos;
            while self.pos < self.input.len() && self.input[self.pos].is_ascii_hexdigit() {
                self.advance_char();
            }
            if self.pos == hex_start {
                return Err(FkError::new(span, "expected hex digits after 0x"));
            }
            let hex: String = self.input[hex_start..self.pos].iter().collect();
            let num = u64::from_str_radix(&hex, 16)
                .map_err(|_| FkError::new(span, format!("invalid hex literal: 0x{}", hex)))?;
            return Ok(Token::Number(num as f64));
        }

        let mut has_dot = false;
        while self.pos < self.input.len() {
            let ch = self.input[self.pos];
            if ch.is_ascii_digit() {
                self.advance_char();
            } else if ch == '.' && !has_dot {
                has_dot = true;
                self.advance_char();
            } else {
                break;
            }
        }
        let s: String = self.input[start..self.pos].iter().collect();
        let num: f64 = s.parse().map_err(|_| FkError::new(span, format!("invalid number: {}", s)))?;
        Ok(Token::Number(num))
    }

    fn read_ident(&mut self) -> Token {
        let s = self.read_ident_str();
        match s.as_str() {
            "BEGIN" => Token::Begin,
            "END" => Token::End,
            "BEGINFILE" => Token::Beginfile,
            "ENDFILE" => Token::Endfile,
            "print" => Token::Print,
            "printf" => Token::Printf,
            "if" => Token::If,
            "else" => Token::Else,
            "while" => Token::While,
            "for" => Token::For,
            "in" => Token::In,
            "delete" => Token::Delete,
            "do" => Token::Do,
            "function" => Token::Function,
            "return" => Token::Return,
            "getline" => Token::Getline,
            "next" => Token::Next,
            "nextfile" => Token::Nextfile,
            "break" => Token::Break,
            "continue" => Token::Continue,
            "exit" => Token::Exit,
            _ => Token::Ident(s),
        }
    }

    /// Read exactly `count` hex digits and return the corresponding char.
    /// Returns None if not enough hex digits are available.
    fn read_hex_escape(&mut self, count: usize) -> Option<char> {
        let start = self.pos;
        let saved_line = self.line;
        let saved_col = self.col;
        let mut digits = 0;
        while digits < count
            && self.pos < self.input.len()
            && self.input[self.pos].is_ascii_hexdigit()
        {
            self.advance_char();
            digits += 1;
        }
        if digits < count {
            self.pos = start;
            self.line = saved_line;
            self.col = saved_col;
            return None;
        }
        let hex: String = self.input[start..self.pos].iter().collect();
        let code = u32::from_str_radix(&hex, 16).ok()?;
        char::from_u32(code)
    }

    fn read_ident_str(&mut self) -> String {
        let start = self.pos;
        while self.pos < self.input.len()
            && (self.input[self.pos].is_ascii_alphanumeric() || self.input[self.pos] == '_')
        {
            self.advance_char();
        }
        self.input[start..self.pos].iter().collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Extract bare tokens from spanned output for easy comparison.
    fn tokens(spanned: Vec<Spanned>) -> Vec<Token> {
        spanned.into_iter().map(|s| s.token).collect()
    }

    #[test]
    fn simple_print() {
        let mut lexer = Lexer::new("{ print $1 }");
        let toks = tokens(lexer.tokenize().unwrap());
        assert_eq!(
            toks,
            vec![Token::LBrace, Token::Print, Token::Field(1), Token::RBrace, Token::Eof]
        );
    }

    #[test]
    fn regex_pattern() {
        let mut lexer = Lexer::new("/error/ { print $0 }");
        let toks = tokens(lexer.tokenize().unwrap());
        assert_eq!(
            toks,
            vec![
                Token::Regex("error".to_string()),
                Token::LBrace,
                Token::Print,
                Token::Field(0),
                Token::RBrace,
                Token::Eof,
            ]
        );
    }

    #[test]
    fn begin_end() {
        let mut lexer = Lexer::new("BEGIN { print \"start\" } END { print \"done\" }");
        let toks = tokens(lexer.tokenize().unwrap());
        assert!(matches!(toks[0], Token::Begin));
        assert!(toks.iter().any(|t| matches!(t, Token::End)));
    }

    #[test]
    fn increment_decrement() {
        let mut lexer = Lexer::new("{ i++; j-- }");
        let toks = tokens(lexer.tokenize().unwrap());
        assert!(toks.contains(&Token::Increment));
        assert!(toks.contains(&Token::Decrement));
    }

    #[test]
    fn compound_assign() {
        let mut lexer = Lexer::new("{ x += 1; y -= 2; z *= 3; w /= 4; m %= 5 }");
        let toks = tokens(lexer.tokenize().unwrap());
        assert!(toks.contains(&Token::PlusAssign));
        assert!(toks.contains(&Token::MinusAssign));
        assert!(toks.contains(&Token::StarAssign));
        assert!(toks.contains(&Token::SlashAssign));
        assert!(toks.contains(&Token::PercentAssign));
    }

    #[test]
    fn array_access() {
        let mut lexer = Lexer::new("{ a[\"key\"] = 1 }");
        let toks = tokens(lexer.tokenize().unwrap());
        assert!(toks.contains(&Token::LBracket));
        assert!(toks.contains(&Token::RBracket));
    }

    #[test]
    fn keywords_phase1() {
        let mut lexer = Lexer::new("if else for while in delete printf");
        let toks = tokens(lexer.tokenize().unwrap());
        assert!(toks.contains(&Token::If));
        assert!(toks.contains(&Token::Else));
        assert!(toks.contains(&Token::For));
        assert!(toks.contains(&Token::While));
        assert!(toks.contains(&Token::In));
        assert!(toks.contains(&Token::Delete));
        assert!(toks.contains(&Token::Printf));
    }

    #[test]
    fn span_tracks_line_and_column() {
        let mut lexer = Lexer::new("{\n  print $1\n}");
        let spanned = lexer.tokenize().unwrap();
        // { at 1:1
        assert_eq!(spanned[0].span, Span::new(1, 1));
        // \n at 1:2
        assert_eq!(spanned[1].span, Span::new(1, 2));
        // print at 2:3
        assert_eq!(spanned[2].span, Span::new(2, 3));
        // $1 at 2:9
        assert_eq!(spanned[3].span, Span::new(2, 9));
        // \n at 2:11
        assert_eq!(spanned[4].span, Span::new(2, 11));
        // } at 3:1
        assert_eq!(spanned[5].span, Span::new(3, 1));
    }
}
