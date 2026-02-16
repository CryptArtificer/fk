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

    // Keywords
    Begin,
    End,
    Print,
    If,
    Else,
    While,
    For,

    // Operators
    Plus,
    Minus,
    Star,
    Slash,
    Percent,
    Assign,
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
    Append,      // +=

    // Delimiters
    LBrace,
    RBrace,
    LParen,
    RParen,
    Semicolon,
    Comma,
    Newline,

    // Special
    Eof,
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

    pub fn tokenize(&mut self) -> Result<Vec<Token>, String> {
        let mut tokens = Vec::new();

        loop {
            self.skip_whitespace();
            if self.pos >= self.input.len() {
                tokens.push(Token::Eof);
                break;
            }

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
                ';' => { self.pos += 1; Token::Semicolon }
                ',' => { self.pos += 1; Token::Comma }
                '+' => {
                    self.pos += 1;
                    if self.peek() == Some('=') {
                        self.pos += 1;
                        Token::Append
                    } else {
                        Token::Plus
                    }
                }
                '-' => { self.pos += 1; Token::Minus }
                '*' => { self.pos += 1; Token::Star }
                '%' => { self.pos += 1; Token::Percent }
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
                    if self.peek() == Some('=') {
                        self.pos += 1;
                        Token::Ge
                    } else {
                        Token::Gt
                    }
                }
                '~' => { self.pos += 1; Token::Match }
                '&' => {
                    self.pos += 1;
                    if self.peek() == Some('&') {
                        self.pos += 1;
                        Token::And
                    } else {
                        return Err(format!("unexpected character '&' at position {}", self.pos - 1));
                    }
                }
                '|' => {
                    self.pos += 1;
                    if self.peek() == Some('|') {
                        self.pos += 1;
                        Token::Or
                    } else {
                        return Err(format!("unexpected character '|' at position {}", self.pos - 1));
                    }
                }
                '/' => {
                    // Regex literal: only when we expect a pattern (start of program,
                    // after { or ; or newline, or after && / ||).
                    if self.is_regex_context(&tokens) {
                        self.read_regex()?
                    } else {
                        self.pos += 1;
                        Token::Slash
                    }
                }
                '"' => self.read_string()?,
                '$' => self.read_field()?,
                _ if ch.is_ascii_digit() || ch == '.' => self.read_number()?,
                _ if ch.is_ascii_alphabetic() || ch == '_' => self.read_ident(),
                _ => {
                    return Err(format!("unexpected character '{}' at position {}", ch, self.pos));
                }
            };

            tokens.push(token);
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

    fn is_regex_context(&self, tokens: &[Token]) -> bool {
        match tokens.last() {
            None => true,
            Some(Token::LBrace) | Some(Token::Semicolon) | Some(Token::Newline)
            | Some(Token::And) | Some(Token::Or) | Some(Token::Not)
            | Some(Token::LParen) | Some(Token::Comma) => true,
            _ => false,
        }
    }

    fn read_regex(&mut self) -> Result<Token, String> {
        self.pos += 1; // skip opening /
        let mut pattern = String::new();
        loop {
            if self.pos >= self.input.len() {
                return Err("unterminated regex".to_string());
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
        self.pos += 1; // skip opening "
        let mut s = String::new();
        loop {
            if self.pos >= self.input.len() {
                return Err("unterminated string".to_string());
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
                    'n' => s.push('\n'),
                    't' => s.push('\t'),
                    '\\' => s.push('\\'),
                    '"' => s.push('"'),
                    '/' => s.push('/'),
                    _ => {
                        s.push('\\');
                        s.push(escaped);
                    }
                }
                self.pos += 1;
            } else {
                s.push(ch);
                self.pos += 1;
            }
        }
    }

    fn read_field(&mut self) -> Result<Token, String> {
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
                .map_err(|_| "invalid field number".to_string())?;
            Ok(Token::Field(num))
        } else if self.pos < self.input.len()
            && (self.input[self.pos].is_ascii_alphabetic() || self.input[self.pos] == '_')
        {
            let ident = self.read_ident_str();
            Ok(Token::FieldVar(ident))
        } else {
            Err("expected field number or variable after $".to_string())
        }
    }

    fn read_number(&mut self) -> Result<Token, String> {
        let start = self.pos;
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
        let num: f64 = s.parse().map_err(|_| format!("invalid number: {}", s))?;
        Ok(Token::Number(num))
    }

    fn read_ident(&mut self) -> Token {
        let s = self.read_ident_str();
        match s.as_str() {
            "BEGIN" => Token::Begin,
            "END" => Token::End,
            "print" => Token::Print,
            "if" => Token::If,
            "else" => Token::Else,
            "while" => Token::While,
            "for" => Token::For,
            _ => Token::Ident(s),
        }
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

    #[test]
    fn simple_print() {
        let mut lexer = Lexer::new("{ print $1 }");
        let tokens = lexer.tokenize().unwrap();
        assert_eq!(
            tokens,
            vec![Token::LBrace, Token::Print, Token::Field(1), Token::RBrace, Token::Eof]
        );
    }

    #[test]
    fn regex_pattern() {
        let mut lexer = Lexer::new("/error/ { print $0 }");
        let tokens = lexer.tokenize().unwrap();
        assert_eq!(
            tokens,
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
        let tokens = lexer.tokenize().unwrap();
        assert!(matches!(tokens[0], Token::Begin));
        assert!(tokens.iter().any(|t| matches!(t, Token::End)));
    }
}
