use std::collections::HashMap;
use std::fmt::Write;
use std::path::Path;

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
            match target.as_ref() {
                Expr::ArrayRef(name, _) => {
                    info.array_sources.entry(name.clone())
                        .or_insert_with(|| val.as_ref().clone());
                }
                Expr::Var(name) if matches!(expr, Expr::Assign(..)) => {
                    info.var_sources.entry(name.clone())
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

// ── Expression formatter & smart title builder ──────────────────────

/// Strip `+ 0` / `0 +` numeric coercion wrappers.
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

/// Build a human-readable data-source description from an array's RHS
/// expression and the current FILENAME.  Used as a chart subtitle.
///
/// Smart cases:
///   jpath($0, ".ms") + 0  from api.jsonl  →  "api.jsonl — [].ms"
///   $3                     from data.csv   →  "data.csv — $3"
///   $"latency"             from data.csv   →  "data.csv — latency"
///   $1 + $2                from data.csv   →  "data.csv — $1 + $2"
///   $1                     from stdin      →  "$1"
pub fn build_array_description(
    expr: &Expr,
    filename: &str,
    var_sources: &HashMap<String, Expr>,
) -> String {
    let expr = unwrap_coercion(expr);
    let expr = if let Expr::Var(name) = expr {
        var_sources.get(name).map_or(expr, |e| unwrap_coercion(e))
    } else {
        expr
    };
    let base = friendly_filename(filename);

    let source_part = describe_expr(expr);
    if base.is_empty() {
        source_part
    } else {
        format!("{base} — {source_part}")
    }
}

fn describe_expr(expr: &Expr) -> String {
    if let Expr::FuncCall(name, args) = expr
        && name == "jpath" && args.len() >= 2
        && matches!(&args[0], Expr::Field(inner)
            if matches!(inner.as_ref(), Expr::NumberLit(n) if *n == 0.0))
        && let Expr::StringLit(path) = &args[1]
    {
        return format!("[]{path}");
    }

    if let Expr::Field(inner) = expr {
        match inner.as_ref() {
            Expr::NumberLit(n) => return format!("${}", *n as i64),
            Expr::StringLit(col) => return col.clone(),
            _ => {}
        }
    }

    expr_to_source(expr)
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

// ── Program explainer ───────────────────────────────────────────

const STAT_BUILTINS: &[&str] = &[
    "mean", "median", "stddev", "variance", "sum", "min", "max",
    "p", "percentile", "quantile", "iqm",
];
const CHART_BUILTINS: &[&str] = &["hist", "plotbox", "plot"];

const EXPLAIN_BUDGET: usize = 72;

/// Runtime environment context for explain().
#[derive(Debug, Default)]
pub struct ExplainContext {
    pub input_mode: Option<String>,   // "CSV", "TSV", "JSON", "Parquet"
    pub headers: bool,                // -H flag
    pub compressed: Option<String>,   // "gzip", "zstd", "bz2", "xz", "lz4"
    pub field_sep: Option<String>,    // -F value
    pub files: Vec<String>,           // input filenames
}

impl ExplainContext {
    pub fn from_cli(
        mode: &str, headers: bool, field_sep: Option<&str>, files: &[String],
    ) -> Self {
        let mut input_mode = match mode {
            "line" => None,
            m => Some(m.to_uppercase()),
        };

        // Auto-detect from file extension when mode is line and no -F
        if input_mode.is_none() && field_sep.is_none()
            && let Some(f) = files.first()
        {
            input_mode = detect_format_from_ext(f);
        }

        let compressed = files.first().and_then(|f| detect_compression(f));

        let filenames: Vec<String> = files.iter()
            .map(|f| Path::new(f).file_name()
                .map_or_else(|| f.clone(), |n| n.to_string_lossy().into_owned()))
            .collect();

        Self {
            input_mode,
            headers,
            compressed,
            field_sep: field_sep.map(|s| s.to_string()),
            files: filenames,
        }
    }

    fn to_suffix(&self) -> Option<String> {
        let mut parts: Vec<String> = Vec::new();
        if let Some(ref m) = self.input_mode   { parts.push(m.clone()); }
        if let Some(ref c) = self.compressed   { parts.push(c.clone()); }
        if self.headers                        { parts.push("headers".into()); }
        if let Some(ref f) = self.field_sep    { parts.push(format!("-F '{f}'")); }
        match self.files.len() {
            0 => {}
            1 => parts.push(self.files[0].clone()),
            n => parts.push(format!("{n} files")),
        }
        if parts.is_empty() { return None; }
        Some(format!("({})", parts.join(", ")))
    }
}

fn detect_format_from_ext(path: &str) -> Option<String> {
    let base = path.trim_end_matches(".gz")
        .trim_end_matches(".zst").trim_end_matches(".zstd")
        .trim_end_matches(".bz2").trim_end_matches(".xz")
        .trim_end_matches(".lz4");
    if base.ends_with(".csv") { Some("CSV".into()) }
    else if base.ends_with(".tsv") || base.ends_with(".tab") { Some("TSV".into()) }
    else if base.ends_with(".json") || base.ends_with(".jsonl") || base.ends_with(".ndjson") { Some("JSON".into()) }
    else if base.ends_with(".parquet") { Some("Parquet".into()) }
    else { None }
}

fn detect_compression(path: &str) -> Option<String> {
    if path.ends_with(".gz") { Some("gzip".into()) }
    else if path.ends_with(".zst") || path.ends_with(".zstd") { Some("zstd".into()) }
    else if path.ends_with(".bz2") { Some("bzip2".into()) }
    else if path.ends_with(".xz") { Some("xz".into()) }
    else if path.ends_with(".lz4") { Some("lz4".into()) }
    else { None }
}

/// Fragment tag — each variant carries its own priority and subsumption rules.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum FragTag {
    Chart, Stats, Aggregate, Frequency, Sum, Count,
    Transform, Extract, Filter, Rewrite, Select,
    Compute, Number, Collect, Generate, Slurp,
}

impl FragTag {
    fn sig(self) -> u8 {
        match self {
            Self::Chart     => 90, Self::Stats     => 85,
            Self::Aggregate => 80, Self::Frequency => 75,
            Self::Sum       => 70,
            Self::Count | Self::Collect => 65,
            Self::Transform => 60,
            Self::Extract | Self::Slurp => 55,
            Self::Filter    => 40, Self::Rewrite   => 35,
            Self::Select | Self::Compute | Self::Generate => 30,
            Self::Number    => 25,
        }
    }

    /// Tags whose presence makes `self` redundant.
    fn subsumed_by(self) -> &'static [FragTag] {
        use FragTag::*;
        match self {
            Stats     => &[Chart],
            Aggregate | Sum => &[Stats],
            Collect   => &[Chart, Stats, Aggregate, Frequency],
            Compute   => &[Chart, Stats, Aggregate, Frequency, Sum, Count,
                           Transform, Extract, Select],
            Number    => &[Select, Compute, Count],
            _         => &[],
        }
    }
}

#[derive(Debug, Clone)]
struct Fragment { text: String, tag: FragTag }

impl Fragment {
    fn new(text: impl Into<String>, tag: FragTag) -> Self {
        Self { text: text.into(), tag }
    }
}

// ── Pipeline: scan → collect → reduce → render ──────────────────

pub fn explain(program: &Program, ctx: Option<&ExplainContext>) -> String {
    let info = analyze(program);
    let env = ctx.and_then(|c| c.to_suffix());

    let base = detect_idioms(program).unwrap_or_else(|| {
        let mut frags = collect_fragments(program, &info);
        reduce(&mut frags);
        render(&frags, EXPLAIN_BUDGET)
    });

    match env.as_deref() {
        None => base,
        Some(e) if base.is_empty() && e.len() <= EXPLAIN_BUDGET => e.to_string(),
        Some(e) => {
            let combined = format!("{base} {e}");
            if combined.len() <= EXPLAIN_BUDGET { combined } else { base }
        }
    }
}

fn detect_idioms(program: &Program) -> Option<String> {
    if program.end.is_none() && program.begin.is_none() && program.rules.len() == 1
        && let Some(key) = detect_dedup_pattern(&program.rules[0])
    {
        return Some(format!("unique {key}"));
    }
    detect_join_idiom(program).or_else(|| detect_count_match(program))
}

/// Walk → emit: one scan, one function, all fragments.
fn collect_fragments(program: &Program, info: &ProgramInfo) -> Vec<Fragment> {
    let s = scan_program(program);
    let source = resolve_source(info);
    let has_end = program.end.is_some();
    let has_rules = !program.rules.is_empty();
    let begin_only = !has_rules && !has_end && program.begin.is_some();

    let mut f = Vec::new();

    // Filters
    for rule in &program.rules {
        if let Some(pat) = &rule.pattern && !is_nr_eq_fnr(pat) {
            let p = describe_pattern(pat);
            if p != "1" { f.push(Fragment::new(format!("filter {p}"), FragTag::Filter)); }
        }
    }
    // END builtins (chart/stats)
    if has_end {
        if s.called_fns.iter().any(|c| CHART_BUILTINS.contains(&c.as_str())) {
            let op = if s.called_fns.iter().any(|c| c == "hist") { "histogram" } else { "chart" };
            let text = source.as_ref().map_or_else(|| op.into(), |s| format!("{op} {s}"));
            f.push(Fragment::new(text, FragTag::Chart));
        }
        if s.called_fns.iter().any(|c| STAT_BUILTINS.contains(&c.as_str())) {
            let text = source.as_ref().map_or_else(|| "stats".into(), |s| format!("stats {s}"));
            f.push(Fragment::new(text, FragTag::Stats));
        }
    }
    // Accumulation
    if let Some((ref v, ref k)) = s.aggregate_by {
        f.push(Fragment::new(format!("sum {v} by {k}"), FragTag::Aggregate));
    }
    if let Some(ref k) = s.frequency_key {
        f.push(Fragment::new(format!("frequency {k}"), FragTag::Frequency));
    }
    if let Some(ref fld) = s.accum_field {
        f.push(Fragment::new(format!("sum {fld}"), FragTag::Sum));
    }
    // Count
    let hi = f.iter().any(|x| matches!(x.tag, FragTag::Chart | FragTag::Stats));
    if has_end && !has_rules && !hi {
        f.push(Fragment::new("count", FragTag::Count));
    }
    if s.has_collect && has_end {
        f.push(Fragment::new("collect + emit", FragTag::Collect));
    }
    // Transform
    if s.has_transform {
        f.push(Fragment::new(
            s.transform_desc.as_deref().unwrap_or("transform"), FragTag::Transform));
    }
    if s.has_field_iteration {
        f.push(Fragment::new("iterate fields", FragTag::Transform));
    }
    // Extraction
    match (s.has_match, s.has_jpath, s.has_format) {
        (true, _, true)  => f.push(Fragment::new("regex extract + format", FragTag::Extract)),
        (true, _, false) => f.push(Fragment::new("regex extract", FragTag::Extract)),
        (false, true, true)  => f.push(Fragment::new("extract + format JSON fields", FragTag::Extract)),
        (false, true, false) => f.push(Fragment::new("extract JSON fields", FragTag::Extract)),
        _ => {}
    }
    // Rewrite
    if s.has_field_assign  { f.push(Fragment::new("rewrite fields", FragTag::Rewrite)); }
    if s.has_reformat      { f.push(Fragment::new("reformat output", FragTag::Rewrite)); }
    // Selection
    if !s.select_fields.is_empty() && !s.has_field_iteration {
        let n = s.select_fields.len();
        let text = if n > SELECT_FIELD_LIMIT {
            format!("select {n} fields")
        } else {
            format!("select {}", s.select_fields.join(", "))
        };
        f.push(Fragment::new(text, FragTag::Select));
    }
    if s.prints_computed   { f.push(Fragment::new("compute", FragTag::Compute)); }
    if s.prints_line_no && s.has_output {
        f.push(Fragment::new("number lines", FragTag::Number));
    }
    // BEGIN-only
    if begin_only {
        if s.called_fns.iter().any(|c| CHART_BUILTINS.contains(&c.as_str())) {
            f.push(Fragment::new("chart", FragTag::Chart));
        } else if s.called_fns.iter().any(|c| c == "slurp") {
            f.push(Fragment::new("slurp + aggregate", FragTag::Slurp));
        } else if s.has_output {
            f.push(Fragment::new("generate", FragTag::Generate));
        }
    }
    f
}

fn scan_program(program: &Program) -> ScanState {
    let mut s = ScanState::default();
    if let Some(b) = &program.begin { for stmt in b { scan_stmt(stmt, &mut s); } }
    for rule in &program.rules { for stmt in &rule.action { scan_stmt(stmt, &mut s); } }
    if let Some(b) = &program.end { for stmt in b { scan_stmt(stmt, &mut s); } }
    s
}

/// Table-driven subsumption + one special merge rule.
fn reduce(frags: &mut Vec<Fragment>) {
    let tags: Vec<FragTag> = frags.iter().map(|f| f.tag).collect();
    let has = |t: FragTag| tags.contains(&t);

    // Aggregate + Frequency → relabel as "aggregate", drop Frequency
    if has(FragTag::Aggregate) && has(FragTag::Frequency) {
        for f in frags.iter_mut() {
            if f.tag == FragTag::Aggregate {
                f.text = f.text.replacen("sum ", "aggregate ", 1);
            }
        }
        frags.retain(|f| f.tag != FragTag::Frequency);
    }

    frags.retain(|f| !f.tag.subsumed_by().iter().any(|s| tags.contains(s)));
}

fn render(frags: &[Fragment], budget: usize) -> String {
    if frags.is_empty() { return String::new(); }
    let mut sorted: Vec<&Fragment> = frags.iter().collect();
    sorted.sort_by_key(|f| std::cmp::Reverse(f.tag.sig()));

    for take in (1..=sorted.len()).rev() {
        let parts: Vec<&str> = sorted[..take].iter().map(|f| f.text.as_str()).collect();
        let mut text = parts.join(", ");
        if sorted.len() - take > 0 { text.push_str(", …"); }
        if text.len() <= budget || take == 1 {
            if text.len() > budget {
                text.truncate(budget.saturating_sub(1));
                text.push('…');
            }
            return text;
        }
    }
    unreachable!()
}

/// Detect `!seen[key]++` or `!a[key]++` (deduplicate idiom).
fn detect_dedup_pattern(rule: &Rule) -> Option<String> {
    if let Some(Pattern::Expression(Expr::LogicalNot(inner))) = &rule.pattern
        && let Expr::Increment(arr_ref, false) = inner.as_ref()
        && let Expr::ArrayRef(_, key) = arr_ref.as_ref()
    {
        return Some(expr_to_source(key));
    }
    None
}

/// Detect NR==FNR{...;next} + second rule → join/anti-join/semi-join.
fn detect_join_idiom(program: &Program) -> Option<String> {
    if program.rules.len() < 2 { return None; }
    let first = &program.rules[0];
    if !is_nr_eq_fnr(first.pattern.as_ref()?) { return None; }
    if !first.action.iter().any(|s| matches!(s, Statement::Next)) { return None; }

    let second = &program.rules[1];
    let second_pat = second.pattern.as_ref().map(describe_pattern);

    match second_pat.as_deref() {
        Some(p) if p.contains("!") && p.contains("in") => Some("anti-join".to_string()),
        Some(p) if p.contains("in") => Some("semi-join".to_string()),
        _ => Some("join".to_string()),
    }
}

fn is_nr_eq_fnr(pat: &Pattern) -> bool {
    if let Pattern::Expression(Expr::BinOp(l, BinOp::Eq, r)) = pat {
        return (matches!(l.as_ref(), Expr::Var(n) if n == "NR")
                && matches!(r.as_ref(), Expr::Var(n) if n == "FNR"))
            || (matches!(l.as_ref(), Expr::Var(n) if n == "FNR")
                && matches!(r.as_ref(), Expr::Var(n) if n == "NR"));
    }
    false
}

/// Detect /pattern/{n++}; END{print n} → "count /pattern/".
fn detect_count_match(program: &Program) -> Option<String> {
    program.end.as_ref()?;
    for rule in &program.rules {
        let pat = rule.pattern.as_ref()?;
        let has_incr = rule.action.iter().any(|s| {
            matches!(s, Statement::Expression(Expr::Increment(inner, _)) if matches!(inner.as_ref(), Expr::Var(_)))
        });
        if has_incr {
            return Some(format!("count {}", describe_pattern(pat)));
        }
    }
    None
}

fn resolve_source(info: &ProgramInfo) -> Option<String> {
    info.array_sources.iter().next().and_then(|(_, expr)| {
        let expr = unwrap_coercion(expr);
        let expr = if let Expr::Var(name) = expr {
            info.var_sources.get(name).map_or(expr, |e| unwrap_coercion(e))
        } else {
            expr
        };
        let s = describe_expr(expr);
        if s.is_empty() { None } else { Some(s) }
    })
}

fn describe_pattern(pat: &Pattern) -> String {
    match pat {
        Pattern::Regex(s) => format!("/{s}/"),
        Pattern::Expression(e) => expr_to_source(e),
        Pattern::Range(a, b) => format!("{},{}", describe_pattern(a), describe_pattern(b)),
    }
}

#[derive(Default)]
struct ScanState {
    has_output: bool,
    frequency_key: Option<String>,
    accum_field: Option<String>,
    aggregate_by: Option<(String, String)>,
    has_transform: bool,
    transform_desc: Option<String>,
    has_jpath: bool,
    has_match: bool,
    has_format: bool,
    select_fields: Vec<String>,
    has_field_assign: bool,
    has_field_iteration: bool,
    has_reformat: bool,
    has_collect: bool,
    prints_line_no: bool,
    prints_computed: bool,
    called_fns: Vec<String>,
}

const SELECT_FIELD_LIMIT: usize = 5;

fn field_display(inner: &Expr) -> Option<String> {
    match inner {
        Expr::NumberLit(n) if *n == 0.0 => None,
        Expr::NumberLit(n) => Some(format!("${}", *n as i64)),
        Expr::StringLit(s) => Some(s.clone()),
        Expr::Var(name) if matches!(name.as_str(), "NR" | "NF" | "FNR" | "FILENAME") => None,
        Expr::Var(name) => Some(format!("${name}")),
        _ => None,
    }
}

fn collect_output_fields(exprs: &[Expr], fields: &mut Vec<String>) {
    for e in exprs {
        if let Expr::Field(inner) = e
            && let Some(name) = field_display(inner)
            && !fields.contains(&name)
        {
            fields.push(name);
        }
    }
}


// ── AST normalisation helpers ────────────────────────────────────

const OUTPUT_VARS: &[&str] = &["ORS", "OFS", "OFMT"];
const RECORD_COUNTERS: &[&str] = &["NR", "FNR"];
const TRANSFORM_BUILTINS: &[&str] = &[
    "gsub", "sub", "gensub", "trim", "ltrim", "rtrim",
    "reverse", "toupper", "tolower",
];

/// Structural equality — enough for `x = x + y` ≡ `x += y` normalisation.
fn exprs_equal(a: &Expr, b: &Expr) -> bool {
    match (a, b) {
        (Expr::Var(n1), Expr::Var(n2)) => n1 == n2,
        (Expr::ArrayRef(n1, k1), Expr::ArrayRef(n2, k2)) => {
            n1 == n2 && exprs_equal(k1, k2)
        }
        (Expr::Field(e1), Expr::Field(e2)) => exprs_equal(e1, e2),
        (Expr::NumberLit(a), Expr::NumberLit(b)) => a == b,
        (Expr::StringLit(a), Expr::StringLit(b)) => a == b,
        _ => false,
    }
}

/// Normalise additive accumulation forms to (target, delta):
///   x += y           →  (x, y)
///   x = x + y        →  (x, y)      (commutative)
///   x++  /  ++x      →  (x, <1>)
///   x = x - y        →  (x, y)      (detected as subtraction, still accumulation)
fn as_additive_accum(expr: &Expr) -> Option<(&Expr, Option<&Expr>)> {
    match expr {
        Expr::CompoundAssign(target, BinOp::Add, val) => {
            Some((target, Some(val)))
        }
        Expr::Assign(target, val) => {
            if let Expr::BinOp(l, BinOp::Add, r) = val.as_ref() {
                if exprs_equal(target, l) {
                    return Some((target, Some(r)));
                }
                if exprs_equal(target, r) {
                    return Some((target, Some(l)));
                }
            }
            None
        }
        Expr::Increment(inner, _) | Expr::Decrement(inner, _) => {
            Some((inner, None))
        }
        _ => None,
    }
}

// ── Unified recursive expression scanner ─────────────────────────

/// Walk an expression tree and set signal flags in ScanState.
/// Returns true if the expression contains non-trivial computation
/// (function calls, arithmetic) — used by the caller to detect
/// "computed output" vs plain field selection.
fn scan_expr(expr: &Expr, s: &mut ScanState) -> bool {
    match expr {
        Expr::FuncCall(name, args) => {
            if !s.called_fns.contains(name) {
                s.called_fns.push(name.clone());
            }
            match name.as_str() {
                "jpath" => s.has_jpath = true,
                "match" => s.has_match = true,
                n if TRANSFORM_BUILTINS.contains(&n) => {
                    s.has_transform = true;
                    if s.transform_desc.is_none() && args.len() >= 2
                        && matches!(n, "gsub" | "sub" | "gensub")
                    {
                        let pat = fmt_regex_or_expr(&args[0]);
                        let repl = expr_to_source(&args[1]);
                        s.transform_desc = Some(format!("{n} {pat} → {repl}"));
                    }
                }
                _ => {}
            }
            for a in args { scan_expr(a, s); }
            true
        }
        Expr::BinOp(l, _, r) => {
            scan_expr(l, s);
            scan_expr(r, s);
            true
        }
        Expr::UnaryMinus(e) => {
            scan_expr(e, s);
            true
        }
        Expr::Concat(l, r) | Expr::Assign(l, r) | Expr::CompoundAssign(l, _, r)
        | Expr::LogicalAnd(l, r) | Expr::LogicalOr(l, r) => {
            let a = scan_expr(l, s);
            let b = scan_expr(r, s);
            a || b
        }
        Expr::LogicalNot(e) | Expr::Field(e)
        | Expr::Increment(e, _) | Expr::Decrement(e, _) => {
            scan_expr(e, s)
        }
        _ => false,
    }
}

// ── Statement scanning ───────────────────────────────────────────

fn scan_stmt(stmt: &Statement, s: &mut ScanState) {
    match stmt {
        Statement::Print(exprs, _) => {
            s.has_output = true;
            collect_output_fields(exprs, &mut s.select_fields);
            if exprs.iter().any(|e| scan_expr(e, s)) {
                s.prints_computed = true;
            }
            if exprs.iter().any(|e| expr_mentions_any(e, RECORD_COUNTERS)) {
                s.prints_line_no = true;
            }
        }
        Statement::Printf(exprs, _) => {
            s.has_output = true;
            s.has_format = true;
            if exprs.len() > 1 {
                collect_output_fields(&exprs[1..], &mut s.select_fields);
            }
            for e in exprs { scan_expr(e, s); }
            if exprs.iter().any(|e| expr_mentions_any(e, RECORD_COUNTERS)) {
                s.prints_line_no = true;
            }
        }
        Statement::Expression(expr) => {
            if matches!(expr, Expr::FuncCall(name, _) if name == "dump") {
                s.has_output = true;
            }
            scan_accum(expr, s);
            scan_assign(expr, s);
            scan_expr(expr, s);
        }
        Statement::For(_, _, _, body) => {
            if for_mentions(stmt, "NF") {
                s.has_field_iteration = true;
            }
            for st in body { scan_stmt(st, s); }
        }
        Statement::Block(b) => {
            for st in b { scan_stmt(st, s); }
        }
        Statement::If(_, then_b, else_b) => {
            for st in then_b { scan_stmt(st, s); }
            if let Some(eb) = else_b {
                for st in eb { scan_stmt(st, s); }
            }
        }
        _ => {}
    }
}

/// Detect accumulation patterns via normalisation.
fn scan_accum(expr: &Expr, s: &mut ScanState) {
    let Some((target, delta)) = as_additive_accum(expr) else { return };
    let is_unit = delta.is_none()
        || matches!(delta, Some(Expr::NumberLit(n)) if *n == 1.0);
    match target {
        Expr::Var(_) => {
            if let Some(d) = delta {
                s.accum_field = Some(expr_to_source(d));
            }
        }
        Expr::ArrayRef(_, key) => {
            if is_unit {
                s.frequency_key = Some(expr_to_source(key));
            } else if let Some(d) = delta {
                s.aggregate_by = Some((expr_to_source(d), expr_to_source(key)));
            }
        }
        _ => {}
    }
}

/// Detect assign-level signals: field writes, ORS/OFS changes, collection.
fn scan_assign(expr: &Expr, s: &mut ScanState) {
    let Expr::Assign(target, _) = expr else { return };
    match target.as_ref() {
        Expr::ArrayRef(_, key)
            if expr_mentions_any(key, RECORD_COUNTERS) => s.has_collect = true,
        Expr::Field(_) => s.has_field_assign = true,
        Expr::Var(n) if OUTPUT_VARS.contains(&n.as_str()) => s.has_reformat = true,
        _ => {}
    }
}

/// If an expr is `Match($0, StringLit(pat))` (bare regex), format as /pat/.
/// Otherwise fall back to `expr_to_source`.
fn fmt_regex_or_expr(expr: &Expr) -> String {
    if let Expr::Match(lhs, rhs) = expr
        && matches!(lhs.as_ref(), Expr::Field(inner) if matches!(inner.as_ref(), Expr::NumberLit(n) if *n == 0.0))
        && let Expr::StringLit(pat) = rhs.as_ref()
    {
        return format!("/{pat}/");
    }
    expr_to_source(expr)
}

// ── Generic AST predicate: does expression mention variable `name`? ──

fn expr_mentions(expr: &Expr, var: &str) -> bool {
    match expr {
        Expr::Var(name) => name == var,
        Expr::BinOp(l, _, r) | Expr::Concat(l, r) | Expr::LogicalAnd(l, r)
        | Expr::LogicalOr(l, r) | Expr::Match(l, r) | Expr::NotMatch(l, r)
        | Expr::Assign(l, r) | Expr::CompoundAssign(l, _, r) => {
            expr_mentions(l, var) || expr_mentions(r, var)
        }
        Expr::UnaryMinus(e) | Expr::LogicalNot(e) | Expr::Field(e)
        | Expr::Increment(e, _) | Expr::Decrement(e, _) => expr_mentions(e, var),
        Expr::FuncCall(_, args) => args.iter().any(|a| expr_mentions(a, var)),
        _ => false,
    }
}

fn expr_mentions_any(expr: &Expr, vars: &[&str]) -> bool {
    vars.iter().any(|v| expr_mentions(expr, v))
}

fn for_mentions(stmt: &Statement, var: &str) -> bool {
    if let Statement::For(init, cond, incr, _) = stmt {
        let in_init = init.as_ref().is_some_and(|s| {
            matches!(s.as_ref(), Statement::Expression(e) if expr_mentions(e, var))
        });
        let in_cond = cond.as_ref().is_some_and(|e| expr_mentions(e, var));
        let in_incr = incr.as_ref().is_some_and(|s| {
            matches!(s.as_ref(), Statement::Expression(e) if expr_mentions(e, var))
        });
        return in_init || in_cond || in_incr;
    }
    false
}




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
    if depth > 10 { buf.push_str("..."); return; }
    match expr {
        Expr::Field(inner) => {
            buf.push('$');
            let needs_parens = !matches!(inner.as_ref(),
                Expr::NumberLit(_) | Expr::Var(_) | Expr::StringLit(_));
            if needs_parens { buf.push('('); }
            fmt_expr(inner, buf, depth + 1);
            if needs_parens { buf.push(')'); }
        }
        Expr::NumberLit(n) => {
            if *n == (*n as i64) as f64 {
                let _ = write!(buf, "{}", *n as i64);
            } else {
                let _ = write!(buf, "{n}");
            }
        }
        Expr::StringLit(s) => { let _ = write!(buf, "\"{s}\""); }
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
                BinOp::Add => " + ", BinOp::Sub => " - ",
                BinOp::Mul => " * ", BinOp::Div => " / ",
                BinOp::Mod => " % ", BinOp::Pow => " ** ",
                BinOp::Eq => " == ", BinOp::Ne => " != ",
                BinOp::Lt => " < ",  BinOp::Le => " <= ",
                BinOp::Gt => " > ",  BinOp::Ge => " >= ",
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
                BinOp::Add => " += ", BinOp::Sub => " -= ",
                BinOp::Mul => " *= ", BinOp::Div => " /= ",
                BinOp::Mod => " %= ", BinOp::Pow => " **= ",
                _ => " ?= ",
            });
            fmt_expr(val, buf, depth + 1);
        }
        Expr::Increment(e, pre) => {
            if *pre { buf.push_str("++"); }
            fmt_expr(e, buf, depth + 1);
            if !*pre { buf.push_str("++"); }
        }
        Expr::Decrement(e, pre) => {
            if *pre { buf.push_str("--"); }
            fmt_expr(e, buf, depth + 1);
            if !*pre { buf.push_str("--"); }
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
            let name = if let Expr::FuncCall(n, _) = expr { n.as_str() } else { "sprintf" };
            buf.push_str(name);
            buf.push('(');
            for (i, a) in args.iter().enumerate() {
                if i > 0 { buf.push_str(", "); }
                fmt_expr(a, buf, depth + 1);
            }
            buf.push(')');
        }
        Expr::Getline(var, source) => {
            buf.push_str("getline");
            if let Some(v) = var { let _ = write!(buf, " {v}"); }
            if let Some(src) = source {
                buf.push_str(" < ");
                fmt_expr(src, buf, depth + 1);
            }
        }
        Expr::GetlinePipe(cmd, var) => {
            fmt_expr(cmd, buf, depth + 1);
            buf.push_str(" | getline");
            if let Some(v) = var { let _ = write!(buf, " {v}"); }
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
        assert_eq!(expr_to_source(info.var_sources.get("x").unwrap()), "$1 + $2");
        assert_eq!(expr_to_source(info.array_sources.get("a").unwrap()), "x");
    }

    #[test]
    fn description_jpath_with_file() {
        let info = analyze_program("{ ms = jpath($0, \".ms\") + 0; lat[NR] = ms }");
        let expr = info.array_sources.get("lat").unwrap();
        let desc = build_array_description(expr, "api.jsonl", &info.var_sources);
        assert_eq!(desc, "api.jsonl — [].ms");
    }

    #[test]
    fn description_field_with_file() {
        let info = analyze_program("{ a[NR] = $3 }");
        let expr = info.array_sources.get("a").unwrap();
        let desc = build_array_description(expr, "data.csv", &info.var_sources);
        assert_eq!(desc, "data.csv — $3");
    }

    #[test]
    fn description_field_stdin() {
        let info = analyze_program("{ a[NR] = $1 }");
        let expr = info.array_sources.get("a").unwrap();
        let desc = build_array_description(expr, "", &info.var_sources);
        assert_eq!(desc, "$1");
    }

    #[test]
    fn description_named_column_with_file() {
        let info = analyze_program("{ a[NR] = $\"latency\" }");
        let expr = info.array_sources.get("a").unwrap();
        let desc = build_array_description(expr, "metrics.csv", &info.var_sources);
        assert_eq!(desc, "metrics.csv — latency");
    }

    #[test]
    fn description_expr_with_file() {
        let info = analyze_program("{ a[NR] = $1 + $2 }");
        let expr = info.array_sources.get("a").unwrap();
        let desc = build_array_description(expr, "data.csv", &info.var_sources);
        assert_eq!(desc, "data.csv — $1 + $2");
    }

    fn explain_program(src: &str) -> String {
        let tokens = Lexer::new(src).tokenize().unwrap();
        let program = Parser::new(tokens).parse().unwrap();
        explain(&program, None)
    }

    fn explain_with_ctx(src: &str, ctx: &ExplainContext) -> String {
        let tokens = Lexer::new(src).tokenize().unwrap();
        let program = Parser::new(tokens).parse().unwrap();
        explain(&program, Some(ctx))
    }

    #[test]
    fn explain_select_fields() {
        assert_eq!(explain_program("{ print $1, $2 }"), "select $1, $2");
    }

    #[test]
    fn explain_passthrough_is_empty() {
        assert_eq!(explain_program("{ print }"), "");
        assert_eq!(explain_program("{ print $0 }"), "");
    }

    #[test]
    fn explain_filter_pattern() {
        assert_eq!(
            explain_program("/Math/ { print $1, $2 }"),
            "filter /Math/, select $1, $2",
        );
    }

    #[test]
    fn explain_filter_comparison() {
        assert_eq!(
            explain_program("$2 > 90 { print $1 }"),
            "filter $2 > 90, select $1",
        );
    }

    #[test]
    fn explain_sum() {
        assert_eq!(explain_program("{ sum += $2 } END { print sum }"), "sum $2");
    }

    #[test]
    fn explain_frequency() {
        assert_eq!(explain_program("{ a[$1]++ } END { for (k in a) print k }"), "frequency $1");
    }

    #[test]
    fn explain_histogram() {
        assert_eq!(
            explain_program("{ a[NR]=$1 } END { print plotbox(hist(a)) }"),
            "histogram $1",
        );
    }

    #[test]
    fn explain_stats() {
        assert_eq!(
            explain_program("{ a[NR]=$2 } END { print mean(a), median(a) }"),
            "stats $2",
        );
    }

    #[test]
    fn explain_count() {
        assert_eq!(explain_program("END { print NR }"), "count");
    }

    #[test]
    fn explain_gsub_transform() {
        assert_eq!(
            explain_program("{ gsub(/foo/, \"bar\"); print }"),
            "gsub /foo/ → \"bar\"",
        );
    }

    #[test]
    fn explain_jpath_format() {
        assert_eq!(
            explain_program("{ m = jpath($0, \".method\"); printf \"%s\\n\", m }"),
            "extract + format JSON fields",
        );
    }

    #[test]
    fn explain_compound_assign_tracked() {
        assert_eq!(
            explain_program("{ rev[$1] += $2 } END { print mean(rev) }"),
            "stats $2",
        );
    }

    #[test]
    fn explain_unique() {
        assert_eq!(explain_program("!seen[$0]++"), "unique $0");
    }

    #[test]
    fn explain_unique_multikey() {
        assert_eq!(explain_program("!seen[$1,$2]++"), "unique $1, $2");
    }

    #[test]
    fn explain_join() {
        assert_eq!(
            explain_program("NR==FNR{price[$1]=$2; next} {print $0, price[$1]+0}"),
            "join",
        );
    }

    #[test]
    fn explain_anti_join() {
        assert_eq!(
            explain_program("NR==FNR{skip[$1]=1; next} !($1 in skip)"),
            "anti-join",
        );
    }

    #[test]
    fn explain_semi_join() {
        assert_eq!(
            explain_program("NR==FNR{keep[$1]=1; next} $1 in keep"),
            "semi-join",
        );
    }

    #[test]
    fn explain_count_pattern() {
        assert_eq!(
            explain_program("/Beth/{n++}; END {print n+0}"),
            "count /Beth/",
        );
    }

    #[test]
    fn explain_aggregate_by() {
        assert_eq!(
            explain_program("{ s[$1]+=$2; c[$1]++ } END { for(k in s) print k, s[k]/c[k] }"),
            "aggregate $2 by $1",
        );
    }

    #[test]
    fn explain_sum_by_group() {
        assert_eq!(
            explain_program("{ rev[$region] += $revenue } END { for (r in rev) printf \"%s: %.2f\\n\", r, rev[r] }"),
            "sum $revenue by $region",
        );
    }

    #[test]
    fn explain_transform_suppresses_filter_1() {
        assert_eq!(
            explain_program("{sub(/\\r$/,\"\")};1"),
            "sub /\\r$/ → \"\"",
        );
    }

    #[test]
    fn explain_regex_extract_format() {
        assert_eq!(
            explain_program("{ match($0, \"pattern\", c); printf \"%s\\n\", c[1] }"),
            "regex extract + format",
        );
    }

    #[test]
    fn explain_multi_fragment_renders_all() {
        assert_eq!(
            explain_program("/baz/ { gsub(/foo/, \"bar\"); print }"),
            "gsub /foo/ → \"bar\", filter /baz/",
        );
    }

    #[test]
    fn explain_chart_subsumes_stats() {
        assert_eq!(
            explain_program("{ a[NR]=$1 } END { print plotbox(hist(a)), mean(a) }"),
            "histogram $1",
        );
    }

    #[test]
    fn explain_stats_subsumes_sum_by() {
        assert_eq!(
            explain_program("{ rev[$1] += $2 } END { printf \"%.2f\\n\", mean(rev) }"),
            "stats $2",
        );
    }

    #[test]
    fn render_budget_truncation() {
        let frags = vec![
            Fragment::new("histogram of some very long expression name", FragTag::Chart),
            Fragment::new("filter $7 ~ /^extremely-long-pattern-that-keeps-going$/", FragTag::Filter),
        ];
        let rendered = render(&frags, 72);
        assert!(rendered.len() <= 72, "rendered len {} > 72: {rendered}", rendered.len());
        assert!(rendered.contains("histogram"));
        assert!(rendered.ends_with('…'));
    }

    #[test]
    fn render_drops_least_significant_first() {
        let frags = vec![
            Fragment::new("stats $2", FragTag::Stats),
            Fragment::new("filter /foo/", FragTag::Filter),
        ];
        let rendered = render(&frags, 72);
        assert_eq!(rendered, "stats $2, filter /foo/");
    }

    #[test]
    fn render_empty_frags_returns_empty() {
        assert_eq!(render(&[], 72), "");
    }

    #[test]
    fn explain_env_csv_headers() {
        let ctx = ExplainContext::from_cli("csv", true, None, &["sales.csv".into()]);
        assert_eq!(
            explain_with_ctx("{ sum += $2 } END { print sum }", &ctx),
            "sum $2 (CSV, headers, sales.csv)",
        );
    }

    #[test]
    fn explain_env_compressed_json() {
        let ctx = ExplainContext::from_cli(
            "json", false, None, &["api.jsonl.gz".into()],
        );
        assert_eq!(
            explain_with_ctx("{ a[NR]=$1 } END { print plotbox(hist(a)) }", &ctx),
            "histogram $1 (JSON, gzip, api.jsonl.gz)",
        );
    }

    #[test]
    fn explain_env_field_sep() {
        let ctx = ExplainContext::from_cli("line", false, Some(":"), &[]);
        assert_eq!(
            explain_with_ctx("{ print $1 }", &ctx),
            "select $1 (-F ':')",
        );
    }

    #[test]
    fn explain_env_multiple_files() {
        let ctx = ExplainContext::from_cli(
            "line", false, None, &["a.txt".into(), "b.txt".into(), "c.txt".into()],
        );
        assert_eq!(
            explain_with_ctx("/foo/ { print }", &ctx),
            "filter /foo/ (3 files)",
        );
    }

    #[test]
    fn explain_env_select_no_env() {
        let ctx = ExplainContext::from_cli("line", false, None, &[]);
        assert_eq!(explain_with_ctx("{ print $1, $2 }", &ctx), "select $1, $2");
    }

    #[test]
    fn explain_env_passthrough_no_env() {
        let ctx = ExplainContext::from_cli("line", false, None, &[]);
        assert_eq!(explain_with_ctx("{ print }", &ctx), "");
    }

    #[test]
    fn explain_env_idiom_with_context() {
        let ctx = ExplainContext::from_cli("csv", true, None, &["data.csv".into()]);
        assert_eq!(
            explain_with_ctx("!seen[$0]++", &ctx),
            "unique $0 (CSV, headers, data.csv)",
        );
    }

    #[test]
    fn explain_env_auto_detected_line_mode_no_noise() {
        let ctx = ExplainContext::from_cli("line", false, None, &["data.txt".into()]);
        assert_eq!(
            explain_with_ctx("{ sum += $1 } END { print sum }", &ctx),
            "sum $1 (data.txt)",
        );
    }

    #[test]
    fn explain_select_named_columns() {
        assert_eq!(
            explain_program("{ print $\"host-name\", $\"cpu-usage\" }"),
            "select host-name, cpu-usage",
        );
    }

    #[test]
    fn explain_select_many_fields_summarized() {
        assert_eq!(
            explain_program("{ print $1, $2, $3, $4, $5, $6 }"),
            "select 6 fields",
        );
    }

    #[test]
    fn explain_select_printf() {
        assert_eq!(
            explain_program("{ printf \"%s %s\\n\", $1, $2 }"),
            "select $1, $2",
        );
    }

    #[test]
    fn explain_number_lines() {
        assert_eq!(explain_program("{print FNR \"\\t\" $0}"), "number lines");
        assert_eq!(
            explain_program("{printf \"%5d : %s\\n\", NR, $0}"),
            "number lines",
        );
    }

    #[test]
    fn explain_iterate_fields() {
        assert_eq!(
            explain_program("{for (i=NF; i>0; i--) printf \"%s \",$i; print \"\"}"),
            "iterate fields",
        );
    }

    #[test]
    fn explain_collect_emit() {
        assert_eq!(
            explain_program("{ a[NR]=$0 } END { for(i=NR;i>=1;i--) print a[i] }"),
            "collect + emit",
        );
    }

    #[test]
    fn explain_sum_noncompound() {
        assert_eq!(
            explain_program("{ total = total + NF }; END {print total}"),
            "sum NF",
        );
    }

    #[test]
    fn explain_rewrite_fields() {
        assert_eq!(explain_program("{ $2 = \"\"; print }"), "rewrite fields");
    }

    #[test]
    fn explain_reformat_output() {
        assert_eq!(explain_program("BEGIN{ORS=\"\\n\\n\"};1"), "reformat output");
    }

    #[test]
    fn explain_compute() {
        assert_eq!(explain_program("{ print length($0) }"), "compute");
        assert_eq!(explain_program("{ print $1 + $2 }"), "compute");
    }

    #[test]
    fn explain_field_iteration_sum() {
        assert_eq!(
            explain_program("{s=0; for (i=1; i<=NF; i++) s=s+$i; print s}"),
            "sum $i, iterate fields",
        );
    }
}
