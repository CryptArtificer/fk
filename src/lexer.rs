use crate::error::Span;

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
    Print,
    Printf,
    If,
    Else,
    While,
    For,
    In,
    Delete,
    Function,
    Return,
    Getline,

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

/// A token with its source location.
#[derive(Debug, Clone)]
pub struct Spanned {
    pub token: Token,
    pub span: Span,
}

pub struct Lexer {
    input: Vec<char>,
    pos: usize,
}

impl Lexer {
    pub fn new(input: &str) -> Self {
        Lexer {
            input: input.chars().collect(),
            pos: 0,
        }
    }

    /// Compute the line and column for a given byte position.
    fn span_at(&self, pos: usize) -> Span {
        let mut line = 1;
        let mut col = 1;
        for i in 0..pos.min(self.input.len()) {
            if self.input[i] == '\n' {
                line += 1;
                col = 1;
            } else {
                col += 1;
            }
        }
        Span::new(line, col)
    }

    pub fn tokenize(&mut self) -> Result<Vec<Spanned>, String> {
        let mut tokens = Vec::new();

        loop {
            self.skip_whitespace();
            if self.pos >= self.input.len() {
                let span = self.span_at(self.pos);
                tokens.push(Spanned { token: Token::Eof, span });
                break;
            }

            let span = self.span_at(self.pos);
            let ch = self.input[self.pos];

            // Comments
            if ch == '#' {
                while self.pos < self.input.len() && self.input[self.pos] != '\n' {
                    self.pos += 1;
                }
                continue;
            }

            let token = match ch {
                '\n' => { self.pos += 1; Token::Newline }
                '{' => { self.pos += 1; Token::LBrace }
                '}' => { self.pos += 1; Token::RBrace }
                '(' => { self.pos += 1; Token::LParen }
                ')' => { self.pos += 1; Token::RParen }
                '[' => { self.pos += 1; Token::LBracket }
                ']' => { self.pos += 1; Token::RBracket }
                ';' => { self.pos += 1; Token::Semicolon }
                ',' => { self.pos += 1; Token::Comma }
                '+' => {
                    self.pos += 1;
                    if self.peek() == Some('=') {
                        self.pos += 1;
                        Token::PlusAssign
                    } else if self.peek() == Some('+') {
                        self.pos += 1;
                        Token::Increment
                    } else {
                        Token::Plus
                    }
                }
                '-' => {
                    self.pos += 1;
                    if self.peek() == Some('=') {
                        self.pos += 1;
                        Token::MinusAssign
                    } else if self.peek() == Some('-') {
                        self.pos += 1;
                        Token::Decrement
                    } else {
                        Token::Minus
                    }
                }
                '*' => {
                    self.pos += 1;
                    if self.peek() == Some('*') {
                        self.pos += 1;
                        Token::Power
                    } else if self.peek() == Some('=') {
                        self.pos += 1;
                        Token::StarAssign
                    } else {
                        Token::Star
                    }
                }
                '%' => {
                    self.pos += 1;
                    if self.peek() == Some('=') {
                        self.pos += 1;
                        Token::PercentAssign
                    } else {
                        Token::Percent
                    }
                }
                '/' => {
                    if self.is_regex_context(&tokens) {
                        self.read_regex()?
                    } else {
                        self.pos += 1;
                        if self.peek() == Some('=') {
                            self.pos += 1;
                            Token::SlashAssign
                        } else {
                            Token::Slash
                        }
                    }
                }
                '=' => {
                    self.pos += 1;
                    if self.peek() == Some('=') {
                        self.pos += 1;
                        Token::Eq
                    } else {
                        Token::Assign
                    }
                }
                '!' => {
                    self.pos += 1;
                    if self.peek() == Some('=') {
                        self.pos += 1;
                        Token::Ne
                    } else if self.peek() == Some('~') {
                        self.pos += 1;
                        Token::NotMatch
                    } else {
                        Token::Not
                    }
                }
                '<' => {
                    self.pos += 1;
                    if self.peek() == Some('=') {
                        self.pos += 1;
                        Token::Le
                    } else {
                        Token::Lt
                    }
                }
                '>' => {
                    self.pos += 1;
                    if self.peek() == Some('>') {
                        self.pos += 1;
                        Token::Append
                    } else if self.peek() == Some('=') {
                        self.pos += 1;
                        Token::Ge
                    } else {
                        Token::Gt
                    }
                }
                '~' => { self.pos += 1; Token::Match }
                '?' => { self.pos += 1; Token::Question }
                ':' => { self.pos += 1; Token::Colon }
                '&' => {
                    self.pos += 1;
                    if self.peek() == Some('&') {
                        self.pos += 1;
                        Token::And
                    } else {
                        return Err(format!("{}: unexpected character '&'", span));
                    }
                }
                '|' => {
                    self.pos += 1;
                    if self.peek() == Some('|') {
                        self.pos += 1;
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
                    return Err(format!("{}: unexpected character '{}'", span, ch));
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
                self.pos += 1;
            } else {
                break;
            }
        }
    }

    fn is_regex_context(&self, tokens: &[Spanned]) -> bool {
        match tokens.last().map(|s| &s.token) {
            None => true,
            Some(Token::LBrace) | Some(Token::Semicolon) | Some(Token::Newline)
            | Some(Token::And) | Some(Token::Or) | Some(Token::Not)
            | Some(Token::LParen) | Some(Token::Comma)
            | Some(Token::Match) | Some(Token::NotMatch) => true,
            _ => false,
        }
    }

    fn read_regex(&mut self) -> Result<Token, String> {
        let span = self.span_at(self.pos);
        self.pos += 1; // skip opening /
        let mut pattern = String::new();
        loop {
            if self.pos >= self.input.len() {
                return Err(format!("{}: unterminated regex", span));
            }
            let ch = self.input[self.pos];
            if ch == '/' {
                self.pos += 1;
                return Ok(Token::Regex(pattern));
            }
            if ch == '\\' && self.pos + 1 < self.input.len() {
                pattern.push(self.input[self.pos + 1]);
                self.pos += 2;
            } else {
                pattern.push(ch);
                self.pos += 1;
            }
        }
    }

    fn read_string(&mut self) -> Result<Token, String> {
        let span = self.span_at(self.pos);
        self.pos += 1; // skip opening "
        let mut s = String::new();
        loop {
            if self.pos >= self.input.len() {
                return Err(format!("{}: unterminated string", span));
            }
            let ch = self.input[self.pos];
            if ch == '"' {
                self.pos += 1;
                return Ok(Token::StringLit(s));
            }
            if ch == '\\' && self.pos + 1 < self.input.len() {
                self.pos += 1;
                let escaped = self.input[self.pos];
                match escaped {
                    'n' => { s.push('\n'); self.pos += 1; }
                    't' => { s.push('\t'); self.pos += 1; }
                    '\\' => { s.push('\\'); self.pos += 1; }
                    '"' => { s.push('"'); self.pos += 1; }
                    '/' => { s.push('/'); self.pos += 1; }
                    'x' => {
                        self.pos += 1; // skip 'x'
                        if let Some(ch) = self.read_hex_escape(2) {
                            s.push(ch);
                        } else {
                            s.push_str("\\x");
                        }
                    }
                    'u' => {
                        self.pos += 1; // skip 'u'
                        if let Some(ch) = self.read_hex_escape(4) {
                            s.push(ch);
                        } else {
                            s.push_str("\\u");
                        }
                    }
                    _ => {
                        s.push('\\');
                        s.push(escaped);
                        self.pos += 1;
                    }
                }
            } else {
                s.push(ch);
                self.pos += 1;
            }
        }
    }

    fn read_field(&mut self) -> Result<Token, String> {
        let span = self.span_at(self.pos);
        self.pos += 1; // skip $
        if self.pos < self.input.len() && self.input[self.pos].is_ascii_digit() {
            let start = self.pos;
            while self.pos < self.input.len() && self.input[self.pos].is_ascii_digit() {
                self.pos += 1;
            }
            let num: u32 = self.input[start..self.pos]
                .iter()
                .collect::<String>()
                .parse()
                .map_err(|_| format!("{}: invalid field number", span))?;
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

    fn read_number(&mut self) -> Result<Token, String> {
        let span = self.span_at(self.pos);
        let start = self.pos;

        // Hex literal: 0x...
        if self.input[self.pos] == '0'
            && self.pos + 1 < self.input.len()
            && (self.input[self.pos + 1] == 'x' || self.input[self.pos + 1] == 'X')
        {
            self.pos += 2; // skip "0x"
            let hex_start = self.pos;
            while self.pos < self.input.len() && self.input[self.pos].is_ascii_hexdigit() {
                self.pos += 1;
            }
            if self.pos == hex_start {
                return Err(format!("{}: expected hex digits after 0x", span));
            }
            let hex: String = self.input[hex_start..self.pos].iter().collect();
            let num = u64::from_str_radix(&hex, 16)
                .map_err(|_| format!("{}: invalid hex literal: 0x{}", span, hex))?;
            return Ok(Token::Number(num as f64));
        }

        let mut has_dot = false;
        while self.pos < self.input.len() {
            let ch = self.input[self.pos];
            if ch.is_ascii_digit() {
                self.pos += 1;
            } else if ch == '.' && !has_dot {
                has_dot = true;
                self.pos += 1;
            } else {
                break;
            }
        }
        let s: String = self.input[start..self.pos].iter().collect();
        let num: f64 = s.parse().map_err(|_| format!("{}: invalid number: {}", span, s))?;
        Ok(Token::Number(num))
    }

    fn read_ident(&mut self) -> Token {
        let s = self.read_ident_str();
        match s.as_str() {
            "BEGIN" => Token::Begin,
            "END" => Token::End,
            "print" => Token::Print,
            "printf" => Token::Printf,
            "if" => Token::If,
            "else" => Token::Else,
            "while" => Token::While,
            "for" => Token::For,
            "in" => Token::In,
            "delete" => Token::Delete,
            "function" => Token::Function,
            "return" => Token::Return,
            "getline" => Token::Getline,
            _ => Token::Ident(s),
        }
    }

    /// Read exactly `count` hex digits and return the corresponding char.
    /// Returns None if not enough hex digits are available.
    fn read_hex_escape(&mut self, count: usize) -> Option<char> {
        let start = self.pos;
        let mut digits = 0;
        while digits < count
            && self.pos < self.input.len()
            && self.input[self.pos].is_ascii_hexdigit()
        {
            self.pos += 1;
            digits += 1;
        }
        if digits < count {
            self.pos = start; // backtrack
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
            self.pos += 1;
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
