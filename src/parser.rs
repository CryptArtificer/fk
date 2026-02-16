use crate::error::Span;
use crate::lexer::{Spanned, Token};

/// A complete fk program: optional BEGIN, a set of rules, optional END, and functions.
#[derive(Debug)]
pub struct Program {
    pub begin: Option<Block>,
    pub rules: Vec<Rule>,
    pub end: Option<Block>,
    pub functions: Vec<FuncDef>,
}

/// A user-defined function.
#[derive(Debug, Clone)]
pub struct FuncDef {
    pub name: String,
    pub params: Vec<String>,
    pub body: Block,
}

/// A single pattern-action rule.
#[derive(Debug)]
pub struct Rule {
    pub pattern: Option<Pattern>,
    pub action: Block,
}

/// A block is a list of statements.
pub type Block = Vec<Statement>;

#[derive(Debug, Clone)]
pub enum Pattern {
    Regex(String),
    Expression(Expr),
    Range(Box<Pattern>, Box<Pattern>),
}

#[derive(Debug, Clone)]
pub enum Redirect {
    Overwrite(Expr),  // > file
    Append(Expr),     // >> file
    Pipe(Expr),       // | command
}

#[derive(Debug, Clone)]
pub enum Statement {
    Print(Vec<Expr>, Option<Redirect>),
    Printf(Vec<Expr>, Option<Redirect>),
    If(Expr, Block, Option<Block>),
    While(Expr, Block),
    For(Option<Box<Statement>>, Option<Expr>, Option<Box<Statement>>, Block),
    ForIn(String, String, Block),
    Delete(String, Expr),
    Return(Option<Expr>),
    Block(Block),
    Expression(Expr),
}

#[derive(Debug, Clone)]
pub enum Expr {
    Field(Box<Expr>),
    NumberLit(f64),
    StringLit(String),
    Var(String),
    ArrayRef(String, Box<Expr>),
    ArrayIn(String, String),
    BinOp(Box<Expr>, BinOp, Box<Expr>),
    LogicalAnd(Box<Expr>, Box<Expr>),
    LogicalOr(Box<Expr>, Box<Expr>),
    LogicalNot(Box<Expr>),
    Match(Box<Expr>, String),
    NotMatch(Box<Expr>, String),
    Assign(Box<Expr>, Box<Expr>),
    CompoundAssign(Box<Expr>, BinOp, Box<Expr>),
    Increment(Box<Expr>, bool),  // bool: true = pre (++x), false = post (x++)
    Decrement(Box<Expr>, bool),
    UnaryMinus(Box<Expr>),
    Concat(Box<Expr>, Box<Expr>),
    Ternary(Box<Expr>, Box<Expr>, Box<Expr>),
    Sprintf(Vec<Expr>),
    FuncCall(String, Vec<Expr>),
    /// getline [var] [< file]. Fields: optional var name, optional source file expr.
    Getline(Option<String>, Option<Box<Expr>>),
    /// "cmd" | getline [var]. Fields: command expr, optional var name.
    GetlinePipe(Box<Expr>, Option<String>),
}

#[derive(Debug, Clone)]
pub enum BinOp {
    Add,
    Sub,
    Mul,
    Div,
    Mod,
    Pow,
    Eq,
    Ne,
    Lt,
    Le,
    Gt,
    Ge,
}

pub struct Parser {
    tokens: Vec<Spanned>,
    pos: usize,
}

impl Parser {
    pub fn new(tokens: Vec<Spanned>) -> Self {
        Parser { tokens, pos: 0 }
    }

    fn current(&self) -> &Token {
        self.tokens.get(self.pos)
            .map(|s| &s.token)
            .unwrap_or(&Token::Eof)
    }

    fn current_span(&self) -> Span {
        self.tokens.get(self.pos)
            .map(|s| s.span)
            .unwrap_or(Span::new(0, 0))
    }

    pub fn parse(&mut self) -> Result<Program, String> {
        let mut begin = None;
        let mut rules = Vec::new();
        let mut end = None;
        let mut functions = Vec::new();

        self.skip_terminators();

        while !self.at_eof() {
            match self.current() {
                Token::Begin => {
                    self.advance();
                    self.skip_terminators();
                    let block = self.parse_brace_block()?;
                    begin = Some(block);
                }
                Token::End => {
                    self.advance();
                    self.skip_terminators();
                    let block = self.parse_brace_block()?;
                    end = Some(block);
                }
                Token::Function => {
                    let func = self.parse_func_def()?;
                    functions.push(func);
                }
                _ => {
                    let rule = self.parse_rule()?;
                    rules.push(rule);
                }
            }
            self.skip_terminators();
        }

        Ok(Program { begin, rules, end, functions })
    }

    fn parse_func_def(&mut self) -> Result<FuncDef, String> {
        self.advance(); // consume 'function'
        let name = match self.current().clone() {
            Token::Ident(n) => { self.advance(); n }
            _ => return Err(format!("{}: expected function name", self.current_span())),
        };
        self.expect(&Token::LParen)?;

        let mut params = Vec::new();
        if !self.check(&Token::RParen) {
            match self.current().clone() {
                Token::Ident(p) => { self.advance(); params.push(p); }
                _ => return Err(format!("{}: expected parameter name", self.current_span())),
            }
            while self.check(&Token::Comma) {
                self.advance();
                match self.current().clone() {
                    Token::Ident(p) => { self.advance(); params.push(p); }
                    _ => return Err(format!("{}: expected parameter name", self.current_span())),
                }
            }
        }
        self.expect(&Token::RParen)?;
        self.skip_terminators();

        let body = self.parse_brace_block()?;

        Ok(FuncDef { name, params, body })
    }

    fn parse_rule(&mut self) -> Result<Rule, String> {
        let pattern;
        let action;

        if self.check(&Token::LBrace) {
            pattern = None;
            action = self.parse_brace_block()?;
        } else {
            pattern = Some(self.parse_pattern()?);
            self.skip_terminators();
            if self.check(&Token::LBrace) {
                action = self.parse_brace_block()?;
            } else {
                action = vec![Statement::Print(vec![Expr::Field(Box::new(Expr::NumberLit(0.0)))], None)];
            }
        }

        Ok(Rule { pattern, action })
    }

    fn parse_pattern(&mut self) -> Result<Pattern, String> {
        let first = match self.current() {
            Token::Regex(s) => {
                let pat = Pattern::Regex(s.clone());
                self.advance();
                pat
            }
            _ => {
                let expr = self.parse_expr()?;
                Pattern::Expression(expr)
            }
        };

        // Check for range pattern: pat1, pat2
        if self.check(&Token::Comma) {
            self.advance();
            let second = match self.current() {
                Token::Regex(s) => {
                    let pat = Pattern::Regex(s.clone());
                    self.advance();
                    pat
                }
                _ => {
                    let expr = self.parse_expr()?;
                    Pattern::Expression(expr)
                }
            };
            Ok(Pattern::Range(Box::new(first), Box::new(second)))
        } else {
            Ok(first)
        }
    }

    fn parse_brace_block(&mut self) -> Result<Block, String> {
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
            Token::Print => self.parse_print(),
            Token::Printf => self.parse_printf(),
            Token::If => self.parse_if(),
            Token::While => self.parse_while(),
            Token::For => self.parse_for(),
            Token::Delete => self.parse_delete(),
            Token::Return => self.parse_return(),
            Token::LBrace => {
                let block = self.parse_brace_block()?;
                Ok(Statement::Block(block))
            }
            _ => {
                let expr = self.parse_expr()?;
                Ok(Statement::Expression(expr))
            }
        }
    }

    fn parse_print(&mut self) -> Result<Statement, String> {
        self.advance(); // consume 'print'
        let mut args = Vec::new();
        if !self.is_terminator() && !self.check(&Token::RBrace)
            && !self.check(&Token::Gt) && !self.check(&Token::Append) && !self.check(&Token::Pipe)
        {
            args.push(self.parse_non_redirect_expr()?);
            while self.check(&Token::Comma) {
                self.advance();
                args.push(self.parse_non_redirect_expr()?);
            }
        }
        if args.is_empty() {
            args.push(Expr::Field(Box::new(Expr::NumberLit(0.0))));
        }
        let redir = self.parse_redirect()?;
        Ok(Statement::Print(args, redir))
    }

    fn parse_printf(&mut self) -> Result<Statement, String> {
        let span = self.current_span();
        self.advance(); // consume 'printf'
        let mut args = Vec::new();
        if !self.is_terminator() && !self.check(&Token::RBrace) {
            args.push(self.parse_non_redirect_expr()?);
            while self.check(&Token::Comma) {
                self.advance();
                args.push(self.parse_non_redirect_expr()?);
            }
        }
        if args.is_empty() {
            return Err(format!("{}: printf requires a format string", span));
        }
        let redir = self.parse_redirect()?;
        Ok(Statement::Printf(args, redir))
    }

    /// Parse an expression for print/printf arguments. Parses up to but not
    /// including `>`, `>>`, or `|` at the top level, so they are consumed as
    /// redirection operators. Comparisons work inside parens: `print ($0 > 5)`.
    fn parse_non_redirect_expr(&mut self) -> Result<Expr, String> {
        self.parse_concatenation()
    }

    fn parse_redirect(&mut self) -> Result<Option<Redirect>, String> {
        if self.check(&Token::Gt) {
            self.advance();
            let target = self.parse_primary()?;
            Ok(Some(Redirect::Overwrite(target)))
        } else if self.check(&Token::Append) {
            self.advance();
            let target = self.parse_primary()?;
            Ok(Some(Redirect::Append(target)))
        } else if self.check(&Token::Pipe) {
            self.advance();
            let target = self.parse_primary()?;
            Ok(Some(Redirect::Pipe(target)))
        } else {
            Ok(None)
        }
    }

    fn parse_return(&mut self) -> Result<Statement, String> {
        self.advance(); // consume 'return'
        if self.is_terminator() || self.check(&Token::RBrace) {
            Ok(Statement::Return(None))
        } else {
            let expr = self.parse_expr()?;
            Ok(Statement::Return(Some(expr)))
        }
    }

    fn parse_if(&mut self) -> Result<Statement, String> {
        self.advance(); // consume 'if'
        self.expect(&Token::LParen)?;
        let cond = self.parse_expr()?;
        self.expect(&Token::RParen)?;
        self.skip_terminators();

        let then_block = if self.check(&Token::LBrace) {
            self.parse_brace_block()?
        } else {
            vec![self.parse_statement()?]
        };

        self.skip_terminators();

        let else_block = if self.check(&Token::Else) {
            self.advance();
            self.skip_terminators();
            if self.check(&Token::If) {
                Some(vec![self.parse_if()?])
            } else if self.check(&Token::LBrace) {
                Some(self.parse_brace_block()?)
            } else {
                Some(vec![self.parse_statement()?])
            }
        } else {
            None
        };

        Ok(Statement::If(cond, then_block, else_block))
    }

    fn parse_while(&mut self) -> Result<Statement, String> {
        self.advance(); // consume 'while'
        self.expect(&Token::LParen)?;
        let cond = self.parse_expr()?;
        self.expect(&Token::RParen)?;
        self.skip_terminators();

        let body = if self.check(&Token::LBrace) {
            self.parse_brace_block()?
        } else {
            vec![self.parse_statement()?]
        };

        Ok(Statement::While(cond, body))
    }

    fn parse_for(&mut self) -> Result<Statement, String> {
        self.advance(); // consume 'for'
        self.expect(&Token::LParen)?;

        // Check for `for (var in array)` pattern
        if let Token::Ident(name) = self.current().clone() {
            let saved_pos = self.pos;
            self.advance();
            if self.check(&Token::In) {
                self.advance();
                if let Token::Ident(arr) = self.current().clone() {
                    self.advance();
                    self.expect(&Token::RParen)?;
                    self.skip_terminators();
                    let body = if self.check(&Token::LBrace) {
                        self.parse_brace_block()?
                    } else {
                        vec![self.parse_statement()?]
                    };
                    return Ok(Statement::ForIn(name, arr, body));
                }
            }
            // Backtrack — not a for-in
            self.pos = saved_pos;
        }

        // C-style for (init; cond; update)
        let init = if self.check(&Token::Semicolon) {
            None
        } else {
            let expr = self.parse_expr()?;
            Some(Box::new(Statement::Expression(expr)))
        };
        self.expect(&Token::Semicolon)?;

        let cond = if self.check(&Token::Semicolon) {
            None
        } else {
            Some(self.parse_expr()?)
        };
        self.expect(&Token::Semicolon)?;

        let update = if self.check(&Token::RParen) {
            None
        } else {
            let expr = self.parse_expr()?;
            Some(Box::new(Statement::Expression(expr)))
        };
        self.expect(&Token::RParen)?;
        self.skip_terminators();

        let body = if self.check(&Token::LBrace) {
            self.parse_brace_block()?
        } else {
            vec![self.parse_statement()?]
        };

        Ok(Statement::For(init, cond, update, body))
    }

    fn parse_delete(&mut self) -> Result<Statement, String> {
        let span = self.current_span();
        self.advance(); // consume 'delete'
        if let Token::Ident(name) = self.current().clone() {
            self.advance();
            self.expect(&Token::LBracket)?;
            let key = self.parse_expr()?;
            self.expect(&Token::RBracket)?;
            Ok(Statement::Delete(name, key))
        } else {
            Err(format!("{}: delete requires array[subscript]", span))
        }
    }

    // --- expression parsing (precedence climbing) ---

    fn parse_expr(&mut self) -> Result<Expr, String> {
        self.parse_assignment()
    }

    fn parse_assignment(&mut self) -> Result<Expr, String> {
        let expr = self.parse_ternary()?;

        match self.current() {
            Token::Assign => {
                self.check_lvalue(&expr)?;
                self.advance();
                let value = self.parse_assignment()?;
                Ok(Expr::Assign(Box::new(expr), Box::new(value)))
            }
            Token::PlusAssign => {
                self.check_lvalue(&expr)?;
                self.advance();
                let value = self.parse_assignment()?;
                Ok(Expr::CompoundAssign(Box::new(expr), BinOp::Add, Box::new(value)))
            }
            Token::MinusAssign => {
                self.check_lvalue(&expr)?;
                self.advance();
                let value = self.parse_assignment()?;
                Ok(Expr::CompoundAssign(Box::new(expr), BinOp::Sub, Box::new(value)))
            }
            Token::StarAssign => {
                self.check_lvalue(&expr)?;
                self.advance();
                let value = self.parse_assignment()?;
                Ok(Expr::CompoundAssign(Box::new(expr), BinOp::Mul, Box::new(value)))
            }
            Token::SlashAssign => {
                self.check_lvalue(&expr)?;
                self.advance();
                let value = self.parse_assignment()?;
                Ok(Expr::CompoundAssign(Box::new(expr), BinOp::Div, Box::new(value)))
            }
            Token::PercentAssign => {
                self.check_lvalue(&expr)?;
                self.advance();
                let value = self.parse_assignment()?;
                Ok(Expr::CompoundAssign(Box::new(expr), BinOp::Mod, Box::new(value)))
            }
            _ => Ok(expr),
        }
    }

    fn check_lvalue(&self, expr: &Expr) -> Result<(), String> {
        match expr {
            Expr::Var(_) | Expr::ArrayRef(_, _) | Expr::Field(_) => Ok(()),
            _ => Err(format!("{}: invalid assignment target", self.current_span())),
        }
    }

    fn parse_ternary(&mut self) -> Result<Expr, String> {
        let expr = self.parse_logical_or()?;

        if self.check(&Token::Question) {
            self.advance();
            let then_expr = self.parse_expr()?;
            self.expect(&Token::Colon)?;
            let else_expr = self.parse_expr()?;
            Ok(Expr::Ternary(Box::new(expr), Box::new(then_expr), Box::new(else_expr)))
        } else {
            Ok(expr)
        }
    }

    fn parse_logical_or(&mut self) -> Result<Expr, String> {
        let mut left = self.parse_logical_and()?;

        while self.check(&Token::Or) {
            self.advance();
            let right = self.parse_logical_and()?;
            left = Expr::LogicalOr(Box::new(left), Box::new(right));
        }

        Ok(left)
    }

    fn parse_logical_and(&mut self) -> Result<Expr, String> {
        let mut left = self.parse_match_expr()?;

        while self.check(&Token::And) {
            self.advance();
            let right = self.parse_match_expr()?;
            left = Expr::LogicalAnd(Box::new(left), Box::new(right));
        }

        Ok(left)
    }

    fn parse_match_expr(&mut self) -> Result<Expr, String> {
        let left = self.parse_comparison()?;

        // "cmd" | getline [var]
        if self.check(&Token::Pipe) {
            let saved = self.pos;
            self.advance();
            if self.check(&Token::Getline) {
                self.advance();
                let var = if let Token::Ident(name) = self.current().clone() {
                    self.advance();
                    Some(name)
                } else {
                    None
                };
                return Ok(Expr::GetlinePipe(Box::new(left), var));
            }
            // Not a getline pipe, backtrack
            self.pos = saved;
        }

        if self.check(&Token::Match) {
            self.advance();
            if let Token::Regex(pat) = self.current().clone() {
                self.advance();
                return Ok(Expr::Match(Box::new(left), pat));
            } else {
                return Err(format!("{}: expected regex after ~", self.current_span()));
            }
        }
        if self.check(&Token::NotMatch) {
            self.advance();
            if let Token::Regex(pat) = self.current().clone() {
                self.advance();
                return Ok(Expr::NotMatch(Box::new(left), pat));
            } else {
                return Err(format!("{}: expected regex after !~", self.current_span()));
            }
        }

        Ok(left)
    }

    fn parse_comparison(&mut self) -> Result<Expr, String> {
        let mut left = self.parse_concatenation()?;

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
            let right = self.parse_concatenation()?;
            left = Expr::BinOp(Box::new(left), op, Box::new(right));
        }

        Ok(left)
    }

    fn parse_concatenation(&mut self) -> Result<Expr, String> {
        let mut left = self.parse_addition()?;

        // Implicit concatenation: two adjacent values with no operator between them
        while self.is_concat_start() {
            let right = self.parse_addition()?;
            left = Expr::Concat(Box::new(left), Box::new(right));
        }

        Ok(left)
    }

    fn is_concat_start(&self) -> bool {
        matches!(
            self.current(),
            Token::Number(_)
                | Token::StringLit(_)
                | Token::Ident(_)
                | Token::Field(_)
                | Token::FieldVar(_)
                | Token::LParen
        )
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
        let mut left = self.parse_exponentiation()?;

        loop {
            match self.current() {
                Token::Star => {
                    self.advance();
                    let right = self.parse_exponentiation()?;
                    left = Expr::BinOp(Box::new(left), BinOp::Mul, Box::new(right));
                }
                Token::Slash => {
                    self.advance();
                    let right = self.parse_exponentiation()?;
                    left = Expr::BinOp(Box::new(left), BinOp::Div, Box::new(right));
                }
                Token::Percent => {
                    self.advance();
                    let right = self.parse_exponentiation()?;
                    left = Expr::BinOp(Box::new(left), BinOp::Mod, Box::new(right));
                }
                _ => break,
            }
        }

        Ok(left)
    }

    /// Exponentiation: right-associative, higher precedence than multiplication.
    fn parse_exponentiation(&mut self) -> Result<Expr, String> {
        let base = self.parse_unary()?;

        if self.check(&Token::Power) {
            self.advance();
            let exp = self.parse_exponentiation()?; // right-associative
            Ok(Expr::BinOp(Box::new(base), BinOp::Pow, Box::new(exp)))
        } else {
            Ok(base)
        }
    }

    fn parse_unary(&mut self) -> Result<Expr, String> {
        if self.check(&Token::Minus) {
            self.advance();
            let expr = self.parse_unary()?;
            return Ok(Expr::UnaryMinus(Box::new(expr)));
        }
        if self.check(&Token::Not) {
            self.advance();
            let expr = self.parse_unary()?;
            return Ok(Expr::LogicalNot(Box::new(expr)));
        }
        // Pre-increment / pre-decrement
        if self.check(&Token::Increment) {
            self.advance();
            let expr = self.parse_postfix()?;
            return Ok(Expr::Increment(Box::new(expr), true));
        }
        if self.check(&Token::Decrement) {
            self.advance();
            let expr = self.parse_postfix()?;
            return Ok(Expr::Decrement(Box::new(expr), true));
        }
        self.parse_postfix()
    }

    fn parse_postfix(&mut self) -> Result<Expr, String> {
        let mut expr = self.parse_primary()?;

        loop {
            if self.check(&Token::LBracket) {
                // Array subscript
                if let Expr::Var(name) = expr {
                    self.advance();
                    let key = self.parse_expr()?;
                    self.expect(&Token::RBracket)?;
                    expr = Expr::ArrayRef(name, Box::new(key));
                } else {
                    break;
                }
            } else if self.check(&Token::Increment) {
                self.advance();
                expr = Expr::Increment(Box::new(expr), false);
            } else if self.check(&Token::Decrement) {
                self.advance();
                expr = Expr::Decrement(Box::new(expr), false);
            } else {
                break;
            }
        }

        Ok(expr)
    }

    fn parse_primary(&mut self) -> Result<Expr, String> {
        match self.current().clone() {
            Token::Number(n) => {
                self.advance();
                Ok(Expr::NumberLit(n))
            }
            Token::StringLit(s) => {
                self.advance();
                Ok(Expr::StringLit(s))
            }
            Token::Field(n) => {
                self.advance();
                Ok(Expr::Field(Box::new(Expr::NumberLit(n as f64))))
            }
            Token::FieldVar(name) => {
                self.advance();
                Ok(Expr::Field(Box::new(Expr::Var(name))))
            }
            Token::Ident(name) => {
                self.advance();

                // Check for function call: ident(
                if self.check(&Token::LParen) {
                    if name == "sprintf" {
                        return self.parse_sprintf_args();
                    }
                    return self.parse_func_call(name);
                }

                // Check for `var in array` (used in conditions)
                if self.check(&Token::In) {
                    self.advance();
                    if let Token::Ident(arr) = self.current().clone() {
                        self.advance();
                        return Ok(Expr::ArrayIn(name, arr));
                    } else {
                        return Err(format!("{}: expected array name after 'in'", self.current_span()));
                    }
                }

                Ok(Expr::Var(name))
            }
            Token::Getline => {
                self.advance();
                // getline [var] [< file]
                let var = if let Token::Ident(name) = self.current().clone() {
                    let saved = self.pos;
                    self.advance();
                    if self.check(&Token::Lt) || self.is_terminator()
                        || self.check(&Token::RBrace) || self.check(&Token::Semicolon)
                        || self.check(&Token::RParen) || self.at_eof()
                    {
                        Some(name)
                    } else {
                        self.pos = saved;
                        None
                    }
                } else {
                    None
                };
                let source = if self.check(&Token::Lt) {
                    self.advance();
                    Some(Box::new(self.parse_primary()?))
                } else {
                    None
                };
                Ok(Expr::Getline(var, source))
            }
            Token::LParen => {
                self.advance();
                let expr = self.parse_expr()?;
                self.expect(&Token::RParen)?;
                Ok(expr)
            }
            _ => Err(format!("{}: unexpected token: {:?}", self.current_span(), self.current())),
        }
    }

    fn parse_sprintf_args(&mut self) -> Result<Expr, String> {
        let span = self.current_span();
        self.expect(&Token::LParen)?;
        let mut args = Vec::new();
        if !self.check(&Token::RParen) {
            args.push(self.parse_expr()?);
            while self.check(&Token::Comma) {
                self.advance();
                args.push(self.parse_expr()?);
            }
        }
        self.expect(&Token::RParen)?;
        if args.is_empty() {
            return Err(format!("{}: sprintf requires a format string", span));
        }
        Ok(Expr::Sprintf(args))
    }

    fn parse_func_call(&mut self, name: String) -> Result<Expr, String> {
        self.expect(&Token::LParen)?;
        let mut args = Vec::new();
        if !self.check(&Token::RParen) {
            args.push(self.parse_expr()?);
            while self.check(&Token::Comma) {
                self.advance();
                args.push(self.parse_expr()?);
            }
        }
        self.expect(&Token::RParen)?;
        Ok(Expr::FuncCall(name, args))
    }

    // --- helpers ---

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
            Err(format!("{}: expected {:?}, got {:?}", self.current_span(), token, self.current()))
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
