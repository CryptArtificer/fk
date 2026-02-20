use std::collections::HashMap;
use std::fmt::Write;

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
    /// For each array, the RHS expression of its first direct assignment
    /// (e.g. `a[NR] = $3` stores `$3`). Used to derive plot titles.
    pub array_sources: HashMap<String, Expr>,
    /// Simple variable assignments (first `v = expr` seen per var).
    /// Used to resolve one level of indirection in array source tracking.
    pub var_sources: HashMap<String, Expr>,
}

pub fn analyze(program: &Program) -> ProgramInfo {
    let mut info = ProgramInfo {
        needs_fields: false,
        needs_nf: false,
        max_field: Some(0),
        regex_literals: Vec::new(),
        array_sources: HashMap::new(),
        var_sources: HashMap::new(),
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
            for e in exprs {
                walk_expr(e, info);
            }
            if let Some(r) = redir {
                walk_redirect(r, info);
            }
        }
        Statement::If(cond, then_b, else_b) => {
            walk_expr(cond, info);
            walk_block(then_b, info);
            if let Some(eb) = else_b {
                walk_block(eb, info);
            }
        }
        Statement::While(cond, body) | Statement::DoWhile(body, cond) => {
            walk_expr(cond, info);
            walk_block(body, info);
        }
        Statement::For(init, cond, update, body) => {
            if let Some(s) = init {
                walk_stmt(s, info);
            }
            if let Some(e) = cond {
                walk_expr(e, info);
            }
            if let Some(s) = update {
                walk_stmt(s, info);
            }
            walk_block(body, info);
        }
        Statement::ForIn(_, _, body) => walk_block(body, info),
        Statement::Delete(_, e) => walk_expr(e, info),
        Statement::Exit(Some(e)) => walk_expr(e, info),
        Statement::Return(Some(e)) => walk_expr(e, info),
        Statement::Block(b) => walk_block(b, info),
        Statement::Expression(e) => walk_expr(e, info),
        Statement::DeleteAll(_)
        | Statement::Next
        | Statement::Nextfile
        | Statement::Break
        | Statement::Continue
        | Statement::Exit(None)
        | Statement::Return(None) => {}
    }
}

fn walk_expr(expr: &Expr, info: &mut ProgramInfo) {
    match expr {
        Expr::Field(inner) => match inner.as_ref() {
            Expr::NumberLit(n) => {
                let idx = *n as isize;
                if idx != 0 {
                    info.needs_fields = true;
                    if idx > 0
                        && let Some(ref mut max) = info.max_field
                    {
                        let u = idx as usize;
                        if u > *max {
                            *max = u;
                        }
                    }
                }
            }
            _ => {
                info.needs_fields = true;
                info.max_field = None;
                walk_expr(inner, info);
            }
        },
        Expr::Var(name) => {
            if name == "NF" {
                info.needs_nf = true;
            }
        }
        Expr::Getline(None, source) => {
            info.needs_fields = true;
            info.max_field = None;
            if let Some(e) = source {
                walk_expr(e, info);
            }
        }
        Expr::Getline(Some(_), source) => {
            if let Some(e) = source {
                walk_expr(e, info);
            }
        }
        Expr::GetlinePipe(cmd, _) => walk_expr(cmd, info),
        Expr::ArrayRef(_, key) => walk_expr(key, info),
        Expr::ArrayIn(key, _) => walk_expr(key, info),
        Expr::Assign(target, val) | Expr::CompoundAssign(target, _, val) => {
            match target.as_ref() {
                Expr::ArrayRef(name, _) => {
                    info.array_sources
                        .entry(name.clone())
                        .or_insert_with(|| val.as_ref().clone());
                }
                Expr::Var(name) if matches!(expr, Expr::Assign(..)) => {
                    info.var_sources
                        .entry(name.clone())
                        .or_insert_with(|| val.as_ref().clone());
                }
                _ => {}
            }
            if matches!(target.as_ref(), Expr::Field(inner) if !matches!(inner.as_ref(), Expr::NumberLit(n) if *n == 0.0))
            {
                info.max_field = None;
            }
            walk_expr(target, info);
            walk_expr(val, info);
        }
        Expr::BinOp(l, _, r)
        | Expr::LogicalAnd(l, r)
        | Expr::LogicalOr(l, r)
        | Expr::Concat(l, r)
        | Expr::Match(l, r)
        | Expr::NotMatch(l, r) => {
            walk_expr(l, info);
            walk_expr(r, info);
        }
        Expr::LogicalNot(e)
        | Expr::UnaryMinus(e)
        | Expr::Increment(e, _)
        | Expr::Decrement(e, _) => {
            walk_expr(e, info);
        }
        Expr::Ternary(c, t, f) => {
            walk_expr(c, info);
            walk_expr(t, info);
            walk_expr(f, info);
        }
        Expr::Sprintf(args) | Expr::FuncCall(_, args) => {
            for a in args {
                walk_expr(a, info);
            }
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

// ── Expression formatter & smart title builder ──────────────────────

/// Strip `+ 0` / `0 +` numeric coercion wrappers.
pub(crate) fn unwrap_coercion(expr: &Expr) -> &Expr {
    if let Expr::BinOp(l, BinOp::Add, r) = expr {
        if matches!(r.as_ref(), Expr::NumberLit(n) if *n == 0.0) {
            return unwrap_coercion(l);
        }
        if matches!(l.as_ref(), Expr::NumberLit(n) if *n == 0.0) {
            return unwrap_coercion(r);
        }
    }
    expr
}

/// Build a human-readable data-source description from an array's RHS
/// expression and the current FILENAME.  Used as a chart subtitle.
///
/// The `budget` parameter caps the result length (in characters).
///
/// Smart cases:
///   jpath($0, ".ms") + 0  from api.jsonl  →  "ms — api.jsonl"
///   $3                     from data.csv   →  "column 3 — data.csv"
///   $"latency"             from data.csv   →  "latency — data.csv"
///   $1 + $2                from data.csv   →  "columns 1–2 — data.csv"
///   $1                     from stdin      →  "column 1"
pub fn build_array_description(
    expr: &Expr,
    filename: &str,
    var_sources: &HashMap<String, Expr>,
    budget: usize,
) -> String {
    let expr = unwrap_coercion(expr);
    let expr = if let Expr::Var(name) = expr {
        var_sources.get(name).map_or(expr, |e| unwrap_coercion(e))
    } else {
        expr
    };
    let base = friendly_filename(filename);

    let source_part = humanize_expr(expr);
    if base.is_empty() {
        return source_part;
    }
    let full = format!("{source_part} — {base}");
    if full.chars().count() <= budget {
        full
    } else {
        source_part
    }
}

fn humanize_expr(expr: &Expr) -> String {
    if let Expr::FuncCall(name, args) = expr
        && name == "jpath"
        && args.len() >= 2
        && matches!(&args[0], Expr::Field(inner)
            if matches!(inner.as_ref(), Expr::NumberLit(n) if *n == 0.0))
        && let Expr::StringLit(path) = &args[1]
    {
        return path.trim_start_matches('.').to_string();
    }

    if let Expr::Field(inner) = expr {
        match inner.as_ref() {
            Expr::NumberLit(n) => {
                let n = *n as i64;
                return if n > 0 {
                    format!("column {n}")
                } else {
                    format!("${n}")
                };
            }
            Expr::StringLit(col) => return col.clone(),
            _ => {}
        }
    }

    humanize_fields(&expr_to_source(expr))
}

fn humanize_fields(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    let bytes = s.as_bytes();
    let mut i = 0;
    while i < bytes.len() {
        if bytes[i] == b'$' {
            let start = i + 1;
            if start < bytes.len() && bytes[start] == b'"' {
                let name_start = start + 1;
                if let Some(end) = s[name_start..].find('"') {
                    out.push_str(&s[name_start..name_start + end]);
                    i = name_start + end + 1;
                    continue;
                }
            }
            let mut end = start;
            while end < bytes.len() && bytes[end].is_ascii_digit() {
                end += 1;
            }
            if end > start {
                let n: i64 = s[start..end].parse().unwrap_or(0);
                if n > 0 {
                    out.push_str("column ");
                }
                out.push_str(&s[start..end]);
                i = end;
                continue;
            }
            let mut end = start;
            while end < bytes.len()
                && (bytes[end].is_ascii_alphanumeric() || bytes[end] == b'_' || bytes[end] == b'-')
            {
                end += 1;
            }
            if end > start {
                out.push_str(&s[start..end]);
                i = end;
                continue;
            }
        }
        out.push(bytes[i] as char);
        i += 1;
    }
    out
}

fn friendly_filename(filename: &str) -> &str {
    if filename.is_empty() || filename == "-" || filename == "/dev/stdin" {
        ""
    } else {
        std::path::Path::new(filename)
            .file_name()
            .and_then(|f| f.to_str())
            .unwrap_or(filename)
    }
}

// (explain functionality moved to src/explain/ module)

/// Format an Expr back to readable fk source (truncated at 80 chars).
pub fn expr_to_source(expr: &Expr) -> String {
    let mut buf = String::new();
    fmt_expr(expr, &mut buf, 0);
    if buf.len() > 80 {
        buf.truncate(77);
        buf.push_str("...");
    }
    buf
}

fn is_dollar_zero(expr: &Expr) -> bool {
    matches!(expr, Expr::Field(inner) if matches!(inner.as_ref(), Expr::NumberLit(n) if *n == 0.0))
}

fn fmt_regex_lit_or_expr(expr: &Expr, buf: &mut String, depth: usize) {
    if let Expr::StringLit(pat) = expr {
        let _ = write!(buf, "/{pat}/");
    } else {
        fmt_expr(expr, buf, depth);
    }
}

fn fmt_expr(expr: &Expr, buf: &mut String, depth: usize) {
    if depth > 10 {
        buf.push_str("...");
        return;
    }
    match expr {
        Expr::Field(inner) => {
            buf.push('$');
            let needs_parens = !matches!(
                inner.as_ref(),
                Expr::NumberLit(_) | Expr::Var(_) | Expr::StringLit(_)
            );
            if needs_parens {
                buf.push('(');
            }
            fmt_expr(inner, buf, depth + 1);
            if needs_parens {
                buf.push(')');
            }
        }
        Expr::NumberLit(n) => {
            if *n == (*n as i64) as f64 {
                let _ = write!(buf, "{}", *n as i64);
            } else {
                let _ = write!(buf, "{n}");
            }
        }
        Expr::StringLit(s) => {
            let _ = write!(buf, "\"{s}\"");
        }
        Expr::Var(name) => buf.push_str(name),
        Expr::ArrayRef(name, key) => {
            buf.push_str(name);
            buf.push('[');
            fmt_expr(key, buf, depth + 1);
            buf.push(']');
        }
        Expr::ArrayIn(key, arr) => {
            fmt_expr(key, buf, depth + 1);
            buf.push_str(" in ");
            buf.push_str(arr);
        }
        Expr::BinOp(l, op, r) => {
            fmt_expr(l, buf, depth + 1);
            buf.push_str(match op {
                BinOp::Add => " + ",
                BinOp::Sub => " - ",
                BinOp::Mul => " * ",
                BinOp::Div => " / ",
                BinOp::Mod => " % ",
                BinOp::Pow => " ** ",
                BinOp::Eq => " == ",
                BinOp::Ne => " != ",
                BinOp::Lt => " < ",
                BinOp::Le => " <= ",
                BinOp::Gt => " > ",
                BinOp::Ge => " >= ",
            });
            fmt_expr(r, buf, depth + 1);
        }
        Expr::LogicalAnd(l, r) => {
            fmt_expr(l, buf, depth + 1);
            buf.push_str(" && ");
            fmt_expr(r, buf, depth + 1);
        }
        Expr::LogicalOr(l, r) => {
            fmt_expr(l, buf, depth + 1);
            buf.push_str(" || ");
            fmt_expr(r, buf, depth + 1);
        }
        Expr::LogicalNot(e) => {
            buf.push('!');
            fmt_expr(e, buf, depth + 1);
        }
        Expr::Match(l, r) => {
            if is_dollar_zero(l)
                && let Expr::StringLit(pat) = r.as_ref()
            {
                let _ = write!(buf, "/{pat}/");
                return;
            }
            fmt_expr(l, buf, depth + 1);
            buf.push_str(" ~ ");
            fmt_regex_lit_or_expr(r, buf, depth + 1);
        }
        Expr::NotMatch(l, r) => {
            if is_dollar_zero(l)
                && let Expr::StringLit(pat) = r.as_ref()
            {
                let _ = write!(buf, "!/{pat}/");
                return;
            }
            fmt_expr(l, buf, depth + 1);
            buf.push_str(" !~ ");
            fmt_regex_lit_or_expr(r, buf, depth + 1);
        }
        Expr::Assign(target, val) => {
            fmt_expr(target, buf, depth + 1);
            buf.push_str(" = ");
            fmt_expr(val, buf, depth + 1);
        }
        Expr::CompoundAssign(target, op, val) => {
            fmt_expr(target, buf, depth + 1);
            buf.push_str(match op {
                BinOp::Add => " += ",
                BinOp::Sub => " -= ",
                BinOp::Mul => " *= ",
                BinOp::Div => " /= ",
                BinOp::Mod => " %= ",
                BinOp::Pow => " **= ",
                _ => " ?= ",
            });
            fmt_expr(val, buf, depth + 1);
        }
        Expr::Increment(e, pre) => {
            if *pre {
                buf.push_str("++");
            }
            fmt_expr(e, buf, depth + 1);
            if !*pre {
                buf.push_str("++");
            }
        }
        Expr::Decrement(e, pre) => {
            if *pre {
                buf.push_str("--");
            }
            fmt_expr(e, buf, depth + 1);
            if !*pre {
                buf.push_str("--");
            }
        }
        Expr::UnaryMinus(e) => {
            buf.push('-');
            fmt_expr(e, buf, depth + 1);
        }
        Expr::Concat(l, r) => {
            if matches!(r.as_ref(), Expr::Var(n) if n == "SUBSEP") {
                fmt_expr(l, buf, depth + 1);
                buf.push_str(", ");
            } else {
                fmt_expr(l, buf, depth + 1);
                if !buf.ends_with(", ") {
                    buf.push(' ');
                }
                fmt_expr(r, buf, depth + 1);
            }
        }
        Expr::Ternary(c, t, f) => {
            fmt_expr(c, buf, depth + 1);
            buf.push_str(" ? ");
            fmt_expr(t, buf, depth + 1);
            buf.push_str(" : ");
            fmt_expr(f, buf, depth + 1);
        }
        Expr::Sprintf(args) | Expr::FuncCall(_, args) => {
            let name = if let Expr::FuncCall(n, _) = expr {
                n.as_str()
            } else {
                "sprintf"
            };
            buf.push_str(name);
            buf.push('(');
            for (i, a) in args.iter().enumerate() {
                if i > 0 {
                    buf.push_str(", ");
                }
                fmt_expr(a, buf, depth + 1);
            }
            buf.push(')');
        }
        Expr::Getline(var, source) => {
            buf.push_str("getline");
            if let Some(v) = var {
                let _ = write!(buf, " {v}");
            }
            if let Some(src) = source {
                buf.push_str(" < ");
                fmt_expr(src, buf, depth + 1);
            }
        }
        Expr::GetlinePipe(cmd, var) => {
            fmt_expr(cmd, buf, depth + 1);
            buf.push_str(" | getline");
            if let Some(v) = var {
                let _ = write!(buf, " {v}");
            }
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

    #[test]
    fn array_source_tracked() {
        let info = analyze_program("{ a[NR] = $3 }");
        assert!(info.array_sources.contains_key("a"));
        assert_eq!(expr_to_source(info.array_sources.get("a").unwrap()), "$3");
    }

    #[test]
    fn var_source_tracked() {
        let info = analyze_program("{ x = $1 + $2; a[NR] = x }");
        assert_eq!(
            expr_to_source(info.var_sources.get("x").unwrap()),
            "$1 + $2"
        );
        assert_eq!(expr_to_source(info.array_sources.get("a").unwrap()), "x");
    }

    #[test]
    fn description_jpath_with_file() {
        let info = analyze_program("{ ms = jpath($0, \".ms\") + 0; lat[NR] = ms }");
        let expr = info.array_sources.get("lat").unwrap();
        let desc = build_array_description(expr, "api.jsonl", &info.var_sources, 60);
        assert_eq!(desc, "ms — api.jsonl");
    }

    #[test]
    fn description_field_with_file() {
        let info = analyze_program("{ a[NR] = $3 }");
        let expr = info.array_sources.get("a").unwrap();
        let desc = build_array_description(expr, "data.csv", &info.var_sources, 60);
        assert_eq!(desc, "column 3 — data.csv");
    }

    #[test]
    fn description_field_stdin() {
        let info = analyze_program("{ a[NR] = $1 }");
        let expr = info.array_sources.get("a").unwrap();
        let desc = build_array_description(expr, "", &info.var_sources, 60);
        assert_eq!(desc, "column 1");
    }

    #[test]
    fn description_named_column_with_file() {
        let info = analyze_program("{ a[NR] = $\"latency\" }");
        let expr = info.array_sources.get("a").unwrap();
        let desc = build_array_description(expr, "metrics.csv", &info.var_sources, 60);
        assert_eq!(desc, "latency — metrics.csv");
    }

    #[test]
    fn description_expr_with_file() {
        let info = analyze_program("{ a[NR] = $1 + $2 }");
        let expr = info.array_sources.get("a").unwrap();
        let desc = build_array_description(expr, "data.csv", &info.var_sources, 60);
        assert_eq!(desc, "column 1 + column 2 — data.csv");
    }
}
