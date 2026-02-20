//! Pretty-print fk programs: reasonable line-breaking and indentation.

use crate::error::FkError;
use crate::lexer::Lexer;
use crate::parser::Parser;
use crate::parser::{
    BinOp, Block, Expr, FuncDef, Pattern, Program, Redirect, Rule, SortMode, Statement,
};
use std::fmt::Write;

const INDENT: &str = "  ";

/// Format source code: parse then pretty-print with indentation.
pub fn format_program(source: &str) -> Result<String, FkError> {
    let mut lexer = Lexer::new(source);
    let tokens = lexer.tokenize()?;
    let mut parser = Parser::new(tokens);
    let program = parser.parse()?;
    Ok(pretty_program(&program))
}

/// Format an already-parsed program.
#[must_use]
pub fn pretty_program(program: &Program) -> String {
    let mut p = Pretty {
        out: String::new(),
        indent: 0,
        indent_cache: String::new(),
    };
    p.program(program);
    p.out
}

struct Pretty {
    out: String,
    indent: usize,
    /// Cached indent string (grown as needed) to avoid repeated INDENT.repeat().
    indent_cache: String,
}

impl Pretty {
    fn nl(&mut self) {
        self.out.push('\n');
    }

    fn space(&mut self) {
        self.out.push(' ');
    }

    fn ensure_indent_cache(&mut self) {
        let need = self.indent * INDENT.len();
        while self.indent_cache.len() < need {
            self.indent_cache.push_str(INDENT);
        }
    }

    /// Append current indent to output. Call after changing indent; ensures cache is grown.
    fn write_indent(&mut self) {
        self.ensure_indent_cache();
        let end = self.indent * INDENT.len();
        self.out.push_str(&self.indent_cache[..end]);
    }

    fn program(&mut self, prog: &Program) {
        if let Some(ref b) = prog.begin {
            self.keyword("BEGIN");
            self.space();
            self.block(b);
            self.nl();
        }
        for rule in &prog.rules {
            self.rule(rule);
            self.nl();
        }
        if let Some(ref b) = prog.end {
            self.keyword("END");
            self.space();
            self.block(b);
            self.nl();
        }
        if let Some(ref b) = prog.beginfile {
            self.keyword("BEGINFILE");
            self.space();
            self.block(b);
            self.nl();
        }
        if let Some(ref b) = prog.endfile {
            self.keyword("ENDFILE");
            self.space();
            self.block(b);
            self.nl();
        }
        for func in &prog.functions {
            self.func_def(func);
            self.nl();
        }
        // Trim trailing newline from last block
        if self.out.ends_with('\n') {
            self.out.pop();
        }
    }

    fn rule(&mut self, rule: &Rule) {
        if let Some(ref pat) = rule.pattern {
            self.pattern(pat);
            self.space();
        }
        self.block(&rule.action);
    }

    fn pattern(&mut self, pat: &Pattern) {
        match pat {
            Pattern::Regex(s) => {
                self.out.push('/');
                self.out.push_str(&escape_regex(s));
                self.out.push('/');
            }
            Pattern::Expression(e) => self.expr(e, 0),
            Pattern::Range(a, b) => {
                self.pattern(a);
                self.out.push(',');
                self.space();
                self.pattern(b);
            }
            Pattern::Last(e) => {
                self.out.push_str("last ");
                self.expr(e, 0);
            }
        }
    }

    fn block(&mut self, block: &Block) {
        self.out.push('{');
        if block.is_empty() {
            self.out.push('}');
            return;
        }
        self.nl();
        self.indent += 1;
        for (i, stmt) in block.iter().enumerate() {
            self.write_indent();
            self.stmt(stmt);
            if i + 1 < block.len() {
                self.nl();
            }
        }
        self.indent -= 1;
        self.nl();
        self.write_indent();
        self.out.push('}');
    }

    fn keyword(&mut self, k: &str) {
        self.out.push_str(k);
    }

    fn stmt(&mut self, s: &Statement) {
        match s {
            Statement::Print(args, redir) => {
                self.keyword("print");
                for (i, a) in args.iter().enumerate() {
                    if i > 0 {
                        self.out.push(',');
                        self.space();
                    }
                    self.expr(a, 0);
                }
                self.redirect(redir);
            }
            Statement::Printf(args, redir) => {
                self.keyword("printf");
                for (i, a) in args.iter().enumerate() {
                    if i > 0 {
                        self.out.push(',');
                        self.space();
                    }
                    self.expr(a, 0);
                }
                self.redirect(redir);
            }
            Statement::If(cond, then_b, else_b) => {
                self.keyword("if");
                self.space();
                self.out.push('(');
                self.expr(cond, 0);
                self.out.push(')');
                self.space();
                if then_b.len() == 1 && !matches!(then_b[0], Statement::Block(_)) {
                    self.stmt(&then_b[0]);
                } else {
                    self.block(then_b);
                }
                if let Some(eb) = else_b {
                    self.space();
                    self.keyword("else");
                    self.space();
                    if eb.len() == 1 && !matches!(eb[0], Statement::Block(_)) {
                        self.stmt(&eb[0]);
                    } else {
                        self.block(eb);
                    }
                }
            }
            Statement::While(cond, body) => {
                self.keyword("while");
                self.space();
                self.out.push('(');
                self.expr(cond, 0);
                self.out.push(')');
                self.space();
                if body.len() == 1 && !matches!(body[0], Statement::Block(_)) {
                    self.stmt(&body[0]);
                } else {
                    self.block(body);
                }
            }
            Statement::DoWhile(body, cond) => {
                self.keyword("do");
                self.space();
                if body.len() == 1 && !matches!(body[0], Statement::Block(_)) {
                    self.stmt(&body[0]);
                } else {
                    self.block(body);
                }
                self.space();
                self.keyword("while");
                self.space();
                self.out.push('(');
                self.expr(cond, 0);
                self.out.push(')');
            }
            Statement::For(init, cond, update, body) => {
                self.keyword("for");
                self.space();
                self.out.push('(');
                if let Some(i) = init {
                    self.stmt(i);
                }
                self.out.push(';');
                self.space();
                if let Some(c) = cond {
                    self.expr(c, 0);
                }
                self.out.push(';');
                self.space();
                if let Some(u) = update {
                    self.stmt(u);
                }
                self.out.push(')');
                self.space();
                if body.len() == 1 && !matches!(body[0], Statement::Block(_)) {
                    self.stmt(&body[0]);
                } else {
                    self.block(body);
                }
            }
            Statement::ForIn(var, arr, sort_mode, body) => {
                self.keyword("for");
                self.space();
                self.out.push('(');
                self.out.push_str(var);
                self.space();
                self.keyword("in");
                self.space();
                self.out.push_str(arr);
                self.out.push(')');
                if let Some(mode) = sort_mode {
                    self.out.push(' ');
                    self.out.push('@');
                    self.out.push_str(match mode {
                        SortMode::Asc => "sort",
                        SortMode::Desc => "rsort",
                        SortMode::NumAsc => "nsort",
                        SortMode::NumDesc => "rnsort",
                        SortMode::ValAsc => "val",
                        SortMode::ValDesc => "rval",
                    });
                }
                self.space();
                if body.len() == 1 && !matches!(body[0], Statement::Block(_)) {
                    self.stmt(&body[0]);
                } else {
                    self.block(body);
                }
            }
            Statement::Delete(name, key) => {
                self.keyword("delete");
                self.space();
                self.out.push_str(name);
                self.out.push('[');
                self.expr(key, 0);
                self.out.push(']');
            }
            Statement::DeleteAll(name) => {
                self.keyword("delete");
                self.space();
                self.out.push_str(name);
            }
            Statement::Next => self.keyword("next"),
            Statement::Nextfile => self.keyword("nextfile"),
            Statement::Break => self.keyword("break"),
            Statement::Continue => self.keyword("continue"),
            Statement::Exit(Some(e)) => {
                self.keyword("exit");
                self.space();
                self.expr(e, 0);
            }
            Statement::Exit(None) => self.keyword("exit"),
            Statement::Return(Some(e)) => {
                self.keyword("return");
                self.space();
                self.expr(e, 0);
            }
            Statement::Return(None) => self.keyword("return"),
            Statement::Block(b) => self.block(b),
            Statement::Expression(e) => self.expr(e, 0),
        }
    }

    fn redirect(&mut self, redir: &Option<Redirect>) {
        if let Some(r) = redir {
            self.space();
            match r {
                Redirect::Overwrite(e) => {
                    self.out.push('>');
                    self.expr(e, 0);
                }
                Redirect::Append(e) => {
                    self.out.push('>');
                    self.out.push('>');
                    self.expr(e, 0);
                }
                Redirect::Pipe(e) => {
                    self.out.push('|');
                    self.expr(e, 0);
                }
            }
        }
    }

    fn func_def(&mut self, f: &FuncDef) {
        self.keyword("function");
        self.space();
        self.out.push_str(&f.name);
        self.out.push('(');
        for (i, p) in f.params.iter().enumerate() {
            if i > 0 {
                self.out.push(',');
                self.space();
            }
            self.out.push_str(p);
        }
        self.out.push(')');
        self.space();
        self.block(&f.body);
    }

    fn expr(&mut self, e: &Expr, _prec: u8) {
        match e {
            Expr::Field(sub) => {
                self.out.push('$');
                if let Expr::NumberLit(n) = sub.as_ref()
                    && *n >= 0.0
                    && n.fract() == 0.0
                {
                    self.out.push_str(&(*n as i64).to_string());
                    return;
                }
                self.expr(sub, 0);
            }
            Expr::NumberLit(n) => {
                if n.fract() == 0.0 && *n >= 0.0 {
                    self.out.push_str(&(*n as i64).to_string());
                } else {
                    let _ = write!(self.out, "{}", n);
                }
            }
            Expr::StringLit(s) => {
                self.out.push('"');
                self.out.push_str(&escape_string(s));
                self.out.push('"');
            }
            Expr::Var(name) => self.out.push_str(name),
            Expr::ArrayRef(name, key) => {
                self.out.push_str(name);
                self.out.push('[');
                self.expr(key, 0);
                self.out.push(']');
            }
            Expr::ArrayIn(expr, arr) => {
                self.expr(expr, 0);
                self.space();
                self.keyword("in");
                self.space();
                self.out.push_str(arr);
            }
            Expr::BinOp(l, op, r) => {
                self.expr(l, 0);
                self.space();
                self.out.push_str(binop_str(op));
                self.space();
                self.expr(r, 0);
            }
            Expr::LogicalAnd(l, r) => {
                self.expr(l, 0);
                self.space();
                self.out.push_str("&&");
                self.space();
                self.expr(r, 0);
            }
            Expr::LogicalOr(l, r) => {
                self.expr(l, 0);
                self.space();
                self.out.push_str("||");
                self.space();
                self.expr(r, 0);
            }
            Expr::LogicalNot(x) => {
                self.out.push('!');
                self.expr(x, 0);
            }
            Expr::Match(l, r) => {
                self.expr(l, 0);
                self.space();
                self.out.push('~');
                self.space();
                self.expr(r, 0);
            }
            Expr::NotMatch(l, r) => {
                self.expr(l, 0);
                self.space();
                self.out.push_str("!~");
                self.space();
                self.expr(r, 0);
            }
            Expr::Assign(l, r) => {
                self.expr(l, 0);
                self.space();
                self.out.push('=');
                self.space();
                self.expr(r, 0);
            }
            Expr::CompoundAssign(l, op, r) => {
                self.expr(l, 0);
                self.space();
                self.out.push_str(compound_assign_str(op));
                self.space();
                self.expr(r, 0);
            }
            Expr::Increment(x, true) => {
                self.out.push_str("++");
                self.expr(x, 0);
            }
            Expr::Increment(x, false) => {
                self.expr(x, 0);
                self.out.push_str("++");
            }
            Expr::Decrement(x, true) => {
                self.out.push_str("--");
                self.expr(x, 0);
            }
            Expr::Decrement(x, false) => {
                self.expr(x, 0);
                self.out.push_str("--");
            }
            Expr::UnaryMinus(x) => {
                self.out.push('-');
                self.expr(x, 0);
            }
            Expr::Concat(l, r) => {
                self.expr(l, 0);
                self.space();
                self.expr(r, 0);
            }
            Expr::Ternary(cond, then_e, else_e) => {
                self.expr(cond, 0);
                self.space();
                self.out.push('?');
                self.space();
                self.expr(then_e, 0);
                self.space();
                self.out.push(':');
                self.space();
                self.expr(else_e, 0);
            }
            Expr::Sprintf(args) => {
                self.out.push_str("sprintf");
                self.out.push('(');
                for (i, a) in args.iter().enumerate() {
                    if i > 0 {
                        self.out.push(',');
                        self.space();
                    }
                    self.expr(a, 0);
                }
                self.out.push(')');
            }
            Expr::FuncCall(name, args) => {
                self.out.push_str(name);
                self.out.push('(');
                for (i, a) in args.iter().enumerate() {
                    if i > 0 {
                        self.out.push(',');
                        self.space();
                    }
                    self.expr(a, 0);
                }
                self.out.push(')');
            }
            Expr::Getline(var, source) => {
                self.keyword("getline");
                if let Some(v) = var {
                    self.space();
                    self.out.push_str(v);
                }
                if let Some(src) = source {
                    self.space();
                    self.out.push('<');
                    self.space();
                    self.expr(src, 0);
                }
            }
            Expr::GetlinePipe(cmd, var) => {
                self.expr(cmd, 0);
                self.space();
                self.out.push('|');
                self.space();
                self.keyword("getline");
                if let Some(v) = var {
                    self.space();
                    self.out.push_str(v);
                }
            }
        }
    }
}

fn binop_str(op: &BinOp) -> &'static str {
    match *op {
        BinOp::Add => "+",
        BinOp::Sub => "-",
        BinOp::Mul => "*",
        BinOp::Div => "/",
        BinOp::Mod => "%",
        BinOp::Pow => "**",
        BinOp::Eq => "==",
        BinOp::Ne => "!=",
        BinOp::Lt => "<",
        BinOp::Le => "<=",
        BinOp::Gt => ">",
        BinOp::Ge => ">=",
    }
}

fn compound_assign_str(op: &BinOp) -> &'static str {
    match *op {
        BinOp::Add => "+=",
        BinOp::Sub => "-=",
        BinOp::Mul => "*=",
        BinOp::Div => "/=",
        BinOp::Mod => "%=",
        BinOp::Pow => "**=",
        _ => "=",
    }
}

fn escape_string(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    for c in s.chars() {
        match c {
            '\\' => out.push_str("\\\\"),
            '"' => out.push_str("\\\""),
            '\n' => out.push_str("\\n"),
            '\t' => out.push_str("\\t"),
            _ => out.push(c),
        }
    }
    out
}

fn escape_regex(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    for c in s.chars() {
        match c {
            '\\' => out.push_str("\\\\"),
            '/' => out.push_str("\\/"),
            _ => out.push(c),
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::format_program;

    #[test]
    fn format_simple_program() {
        let src = "{ print $1 }";
        let out = format_program(src).unwrap();
        assert!(out.contains("print"));
        assert!(out.contains("$1"));
        assert!(out.contains('{') && out.contains('}'));
    }

    #[test]
    fn format_begin_end_indented() {
        let src = "BEGIN{print 1}END{print 2}";
        let out = format_program(src).unwrap();
        assert!(out.contains("BEGIN"));
        assert!(out.contains("END"));
        assert!(out.contains('\n'));
    }
}
