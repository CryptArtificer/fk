use std::collections::HashMap;

use crate::analyze::ProgramInfo;
use crate::parser::*;

// ── Flat semantic operation ──────────────────────────────────────

/// One semantic operation extracted from the AST.
///
/// Low-level ops come from lowering; high-level ops are produced by
/// reduction passes that recognize patterns and collapse them.
#[derive(Debug, Clone, PartialEq)]
pub enum Op {
    // ── Low-level (from AST lowering) ──

    // Patterns / filters
    Filter(String),
    PatternDedup(String),
    PatternNrFnr,

    // Output
    Emit(Vec<String>),
    EmitFmt(Vec<String>),
    EmitCounter,
    Redirect(String),

    // Assignment & accumulation
    AssignField(String),
    ArrayPut { arr: String, key: String, val: String },
    ArrayAccum { arr: String, key: String, val: String },
    ArrayInc { arr: String, key: String },
    Accum { var: String, source: String },
    Inc(String),

    // Functions
    Fn(String),
    SubGsub { kind: String, pat: String, repl: String },
    MatchCall,
    Jpath(String),

    // Control
    Next,
    ForIn(String),
    IterNF,

    // Environment
    Reformat(String),
    Timed,

    // ── High-level (from reduction) ──
    Where(String),
    Select(Vec<String>),
    Freq(String),
    Sum(String),
    Agg(String, String),
    Histogram(String),
    Stats(String),
    Count(Option<String>),
    Dedup(String),
    Join(String, String),
    Transform(String),
    Extract(String),
    NumberLines,
    Rewrite,
    Collect,
    Generate,
}

// ── Structured program description ──────────────────────────────

/// Flat-op description of a program, preserving BEGIN/rules/END structure.
pub struct Desc {
    pub begin: Vec<Op>,
    pub rules: Vec<RuleDesc>,
    pub end: Vec<Op>,
    pub flags: Flags,
}

pub struct RuleDesc {
    pub filter: Option<Op>,
    pub body: Vec<Op>,
}

#[derive(Default)]
pub struct Flags {
    pub has_timing: bool,
}

// ── Lowering: AST → Desc ────────────────────────────────────────

pub(crate) fn lower(program: &Program, info: &ProgramInfo) -> Desc {
    let vs = &info.var_sources;
    let mut flags = Flags::default();

    let begin = program
        .begin
        .as_ref()
        .map(|b| lower_block(b, vs, &mut flags))
        .unwrap_or_default();

    let rules: Vec<RuleDesc> = program
        .rules
        .iter()
        .map(|r| lower_rule(r, vs, &mut flags))
        .collect();

    let end = program
        .end
        .as_ref()
        .map(|b| lower_block(b, vs, &mut flags))
        .unwrap_or_default();

    Desc { begin, rules, end, flags }
}

fn lower_rule(rule: &Rule, vs: &HashMap<String, Expr>, flags: &mut Flags) -> RuleDesc {
    let filter = rule.pattern.as_ref().map(lower_pattern);
    let body = lower_block(&rule.action, vs, flags);
    RuleDesc { filter, body }
}

// ── Pattern lowering ────────────────────────────────────────────

fn lower_pattern(pat: &Pattern) -> Op {
    match pat {
        Pattern::Expression(Expr::LogicalNot(inner))
            if matches!(inner.as_ref(), Expr::Increment(arr, false)
                if matches!(arr.as_ref(), Expr::ArrayRef(_, _))) =>
        {
            if let Expr::Increment(arr, _) = inner.as_ref()
                && let Expr::ArrayRef(_, key) = arr.as_ref()
            {
                return Op::PatternDedup(expr_text(key));
            }
            Op::Filter(describe_pattern(pat))
        }
        Pattern::Expression(Expr::BinOp(l, BinOp::Eq, r))
            if is_nr_fnr(l, r) || is_nr_fnr(r, l) =>
        {
            Op::PatternNrFnr
        }
        _ => Op::Filter(describe_pattern(pat)),
    }
}

fn is_nr_fnr(a: &Expr, b: &Expr) -> bool {
    matches!(a, Expr::Var(n) if n == "NR") && matches!(b, Expr::Var(n) if n == "FNR")
}

// ── Block / statement lowering ──────────────────────────────────

fn lower_block(
    block: &Block,
    vs: &HashMap<String, Expr>,
    flags: &mut Flags,
) -> Vec<Op> {
    let mut ops = Vec::new();
    for stmt in block {
        lower_stmt(stmt, vs, flags, &mut ops);
    }
    ops
}

fn lower_stmt(
    stmt: &Statement,
    vs: &HashMap<String, Expr>,
    flags: &mut Flags,
    ops: &mut Vec<Op>,
) {
    match stmt {
        Statement::Print(exprs, redir) => {
            let fields = collect_output_refs(exprs, vs);
            for e in exprs {
                scan_expr_fns(e, flags, ops);
            }
            let has_counter = exprs.iter().any(expr_mentions_counter);
            if has_counter && fields.is_empty() {
                ops.push(Op::EmitCounter);
            } else {
                ops.push(Op::Emit(fields));
            }
            if let Some(r) = redir {
                ops.push(Op::Redirect(redirect_text(r)));
            }
        }
        Statement::Printf(exprs, redir) => {
            let fields = if exprs.len() > 1 {
                collect_output_refs(&exprs[1..], vs)
            } else {
                vec![]
            };
            for e in exprs {
                scan_expr_fns(e, flags, ops);
            }
            let has_counter = exprs.iter().any(expr_mentions_counter);
            if has_counter && fields.is_empty() {
                ops.push(Op::EmitCounter);
            } else {
                ops.push(Op::EmitFmt(fields));
            }
            if let Some(r) = redir {
                ops.push(Op::Redirect(redirect_text(r)));
            }
        }
        Statement::Expression(expr) => lower_effect(expr, vs, flags, ops),
        Statement::If(_, then_b, else_b) => {
            lower_block_into(then_b, vs, flags, ops);
            if let Some(eb) = else_b {
                lower_block_into(eb, vs, flags, ops);
            }
        }
        Statement::While(_, body) | Statement::DoWhile(body, _) => {
            lower_block_into(body, vs, flags, ops);
        }
        Statement::For(init, cond, update, body) => {
            let has_nf = cond.as_ref().is_some_and(mentions_nf)
                || init.as_ref().is_some_and(|s| stmt_mentions_nf(s));
            if has_nf {
                ops.push(Op::IterNF);
            }
            if let Some(s) = init {
                lower_stmt(s, vs, flags, ops);
            }
            if let Some(s) = update {
                lower_stmt(s, vs, flags, ops);
            }
            lower_block_into(body, vs, flags, ops);
        }
        Statement::ForIn(_, arr, body) => {
            ops.push(Op::ForIn(arr.clone()));
            lower_block_into(body, vs, flags, ops);
        }
        Statement::Block(b) => lower_block_into(b, vs, flags, ops),
        Statement::Next | Statement::Nextfile => ops.push(Op::Next),
        Statement::Delete(_, _) | Statement::DeleteAll(_) => {}
        Statement::Exit(_) | Statement::Return(_) | Statement::Break | Statement::Continue => {}
    }
}

fn lower_block_into(
    block: &Block,
    vs: &HashMap<String, Expr>,
    flags: &mut Flags,
    ops: &mut Vec<Op>,
) {
    for stmt in block {
        lower_stmt(stmt, vs, flags, ops);
    }
}

// ── Expression effect lowering ──────────────────────────────────
//
// Only captures observable effects: assignments, accumulation,
// function calls with side effects. Pure computation is ignored.

fn lower_effect(
    expr: &Expr,
    vs: &HashMap<String, Expr>,
    flags: &mut Flags,
    ops: &mut Vec<Op>,
) {
    match expr {
        // arr[key] = val
        Expr::Assign(target, val) if matches!(target.as_ref(), Expr::ArrayRef(_, _)) => {
            if let Expr::ArrayRef(arr, key) = target.as_ref() {
                ops.push(Op::ArrayPut {
                    arr: arr.clone(),
                    key: field_ref(key),
                    val: field_ref_deep(val, vs),
                });
            }
        }
        // arr[key] += val
        Expr::CompoundAssign(target, BinOp::Add, val)
            if matches!(target.as_ref(), Expr::ArrayRef(_, _)) =>
        {
            if let Expr::ArrayRef(arr, key) = target.as_ref() {
                ops.push(Op::ArrayAccum {
                    arr: arr.clone(),
                    key: field_ref(key),
                    val: field_ref_deep(val, vs),
                });
            }
        }
        // $N = expr
        Expr::Assign(target, _) if matches!(target.as_ref(), Expr::Field(_)) => {
            if let Expr::Field(inner) = target.as_ref() {
                ops.push(Op::AssignField(field_name(inner)));
            }
        }
        // x = x + y (normalized to accum)
        Expr::Assign(target, val) if is_additive_self(target, val) => {
            if let Expr::Var(name) = target.as_ref()
                && let Some(delta) = additive_delta(target, val)
            {
                ops.push(Op::Accum {
                    var: name.clone(),
                    source: field_ref_deep(delta, vs),
                });
            }
        }
        // var = expr (non-array, non-field)
        Expr::Assign(target, _) => {
            if let Expr::Var(name) = target.as_ref() {
                check_reformat(name, ops);
            }
        }
        // var += expr
        Expr::CompoundAssign(target, BinOp::Add, val) => {
            if let Expr::Var(name) = target.as_ref() {
                ops.push(Op::Accum {
                    var: name.clone(),
                    source: field_ref_deep(val, vs),
                });
            }
        }
        // arr[key]++
        Expr::Increment(inner, _) if matches!(inner.as_ref(), Expr::ArrayRef(_, _)) => {
            if let Expr::ArrayRef(arr, key) = inner.as_ref() {
                ops.push(Op::ArrayInc {
                    arr: arr.clone(),
                    key: field_ref(key),
                });
            }
        }
        // var++
        Expr::Increment(inner, _) => {
            if let Expr::Var(name) = inner.as_ref() {
                ops.push(Op::Inc(name.clone()));
            }
        }
        // var--
        Expr::Decrement(_, _) => {}
        // gsub/sub
        Expr::FuncCall(name, args) if matches!(name.as_str(), "gsub" | "sub" | "gensub") => {
            let pat = args.first().map(fmt_regex_or_expr).unwrap_or_default();
            let repl = args.get(1).map(expr_text).unwrap_or_default();
            ops.push(Op::SubGsub {
                kind: name.clone(),
                pat,
                repl,
            });
        }
        // match()
        Expr::FuncCall(name, _) if name == "match" => {
            ops.push(Op::MatchCall);
        }
        // jpath()
        Expr::FuncCall(name, args) if name == "jpath" && args.len() >= 2 => {
            if let Expr::StringLit(path) = &args[1] {
                let clean = path.trim_start_matches('.').to_string();
                ops.push(Op::Jpath(clean));
            } else {
                ops.push(Op::Fn(name.clone()));
            }
        }
        // timing functions
        Expr::FuncCall(name, _) if matches!(name.as_str(), "tic" | "toc" | "clk") => {
            flags.has_timing = true;
        }
        // dump()
        Expr::FuncCall(name, _) if name == "dump" => {
            ops.push(Op::Emit(vec![]));
        }
        // other function calls
        Expr::FuncCall(name, _) => {
            ops.push(Op::Fn(name.clone()));
        }
        // compound expressions: recurse for effects
        Expr::BinOp(l, _, r)
        | Expr::LogicalAnd(l, r)
        | Expr::LogicalOr(l, r)
        | Expr::Concat(l, r)
        | Expr::CompoundAssign(l, _, r) => {
            lower_effect(l, vs, flags, ops);
            lower_effect(r, vs, flags, ops);
        }
        Expr::Ternary(_, t, f) => {
            lower_effect(t, vs, flags, ops);
            lower_effect(f, vs, flags, ops);
        }
        Expr::LogicalNot(e) | Expr::UnaryMinus(e) => {
            lower_effect(e, vs, flags, ops);
        }
        _ => {}
    }
}

/// Scan an expression tree for function calls and timing, emitting Fn ops.
fn scan_expr_fns(expr: &Expr, flags: &mut Flags, ops: &mut Vec<Op>) {
    match expr {
        Expr::FuncCall(name, args) => {
            match name.as_str() {
                "tic" | "toc" | "clk" => flags.has_timing = true,
                _ => ops.push(Op::Fn(name.clone())),
            }
            for a in args {
                scan_expr_fns(a, flags, ops);
            }
        }
        Expr::BinOp(l, _, r)
        | Expr::Concat(l, r)
        | Expr::LogicalAnd(l, r)
        | Expr::LogicalOr(l, r) => {
            scan_expr_fns(l, flags, ops);
            scan_expr_fns(r, flags, ops);
        }
        Expr::UnaryMinus(e) | Expr::LogicalNot(e) | Expr::Field(e) => {
            scan_expr_fns(e, flags, ops);
        }
        Expr::Ternary(c, t, f) => {
            scan_expr_fns(c, flags, ops);
            scan_expr_fns(t, flags, ops);
            scan_expr_fns(f, flags, ops);
        }
        Expr::Sprintf(args) => {
            for a in args {
                scan_expr_fns(a, flags, ops);
            }
        }
        _ => {}
    }
}

const COUNTER_VARS: &[&str] = &["NR", "FNR"];

fn expr_mentions_counter(expr: &Expr) -> bool {
    match expr {
        Expr::Var(name) => COUNTER_VARS.contains(&name.as_str()),
        Expr::BinOp(l, _, r) | Expr::Concat(l, r) => {
            expr_mentions_counter(l) || expr_mentions_counter(r)
        }
        Expr::FuncCall(_, args) | Expr::Sprintf(args) => {
            args.iter().any(expr_mentions_counter)
        }
        Expr::Ternary(c, t, f) => {
            expr_mentions_counter(c) || expr_mentions_counter(t) || expr_mentions_counter(f)
        }
        Expr::Field(e) | Expr::UnaryMinus(e) | Expr::LogicalNot(e) => expr_mentions_counter(e),
        _ => false,
    }
}

fn check_reformat(var: &str, ops: &mut Vec<Op>) {
    match var {
        "ORS" => ops.push(Op::Reformat("ORS".into())),
        "OFS" => ops.push(Op::Reformat("OFS".into())),
        "OFMT" => ops.push(Op::Reformat("OFMT".into())),
        _ => {}
    }
}

fn stmt_mentions_nf(stmt: &Statement) -> bool {
    match stmt {
        Statement::Expression(e) => mentions_nf(e),
        _ => false,
    }
}

fn mentions_nf(expr: &Expr) -> bool {
    match expr {
        Expr::Var(name) => name == "NF",
        Expr::BinOp(l, _, r) | Expr::Concat(l, r) | Expr::LogicalAnd(l, r) | Expr::LogicalOr(l, r) => {
            mentions_nf(l) || mentions_nf(r)
        }
        Expr::Field(e) | Expr::UnaryMinus(e) | Expr::LogicalNot(e) => mentions_nf(e),
        Expr::Assign(l, r) | Expr::CompoundAssign(l, _, r) => mentions_nf(l) || mentions_nf(r),
        _ => false,
    }
}

// ── Output field collection ─────────────────────────────────────
//
// Walk print/printf argument expressions and collect all field
// references, resolving variables through var_sources.  This is
// a depth-limited tree walk — not separate lineage tracking.

fn collect_output_refs(exprs: &[Expr], vs: &HashMap<String, Expr>) -> Vec<String> {
    let mut out = Vec::new();
    for e in exprs {
        collect_expr_refs(e, vs, &mut out, 5);
    }
    out
}

fn collect_expr_refs(
    e: &Expr,
    vs: &HashMap<String, Expr>,
    out: &mut Vec<String>,
    depth: u8,
) {
    if depth == 0 {
        return;
    }
    let e = unwrap_coercion(e);
    match e {
        Expr::Field(inner) => {
            if let Some(name) = field_display(inner)
                && !out.contains(&name)
            {
                out.push(name);
            }
        }
        Expr::Var(name)
            if !matches!(
                name.as_str(),
                "NR" | "NF" | "FNR" | "FILENAME" | "ORS" | "OFS" | "OFMT"
            ) =>
        {
            if let Some(src) = vs.get(name.as_str()) {
                collect_expr_refs(src, vs, out, depth - 1);
            }
        }
        Expr::FuncCall(name, args) if name == "jpath" && args.len() >= 2 => {
            if let Expr::StringLit(path) = &args[1] {
                let clean = path.trim_start_matches('.').to_string();
                if !clean.is_empty() && !out.contains(&clean) {
                    out.push(clean);
                }
            }
        }
        Expr::FuncCall(_, args) => {
            for a in args {
                collect_expr_refs(a, vs, out, depth - 1);
            }
        }
        Expr::Concat(l, r) | Expr::BinOp(l, _, r) => {
            collect_expr_refs(l, vs, out, depth - 1);
            collect_expr_refs(r, vs, out, depth - 1);
        }
        Expr::Ternary(_, then_e, else_e) => {
            collect_expr_refs(then_e, vs, out, depth - 1);
            collect_expr_refs(else_e, vs, out, depth - 1);
        }
        Expr::UnaryMinus(inner) | Expr::LogicalNot(inner) => {
            collect_expr_refs(inner, vs, out, depth - 1);
        }
        _ => {}
    }
}

// ── Helpers ─────────────────────────────────────────────────────

fn unwrap_coercion(expr: &Expr) -> &Expr {
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

/// Human-readable display name for a field inner expression.
fn field_display(inner: &Expr) -> Option<String> {
    match inner {
        Expr::NumberLit(n) if *n == 0.0 => None,
        Expr::NumberLit(n) => Some(format!("{}", *n as i64)),
        Expr::StringLit(s) => Some(s.clone()),
        Expr::Var(name) if matches!(name.as_str(), "NR" | "NF" | "FNR" | "FILENAME") => None,
        Expr::Var(name) => Some(name.clone()),
        _ => None,
    }
}

/// Get the field name/number from a field access, or "?" for dynamic.
fn field_name(inner: &Expr) -> String {
    field_display(inner).unwrap_or_else(|| "?".into())
}

const BUILTIN_VARS: &[&str] = &[
    "NR", "NF", "FNR", "FILENAME", "FS", "RS", "OFS", "ORS", "OFMT",
    "SUBSEP", "ARGC", "ARGV", "ENVIRON", "CONVFMT",
];

/// Extract field reference from an expression (for array keys/values).
fn field_ref(expr: &Expr) -> String {
    match expr {
        Expr::Field(inner) if matches!(inner.as_ref(), Expr::NumberLit(n) if *n == 0.0) => {
            "$0".into()
        }
        Expr::Field(inner) => format!("${}", field_name(inner)),
        Expr::Var(name) if BUILTIN_VARS.contains(&name.as_str()) => name.clone(),
        Expr::Var(name) => format!("${name}"),
        Expr::NumberLit(n) if *n == 0.0 => "$0".into(),
        _ => expr_text(expr),
    }
}

/// Like field_ref but resolves through var_sources and unwraps coercions.
fn field_ref_deep(expr: &Expr, vs: &HashMap<String, Expr>) -> String {
    let expr = unwrap_coercion(expr);
    match expr {
        Expr::Field(inner) if matches!(inner.as_ref(), Expr::NumberLit(n) if *n == 0.0) => {
            "$0".into()
        }
        Expr::Field(inner) => format!("${}", field_name(inner)),
        Expr::Var(name) if BUILTIN_VARS.contains(&name.as_str()) => name.clone(),
        Expr::Var(name) => {
            if let Some(src) = vs.get(name.as_str()) {
                field_ref_deep(src, vs)
            } else {
                format!("${name}")
            }
        }
        _ => expr_text(expr),
    }
}

fn is_additive_self(target: &Expr, val: &Expr) -> bool {
    if let Expr::BinOp(l, BinOp::Add, r) = val {
        exprs_equal(target, l) || exprs_equal(target, r)
    } else {
        false
    }
}

fn additive_delta<'a>(target: &Expr, val: &'a Expr) -> Option<&'a Expr> {
    if let Expr::BinOp(l, BinOp::Add, r) = val {
        if exprs_equal(target, l) {
            return Some(r);
        }
        if exprs_equal(target, r) {
            return Some(l);
        }
    }
    None
}

fn exprs_equal(a: &Expr, b: &Expr) -> bool {
    match (a, b) {
        (Expr::Var(x), Expr::Var(y)) => x == y,
        (Expr::Field(x), Expr::Field(y)) => exprs_equal(x, y),
        (Expr::NumberLit(x), Expr::NumberLit(y)) => (x - y).abs() < f64::EPSILON,
        (Expr::StringLit(x), Expr::StringLit(y)) => x == y,
        (Expr::ArrayRef(a1, k1), Expr::ArrayRef(a2, k2)) => a1 == a2 && exprs_equal(k1, k2),
        _ => false,
    }
}

// ── Text formatters ─────────────────────────────────────────────

fn describe_pattern(pat: &Pattern) -> String {
    match pat {
        Pattern::Regex(s) => format!("/{s}/"),
        Pattern::Expression(e) => expr_text(e),
        Pattern::Range(a, b) => format!("{},{}", describe_pattern(a), describe_pattern(b)),
    }
}

fn redirect_text(r: &Redirect) -> String {
    match r {
        Redirect::Overwrite(e) => format!("> {}", expr_text(e)),
        Redirect::Append(e) => format!(">> {}", expr_text(e)),
        Redirect::Pipe(e) => format!("| {}", expr_text(e)),
    }
}

fn fmt_regex_or_expr(e: &Expr) -> String {
    match e {
        Expr::StringLit(s) => format!("/{s}/"),
        // /regex/ in function args is parsed as Match($0, StringLit(pat))
        Expr::Match(_, r) | Expr::NotMatch(_, r) => {
            if let Expr::StringLit(s) = r.as_ref() {
                format!("/{s}/")
            } else {
                expr_text(e)
            }
        }
        _ => expr_text(e),
    }
}

pub fn expr_text(expr: &Expr) -> String {
    match expr {
        Expr::NumberLit(n) => {
            if *n == (*n as i64) as f64 {
                format!("{}", *n as i64)
            } else {
                format!("{n}")
            }
        }
        Expr::StringLit(s) => format!("\"{s}\""),
        Expr::Var(name) => format!("${name}"),
        Expr::Field(inner) => match inner.as_ref() {
            Expr::NumberLit(n) => format!("${}", *n as i64),
            Expr::StringLit(s) => format!("$\"{s}\""),
            _ => format!("$({})", expr_text(inner)),
        },
        Expr::BinOp(l, op, r) => {
            let op_str = match op {
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
            };
            format!("{} {op_str} {}", expr_text(l), expr_text(r))
        }
        Expr::Concat(l, r) => format!("{} {}", expr_text(l), expr_text(r)),
        Expr::UnaryMinus(e) => format!("-{}", expr_text(e)),
        Expr::LogicalNot(e) => format!("!{}", expr_text(e)),
        Expr::Increment(e, true) => format!("++{}", expr_text(e)),
        Expr::Increment(e, false) => format!("{}++", expr_text(e)),
        Expr::Decrement(e, true) => format!("--{}", expr_text(e)),
        Expr::Decrement(e, false) => format!("{}--", expr_text(e)),
        Expr::ArrayRef(arr, key) => format!("{arr}[{}]", expr_text(key)),
        Expr::ArrayIn(key, arr) => format!("{} in {arr}", expr_text(key)),
        Expr::Assign(l, r) => format!("{} = {}", expr_text(l), expr_text(r)),
        Expr::CompoundAssign(l, op, r) => {
            let op_str = match op {
                BinOp::Add => "+=",
                BinOp::Sub => "-=",
                BinOp::Mul => "*=",
                BinOp::Div => "/=",
                _ => "?=",
            };
            format!("{} {op_str} {}", expr_text(l), expr_text(r))
        }
        Expr::Match(l, r) => format!("{} ~ {}", expr_text(l), expr_text(r)),
        Expr::NotMatch(l, r) => format!("{} !~ {}", expr_text(l), expr_text(r)),
        Expr::LogicalAnd(l, r) => format!("{} && {}", expr_text(l), expr_text(r)),
        Expr::LogicalOr(l, r) => format!("{} || {}", expr_text(l), expr_text(r)),
        Expr::Ternary(c, t, f) => {
            format!("{} ? {} : {}", expr_text(c), expr_text(t), expr_text(f))
        }
        Expr::FuncCall(name, args) => {
            let a: Vec<String> = args.iter().map(expr_text).collect();
            format!("{name}({})", a.join(", "))
        }
        Expr::Sprintf(args) => {
            let a: Vec<String> = args.iter().map(expr_text).collect();
            format!("sprintf({})", a.join(", "))
        }
        Expr::Getline(var, src) => {
            let mut s = "getline".to_string();
            if let Some(v) = var {
                s.push(' ');
                s.push_str(v);
            }
            if let Some(e) = src {
                s.push_str(" < ");
                s.push_str(&expr_text(e));
            }
            s
        }
        Expr::GetlinePipe(cmd, var) => {
            let mut s = format!("{} | getline", expr_text(cmd));
            if let Some(v) = var {
                s.push(' ');
                s.push_str(v);
            }
            s
        }
    }
}
