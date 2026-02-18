use crate::parser::*;

/// Static analysis of a parsed program — flags that let the executor
/// skip unnecessary work on the per-record hot path.
#[derive(Debug)]
pub struct ProgramInfo {
    /// Program accesses $1…$N (not just $0).
    pub needs_fields: bool,
    /// Program reads the NF variable.
    pub needs_nf: bool,
    /// Highest constant field index seen (None = dynamic $expr access).
    /// Only meaningful when needs_fields is true.
    pub max_field: Option<usize>,
    /// All regex literal strings found in patterns and expressions,
    /// suitable for pre-compilation into the regex cache.
    pub regex_literals: Vec<String>,
}

pub fn analyze(program: &Program) -> ProgramInfo {
    let mut info = ProgramInfo {
        needs_fields: false,
        needs_nf: false,
        max_field: Some(0),
        regex_literals: Vec::new(),
    };

    if let Some(block) = &program.begin {
        walk_block(block, &mut info);
    }
    for rule in &program.rules {
        if let Some(pat) = &rule.pattern {
            walk_pattern(pat, &mut info);
        }
        walk_block(&rule.action, &mut info);
    }
    if let Some(block) = &program.end {
        walk_block(block, &mut info);
    }
    if let Some(block) = &program.beginfile {
        walk_block(block, &mut info);
    }
    if let Some(block) = &program.endfile {
        walk_block(block, &mut info);
    }
    for func in &program.functions {
        walk_block(&func.body, &mut info);
    }

    if !info.needs_fields {
        info.max_field = None;
    }

    info
}

fn walk_block(block: &Block, info: &mut ProgramInfo) {
    for stmt in block {
        walk_stmt(stmt, info);
    }
}

fn walk_stmt(stmt: &Statement, info: &mut ProgramInfo) {
    match stmt {
        Statement::Print(exprs, redir) | Statement::Printf(exprs, redir) => {
            for e in exprs { walk_expr(e, info); }
            if let Some(r) = redir { walk_redirect(r, info); }
        }
        Statement::If(cond, then_b, else_b) => {
            walk_expr(cond, info);
            walk_block(then_b, info);
            if let Some(eb) = else_b { walk_block(eb, info); }
        }
        Statement::While(cond, body) | Statement::DoWhile(body, cond) => {
            walk_expr(cond, info);
            walk_block(body, info);
        }
        Statement::For(init, cond, update, body) => {
            if let Some(s) = init { walk_stmt(s, info); }
            if let Some(e) = cond { walk_expr(e, info); }
            if let Some(s) = update { walk_stmt(s, info); }
            walk_block(body, info);
        }
        Statement::ForIn(_, _, body) => walk_block(body, info),
        Statement::Delete(_, e) => walk_expr(e, info),
        Statement::Exit(Some(e)) => walk_expr(e, info),
        Statement::Return(Some(e)) => walk_expr(e, info),
        Statement::Block(b) => walk_block(b, info),
        Statement::Expression(e) => walk_expr(e, info),
        Statement::DeleteAll(_) | Statement::Next | Statement::Nextfile
        | Statement::Break | Statement::Continue
        | Statement::Exit(None) | Statement::Return(None) => {}
    }
}

fn walk_expr(expr: &Expr, info: &mut ProgramInfo) {
    match expr {
        Expr::Field(inner) => {
            match inner.as_ref() {
                Expr::NumberLit(n) => {
                    let idx = *n as isize;
                    if idx != 0 {
                        info.needs_fields = true;
                        if idx > 0 && let Some(ref mut max) = info.max_field {
                            let u = idx as usize;
                            if u > *max { *max = u; }
                        }
                    }
                }
                _ => {
                    info.needs_fields = true;
                    info.max_field = None;
                    walk_expr(inner, info);
                }
            }
        }
        Expr::Var(name) => {
            if name == "NF" {
                info.needs_nf = true;
            }
        }
        Expr::Getline(None, source) => {
            info.needs_fields = true;
            info.max_field = None;
            if let Some(e) = source { walk_expr(e, info); }
        }
        Expr::Getline(Some(_), source) => {
            if let Some(e) = source { walk_expr(e, info); }
        }
        Expr::GetlinePipe(cmd, _) => walk_expr(cmd, info),
        Expr::ArrayRef(_, key) => walk_expr(key, info),
        Expr::ArrayIn(key, _) => walk_expr(key, info),
        Expr::Assign(target, val) | Expr::CompoundAssign(target, _, val) => {
            if matches!(target.as_ref(), Expr::Field(inner) if !matches!(inner.as_ref(), Expr::NumberLit(n) if *n == 0.0))
            {
                info.max_field = None;
            }
            walk_expr(target, info);
            walk_expr(val, info);
        }
        Expr::BinOp(l, _, r) | Expr::LogicalAnd(l, r)
        | Expr::LogicalOr(l, r) | Expr::Concat(l, r)
        | Expr::Match(l, r) | Expr::NotMatch(l, r) => {
            walk_expr(l, info);
            walk_expr(r, info);
        }
        Expr::LogicalNot(e) | Expr::UnaryMinus(e)
        | Expr::Increment(e, _) | Expr::Decrement(e, _) => {
            walk_expr(e, info);
        }
        Expr::Ternary(c, t, f) => {
            walk_expr(c, info);
            walk_expr(t, info);
            walk_expr(f, info);
        }
        Expr::Sprintf(args) | Expr::FuncCall(_, args) => {
            for a in args { walk_expr(a, info); }
        }
        Expr::NumberLit(_) | Expr::StringLit(_) => {}
    }
}

fn walk_pattern(pattern: &Pattern, info: &mut ProgramInfo) {
    match pattern {
        Pattern::Regex(s) => {
            if !info.regex_literals.contains(s) {
                info.regex_literals.push(s.clone());
            }
        }
        Pattern::Expression(e) => walk_expr(e, info),
        Pattern::Range(a, b) => {
            walk_pattern(a, info);
            walk_pattern(b, info);
        }
    }
}

fn walk_redirect(redir: &Redirect, info: &mut ProgramInfo) {
    match redir {
        Redirect::Overwrite(e) | Redirect::Append(e) | Redirect::Pipe(e) => {
            walk_expr(e, info);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::lexer::Lexer;
    use crate::parser::Parser;

    fn analyze_program(src: &str) -> ProgramInfo {
        let tokens = Lexer::new(src).tokenize().unwrap();
        let program = Parser::new(tokens).parse().unwrap();
        analyze(&program)
    }

    #[test]
    fn pattern_print_no_fields() {
        let info = analyze_program("/foo/ { print }");
        assert!(!info.needs_fields, "print $0 should not need fields");
        assert!(!info.needs_nf);
    }

    #[test]
    fn field_access_needs_fields() {
        let info = analyze_program("{ print $2 }");
        assert!(info.needs_fields);
        assert_eq!(info.max_field, Some(2));
    }

    #[test]
    fn nf_access_sets_flag() {
        let info = analyze_program("{ print NF }");
        assert!(info.needs_nf);
    }

    #[test]
    fn dynamic_field_clears_hint() {
        let info = analyze_program("{ print $i }");
        assert!(info.needs_fields);
        assert!(info.max_field.is_none());
    }

    #[test]
    fn max_field_tracks_highest() {
        let info = analyze_program("{ print $1, $5 }");
        assert!(info.needs_fields);
        assert_eq!(info.max_field, Some(5));
    }

    #[test]
    fn regex_pattern_collected() {
        let info = analyze_program("/^start/ { print }");
        assert!(info.regex_literals.contains(&"^start".to_string()));
    }

    #[test]
    fn gsub_on_dollar0_no_fields() {
        let info = analyze_program("{ gsub(/x/, \"y\"); print }");
        assert!(!info.needs_fields);
    }

    #[test]
    fn getline_no_var_needs_fields() {
        let info = analyze_program("{ getline; print }");
        assert!(info.needs_fields);
    }

    #[test]
    fn getline_into_var_no_fields() {
        let info = analyze_program("{ getline line; print line }");
        assert!(!info.needs_fields);
    }

    #[test]
    fn function_body_analyzed() {
        let info = analyze_program("function f() { print $3 } { f() }");
        assert!(info.needs_fields);
        assert_eq!(info.max_field, Some(3));
    }

    #[test]
    fn begin_end_only_no_fields() {
        let info = analyze_program("BEGIN { print \"hello\" }");
        assert!(!info.needs_fields);
        assert!(!info.needs_nf);
    }
}
