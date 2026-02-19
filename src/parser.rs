use crate::error::{FkError, Span};
use crate::lexer::{Spanned, Token};

/// A complete fk program: optional BEGIN, a set of rules, optional END, and functions.
#[derive(Debug)]
pub struct Program {
    pub begin: Option<Block>,
    pub rules: Vec<Rule>,
    pub end: Option<Block>,
    pub beginfile: Option<Block>,
    pub endfile: Option<Block>,
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
    Overwrite(Expr), // > file
    Append(Expr),    // >> file
    Pipe(Expr),      // | command
}

#[derive(Debug, Clone)]
pub enum Statement {
    Print(Vec<Expr>, Option<Redirect>),
    Printf(Vec<Expr>, Option<Redirect>),
    If(Expr, Block, Option<Block>),
    While(Expr, Block),
    DoWhile(Block, Expr),
    For(
        Option<Box<Statement>>,
        Option<Expr>,
        Option<Box<Statement>>,
        Block,
    ),
    ForIn(String, String, Block),
    Delete(String, Expr),
    DeleteAll(String),
    Next,
    Nextfile,
    Break,
    Continue,
    Exit(Option<Expr>),
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
    ArrayIn(Box<Expr>, String),
    BinOp(Box<Expr>, BinOp, Box<Expr>),
    LogicalAnd(Box<Expr>, Box<Expr>),
    LogicalOr(Box<Expr>, Box<Expr>),
    LogicalNot(Box<Expr>),
    Match(Box<Expr>, Box<Expr>),
    NotMatch(Box<Expr>, Box<Expr>),
    Assign(Box<Expr>, Box<Expr>),
    CompoundAssign(Box<Expr>, BinOp, Box<Expr>),
    Increment(Box<Expr>, bool), // bool: true = pre (++x), false = post (x++)
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
    in_print_expr: bool,
}

impl Parser {
    pub fn new(tokens: Vec<Spanned>) -> Self {
        Parser {
            tokens,
            pos: 0,
            in_print_expr: false,
        }
    }

    fn current(&self) -> &Token {
        self.tokens
            .get(self.pos)
            .map(|s| &s.token)
            .unwrap_or(&Token::Eof)
    }

    fn current_span(&self) -> Span {
        self.tokens
            .get(self.pos)
            .map(|s| s.span)
            .unwrap_or(Span::new(0, 0))
    }

    pub fn parse(&mut self) -> Result<Program, FkError> {
        let mut begin = None;
        let mut rules = Vec::new();
        let mut end = None;
        let mut beginfile = None;
        let mut endfile = None;
        let mut functions = Vec::new();

        self.skip_terminators();

        while !self.at_eof() {
            match self.current() {
                Token::Begin => {
                    self.advance();
                    self.skip_terminators();
                    let block = self.parse_brace_block()?;
                    begin.get_or_insert_with(Vec::new).extend(block);
                }
                Token::End => {
                    self.advance();
                    self.skip_terminators();
                    let block = self.parse_brace_block()?;
                    end.get_or_insert_with(Vec::new).extend(block);
                }
                Token::Beginfile => {
                    self.advance();
                    self.skip_terminators();
                    let block = self.parse_brace_block()?;
                    beginfile.get_or_insert_with(Vec::new).extend(block);
                }
                Token::Endfile => {
                    self.advance();
                    self.skip_terminators();
                    let block = self.parse_brace_block()?;
                    endfile.get_or_insert_with(Vec::new).extend(block);
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

        Ok(Program {
            begin,
            rules,
            end,
            beginfile,
            endfile,
            functions,
        })
    }

    fn parse_func_def(&mut self) -> Result<FuncDef, FkError> {
        self.advance(); // consume 'function'
        let name = match self.current().clone() {
            Token::Ident(n) => {
                self.advance();
                n
            }
            _ => return Err(FkError::new(self.current_span(), "expected function name")),
        };
        self.expect(&Token::LParen)?;

        let mut params = Vec::new();
        if !self.check(&Token::RParen) {
            match self.current().clone() {
                Token::Ident(p) => {
                    self.advance();
                    params.push(p);
                }
                _ => return Err(FkError::new(self.current_span(), "expected parameter name")),
            }
            while self.check(&Token::Comma) {
                self.advance();
                match self.current().clone() {
                    Token::Ident(p) => {
                        self.advance();
                        params.push(p);
                    }
                    _ => return Err(FkError::new(self.current_span(), "expected parameter name")),
                }
            }
        }
        self.expect(&Token::RParen)?;
        self.skip_terminators();

        let body = self.parse_brace_block()?;

        Ok(FuncDef { name, params, body })
    }

    fn parse_rule(&mut self) -> Result<Rule, FkError> {
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
                action = vec![Statement::Print(
                    vec![Expr::Field(Box::new(Expr::NumberLit(0.0)))],
                    None,
                )];
            }
        }

        Ok(Rule { pattern, action })
    }

    fn parse_pattern(&mut self) -> Result<Pattern, FkError> {
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

    fn parse_brace_block(&mut self) -> Result<Block, FkError> {
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

    fn parse_statement(&mut self) -> Result<Statement, FkError> {
        match self.current() {
            Token::Print => self.parse_print(),
            Token::Printf => self.parse_printf(),
            Token::If => self.parse_if(),
            Token::While => self.parse_while(),
            Token::Do => self.parse_do_while(),
            Token::For => self.parse_for(),
            Token::Delete => self.parse_delete(),
            Token::Next => {
                self.advance();
                Ok(Statement::Next)
            }
            Token::Nextfile => {
                self.advance();
                Ok(Statement::Nextfile)
            }
            Token::Break => {
                self.advance();
                Ok(Statement::Break)
            }
            Token::Continue => {
                self.advance();
                Ok(Statement::Continue)
            }
            Token::Exit => self.parse_exit(),
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

    fn parse_print(&mut self) -> Result<Statement, FkError> {
        self.advance(); // consume 'print'
        let mut args = Vec::new();
        if !self.is_terminator()
            && !self.check(&Token::RBrace)
            && !self.check(&Token::Gt)
            && !self.check(&Token::Append)
            && !self.check(&Token::Pipe)
        {
            args.push(self.parse_non_redirect_expr()?);
            self.skip_newlines();
            while self.check(&Token::Comma) {
                self.advance();
                self.skip_newlines();
                args.push(self.parse_non_redirect_expr()?);
                self.skip_newlines();
            }
        }
        if args.is_empty() {
            args.push(Expr::Field(Box::new(Expr::NumberLit(0.0))));
        }
        let redir = self.parse_redirect()?;
        Ok(Statement::Print(args, redir))
    }

    fn parse_printf(&mut self) -> Result<Statement, FkError> {
        let span = self.current_span();
        self.advance(); // consume 'printf'
        let mut args = Vec::new();
        if !self.is_terminator() && !self.check(&Token::RBrace) {
            args.push(self.parse_non_redirect_expr()?);
            self.skip_newlines();
            while self.check(&Token::Comma) {
                self.advance();
                self.skip_newlines();
                args.push(self.parse_non_redirect_expr()?);
                self.skip_newlines();
            }
        }
        if args.is_empty() {
            return Err(FkError::new(span, "printf requires a format string"));
        }
        let redir = self.parse_redirect()?;
        Ok(Statement::Printf(args, redir))
    }

    /// Parse an expression for print/printf arguments. Includes ternary,
    /// logical, match, and `in` operators, but `>` and `>=` are reserved for
    /// redirection at this level. Comparisons with `>` work inside parens.
    fn parse_non_redirect_expr(&mut self) -> Result<Expr, FkError> {
        self.in_print_expr = true;
        let result = self.parse_ternary();
        self.in_print_expr = false;
        result
    }

    fn parse_redirect(&mut self) -> Result<Option<Redirect>, FkError> {
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

    fn parse_return(&mut self) -> Result<Statement, FkError> {
        self.advance(); // consume 'return'
        if self.is_terminator() || self.check(&Token::RBrace) {
            Ok(Statement::Return(None))
        } else {
            let expr = self.parse_expr()?;
            Ok(Statement::Return(Some(expr)))
        }
    }

    fn parse_if(&mut self) -> Result<Statement, FkError> {
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

    fn parse_while(&mut self) -> Result<Statement, FkError> {
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

    fn parse_do_while(&mut self) -> Result<Statement, FkError> {
        self.advance(); // consume 'do'
        self.skip_terminators();

        let body = if self.check(&Token::LBrace) {
            self.parse_brace_block()?
        } else {
            vec![self.parse_statement()?]
        };

        self.skip_terminators();
        self.expect(&Token::While)?;
        self.expect(&Token::LParen)?;
        let cond = self.parse_expr()?;
        self.expect(&Token::RParen)?;

        Ok(Statement::DoWhile(body, cond))
    }

    fn parse_exit(&mut self) -> Result<Statement, FkError> {
        self.advance(); // consume 'exit'
        if self.is_terminator() || self.check(&Token::RBrace) {
            Ok(Statement::Exit(None))
        } else {
            let expr = self.parse_expr()?;
            Ok(Statement::Exit(Some(expr)))
        }
    }

    fn parse_for(&mut self) -> Result<Statement, FkError> {
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

    fn parse_delete(&mut self) -> Result<Statement, FkError> {
        let span = self.current_span();
        self.advance(); // consume 'delete'
        if let Token::Ident(name) = self.current().clone() {
            self.advance();
            if self.check(&Token::LBracket) {
                self.advance();
                let key = self.parse_expr()?;
                self.expect(&Token::RBracket)?;
                Ok(Statement::Delete(name, key))
            } else {
                // delete entire array
                Ok(Statement::DeleteAll(name))
            }
        } else {
            Err(FkError::new(
                span,
                "delete requires array or array[subscript]",
            ))
        }
    }

    // --- expression parsing (precedence climbing) ---

    fn parse_expr(&mut self) -> Result<Expr, FkError> {
        self.parse_assignment()
    }

    fn parse_assignment(&mut self) -> Result<Expr, FkError> {
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
                Ok(Expr::CompoundAssign(
                    Box::new(expr),
                    BinOp::Add,
                    Box::new(value),
                ))
            }
            Token::MinusAssign => {
                self.check_lvalue(&expr)?;
                self.advance();
                let value = self.parse_assignment()?;
                Ok(Expr::CompoundAssign(
                    Box::new(expr),
                    BinOp::Sub,
                    Box::new(value),
                ))
            }
            Token::StarAssign => {
                self.check_lvalue(&expr)?;
                self.advance();
                let value = self.parse_assignment()?;
                Ok(Expr::CompoundAssign(
                    Box::new(expr),
                    BinOp::Mul,
                    Box::new(value),
                ))
            }
            Token::SlashAssign => {
                self.check_lvalue(&expr)?;
                self.advance();
                let value = self.parse_assignment()?;
                Ok(Expr::CompoundAssign(
                    Box::new(expr),
                    BinOp::Div,
                    Box::new(value),
                ))
            }
            Token::PercentAssign => {
                self.check_lvalue(&expr)?;
                self.advance();
                let value = self.parse_assignment()?;
                Ok(Expr::CompoundAssign(
                    Box::new(expr),
                    BinOp::Mod,
                    Box::new(value),
                ))
            }
            _ => Ok(expr),
        }
    }

    fn check_lvalue(&self, expr: &Expr) -> Result<(), FkError> {
        match expr {
            Expr::Var(_) | Expr::ArrayRef(_, _) | Expr::Field(_) => Ok(()),
            _ => Err(FkError::new(
                self.current_span(),
                "invalid assignment target",
            )),
        }
    }

    fn parse_ternary(&mut self) -> Result<Expr, FkError> {
        let expr = self.parse_logical_or()?;

        if self.check(&Token::Question) {
            self.advance();
            let then_expr = self.parse_expr()?;
            self.expect(&Token::Colon)?;
            let else_expr = self.parse_expr()?;
            Ok(Expr::Ternary(
                Box::new(expr),
                Box::new(then_expr),
                Box::new(else_expr),
            ))
        } else {
            Ok(expr)
        }
    }

    fn parse_logical_or(&mut self) -> Result<Expr, FkError> {
        let mut left = self.parse_logical_and()?;

        while self.check(&Token::Or) {
            self.advance();
            let right = self.parse_logical_and()?;
            left = Expr::LogicalOr(Box::new(left), Box::new(right));
        }

        Ok(left)
    }

    fn parse_logical_and(&mut self) -> Result<Expr, FkError> {
        let mut left = self.parse_in_expr()?;

        while self.check(&Token::And) {
            self.advance();
            let right = self.parse_in_expr()?;
            left = Expr::LogicalAnd(Box::new(left), Box::new(right));
        }

        Ok(left)
    }

    /// `expr in array` — array membership test. Precedence between
    /// logical-and and match (~, !~).
    fn parse_in_expr(&mut self) -> Result<Expr, FkError> {
        let left = self.parse_match_expr()?;

        if self.check(&Token::In) {
            self.advance();
            if let Token::Ident(arr) = self.current().clone() {
                self.advance();
                return Ok(Expr::ArrayIn(Box::new(left), arr));
            } else {
                return Err(FkError::new(
                    self.current_span(),
                    "expected array name after 'in'",
                ));
            }
        }

        Ok(left)
    }

    fn parse_match_expr(&mut self) -> Result<Expr, FkError> {
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
                return Ok(Expr::Match(Box::new(left), Box::new(Expr::StringLit(pat))));
            } else {
                let expr = self.parse_primary()?;
                return Ok(Expr::Match(Box::new(left), Box::new(expr)));
            }
        }
        if self.check(&Token::NotMatch) {
            self.advance();
            if let Token::Regex(pat) = self.current().clone() {
                self.advance();
                return Ok(Expr::NotMatch(
                    Box::new(left),
                    Box::new(Expr::StringLit(pat)),
                ));
            } else {
                let expr = self.parse_primary()?;
                return Ok(Expr::NotMatch(Box::new(left), Box::new(expr)));
            }
        }

        Ok(left)
    }

    fn parse_comparison(&mut self) -> Result<Expr, FkError> {
        let mut left = self.parse_concatenation()?;

        loop {
            let op = match self.current() {
                Token::Eq => BinOp::Eq,
                Token::Ne => BinOp::Ne,
                Token::Lt => BinOp::Lt,
                Token::Le => BinOp::Le,
                Token::Gt if !self.in_print_expr => BinOp::Gt,
                Token::Ge if !self.in_print_expr => BinOp::Ge,
                _ => break,
            };
            self.advance();
            let right = self.parse_concatenation()?;
            left = Expr::BinOp(Box::new(left), op, Box::new(right));
        }

        Ok(left)
    }

    fn parse_concatenation(&mut self) -> Result<Expr, FkError> {
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
                | Token::Dollar
                | Token::LParen
                | Token::Not
        )
    }

    fn parse_addition(&mut self) -> Result<Expr, FkError> {
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

    fn parse_multiplication(&mut self) -> Result<Expr, FkError> {
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
    fn parse_exponentiation(&mut self) -> Result<Expr, FkError> {
        let base = self.parse_unary()?;

        if self.check(&Token::Power) {
            self.advance();
            let exp = self.parse_exponentiation()?; // right-associative
            Ok(Expr::BinOp(Box::new(base), BinOp::Pow, Box::new(exp)))
        } else {
            Ok(base)
        }
    }

    fn parse_unary(&mut self) -> Result<Expr, FkError> {
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

    fn parse_postfix(&mut self) -> Result<Expr, FkError> {
        let mut expr = self.parse_primary()?;

        loop {
            if self.check(&Token::LBracket) {
                // Array subscript — supports multi-dimensional a[i,j] → a[i SUBSEP j]
                if let Expr::Var(name) = expr {
                    self.advance();
                    let mut key = self.parse_expr()?;
                    while self.check(&Token::Comma) {
                        self.advance();
                        let next = self.parse_expr()?;
                        key = Expr::Concat(
                            Box::new(Expr::Concat(
                                Box::new(key),
                                Box::new(Expr::Var("SUBSEP".to_string())),
                            )),
                            Box::new(next),
                        );
                    }
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

    fn parse_primary(&mut self) -> Result<Expr, FkError> {
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
            Token::Dollar => {
                self.advance();
                let expr = self.parse_unary()?;
                Ok(Expr::Field(Box::new(expr)))
            }
            Token::Regex(pat) => {
                self.advance();
                Ok(Expr::Match(
                    Box::new(Expr::Field(Box::new(Expr::NumberLit(0.0)))),
                    Box::new(Expr::StringLit(pat)),
                ))
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

                // Bare `length` without parens → length($0)
                if name == "length" {
                    return Ok(Expr::FuncCall(
                        name,
                        vec![Expr::Field(Box::new(Expr::NumberLit(0.0)))],
                    ));
                }

                Ok(Expr::Var(name))
            }
            Token::Getline => {
                self.advance();
                // getline [var] [< file]
                let var = if let Token::Ident(name) = self.current().clone() {
                    let saved = self.pos;
                    self.advance();
                    if self.check(&Token::Lt)
                        || self.is_terminator()
                        || self.check(&Token::RBrace)
                        || self.check(&Token::Semicolon)
                        || self.check(&Token::RParen)
                        || self.at_eof()
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
                let saved_print = self.in_print_expr;
                self.in_print_expr = false;
                let expr = self.parse_expr()?;
                self.in_print_expr = saved_print;
                if self.check(&Token::Comma) {
                    // (expr, expr, ...) in array — multi-dimensional key
                    let mut parts = vec![expr];
                    while self.check(&Token::Comma) {
                        self.advance();
                        parts.push(self.parse_expr()?);
                    }
                    self.expect(&Token::RParen)?;
                    let key = Self::join_subsep(parts);
                    if self.check(&Token::In) {
                        self.advance();
                        if let Token::Ident(arr) = self.current().clone() {
                            self.advance();
                            return Ok(Expr::ArrayIn(Box::new(key), arr));
                        } else {
                            return Err(FkError::new(
                                self.current_span(),
                                "expected array name after 'in'",
                            ));
                        }
                    }
                    return Err(FkError::new(
                        self.current_span(),
                        "(expr, expr) requires 'in array'",
                    ));
                }
                self.expect(&Token::RParen)?;
                Ok(expr)
            }
            _ => Err(FkError::new(
                self.current_span(),
                format!("unexpected token: {:?}", self.current()),
            )),
        }
    }

    fn parse_sprintf_args(&mut self) -> Result<Expr, FkError> {
        let span = self.current_span();
        self.expect(&Token::LParen)?;
        let mut args = Vec::new();
        self.skip_newlines();
        if !self.check(&Token::RParen) {
            args.push(self.parse_expr()?);
            self.skip_newlines();
            while self.check(&Token::Comma) {
                self.advance();
                self.skip_newlines();
                args.push(self.parse_expr()?);
                self.skip_newlines();
            }
        }
        self.expect(&Token::RParen)?;
        if args.is_empty() {
            return Err(FkError::new(span, "sprintf requires a format string"));
        }
        Ok(Expr::Sprintf(args))
    }

    fn parse_func_call(&mut self, name: String) -> Result<Expr, FkError> {
        self.expect(&Token::LParen)?;
        let mut args = Vec::new();
        self.skip_newlines();
        if !self.check(&Token::RParen) {
            args.push(self.parse_expr()?);
            self.skip_newlines();
            while self.check(&Token::Comma) {
                self.advance();
                self.skip_newlines();
                args.push(self.parse_expr()?);
                self.skip_newlines();
            }
        }
        self.expect(&Token::RParen)?;
        Ok(Expr::FuncCall(name, args))
    }

    /// Build a SUBSEP-concatenated key from multiple expressions.
    fn join_subsep(parts: Vec<Expr>) -> Expr {
        let mut iter = parts.into_iter();
        let mut key = iter.next().unwrap();
        for part in iter {
            key = Expr::Concat(
                Box::new(Expr::Concat(
                    Box::new(key),
                    Box::new(Expr::Var("SUBSEP".to_string())),
                )),
                Box::new(part),
            );
        }
        key
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

    fn expect(&mut self, token: &Token) -> Result<(), FkError> {
        if self.check(token) {
            self.advance();
            Ok(())
        } else {
            Err(FkError::new(
                self.current_span(),
                format!("expected {:?}, got {:?}", token, self.current()),
            ))
        }
    }

    fn is_terminator(&self) -> bool {
        matches!(
            self.current(),
            Token::Semicolon | Token::Newline | Token::Eof
        )
    }

    fn skip_terminators(&mut self) {
        while matches!(self.current(), Token::Semicolon | Token::Newline) {
            self.advance();
        }
    }

    fn skip_newlines(&mut self) {
        while matches!(self.current(), Token::Newline) {
            self.advance();
        }
    }
}
