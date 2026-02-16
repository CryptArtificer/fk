use crate::lexer::Token;

/// A complete fk program: optional BEGIN, a set of rules, optional END.
#[derive(Debug)]
pub struct Program {
    pub begin: Option<Block>,
    pub rules: Vec<Rule>,
    pub end: Option<Block>,
}

/// A single pattern-action rule.
#[derive(Debug)]
pub struct Rule {
    pub pattern: Option<Pattern>,
    pub action: Block,
}

/// A block is a list of statements.
pub type Block = Vec<Statement>;

#[derive(Debug)]
pub enum Pattern {
    Regex(String),
    Expression(Expr),
}

#[derive(Debug)]
pub enum Statement {
    Print(Vec<Expr>),
    Expression(Expr),
}

#[derive(Debug, Clone)]
pub enum Expr {
    Field(Box<Expr>),
    NumberLit(f64),
    StringLit(String),
    Var(String),
    BinOp(Box<Expr>, BinOp, Box<Expr>),
    Assign(String, Box<Expr>),
    UnaryMinus(Box<Expr>),
}

#[derive(Debug, Clone)]
pub enum BinOp {
    Add,
    Sub,
    Mul,
    Div,
    Mod,
    Eq,
    Ne,
    Lt,
    Le,
    Gt,
    Ge,
    Concat,
}

pub struct Parser {
    tokens: Vec<Token>,
    pos: usize,
}

impl Parser {
    pub fn new(tokens: Vec<Token>) -> Self {
        Parser { tokens, pos: 0 }
    }

    pub fn parse(&mut self) -> Result<Program, String> {
        let mut begin = None;
        let mut rules = Vec::new();
        let mut end = None;

        self.skip_terminators();

        while !self.at_eof() {
            match self.current() {
                Token::Begin => {
                    self.advance();
                    self.skip_terminators();
                    let block = self.parse_block()?;
                    begin = Some(block);
                }
                Token::End => {
                    self.advance();
                    self.skip_terminators();
                    let block = self.parse_block()?;
                    end = Some(block);
                }
                _ => {
                    let rule = self.parse_rule()?;
                    rules.push(rule);
                }
            }
            self.skip_terminators();
        }

        Ok(Program { begin, rules, end })
    }

    fn parse_rule(&mut self) -> Result<Rule, String> {
        // A rule is: [pattern] { action } or just { action }
        // A bare pattern with no action means { print $0 }
        let pattern;
        let action;

        if self.check(&Token::LBrace) {
            pattern = None;
            action = self.parse_block()?;
        } else {
            pattern = Some(self.parse_pattern()?);
            self.skip_terminators();
            if self.check(&Token::LBrace) {
                action = self.parse_block()?;
            } else {
                // Default action: print $0
                action = vec![Statement::Print(vec![Expr::Field(Box::new(Expr::NumberLit(0.0)))])];
            }
        }

        Ok(Rule { pattern, action })
    }

    fn parse_pattern(&mut self) -> Result<Pattern, String> {
        match self.current() {
            Token::Regex(s) => {
                let pat = Pattern::Regex(s.clone());
                self.advance();
                Ok(pat)
            }
            _ => {
                let expr = self.parse_expr()?;
                Ok(Pattern::Expression(expr))
            }
        }
    }

    fn parse_block(&mut self) -> Result<Block, String> {
        self.expect(&Token::LBrace)?;
        self.skip_terminators();

        let mut stmts = Vec::new();
        while !self.check(&Token::RBrace) && !self.at_eof() {
            let stmt = self.parse_statement()?;
            stmts.push(stmt);
            self.skip_terminators();
        }

        self.expect(&Token::RBrace)?;
        Ok(stmts)
    }

    fn parse_statement(&mut self) -> Result<Statement, String> {
        match self.current() {
            Token::Print => {
                self.advance();
                let mut args = Vec::new();
                // Parse print arguments until we hit a statement terminator
                if !self.is_terminator() && !self.check(&Token::RBrace) {
                    args.push(self.parse_expr()?);
                    while self.check(&Token::Comma) {
                        self.advance();
                        args.push(self.parse_expr()?);
                    }
                }
                if args.is_empty() {
                    args.push(Expr::Field(Box::new(Expr::NumberLit(0.0))));
                }
                Ok(Statement::Print(args))
            }
            _ => {
                let expr = self.parse_expr()?;
                Ok(Statement::Expression(expr))
            }
        }
    }

    fn parse_expr(&mut self) -> Result<Expr, String> {
        self.parse_assignment()
    }

    fn parse_assignment(&mut self) -> Result<Expr, String> {
        let expr = self.parse_comparison()?;

        if self.check(&Token::Assign) {
            if let Expr::Var(name) = expr {
                self.advance();
                let value = self.parse_assignment()?;
                return Ok(Expr::Assign(name, Box::new(value)));
            } else {
                return Err("invalid assignment target".to_string());
            }
        }

        if self.check(&Token::Append) {
            if let Expr::Var(ref name) = expr {
                let name = name.clone();
                self.advance();
                let value = self.parse_assignment()?;
                return Ok(Expr::Assign(
                    name.clone(),
                    Box::new(Expr::BinOp(
                        Box::new(Expr::Var(name)),
                        BinOp::Add,
                        Box::new(value),
                    )),
                ));
            } else {
                return Err("invalid += target".to_string());
            }
        }

        Ok(expr)
    }

    fn parse_comparison(&mut self) -> Result<Expr, String> {
        let mut left = self.parse_addition()?;

        loop {
            let op = match self.current() {
                Token::Eq => BinOp::Eq,
                Token::Ne => BinOp::Ne,
                Token::Lt => BinOp::Lt,
                Token::Le => BinOp::Le,
                Token::Gt => BinOp::Gt,
                Token::Ge => BinOp::Ge,
                _ => break,
            };
            self.advance();
            let right = self.parse_addition()?;
            left = Expr::BinOp(Box::new(left), op, Box::new(right));
        }

        Ok(left)
    }

    fn parse_addition(&mut self) -> Result<Expr, String> {
        let mut left = self.parse_multiplication()?;

        loop {
            match self.current() {
                Token::Plus => {
                    self.advance();
                    let right = self.parse_multiplication()?;
                    left = Expr::BinOp(Box::new(left), BinOp::Add, Box::new(right));
                }
                Token::Minus => {
                    self.advance();
                    let right = self.parse_multiplication()?;
                    left = Expr::BinOp(Box::new(left), BinOp::Sub, Box::new(right));
                }
                _ => break,
            }
        }

        Ok(left)
    }

    fn parse_multiplication(&mut self) -> Result<Expr, String> {
        let mut left = self.parse_unary()?;

        loop {
            match self.current() {
                Token::Star => {
                    self.advance();
                    let right = self.parse_unary()?;
                    left = Expr::BinOp(Box::new(left), BinOp::Mul, Box::new(right));
                }
                Token::Slash => {
                    self.advance();
                    let right = self.parse_unary()?;
                    left = Expr::BinOp(Box::new(left), BinOp::Div, Box::new(right));
                }
                Token::Percent => {
                    self.advance();
                    let right = self.parse_unary()?;
                    left = Expr::BinOp(Box::new(left), BinOp::Mod, Box::new(right));
                }
                _ => break,
            }
        }

        Ok(left)
    }

    fn parse_unary(&mut self) -> Result<Expr, String> {
        if self.check(&Token::Minus) {
            self.advance();
            let expr = self.parse_primary()?;
            return Ok(Expr::UnaryMinus(Box::new(expr)));
        }
        self.parse_primary()
    }

    fn parse_primary(&mut self) -> Result<Expr, String> {
        match self.current() {
            Token::Number(n) => {
                let n = *n;
                self.advance();
                Ok(Expr::NumberLit(n))
            }
            Token::StringLit(s) => {
                let s = s.clone();
                self.advance();
                Ok(Expr::StringLit(s))
            }
            Token::Field(n) => {
                let n = *n;
                self.advance();
                Ok(Expr::Field(Box::new(Expr::NumberLit(n as f64))))
            }
            Token::FieldVar(name) => {
                let name = name.clone();
                self.advance();
                Ok(Expr::Field(Box::new(Expr::Var(name))))
            }
            Token::Ident(name) => {
                let name = name.clone();
                self.advance();
                Ok(Expr::Var(name))
            }
            Token::LParen => {
                self.advance();
                let expr = self.parse_expr()?;
                self.expect(&Token::RParen)?;
                Ok(expr)
            }
            _ => Err(format!("unexpected token: {:?}", self.current())),
        }
    }

    // --- helpers ---

    fn current(&self) -> &Token {
        self.tokens.get(self.pos).unwrap_or(&Token::Eof)
    }

    fn advance(&mut self) {
        if self.pos < self.tokens.len() {
            self.pos += 1;
        }
    }

    fn at_eof(&self) -> bool {
        matches!(self.current(), Token::Eof)
    }

    fn check(&self, token: &Token) -> bool {
        std::mem::discriminant(self.current()) == std::mem::discriminant(token)
    }

    fn expect(&mut self, token: &Token) -> Result<(), String> {
        if self.check(token) {
            self.advance();
            Ok(())
        } else {
            Err(format!("expected {:?}, got {:?}", token, self.current()))
        }
    }

    fn is_terminator(&self) -> bool {
        matches!(self.current(), Token::Semicolon | Token::Newline | Token::Eof)
    }

    fn skip_terminators(&mut self) {
        while matches!(self.current(), Token::Semicolon | Token::Newline) {
            self.advance();
        }
    }
}
