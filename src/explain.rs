use std::collections::HashMap;
use std::path::Path;

use crate::analyze::{ProgramInfo, analyze, unwrap_coercion};
use crate::parser::*;

const BUDGET: usize = 72;

#[derive(Debug, Default)]
pub struct ExplainContext {
    pub input_mode: Option<String>,
    pub headers: bool,
    pub compressed: Option<String>,
    pub field_sep: Option<String>,
    pub files: Vec<String>,
}

impl ExplainContext {
    pub fn from_cli(mode: &str, headers: bool, field_sep: Option<&str>, files: &[String]) -> Self {
        let mut input_mode = match mode {
            "line" => None,
            m => Some(m.to_uppercase()),
        };

        if input_mode.is_none()
            && field_sep.is_none()
            && let Some(f) = files.first()
        {
            input_mode = detect_format_from_ext(f);
        }

        let compressed = files.first().and_then(|f| detect_compression(f));

        let filenames: Vec<String> = files
            .iter()
            .map(|f| {
                Path::new(f)
                    .file_name()
                    .map_or_else(|| f.clone(), |n| n.to_string_lossy().into_owned())
            })
            .collect();

        Self {
            input_mode,
            headers,
            compressed,
            field_sep: field_sep.map(|s| s.to_string()),
            files: filenames,
        }
    }

    fn suffix(&self) -> Option<String> {
        let mut parts: Vec<String> = Vec::new();
        if let Some(ref m) = self.input_mode {
            parts.push(m.clone());
        }
        if let Some(ref c) = self.compressed {
            parts.push(c.clone());
        }
        if self.headers {
            parts.push("headers".into());
        }
        if let Some(ref f) = self.field_sep {
            parts.push(format!("-F '{f}'"));
        }
        match self.files.len() {
            0 => {}
            1 => parts.push(self.files[0].clone()),
            n => parts.push(format!("{n} files")),
        }
        if parts.is_empty() {
            return None;
        }
        Some(format!("({})", parts.join(", ")))
    }
}

pub fn explain(program: &Program, ctx: Option<&ExplainContext>) -> String {
    let info = analyze(program);
    let env = ctx.and_then(|c| c.suffix());
    let base = describe(program, &info);

    match env.as_deref() {
        None => base,
        Some(e) if base.is_empty() && e.len() <= BUDGET => e.to_string(),
        Some(e) => {
            let combined = format!("{base} {e}");
            if combined.len() <= BUDGET {
                combined
            } else {
                base
            }
        }
    }
}

// ── Single-pass pattern detection ───────────────────────────────

fn describe(program: &Program, info: &ProgramInfo) -> String {
    let vs = &info.var_sources;

    if let Some(s) = try_dedup(program) {
        return s;
    }
    if let Some(s) = try_join(program) {
        return s;
    }

    let mut phrases: Vec<String> = Vec::new();
    let mut high_level = false;

    if let Some(s) = try_chart_stats(program, vs) {
        phrases.push(s);
        high_level = true;
    } else if let Some(s) = try_count(program) {
        phrases.push(s);
        high_level = true;
    } else if let Some(s) = try_aggregation(program, vs) {
        phrases.push(s);
        high_level = true;
    }

    if !high_level {
        for rule in &program.rules {
            detect_rule_phrases(rule, vs, &mut phrases);
        }
    } else {
        let chart_has_jpath = phrases.iter().any(|p| {
            (p.starts_with("histogram") || p.starts_with("statistics"))
                && !p.ends_with("histogram")
                && !p.ends_with("statistics")
        });
        if !chart_has_jpath {
            for rule in &program.rules {
                detect_extract(&rule.action, vs, &mut phrases);
            }
        }
    }

    if let Some(begin) = &program.begin {
        detect_block_phrases(begin, vs, &mut phrases);
    }

    if phrases.is_empty() {
        detect_fallback(program, vs, &mut phrases);
    }

    budget_join(&phrases, BUDGET)
}

// ── Whole-program idioms ────────────────────────────────────────

fn try_dedup(program: &Program) -> Option<String> {
    if program.rules.len() != 1 || program.begin.is_some() || program.end.is_some() {
        return None;
    }
    let rule = &program.rules[0];
    let pat = rule.pattern.as_ref()?;
    if let Pattern::Expression(Expr::LogicalNot(inner)) = pat
        && let Expr::Increment(arr, false) = inner.as_ref()
        && let Expr::ArrayRef(_, key) = arr.as_ref()
    {
        let key_text = to_title_columns(&humanize(&field_ref(key, &HashMap::new())));
        let by = if key_text == "$0" { "line" } else { &key_text };
        return Some(format!("deduplicate by {by}"));
    }
    None
}

fn try_join(program: &Program) -> Option<String> {
    if program.rules.len() < 2 {
        return None;
    }
    let first = &program.rules[0];
    let pat = first.pattern.as_ref()?;
    if !is_nr_eq_fnr(pat) {
        return None;
    }
    if !block_has_next(&first.action) {
        return None;
    }

    let key = first.action.iter().find_map(|s| {
        if let Statement::Expression(Expr::Assign(target, _)) = s
            && let Expr::ArrayRef(_, k) = target.as_ref()
        {
            Some(humanize(&field_ref(k, &HashMap::new())))
        } else if let Statement::Expression(Expr::Increment(inner, _)) = s
            && let Expr::ArrayRef(_, k) = inner.as_ref()
        {
            Some(humanize(&field_ref(k, &HashMap::new())))
        } else {
            None
        }
    });

    let second = &program.rules[1];
    let kind = match &second.pattern {
        Some(Pattern::Expression(Expr::LogicalNot(inner))) if expr_mentions_in(inner) => {
            "rows without a match"
        }
        Some(Pattern::Expression(e)) if expr_mentions_in(e) => "matching rows",
        _ => "join",
    };

    let text = match key {
        Some(k) => format!("{kind} on {}", to_title_columns(&k)),
        None => kind.into(),
    };
    Some(text)
}

fn try_count(program: &Program) -> Option<String> {
    program.end.as_ref()?;

    let has_array_accum = program.rules.iter().any(|r| {
        r.action.iter().any(|s| match s {
            Statement::Expression(e) => expr_has_array_accum(e),
            _ => false,
        })
    });
    if has_array_accum {
        return None;
    }

    for rule in &program.rules {
        let has_inc = rule.action.iter().any(|s| {
            matches!(s, Statement::Expression(Expr::Increment(inner, _))
                if matches!(inner.as_ref(), Expr::Var(_)))
        });
        if has_inc {
            let pat_text = rule.pattern.as_ref().map(describe_pattern);
            return match pat_text {
                Some(p) => Some(format!("count where {}", to_title_columns(&humanize(&p)))),
                None => Some("count lines".into()),
            };
        }
    }

    if program.rules.is_empty() {
        let end = program.end.as_ref()?;
        let has_emit = end
            .iter()
            .any(|s| matches!(s, Statement::Print(..) | Statement::Printf(..)));
        if has_emit {
            return Some("count lines".into());
        }
    }
    None
}

fn try_chart_stats(program: &Program, vs: &HashMap<String, Expr>) -> Option<String> {
    let end = program.end.as_ref()?;
    let fns = collect_fn_names_block(end);
    let chart_fns = ["hist", "plotbox", "plot"];
    let stat_fns = [
        "mean",
        "median",
        "stddev",
        "variance",
        "sum",
        "min",
        "max",
        "p",
        "percentile",
        "quantile",
        "iqm",
    ];

    let has_chart = fns.iter().any(|f| chart_fns.contains(&f.as_str()));
    let has_stats = fns.iter().any(|f| stat_fns.contains(&f.as_str()));
    if !has_chart && !has_stats {
        return None;
    }

    let source = find_array_source(program, vs);

    let jpath_source = program.rules.iter().find_map(|r| {
        r.action.iter().find_map(|s| match s {
            Statement::Expression(Expr::Assign(_, val)) => jpath_source_label(val),
            Statement::Expression(Expr::FuncCall(name, args)) if name == "jpath" => {
                args.get(1).and_then(|a| {
                    if let Expr::StringLit(p) = a {
                        Some(p.trim_start_matches('.').to_string())
                    } else {
                        None
                    }
                })
            }
            _ => None,
        })
    });

    let label = if has_chart { "histogram" } else { "statistics" };
    let desc = match (&jpath_source, &source) {
        (Some(jp), _) => format!("{label} of {jp}"),
        (None, Some(s)) => format!("{label} of {}", to_title_columns(&humanize(s))),
        (None, None) => label.into(),
    };
    Some(desc)
}

fn try_aggregation(program: &Program, vs: &HashMap<String, Expr>) -> Option<String> {
    let end = program.end.as_ref()?;
    let has_for_in = end.iter().any(|s| matches!(s, Statement::ForIn(..)));

    let mut accum: Option<(String, String)> = None; // (key, val)
    let mut freq: Option<String> = None; // key
    let mut simple_accum: Option<String> = None; // source field

    for rule in &program.rules {
        scan_block_accum(&rule.action, vs, &mut accum, &mut freq, &mut simple_accum);
    }

    if let (Some((ak, av)), Some(fk)) = (&accum, &freq)
        && ak == fk
        && has_for_in
    {
        return Some(format!(
            "aggregation of {} by {}",
            to_title_columns(&humanize(av)),
            to_title_columns(&humanize(ak))
        ));
    }

    if let Some(key) = &freq
        && has_for_in
    {
        return Some(format!("frequency of {}", to_title_columns(&humanize(key))));
    }

    if let Some((key, val)) = &accum
        && has_for_in
    {
        return Some(format!(
            "sum of {} by {}",
            to_title_columns(&humanize(val)),
            to_title_columns(&humanize(key))
        ));
    }

    if let Some(src) = &simple_accum {
        return Some(format!("sum of {}", to_title_columns(&humanize(src))));
    }

    None
}

// ── Per-rule phrase detection ───────────────────────────────────

fn detect_rule_phrases(rule: &Rule, vs: &HashMap<String, Expr>, phrases: &mut Vec<String>) {
    let mut local: Vec<String> = Vec::new();

    detect_capture_filter(&rule.action, &mut local);
    detect_extract(&rule.action, vs, &mut local);
    detect_transform(&rule.action, &mut local);
    detect_rule_accum(&rule.action, vs, &mut local);
    detect_iter_fields(rule, &mut local);
    detect_for_range(&rule.action, vs, &mut local);

    if local.is_empty()
        || !local
            .iter()
            .any(|p| p.starts_with("iterate") || p.starts_with("range") || p.starts_with("for all"))
    {
        detect_select(rule, vs, &mut local);
    }
    detect_number_lines(rule, &mut local);
    detect_rewrite(&rule.action, &mut local);
    if detect_collect(rule) {
        local.push("collect lines".into());
    }

    let filter = if let Some(pat) = &rule.pattern {
        let skip = matches!(pat, Pattern::Expression(Expr::NumberLit(n)) if *n == 1.0);
        if !skip {
            let text = humanize(&describe_pattern(pat));
            if text != "1" {
                Some(format!("where {}", to_title_columns(&text)))
            } else {
                None
            }
        } else {
            None
        }
    } else {
        None
    };

    if let Some(f) = filter {
        if local.is_empty() {
            phrases.push(f);
        } else {
            let action = local.join(", ");
            phrases.push(format!("{f}: {action}"));
        }
    } else {
        for p in local {
            if !phrases.contains(&p) {
                phrases.push(p);
            }
        }
    }
}

fn detect_block_phrases(block: &Block, vs: &HashMap<String, Expr>, phrases: &mut Vec<String>) {
    let mut found_range = false;
    for stmt in block {
        if let Statement::For(init, cond, update, body) = stmt {
            let has_nf = cond.as_ref().is_some_and(mentions_nf)
                || init.as_ref().is_some_and(|s| stmt_mentions_nf(s));
            if has_nf {
                if !phrases.iter().any(|p| p.contains("iterate")) {
                    phrases.push("iterate fields".into());
                }
                continue;
            }
            let (bounds, over_key) = for_range_bounds(init, cond, vs);
            let mut select_fields: Vec<String> = Vec::new();
            collect_block_output_refs(body, vs, &mut select_fields);
            collect_block_output_refs(block, vs, &mut select_fields);
            if let Some(s) = init {
                collect_stmt_output_refs(s, vs, &mut select_fields);
            }
            if let Some(s) = update {
                collect_stmt_output_refs(s, vs, &mut select_fields);
            }
            dedup_preserve_order(&mut select_fields);
            let select_text = if select_fields.is_empty() {
                String::new()
            } else {
                format_field_list(&select_fields)
            };

            let phrase = match (&over_key, &bounds) {
                (Some(k), _) if !select_text.is_empty() => format!("for all {k}: {select_text}"),
                (Some(k), _) => format!("for all {k}"),
                (_, Some(b)) if !select_text.is_empty() => {
                    format!("range {}: {select_text}", b.replace('–', ".."))
                }
                (_, Some(b)) => format!("range {}", b.replace('–', "..")),
                _ if !select_text.is_empty() => format!("range: {select_text}"),
                _ => continue,
            };
            phrases.push(phrase);
            found_range = true;
        } else if !found_range
            && let Statement::Print(exprs, _) | Statement::Printf(exprs, _) = stmt
        {
            let refs = if matches!(stmt, Statement::Printf(..)) && exprs.len() > 1 {
                &exprs[1..]
            } else {
                exprs
            };
            let fields = collect_output_refs(refs, vs);
            if !fields.is_empty() {
                let json = looks_like_json_paths(&fields);
                let text = if json {
                    format!("from JSON: {}", fields.join(", "))
                } else {
                    format!("select {}", format_field_list(&fields))
                };
                if !phrases.contains(&text) {
                    phrases.push(text);
                }
            }
        }
    }

    let has_slurp = block.iter().any(|s| match s {
        Statement::Expression(Expr::FuncCall(name, _)) if name == "slurp" => true,
        Statement::Expression(Expr::Assign(_, val)) => {
            matches!(val.as_ref(), Expr::FuncCall(name, _) if name == "slurp")
        }
        _ => false,
    });
    if has_slurp {
        let path = block.iter().find_map(|s| {
            let call = match s {
                Statement::Expression(Expr::FuncCall(name, args)) if name == "slurp" => Some(args),
                Statement::Expression(Expr::Assign(_, val)) => {
                    if let Expr::FuncCall(name, args) = val.as_ref() {
                        if name == "slurp" { Some(args) } else { None }
                    } else {
                        None
                    }
                }
                _ => None,
            };
            call.and_then(|args| {
                if let Expr::StringLit(p) = args.first()? {
                    Some(p.clone())
                } else {
                    None
                }
            })
        });
        if let Some(p) = path {
            let base = Path::new(&p)
                .file_name()
                .and_then(|n| n.to_str())
                .unwrap_or(&p);
            if let Some(sel) = phrases.iter_mut().find(|p| p.starts_with("from JSON:")) {
                sel.push_str(&format!(", slurped from {base}"));
            } else {
                phrases.push(format!("slurped from {base}"));
            }
        }
    }
}

fn detect_fallback(program: &Program, vs: &HashMap<String, Expr>, phrases: &mut Vec<String>) {
    if let Some(begin) = &program.begin {
        let has_reformat = begin.iter().any(|s| match s {
            Statement::Expression(Expr::Assign(target, _)) =>
                matches!(target.as_ref(), Expr::Var(n) if matches!(n.as_str(), "ORS" | "OFS" | "OFMT")),
            _ => false,
        });
        if has_reformat && !phrases.iter().any(|p| p.contains("reformat")) {
            phrases.push("reformat output".into());
            return;
        }
    }

    let has_output = program.rules.iter().any(|r| {
        r.action
            .iter()
            .any(|s| matches!(s, Statement::Print(..) | Statement::Printf(..)))
    }) || program.begin.as_ref().is_some_and(|b| {
        b.iter()
            .any(|s| matches!(s, Statement::Print(..) | Statement::Printf(..)))
    });

    if has_output && phrases.is_empty() {
        let rule = program.rules.first();
        if let Some(r) = rule {
            let fields = collect_rule_output_refs(r, vs);
            if !fields.is_empty() {
                let json = looks_like_json_paths(&fields);
                if json {
                    phrases.push(format!("from JSON: {}", fields.join(", ")));
                } else {
                    phrases.push(format!("select {}", format_field_list(&fields)));
                }
                return;
            }
        }
        phrases.push("pass through".into());
    }
}

// ── Sub-detectors ───────────────────────────────────────────────

fn detect_capture_filter(block: &Block, phrases: &mut Vec<String>) {
    for stmt in block {
        if let Statement::If(cond, _, _) = stmt
            && let Some(text) = describe_capture_filter_cond(cond)
        {
            phrases.push(text);
        }
    }
}

fn detect_extract(block: &Block, vs: &HashMap<String, Expr>, phrases: &mut Vec<String>) {
    let has_match = block_calls_fn(block, "match");
    let jpath_paths = collect_jpath_paths(block);
    let has_jpath = !jpath_paths.is_empty();
    let has_fmt = block.iter().any(|s| matches!(s, Statement::Printf(..)));

    let jpath_in_select = block.iter().any(|s| {
        if let Statement::Print(exprs, _) | Statement::Printf(exprs, _) = s {
            let fields = collect_output_refs(exprs, vs);
            fields
                .iter()
                .any(|f| f.parse::<usize>().is_err() && !f.is_empty())
        } else {
            false
        }
    });

    let extract = match (has_match, has_jpath && !jpath_in_select, has_fmt) {
        (true, _, true) => Some("pattern extract + format".into()),
        (true, _, false) => Some("pattern extract".into()),
        (false, true, true) => {
            let paths = format_jpath_paths(&jpath_paths);
            Some(if paths.is_empty() {
                "JSON extract + format".into()
            } else {
                format!("JSON extract {paths} + format")
            })
        }
        (false, true, false) => {
            let paths = format_jpath_paths(&jpath_paths);
            Some(if paths.is_empty() {
                "JSON extract".into()
            } else {
                format!("JSON extract {paths}")
            })
        }
        _ => None,
    };
    if let Some(text) = extract {
        phrases.push(text);
    }
}

fn detect_transform(block: &Block, phrases: &mut Vec<String>) {
    for stmt in block {
        if let Statement::Expression(Expr::FuncCall(name, args)) = stmt
            && matches!(name.as_str(), "gsub" | "sub" | "gensub")
        {
            let pat = args.first().map(fmt_regex_or_expr).unwrap_or_default();
            let repl = args.get(1).map(expr_literal_text).unwrap_or_default();
            phrases.push(format!("replace {pat} → {repl}"));
            return;
        }
        if let Statement::Expression(Expr::Assign(_, val)) = stmt
            && let Expr::FuncCall(name, args) = val.as_ref()
            && matches!(name.as_str(), "gsub" | "sub" | "gensub")
        {
            let pat = args.first().map(fmt_regex_or_expr).unwrap_or_default();
            let repl = args.get(1).map(expr_literal_text).unwrap_or_default();
            phrases.push(format!("replace {pat} → {repl}"));
            return;
        }
        if let Statement::Print(exprs, _) | Statement::Printf(exprs, _) = stmt {
            for e in exprs {
                if let Expr::FuncCall(name, args) = e
                    && matches!(name.as_str(), "gsub" | "sub" | "gensub")
                {
                    let pat = args.first().map(fmt_regex_or_expr).unwrap_or_default();
                    let repl = args.get(1).map(expr_literal_text).unwrap_or_default();
                    phrases.push(format!("replace {pat} → {repl}"));
                    return;
                }
            }
        }
    }
}

fn detect_select(rule: &Rule, vs: &HashMap<String, Expr>, phrases: &mut Vec<String>) {
    let fields = collect_rule_output_refs(rule, vs);
    if fields.is_empty() {
        return;
    }

    let transform_verb = phrases.iter().find_map(|p| {
        if p.starts_with("replace") {
            Some("replace")
        } else if p.starts_with("transform") {
            Some("transform")
        } else {
            None
        }
    });

    if fields.len() == 1 {
        let f = &fields[0];
        if let Some(verb) = transform_verb
            && (f == verb || matches!(f.as_str(), "sub" | "gsub" | "gensub"))
        {
            return;
        }
    }

    let json = looks_like_json_paths(&fields);
    let text = if json {
        format!("from JSON: {}", fields.join(", "))
    } else {
        format!("select {}", format_field_list(&fields))
    };
    if !phrases.contains(&text) {
        phrases.push(text);
    }
}

fn detect_number_lines(rule: &Rule, phrases: &mut Vec<String>) {
    for stmt in &rule.action {
        if let Statement::Print(exprs, _) | Statement::Printf(exprs, _) = stmt
            && exprs.iter().any(expr_mentions_counter)
        {
            let fields = exprs
                .iter()
                .filter(|e| !matches!(e, Expr::StringLit(_)))
                .count();
            if fields <= 2 {
                phrases.push("number lines".into());
                return;
            }
        }
    }
}

fn detect_rewrite(block: &Block, phrases: &mut Vec<String>) {
    let has_assign_field = block.iter().any(|s| match s {
        Statement::Expression(Expr::Assign(target, _)) => matches!(target.as_ref(), Expr::Field(_)),
        _ => false,
    });
    let has_reformat = block.iter().any(|s| match s {
        Statement::Expression(Expr::Assign(target, _)) => {
            matches!(target.as_ref(), Expr::Var(n) if matches!(n.as_str(), "ORS" | "OFS" | "OFMT"))
        }
        _ => false,
    });
    if has_assign_field {
        phrases.push("rewrite fields".into());
    } else if has_reformat && phrases.is_empty() {
        phrases.push("reformat output".into());
    }
}

fn detect_rule_accum(block: &Block, vs: &HashMap<String, Expr>, phrases: &mut Vec<String>) {
    let mut accum: Option<(String, String)> = None;
    let mut freq: Option<String> = None;
    let mut simple: Option<String> = None;
    scan_block_accum(block, vs, &mut accum, &mut freq, &mut simple);
    if let Some(src) = simple {
        phrases.push(format!("sum of {}", to_title_columns(&humanize(&src))));
    }
}

fn detect_iter_fields(rule: &Rule, phrases: &mut Vec<String>) {
    for stmt in &rule.action {
        if let Statement::For(init, cond, _, _) = stmt {
            let has_nf = cond.as_ref().is_some_and(mentions_nf)
                || init.as_ref().is_some_and(|s| stmt_mentions_nf(s));
            if has_nf {
                phrases.push("iterate fields".into());
                return;
            }
        }
    }
}

fn detect_for_range(block: &Block, vs: &HashMap<String, Expr>, phrases: &mut Vec<String>) {
    for stmt in block {
        if let Statement::For(init, cond, update, body) = stmt {
            let has_nf = cond.as_ref().is_some_and(mentions_nf)
                || init.as_ref().is_some_and(|s| stmt_mentions_nf(s));
            if has_nf {
                continue;
            }

            let (bounds, over_key) = for_range_bounds(init, cond, vs);

            let mut select_fields: Vec<String> = Vec::new();
            collect_block_output_refs(body, vs, &mut select_fields);
            collect_block_output_refs(block, vs, &mut select_fields);
            if let Some(s) = init {
                collect_stmt_output_refs(s, vs, &mut select_fields);
            }
            if let Some(s) = update {
                collect_stmt_output_refs(s, vs, &mut select_fields);
            }
            dedup_preserve_order(&mut select_fields);
            let select_text = if select_fields.is_empty() {
                String::new()
            } else {
                format_field_list(&select_fields)
            };

            let phrase = match (&over_key, &bounds) {
                (Some(k), _) if !select_text.is_empty() => format!("for all {k}: {select_text}"),
                (Some(k), _) => format!("for all {k}"),
                (_, Some(b)) if !select_text.is_empty() => {
                    format!("range {}: {select_text}", b.replace('–', ".."))
                }
                (_, Some(b)) => format!("range {}", b.replace('–', "..")),
                _ if !select_text.is_empty() => format!("range: {select_text}"),
                _ => continue,
            };
            phrases.push(phrase);
        }
    }
}

fn detect_collect(rule: &Rule) -> bool {
    rule.action.iter().any(|s| match s {
        Statement::Expression(Expr::Assign(target, val)) =>
            matches!(target.as_ref(), Expr::ArrayRef(_, key) if matches!(key.as_ref(), Expr::Var(n) if n == "NR"))
                && matches!(val.as_ref(), Expr::Field(inner) if matches!(inner.as_ref(), Expr::NumberLit(n) if *n == 0.0)),
        _ => false,
    })
}

// ── Output field collection ─────────────────────────────────────

fn collect_rule_output_refs(rule: &Rule, vs: &HashMap<String, Expr>) -> Vec<String> {
    let mut fields: Vec<String> = Vec::new();
    for stmt in &rule.action {
        if let Statement::Print(exprs, _) | Statement::Printf(exprs, _) = stmt {
            let refs = collect_output_refs(
                if matches!(stmt, Statement::Printf(..)) && exprs.len() > 1 {
                    &exprs[1..]
                } else {
                    exprs
                },
                vs,
            );
            for r in refs {
                if !fields.contains(&r) {
                    fields.push(r);
                }
            }
        }
    }
    if fields.len() > 5 {
        return vec![format!("{} columns", fields.len())];
    }
    fields
}

fn collect_output_refs(exprs: &[Expr], vs: &HashMap<String, Expr>) -> Vec<String> {
    let mut out = Vec::new();
    let mut prev_label: Option<String> = None;
    for e in exprs {
        if let Expr::StringLit(s) = e {
            prev_label = Some(simplify_label(s));
            continue;
        }
        let slots = if let Some(lab) = prev_label.take() {
            if !lab.is_empty() {
                vec![lab]
            } else {
                slot_names(e, vs)
            }
        } else {
            slot_names(e, vs)
        };
        if slots.is_empty() {
            prev_label = None;
        } else {
            out.extend(slots);
        }
    }
    dedup_preserve_order(&mut out);
    out
}

fn slot_names(e: &Expr, vs: &HashMap<String, Expr>) -> Vec<String> {
    let e = unwrap_coercion(e);
    match e {
        Expr::FuncCall(name, args) if name == "jpath" && args.len() >= 2 => {
            let path = args.get(1).and_then(|a| {
                if let Expr::StringLit(s) = a {
                    let c = s.trim_start_matches('.').to_string();
                    if c.is_empty() { None } else { Some(c) }
                } else {
                    None
                }
            });
            vec![path.unwrap_or_else(|| name.clone())]
        }
        Expr::FuncCall(name, _) => vec![slot_display_name(name).to_string()],
        Expr::Field(inner) => field_display(inner).map_or_else(Vec::new, |n| vec![n]),
        Expr::Var(name) if !is_builtin_var(name) => {
            if let Some(src) = vs.get(name.as_str()) {
                if expr_has_non_jpath_call(src, vs, 5) {
                    return vec![name.clone()];
                }
                let mut sub = slot_names(src, vs);
                if sub.is_empty() {
                    sub.push(name.clone());
                }
                sub
            } else {
                vec![name.clone()]
            }
        }
        _ => {
            let mut refs = Vec::new();
            collect_expr_refs(e, vs, &mut refs, 5);
            refs
        }
    }
}

fn collect_expr_refs(e: &Expr, vs: &HashMap<String, Expr>, out: &mut Vec<String>, depth: u8) {
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
        Expr::Var(name) if !is_builtin_var(name) => {
            if let Some(src) = vs.get(name.as_str()) {
                if expr_has_non_jpath_call(src, vs, 5) {
                    if !out.contains(name) {
                        out.push(name.clone());
                    }
                    return;
                }
                let before = out.len();
                collect_expr_refs(src, vs, out, depth - 1);
                if out.len() == before {
                    out.push(name.clone());
                }
            } else if !out.contains(name) {
                out.push(name.clone());
            }
        }
        Expr::FuncCall(fn_name, args) if fn_name == "jpath" && args.len() >= 2 => {
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
        Expr::Ternary(_, t, f) => {
            collect_expr_refs(t, vs, out, depth - 1);
            collect_expr_refs(f, vs, out, depth - 1);
        }
        Expr::UnaryMinus(inner) | Expr::LogicalNot(inner) => {
            collect_expr_refs(inner, vs, out, depth - 1);
        }
        _ => {}
    }
}

fn collect_block_output_refs(block: &Block, vs: &HashMap<String, Expr>, out: &mut Vec<String>) {
    for stmt in block {
        collect_stmt_output_refs(stmt, vs, out);
    }
}

fn collect_stmt_output_refs(stmt: &Statement, vs: &HashMap<String, Expr>, out: &mut Vec<String>) {
    match stmt {
        Statement::Print(exprs, _) => {
            let refs = collect_output_refs(exprs, vs);
            for r in refs {
                if !out.contains(&r) {
                    out.push(r);
                }
            }
        }
        Statement::Printf(exprs, _) => {
            let refs = collect_output_refs(if exprs.len() > 1 { &exprs[1..] } else { exprs }, vs);
            for r in refs {
                if !out.contains(&r) {
                    out.push(r);
                }
            }
        }
        Statement::If(_, then_b, else_b) => {
            collect_block_output_refs(then_b, vs, out);
            if let Some(eb) = else_b {
                collect_block_output_refs(eb, vs, out);
            }
        }
        Statement::For(_, _, _, body)
        | Statement::ForIn(_, _, body)
        | Statement::While(_, body)
        | Statement::DoWhile(body, _) => {
            collect_block_output_refs(body, vs, out);
        }
        Statement::Block(b) => collect_block_output_refs(b, vs, out),
        _ => {}
    }
}

// ── Accumulation scanning ───────────────────────────────────────

fn scan_block_accum(
    block: &Block,
    vs: &HashMap<String, Expr>,
    accum: &mut Option<(String, String)>,
    freq: &mut Option<String>,
    simple: &mut Option<String>,
) {
    for stmt in block {
        match stmt {
            Statement::Expression(e) => scan_accum(e, vs, accum, freq, simple),
            Statement::For(_, _, _, body)
            | Statement::ForIn(_, _, body)
            | Statement::While(_, body)
            | Statement::DoWhile(body, _) => scan_block_accum(body, vs, accum, freq, simple),
            Statement::If(_, t, e) => {
                scan_block_accum(t, vs, accum, freq, simple);
                if let Some(b) = e {
                    scan_block_accum(b, vs, accum, freq, simple);
                }
            }
            Statement::Block(b) => scan_block_accum(b, vs, accum, freq, simple),
            _ => {}
        }
    }
}

fn scan_accum(
    e: &Expr,
    vs: &HashMap<String, Expr>,
    accum: &mut Option<(String, String)>,
    freq: &mut Option<String>,
    simple: &mut Option<String>,
) {
    match e {
        Expr::CompoundAssign(target, BinOp::Add, val)
            if matches!(target.as_ref(), Expr::ArrayRef(_, _)) =>
        {
            if let Expr::ArrayRef(_, key) = target.as_ref() {
                let k = field_ref(key, vs);
                let v = field_ref_deep(val, vs);
                if accum.is_none() {
                    *accum = Some((k, v));
                }
            }
        }
        Expr::Increment(inner, _) if matches!(inner.as_ref(), Expr::ArrayRef(_, _)) => {
            if let Expr::ArrayRef(_, key) = inner.as_ref() {
                let k = field_ref(key, vs);
                if freq.is_none() {
                    *freq = Some(k);
                }
            }
        }
        Expr::CompoundAssign(target, BinOp::Add, val) => {
            if let Expr::Var(_) = target.as_ref() {
                let src = field_ref_deep(val, vs);
                if is_data_ref(&src) && simple.is_none() {
                    *simple = Some(src);
                }
            }
        }
        Expr::Assign(target, val) if is_additive_self(target, val) => {
            if let Expr::Var(_) = target.as_ref()
                && let Some(delta) = additive_delta(target, val)
            {
                let src = field_ref_deep(delta, vs);
                if is_data_ref(&src) && simple.is_none() {
                    *simple = Some(src);
                }
            }
        }
        _ => {}
    }
}

// ── Helpers ─────────────────────────────────────────────────────

fn is_nr_eq_fnr(pat: &Pattern) -> bool {
    if let Pattern::Expression(Expr::BinOp(l, BinOp::Eq, r)) = pat {
        (matches!(l.as_ref(), Expr::Var(n) if n == "NR")
            && matches!(r.as_ref(), Expr::Var(n) if n == "FNR"))
            || (matches!(l.as_ref(), Expr::Var(n) if n == "FNR")
                && matches!(r.as_ref(), Expr::Var(n) if n == "NR"))
    } else {
        false
    }
}

fn block_has_next(block: &Block) -> bool {
    block
        .iter()
        .any(|s| matches!(s, Statement::Next | Statement::Nextfile))
}

fn expr_mentions_in(e: &Expr) -> bool {
    match e {
        Expr::ArrayIn(_, _) => true,
        Expr::LogicalNot(inner) => expr_mentions_in(inner),
        _ => false,
    }
}

fn expr_has_array_accum(e: &Expr) -> bool {
    match e {
        Expr::CompoundAssign(target, BinOp::Add, _)
            if matches!(target.as_ref(), Expr::ArrayRef(_, _)) =>
        {
            true
        }
        Expr::Increment(inner, _) if matches!(inner.as_ref(), Expr::ArrayRef(_, _)) => true,
        Expr::Assign(target, _) if matches!(target.as_ref(), Expr::ArrayRef(_, _)) => true,
        _ => false,
    }
}

fn block_calls_fn(block: &Block, fname: &str) -> bool {
    block.iter().any(|s| stmt_calls_fn(s, fname))
}

fn stmt_calls_fn(stmt: &Statement, fname: &str) -> bool {
    match stmt {
        Statement::Expression(e) => expr_calls_fn(e, fname),
        Statement::Print(exprs, _) | Statement::Printf(exprs, _) => {
            exprs.iter().any(|e| expr_calls_fn(e, fname))
        }
        Statement::If(c, t, e) => {
            expr_calls_fn(c, fname)
                || t.iter().any(|s| stmt_calls_fn(s, fname))
                || e.as_ref()
                    .is_some_and(|b| b.iter().any(|s| stmt_calls_fn(s, fname)))
        }
        Statement::For(_, _, _, body)
        | Statement::ForIn(_, _, body)
        | Statement::While(_, body)
        | Statement::DoWhile(body, _) => body.iter().any(|s| stmt_calls_fn(s, fname)),
        Statement::Block(b) => b.iter().any(|s| stmt_calls_fn(s, fname)),
        _ => false,
    }
}

fn expr_calls_fn(e: &Expr, fname: &str) -> bool {
    match e {
        Expr::FuncCall(name, args) => name == fname || args.iter().any(|a| expr_calls_fn(a, fname)),
        Expr::Assign(l, r)
        | Expr::CompoundAssign(l, _, r)
        | Expr::BinOp(l, _, r)
        | Expr::Concat(l, r)
        | Expr::LogicalAnd(l, r)
        | Expr::LogicalOr(l, r) => expr_calls_fn(l, fname) || expr_calls_fn(r, fname),
        Expr::UnaryMinus(e) | Expr::LogicalNot(e) | Expr::Field(e) => expr_calls_fn(e, fname),
        Expr::Ternary(c, t, f) => {
            expr_calls_fn(c, fname) || expr_calls_fn(t, fname) || expr_calls_fn(f, fname)
        }
        Expr::Sprintf(args) => args.iter().any(|a| expr_calls_fn(a, fname)),
        Expr::Increment(e, _) | Expr::Decrement(e, _) => expr_calls_fn(e, fname),
        _ => false,
    }
}

fn collect_jpath_paths(block: &Block) -> Vec<String> {
    let mut paths = Vec::new();
    for stmt in block {
        collect_jpath_paths_stmt(stmt, &mut paths);
    }
    paths
}

fn collect_jpath_paths_stmt(stmt: &Statement, paths: &mut Vec<String>) {
    match stmt {
        Statement::Expression(e) => collect_jpath_paths_expr(e, paths),
        Statement::Print(exprs, _) | Statement::Printf(exprs, _) => {
            for e in exprs {
                collect_jpath_paths_expr(e, paths);
            }
        }
        Statement::If(_, t, e) => {
            for s in t {
                collect_jpath_paths_stmt(s, paths);
            }
            if let Some(b) = e {
                for s in b {
                    collect_jpath_paths_stmt(s, paths);
                }
            }
        }
        _ => {}
    }
}

fn collect_jpath_paths_expr(e: &Expr, paths: &mut Vec<String>) {
    match e {
        Expr::FuncCall(name, args) if name == "jpath" && args.len() >= 2 => {
            if let Expr::StringLit(path) = &args[1] {
                let clean = path.trim_start_matches('.').to_string();
                if !clean.is_empty() && !paths.contains(&clean) {
                    paths.push(clean);
                }
            }
            for a in args {
                collect_jpath_paths_expr(a, paths);
            }
        }
        Expr::Assign(_, r) => collect_jpath_paths_expr(r, paths),
        Expr::FuncCall(_, args) | Expr::Sprintf(args) => {
            for a in args {
                collect_jpath_paths_expr(a, paths);
            }
        }
        Expr::BinOp(l, _, r) | Expr::Concat(l, r) => {
            collect_jpath_paths_expr(l, paths);
            collect_jpath_paths_expr(r, paths);
        }
        _ => {}
    }
}

fn collect_fn_names_block(block: &Block) -> Vec<String> {
    let mut fns = Vec::new();
    for stmt in block {
        collect_fn_names_stmt(stmt, &mut fns);
    }
    fns
}

fn collect_fn_names_stmt(stmt: &Statement, fns: &mut Vec<String>) {
    match stmt {
        Statement::Expression(e) => collect_fn_names_expr(e, fns),
        Statement::Print(exprs, _) | Statement::Printf(exprs, _) => {
            for e in exprs {
                collect_fn_names_expr(e, fns);
            }
        }
        Statement::If(c, t, e) => {
            collect_fn_names_expr(c, fns);
            for s in t {
                collect_fn_names_stmt(s, fns);
            }
            if let Some(b) = e {
                for s in b {
                    collect_fn_names_stmt(s, fns);
                }
            }
        }
        Statement::For(_, _, _, body) | Statement::ForIn(_, _, body) => {
            for s in body {
                collect_fn_names_stmt(s, fns);
            }
        }
        _ => {}
    }
}

fn collect_fn_names_expr(e: &Expr, fns: &mut Vec<String>) {
    match e {
        Expr::FuncCall(name, args) => {
            if !fns.contains(name) {
                fns.push(name.clone());
            }
            for a in args {
                collect_fn_names_expr(a, fns);
            }
        }
        Expr::BinOp(l, _, r)
        | Expr::Concat(l, r)
        | Expr::LogicalAnd(l, r)
        | Expr::LogicalOr(l, r) => {
            collect_fn_names_expr(l, fns);
            collect_fn_names_expr(r, fns);
        }
        Expr::Assign(_, r) | Expr::CompoundAssign(_, _, r) => collect_fn_names_expr(r, fns),
        Expr::UnaryMinus(e) | Expr::LogicalNot(e) | Expr::Field(e) => collect_fn_names_expr(e, fns),
        Expr::Ternary(c, t, f) => {
            collect_fn_names_expr(c, fns);
            collect_fn_names_expr(t, fns);
            collect_fn_names_expr(f, fns);
        }
        Expr::Sprintf(args) => {
            for a in args {
                collect_fn_names_expr(a, fns);
            }
        }
        _ => {}
    }
}

fn jpath_source_label(expr: &Expr) -> Option<String> {
    match expr {
        Expr::FuncCall(name, args) if name == "jpath" && args.len() >= 2 => {
            if let Expr::StringLit(p) = &args[1] {
                let clean = p.trim_start_matches('.').to_string();
                if !clean.is_empty() {
                    return Some(clean);
                }
            }
            None
        }
        Expr::Assign(_, val) => jpath_source_label(val),
        Expr::BinOp(l, BinOp::Add, r) => {
            if matches!(r.as_ref(), Expr::NumberLit(n) if *n == 0.0) {
                return jpath_source_label(l);
            }
            if matches!(l.as_ref(), Expr::NumberLit(n) if *n == 0.0) {
                return jpath_source_label(r);
            }
            None
        }
        _ => None,
    }
}

fn find_array_source(program: &Program, vs: &HashMap<String, Expr>) -> Option<String> {
    for rule in &program.rules {
        for stmt in &rule.action {
            if let Statement::Expression(e) = stmt {
                match e {
                    Expr::Assign(target, val)
                        if matches!(target.as_ref(), Expr::ArrayRef(_, _)) =>
                    {
                        let src = field_ref_deep(val, vs);
                        if is_data_ref(&src) {
                            return Some(src);
                        }
                    }
                    Expr::CompoundAssign(target, BinOp::Add, val)
                        if matches!(target.as_ref(), Expr::ArrayRef(_, _)) =>
                    {
                        let src = field_ref_deep(val, vs);
                        if is_data_ref(&src) {
                            return Some(src);
                        }
                    }
                    _ => {}
                }
            }
        }
    }
    None
}

fn for_range_bounds(
    init: &Option<Box<Statement>>,
    cond: &Option<Expr>,
    vs: &HashMap<String, Expr>,
) -> (Option<String>, Option<String>) {
    let init_stmt = match init.as_ref().map(|b| b.as_ref()) {
        Some(Statement::Expression(Expr::Assign(l, r))) => (l, r),
        _ => return (None, None),
    };
    let Expr::Var(index_var) = init_stmt.0.as_ref() else {
        return (None, None);
    };
    let cond = match cond.as_ref() {
        Some(Expr::BinOp(l, op, r)) => (l, op, r),
        _ => return (None, None),
    };
    if let Expr::NumberLit(lo) = init_stmt.1.as_ref()
        && let Some((lo_i, hi_i)) = match (cond.0.as_ref(), cond.1, cond.2.as_ref()) {
            (Expr::Var(n), BinOp::Le | BinOp::Lt, Expr::NumberLit(hi)) if n == index_var => {
                Some((*lo as i64, *hi as i64))
            }
            (Expr::Var(n), BinOp::Ge | BinOp::Gt, Expr::NumberLit(hi)) if n == index_var => {
                Some((*hi as i64, *lo as i64))
            }
            (Expr::NumberLit(hi), BinOp::Ge | BinOp::Gt, Expr::Var(n)) if n == index_var => {
                Some((*hi as i64, *lo as i64))
            }
            (Expr::NumberLit(hi), BinOp::Le | BinOp::Lt, Expr::Var(n)) if n == index_var => {
                Some((*lo as i64, *hi as i64))
            }
            _ => None,
        }
    {
        let bounds = if lo_i <= hi_i {
            format!("{lo_i}–{hi_i}")
        } else {
            format!("{hi_i}–{lo_i}")
        };
        return (Some(bounds), None);
    }
    let bound_var = match (cond.0.as_ref(), cond.1, cond.2.as_ref()) {
        (Expr::Var(a), BinOp::Le | BinOp::Lt, Expr::Var(b)) if a == index_var => b.as_str(),
        (Expr::Var(a), BinOp::Ge | BinOp::Gt, Expr::Var(b)) if b == index_var => a.as_str(),
        _ => return (None, None),
    };
    if let Some(over_key) = jpath_path_key(vs.get(bound_var)) {
        return (None, Some(over_key));
    }
    (None, None)
}

fn jpath_path_key(expr: Option<&Expr>) -> Option<String> {
    let Expr::FuncCall(name, args) = expr? else {
        return None;
    };
    if name != "jpath" || args.len() < 2 {
        return None;
    }
    let Expr::StringLit(s) = &args[1] else {
        return None;
    };
    let trimmed = s.trim_start_matches('.');
    if trimmed.is_empty() {
        return None;
    }
    Some(trimmed.to_string())
}

fn describe_capture_filter_cond(cond: &Expr) -> Option<String> {
    let cond = unwrap_coercion(cond);
    let (arr_name, index, threshold, op) = match cond {
        Expr::BinOp(l, bin_op, r) => {
            let l = unwrap_coercion(l);
            let r = unwrap_coercion(r);
            match (l, r) {
                (Expr::ArrayRef(name, key), Expr::NumberLit(v)) => {
                    let idx = if let Expr::NumberLit(n) = key.as_ref() {
                        Some(*n as i64)
                    } else {
                        None
                    }?;
                    (name.clone(), idx, v, bin_op.clone())
                }
                (Expr::NumberLit(v), Expr::ArrayRef(name, key)) => {
                    let idx = if let Expr::NumberLit(n) = key.as_ref() {
                        Some(*n as i64)
                    } else {
                        None
                    }?;
                    let flipped = match bin_op {
                        BinOp::Le => BinOp::Ge,
                        BinOp::Lt => BinOp::Gt,
                        BinOp::Ge => BinOp::Le,
                        BinOp::Gt => BinOp::Lt,
                        o => o.clone(),
                    };
                    (name.clone(), idx, v, flipped)
                }
                _ => return None,
            }
        }
        _ => return None,
    };
    if arr_name != "c" {
        return None;
    }
    let val = *threshold as i64;
    let op_str = match op {
        BinOp::Ge => "≥",
        BinOp::Gt => ">",
        BinOp::Le => "≤",
        BinOp::Lt => "<",
        _ => return None,
    };
    Some(format!("where c[{index}] {op_str} {val}"))
}

const BUILTIN_VARS: &[&str] = &[
    "NR", "NF", "FNR", "FILENAME", "FS", "RS", "OFS", "ORS", "OFMT", "SUBSEP", "ARGC", "ARGV",
    "ENVIRON", "CONVFMT",
];

fn is_builtin_var(name: &str) -> bool {
    BUILTIN_VARS.contains(&name)
}

fn field_display(inner: &Expr) -> Option<String> {
    match inner {
        Expr::NumberLit(n) if *n == 0.0 => None,
        Expr::NumberLit(n) => Some(format!("{}", *n as i64)),
        Expr::StringLit(s) => Some(s.clone()),
        Expr::Var(name) if is_builtin_var(name) => None,
        Expr::Var(name) => Some(name.clone()),
        _ => None,
    }
}

fn field_ref(expr: &Expr, vs: &HashMap<String, Expr>) -> String {
    match expr {
        Expr::Field(inner) if matches!(inner.as_ref(), Expr::NumberLit(n) if *n == 0.0) => {
            "$0".into()
        }
        Expr::Field(inner) => {
            let name = field_display(inner).unwrap_or_else(|| "?".into());
            format!("${name}")
        }
        Expr::Var(name) if is_builtin_var(name) => name.clone(),
        Expr::Var(name) => {
            if let Some(src) = vs.get(name.as_str()) {
                field_ref(src, vs)
            } else {
                format!("${name}")
            }
        }
        Expr::Concat(l, r) => {
            let a = field_ref(l, vs);
            let b = field_ref(r, vs);
            format!("{a} {b}")
        }
        Expr::NumberLit(n) if *n == 0.0 => "$0".into(),
        _ => expr_literal_text(expr),
    }
}

fn field_ref_deep(expr: &Expr, vs: &HashMap<String, Expr>) -> String {
    let expr = unwrap_coercion(expr);
    match expr {
        Expr::Field(inner) if matches!(inner.as_ref(), Expr::NumberLit(n) if *n == 0.0) => {
            "$0".into()
        }
        Expr::Field(inner) => {
            let name = field_display(inner).unwrap_or_else(|| "?".into());
            format!("${name}")
        }
        Expr::Var(name) if is_builtin_var(name) => name.clone(),
        Expr::Var(name) => {
            if let Some(src) = vs.get(name.as_str()) {
                field_ref_deep(src, vs)
            } else {
                format!("${name}")
            }
        }
        _ => expr_literal_text(expr),
    }
}

fn is_data_ref(s: &str) -> bool {
    s.starts_with('$') || matches!(s, "NF" | "NR" | "FNR" | "FILENAME")
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

fn expr_has_non_jpath_call(e: &Expr, vs: &HashMap<String, Expr>, depth: u8) -> bool {
    if depth == 0 {
        return false;
    }
    let e = unwrap_coercion(e);
    match e {
        Expr::FuncCall(name, args) => {
            if name != "jpath" {
                return true;
            }
            args.iter()
                .any(|a| expr_has_non_jpath_call(a, vs, depth - 1))
        }
        Expr::Var(name) => vs
            .get(name.as_str())
            .is_some_and(|src| expr_has_non_jpath_call(src, vs, depth - 1)),
        Expr::BinOp(l, _, r)
        | Expr::Concat(l, r)
        | Expr::LogicalAnd(l, r)
        | Expr::LogicalOr(l, r) => {
            expr_has_non_jpath_call(l, vs, depth - 1) || expr_has_non_jpath_call(r, vs, depth - 1)
        }
        Expr::Ternary(_, t, f) => {
            expr_has_non_jpath_call(t, vs, depth - 1) || expr_has_non_jpath_call(f, vs, depth - 1)
        }
        Expr::UnaryMinus(inner) | Expr::LogicalNot(inner) | Expr::Field(inner) => {
            expr_has_non_jpath_call(inner, vs, depth - 1)
        }
        Expr::Assign(l, r) | Expr::CompoundAssign(l, _, r) => {
            expr_has_non_jpath_call(l, vs, depth - 1) || expr_has_non_jpath_call(r, vs, depth - 1)
        }
        _ => false,
    }
}

fn expr_mentions_counter(expr: &Expr) -> bool {
    match expr {
        Expr::Var(name) => matches!(name.as_str(), "NR" | "FNR"),
        Expr::BinOp(l, _, r) | Expr::Concat(l, r) => {
            expr_mentions_counter(l) || expr_mentions_counter(r)
        }
        Expr::FuncCall(_, args) | Expr::Sprintf(args) => args.iter().any(expr_mentions_counter),
        Expr::Ternary(c, t, f) => {
            expr_mentions_counter(c) || expr_mentions_counter(t) || expr_mentions_counter(f)
        }
        Expr::Field(e) | Expr::UnaryMinus(e) | Expr::LogicalNot(e) => expr_mentions_counter(e),
        _ => false,
    }
}

fn mentions_nf(expr: &Expr) -> bool {
    match expr {
        Expr::Var(name) => name == "NF",
        Expr::BinOp(l, _, r)
        | Expr::Concat(l, r)
        | Expr::LogicalAnd(l, r)
        | Expr::LogicalOr(l, r) => mentions_nf(l) || mentions_nf(r),
        Expr::Field(e) | Expr::UnaryMinus(e) | Expr::LogicalNot(e) => mentions_nf(e),
        Expr::Assign(l, r) | Expr::CompoundAssign(l, _, r) => mentions_nf(l) || mentions_nf(r),
        _ => false,
    }
}

fn stmt_mentions_nf(stmt: &Statement) -> bool {
    matches!(stmt, Statement::Expression(e) if mentions_nf(e))
}

// ── Text formatting ─────────────────────────────────────────────

fn describe_pattern(pat: &Pattern) -> String {
    match pat {
        Pattern::Regex(s) => format!("/{s}/"),
        Pattern::Expression(e) => expr_pattern_text(e),
        Pattern::Range(a, b) => format!("{},{}", describe_pattern(a), describe_pattern(b)),
    }
}

fn expr_pattern_text(expr: &Expr) -> String {
    match expr {
        Expr::BinOp(l, op, r) => {
            let op_str = match op {
                BinOp::Gt => ">",
                BinOp::Lt => "<",
                BinOp::Ge => ">=",
                BinOp::Le => "<=",
                BinOp::Eq => "==",
                BinOp::Ne => "!=",
                BinOp::Add => "+",
                BinOp::Sub => "-",
                BinOp::Mul => "*",
                BinOp::Div => "/",
                BinOp::Mod => "%",
                BinOp::Pow => "**",
            };
            format!("{} {op_str} {}", expr_pattern_text(l), expr_pattern_text(r))
        }
        Expr::Field(inner) => match inner.as_ref() {
            Expr::NumberLit(n) => format!("${}", *n as i64),
            Expr::StringLit(s) => format!("$\"{s}\""),
            _ => format!("$({})", expr_pattern_text(inner)),
        },
        Expr::NumberLit(n) => {
            if *n == (*n as i64) as f64 {
                format!("{}", *n as i64)
            } else {
                format!("{n}")
            }
        }
        Expr::StringLit(s) => format!("\"{s}\""),
        Expr::Var(name) => name.clone(),
        Expr::Match(_, r) | Expr::NotMatch(_, r) => {
            if let Expr::StringLit(s) = r.as_ref() {
                format!("/{s}/")
            } else {
                expr_literal_text(expr)
            }
        }
        Expr::LogicalNot(e) => format!("!{}", expr_pattern_text(e)),
        Expr::ArrayIn(key, arr) => format!("{} in {arr}", expr_pattern_text(key)),
        _ => expr_literal_text(expr),
    }
}

fn expr_literal_text(expr: &Expr) -> String {
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
            _ => format!("$({})", expr_literal_text(inner)),
        },
        Expr::FuncCall(name, args) => {
            let a: Vec<String> = args.iter().map(expr_literal_text).collect();
            format!("{name}({})", a.join(", "))
        }
        _ => "?".into(),
    }
}

fn fmt_regex_or_expr(e: &Expr) -> String {
    match e {
        Expr::StringLit(s) => format!("/{s}/"),
        Expr::Match(_, r) | Expr::NotMatch(_, r) => {
            if let Expr::StringLit(s) = r.as_ref() {
                format!("/{s}/")
            } else {
                expr_literal_text(e)
            }
        }
        _ => expr_literal_text(e),
    }
}

fn simplify_label(s: &str) -> String {
    s.trim_end_matches(':')
        .split_whitespace()
        .next()
        .unwrap_or("")
        .to_string()
}

fn slot_display_name(name: &str) -> &str {
    match name {
        "ord" => "code point",
        "chr" => "character",
        "hex" => "hex value",
        _ => name,
    }
}

fn humanize(s: &str) -> String {
    let s = s.replace(" + 0", "").replace("0 + ", "");
    let s = s.replace(" $SUBSEP ", ", ").replace(" SUBSEP ", ", ");
    let mut out = String::with_capacity(s.len());
    let b = s.as_bytes();
    let mut i = 0;
    while i < b.len() {
        if b[i] == b'$' {
            let start = i + 1;
            if start < b.len() && b[start] == b'"' {
                let name_start = start + 1;
                if let Some(end) = s[name_start..].find('"') {
                    out.push_str(&s[name_start..name_start + end]);
                    i = name_start + end + 1;
                    continue;
                }
            }
            let mut end = start;
            while end < b.len() && b[end].is_ascii_digit() {
                end += 1;
            }
            if end > start {
                let n: usize = s[start..end].parse().unwrap_or(0);
                if n > 0 {
                    out.push_str("col ");
                    out.push_str(&s[start..end]);
                    i = end;
                    continue;
                }
                out.push('$');
                out.push_str(&s[start..end]);
                i = end;
                continue;
            }
            let mut end = start;
            while end < b.len()
                && (b[end].is_ascii_alphanumeric() || b[end] == b'_' || b[end] == b'-')
            {
                end += 1;
            }
            if end > start {
                out.push_str(&s[start..end]);
                i = end;
                continue;
            }
        }
        out.push(b[i] as char);
        i += 1;
    }
    out
}

fn to_title_columns(s: &str) -> String {
    let s = s.replace("col ", "column ");
    if s.contains("column ") && s.contains('–') {
        s.replacen("column ", "columns ", 1)
    } else {
        s
    }
}

fn looks_like_json_paths(fields: &[String]) -> bool {
    if fields.is_empty() {
        return false;
    }
    fields.iter().any(|f| f.contains('.')) && fields.iter().all(|f| f.parse::<usize>().is_err())
}

fn format_field_list(fields: &[String]) -> String {
    let nums: Option<Vec<usize>> = fields.iter().map(|f| f.parse::<usize>().ok()).collect();
    if let Some(ref idx) = nums {
        let consecutive = idx.len() > 1 && idx.windows(2).all(|w| w[1] == w[0] + 1);
        if idx.len() == 1 {
            format!("column {}", idx[0])
        } else if consecutive {
            format!("columns {}–{}", idx[0], idx.last().unwrap())
        } else {
            format!(
                "columns {}",
                idx.iter()
                    .map(|n| n.to_string())
                    .collect::<Vec<_>>()
                    .join(", ")
            )
        }
    } else {
        fields
            .iter()
            .map(|f| slot_display_name(f).to_string())
            .collect::<Vec<_>>()
            .join(", ")
    }
}

fn format_jpath_paths(paths: &[String]) -> String {
    if paths.is_empty() {
        return String::new();
    }
    let formatted: Vec<String> = paths
        .iter()
        .map(|p| {
            if p.starts_with('.') {
                p.clone()
            } else {
                format!(".{p}")
            }
        })
        .collect();
    format!("({})", formatted.join(", "))
}

fn dedup_preserve_order(v: &mut Vec<String>) {
    let mut seen = std::collections::HashSet::new();
    v.retain(|r| seen.insert(r.clone()));
}

fn budget_join(phrases: &[String], budget: usize) -> String {
    if phrases.is_empty() {
        return String::new();
    }
    for take in (1..=phrases.len()).rev() {
        let mut text = phrases[..take].join(", ");
        if phrases.len() - take > 0 {
            text.push_str(", …");
        }
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

// ── Format detection helpers ────────────────────────────────────

fn detect_format_from_ext(path: &str) -> Option<String> {
    let base = path
        .trim_end_matches(".gz")
        .trim_end_matches(".zst")
        .trim_end_matches(".zstd")
        .trim_end_matches(".bz2")
        .trim_end_matches(".xz")
        .trim_end_matches(".lz4");
    if base.ends_with(".csv") {
        Some("CSV".into())
    } else if base.ends_with(".tsv") || base.ends_with(".tab") {
        Some("TSV".into())
    } else if base.ends_with(".json") || base.ends_with(".jsonl") || base.ends_with(".ndjson") {
        Some("JSON".into())
    } else if base.ends_with(".parquet") {
        Some("Parquet".into())
    } else {
        None
    }
}

fn detect_compression(path: &str) -> Option<String> {
    if path.ends_with(".gz") {
        Some("gzip".into())
    } else if path.ends_with(".zst") || path.ends_with(".zstd") {
        Some("zstd".into())
    } else if path.ends_with(".bz2") {
        Some("bzip2".into())
    } else if path.ends_with(".xz") {
        Some("xz".into())
    } else if path.ends_with(".lz4") {
        Some("lz4".into())
    } else {
        None
    }
}

// ── Tests ───────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::{ExplainContext, explain};
    use crate::lexer::Lexer;
    use crate::parser::Parser;

    fn ex(src: &str) -> String {
        let tokens = Lexer::new(src).tokenize().unwrap();
        let program = Parser::new(tokens).parse().unwrap();
        explain(&program, None)
    }

    fn ex_ctx(src: &str, ctx: &ExplainContext) -> String {
        let tokens = Lexer::new(src).tokenize().unwrap();
        let program = Parser::new(tokens).parse().unwrap();
        explain(&program, Some(ctx))
    }

    // ── Basic selection ─────────────────────────────────────────

    #[test]
    fn select_fields() {
        assert_eq!(ex("{ print $1, $2 }"), "select columns 1–2");
    }

    #[test]
    fn select_named_columns() {
        assert_eq!(
            ex("{ print $\"host-name\", $\"cpu-usage\" }"),
            "select host-name, cpu-usage"
        );
    }

    #[test]
    fn select_many_fields_summarized() {
        assert_eq!(ex("{ print $1, $2, $3, $4, $5, $6 }"), "select 6 columns");
    }

    #[test]
    fn select_printf() {
        assert_eq!(ex("{ printf \"%s %s\\n\", $1, $2 }"), "select columns 1–2");
    }

    #[test]
    fn passthrough() {
        assert_eq!(ex("{ print }"), "pass through");
        assert_eq!(ex("{ print $0 }"), "pass through");
    }

    // ── Filters ─────────────────────────────────────────────────

    #[test]
    fn filter_pattern() {
        assert_eq!(
            ex("/Math/ { print $1, $2 }"),
            "where /Math/: select columns 1–2"
        );
    }

    #[test]
    fn filter_comparison() {
        assert_eq!(
            ex("$2 > 90 { print $1 }"),
            "where column 2 > 90: select column 1"
        );
    }

    // ── Accumulation ────────────────────────────────────────────

    #[test]
    fn sum() {
        assert_eq!(ex("{ sum += $2 } END { print sum }"), "sum of column 2");
    }

    #[test]
    fn sum_noncompound() {
        assert_eq!(ex("{ total = total + NF }; END {print total}"), "sum of NF");
    }

    #[test]
    fn frequency() {
        assert_eq!(
            ex("{ a[$1]++ } END { for (k in a) print k }"),
            "frequency of column 1"
        );
    }

    #[test]
    fn aggregate_by() {
        assert_eq!(
            ex("{ s[$1]+=$2; c[$1]++ } END { for(k in s) print k, s[k]/c[k] }"),
            "aggregation of column 2 by column 1",
        );
    }

    #[test]
    fn sum_by_group() {
        assert_eq!(
            ex(
                "{ rev[$region] += $revenue } END { for (r in rev) printf \"%s: %.2f\\n\", r, rev[r] }"
            ),
            "sum of revenue by region",
        );
    }

    // ── Chart / stats ───────────────────────────────────────────

    #[test]
    fn histogram() {
        assert_eq!(
            ex("{ a[NR]=$1 } END { print plotbox(hist(a)) }"),
            "histogram of column 1"
        );
    }

    #[test]
    fn stats() {
        assert_eq!(
            ex("{ a[NR]=$2 } END { print mean(a), median(a) }"),
            "statistics of column 2"
        );
    }

    #[test]
    fn chart_subsumes_stats() {
        assert_eq!(
            ex("{ a[NR]=$1 } END { print plotbox(hist(a)), mean(a) }"),
            "histogram of column 1"
        );
    }

    #[test]
    fn stats_subsumes_sum_by() {
        assert_eq!(
            ex("{ rev[$1] += $2 } END { printf \"%.2f\\n\", mean(rev) }"),
            "statistics of column 2",
        );
    }

    #[test]
    fn compound_assign_tracked() {
        assert_eq!(
            ex("{ rev[$1] += $2 } END { print mean(rev) }"),
            "statistics of column 2"
        );
    }

    // ── Count ───────────────────────────────────────────────────

    #[test]
    fn count() {
        assert_eq!(ex("END { print NR }"), "count lines");
    }

    #[test]
    fn count_pattern() {
        assert_eq!(ex("/Beth/{n++}; END {print n+0}"), "count where /Beth/");
    }

    #[test]
    fn count_not_triggered_with_array_accumulation() {
        let out = ex(r#"
            { count++; by_cust[$1]+=$2; n_cust[$1]++ }
            END { printf "total %d\n", count; for (k in by_cust) print k, by_cust[k], n_cust[k] }
        "#);
        assert!(
            !out.eq("count lines"),
            "must not collapse to count lines when array accumulation present; got {:?}",
            out,
        );
        assert!(
            out.contains("aggregation")
                || out.contains("frequency")
                || out.contains("sum")
                || out.contains("statistics"),
            "expected aggregation/frequency/sum/statistics in {:?}",
            out,
        );
    }

    // ── Idioms ──────────────────────────────────────────────────

    #[test]
    fn dedup() {
        assert_eq!(ex("!seen[$0]++"), "deduplicate by line");
    }

    #[test]
    fn dedup_multikey() {
        assert_eq!(ex("!seen[$1,$2]++"), "deduplicate by column 1, column 2");
    }

    #[test]
    fn join() {
        assert_eq!(
            ex("NR==FNR{price[$1]=$2; next} {print $0, price[$1]+0}"),
            "join on column 1",
        );
    }

    #[test]
    fn anti_join() {
        assert_eq!(
            ex("NR==FNR{skip[$1]=1; next} !($1 in skip)"),
            "rows without a match on column 1"
        );
    }

    #[test]
    fn semi_join() {
        assert_eq!(
            ex("NR==FNR{keep[$1]=1; next} $1 in keep"),
            "matching rows on column 1"
        );
    }

    // ── Transform ───────────────────────────────────────────────

    #[test]
    fn gsub_transform() {
        assert_eq!(
            ex("{ gsub(/foo/, \"bar\"); print }"),
            "replace /foo/ → \"bar\""
        );
    }

    #[test]
    fn redacted_output_not_bare_var_name() {
        let out = ex(r#"
            {
                safe = gensub("token=\\S+", "token=***", "g")
                safe = gensub("[\\w.]+@[\\w.]+", "***@***", "g", safe)
                print "  original:", $0
                print "  redacted:", safe
            }
            "#);
        assert!(out.contains("replace"), "expected replace in {:?}", out);
        assert!(
            out.contains("original"),
            "expected 'original' (from literal) in {:?}",
            out
        );
        assert!(
            out.contains("redacted"),
            "expected 'redacted' (from literal) in {:?}",
            out
        );
    }

    #[test]
    fn transform_suppresses_filter_1() {
        assert_eq!(ex("{sub(/\\r$/,\"\")};1"), "replace /\\r$/ → \"\"");
    }

    #[test]
    fn gensub_in_print() {
        let out = ex(r#"{ print gensub("-", " | ", 2) }"#);
        assert!(out.contains("replace"), "expected replace in {:?}", out);
        assert!(!out.eq("transform"), "must not be generic 'transform'");
        assert!(
            !out.ends_with(", gensub") && !out.ends_with(", replace"),
            "must not repeat transform verb as output; got {:?}",
            out,
        );
    }

    #[test]
    fn idiom_ascii_table() {
        let out = ex(r#"
            BEGIN {
                for (i = 33; i <= 126; i++) {
                    printf "  %3d  %4s  %s", i, hex(i), chr(i)
                    if ((i - 32) % 6 == 0) print ""
                }
                print ""
            }
        "#);
        assert_eq!(out, "range 33..126: i, hex value, character");
    }

    #[test]
    fn idiom_per_char_ord_hex() {
        let out = ex(r#"
            { for (i=1; i<=length($0); i++) { c=substr($0,i,1); printf "  %s → %d → %s\n", c, ord(c), hex(ord(c)) } }
        "#);
        assert_eq!(out, "range: c, code point, hex value");
    }

    #[test]
    fn idiom_generic_var_names() {
        let ascii = ex(r#"
            BEGIN { for (k=33; k<=126; k++) printf "%s %s\n", chr(k), hex(k) }
        "#);
        assert_eq!(ascii, "range 33..126: character, hex value");
        let perchar = ex(r#"
            { for (pos=1; pos<=length($0); pos++) { ch=substr($0,pos,1); printf "%s %d %s\n", ch, ord(ch), hex(ord(ch)) } }
        "#);
        assert_eq!(perchar, "range: ch, code point, hex value");
    }

    // ── Extract ─────────────────────────────────────────────────

    #[test]
    fn regex_extract_format() {
        assert_eq!(
            ex("{ match($0, \"pattern\", c); printf \"%s\\n\", c[1] }"),
            "pattern extract + format",
        );
    }

    #[test]
    fn capture_filter_most_significant() {
        let out = ex(r#"
            { match($0, /^(\S+)\s+(\d+)/, c); if (c[2]+0 >= 500) printf "%s %s\n", c[1], c[2] }
        "#);
        assert!(
            out.starts_with("where c[2] ≥ 500"),
            "filter must be first and use generic label; got {:?}",
            out,
        );
        assert!(
            out.contains("pattern extract"),
            "expected extract in {:?}",
            out
        );
    }

    #[test]
    fn jpath_format() {
        assert_eq!(
            ex("{ m = jpath($0, \".method\"); printf \"%s\\n\", m }"),
            "select method",
        );
    }

    #[test]
    fn json_extract_shows_path() {
        let out = ex(r#"{ ms = jpath($0, ".ms"); lat[NR] = ms } END { print mean(lat) }"#);
        assert!(
            out.contains("ms"),
            "extract phrase should include path; got {:?}",
            out,
        );
    }

    #[test]
    fn jpath_loop_for_all_members() {
        let out = ex(r#"
            BEGIN {
                team = jpath($0, ".team")
                n = jpath($0, ".members", m)
                for (i=1; i<=n; i++) printf "  %s: %s (%s)\n", team, jpath(m[i], ".name"), jpath(m[i], ".role")
            }
        "#);
        assert_eq!(out, "for all members: team, name, role");
    }

    #[test]
    fn jpath_loop_for_all_generic() {
        let out = ex(r#"
            BEGIN { k = jpath($0, ".items", arr); for (i=1; i<=k; i++) print jpath(arr[i], ".id") }
        "#);
        assert_eq!(out, "for all items: id");

        let out = ex(r#"
            BEGIN { n = jpath($0, ".data.rows", r); for (j=1; j<=n; j++) print jpath(r[j], ".label") }
        "#);
        assert_eq!(out, "for all data.rows: label");

        let out = ex(r#"
            BEGIN { c = jpath($0, ".users", u); for (i=1; i<=c; i++) printf "%s\n", jpath(u[i], ".login") }
        "#);
        assert_eq!(out, "for all users: login");
    }

    // ── Field operations ────────────────────────────────────────

    #[test]
    fn rewrite_fields() {
        assert_eq!(ex("{ $2 = \"\"; print }"), "rewrite fields");
    }

    #[test]
    fn reformat_output() {
        assert_eq!(ex("BEGIN{ORS=\"\\n\\n\"};1"), "reformat output");
    }

    #[test]
    fn number_lines() {
        assert_eq!(ex("{print FNR \"\\t\" $0}"), "number lines");
        assert_eq!(ex("{printf \"%5d : %s\\n\", NR, $0}"), "number lines");
    }

    #[test]
    fn iterate_fields() {
        assert_eq!(
            ex("{for (i=NF; i>0; i--) printf \"%s \",$i; print \"\"}"),
            "iterate fields",
        );
    }

    #[test]
    fn collect_emit() {
        assert_eq!(
            ex("{ a[NR]=$0 } END { for(i=NR;i>=1;i--) print a[i] }"),
            "collect lines",
        );
    }

    // ── Lineage / variable resolution ───────────────────────────

    #[test]
    fn lineage_through_vars() {
        assert_eq!(
            ex("{ x = $3 * 2; y = $4 + 1; printf \"%s %d %d\\n\", $1, x, y }"),
            "select columns 1, 3, 4",
        );
    }

    #[test]
    fn lineage_coercion() {
        assert_eq!(ex("{ x = $3 + 0; printf \"%d\\n\", x }"), "select column 3");
    }

    #[test]
    fn lineage_named_columns() {
        assert_eq!(
            ex(
                "{ cpu = $\"cpu-usage\" + 0; mem = $\"mem-usage\" + 0; printf \"%s %f %f\\n\", $\"host-name\", cpu, mem }"
            ),
            "select host-name, cpu-usage, mem-usage",
        );
    }

    #[test]
    fn lineage_named_columns_with_computed_var() {
        assert_eq!(
            ex(r#"
            {
                cpu = $"cpu-usage" + 0; mem = $"mem-usage" + 0
                status = "OK"
                if (cpu > 90 || mem > 90) status = "CRITICAL"
                else if (cpu > 70 || mem > 70) status = "WARNING"
                printf "%-12s cpu=%5.1f%% mem=%5.1f%% [%s]\n", $"host-name", cpu, mem, status
            }
            "#),
            "select host-name, cpu-usage, mem-usage, status",
        );
    }

    #[test]
    fn concat_fields() {
        assert_eq!(ex("{ print $1 \" \" $2 }"), "select columns 1–2");
        assert_eq!(ex("{ print $1 \":\" $2 \":\" $3 }"), "select columns 1–3");
    }

    #[test]
    fn concat_lineage() {
        assert_eq!(ex("{ s = $1 \" - \" $2; print s }"), "select columns 1–2");
    }

    #[test]
    fn jpath_lineage() {
        assert_eq!(
            ex(
                "{ m = jpath($0, \".method\"); p = jpath($0, \".path\"); printf \"%s %s\\n\", m, p }"
            ),
            "select method, path",
        );
    }

    #[test]
    fn jpath_from_slurped_json() {
        let out = ex(r#"
            BEGIN {
                json = slurp("/tmp/pretty.json")
                printf "service=%s version=%s timeout=%sms retries=%s\n",
                    jpath(json, ".service"),
                    jpath(json, ".version"),
                    jpath(json, ".limits.timeout_ms"),
                    jpath(json, ".limits.retries")
            }
            "#);
        assert!(
            out.contains("from JSON:"),
            "expected 'from JSON:' in {:?}",
            out
        );
        assert!(out.contains("slurped"), "expected 'slurped' in {:?}", out);
    }

    #[test]
    fn mixed_direct_and_computed() {
        assert_eq!(ex("{ print $1, $2, length($3) }"), "select 1, 2, length");
    }

    #[test]
    fn ternary_in_output() {
        assert_eq!(
            ex("{ print ($1 > 50 ? \"high\" : \"low\"), $2 }"),
            "select column 2"
        );
    }

    #[test]
    fn compute_shows_fields() {
        assert_eq!(ex("{ print length($0) }"), "select length");
        assert_eq!(ex("{ print $1 + $2 }"), "select columns 1–2");
    }

    // ── Multi-fragment ──────────────────────────────────────────

    #[test]
    fn multi_fragment() {
        assert_eq!(
            ex("/baz/ { gsub(/foo/, \"bar\"); print }"),
            "where /baz/: replace /foo/ → \"bar\"",
        );
    }

    #[test]
    fn field_iteration_sum() {
        assert_eq!(
            ex("{s=0; for (i=1; i<=NF; i++) s=s+$i; print s}"),
            "sum of i, iterate fields",
        );
    }

    // ── Timing ──────────────────────────────────────────────────

    #[test]
    fn timing() {
        assert_eq!(
            ex("BEGIN { tic(); for(i=0;i<100000;i++) x+=i; printf \"%.4f\\n\",toc() }"),
            "range 0..100000: toc",
        );
    }

    // ── Environment context ─────────────────────────────────────

    #[test]
    fn env_csv_headers() {
        let ctx = ExplainContext::from_cli("csv", true, None, &["sales.csv".into()]);
        assert_eq!(
            ex_ctx("{ sum += $2 } END { print sum }", &ctx),
            "sum of column 2 (CSV, headers, sales.csv)",
        );
    }

    #[test]
    fn env_compressed_json() {
        let ctx = ExplainContext::from_cli("json", false, None, &["api.jsonl.gz".into()]);
        assert_eq!(
            ex_ctx("{ a[NR]=$1 } END { print plotbox(hist(a)) }", &ctx),
            "histogram of column 1 (JSON, gzip, api.jsonl.gz)",
        );
    }

    #[test]
    fn env_field_sep() {
        let ctx = ExplainContext::from_cli("line", false, Some(":"), &[]);
        assert_eq!(ex_ctx("{ print $1 }", &ctx), "select column 1 (-F ':')");
    }

    #[test]
    fn env_multiple_files() {
        let ctx = ExplainContext::from_cli(
            "line",
            false,
            None,
            &["a.txt".into(), "b.txt".into(), "c.txt".into()],
        );
        assert_eq!(ex_ctx("/foo/ { print }", &ctx), "where /foo/ (3 files)");
    }

    #[test]
    fn env_select_no_env() {
        let ctx = ExplainContext::from_cli("line", false, None, &[]);
        assert_eq!(ex_ctx("{ print $1, $2 }", &ctx), "select columns 1–2");
    }

    #[test]
    fn env_passthrough_no_env() {
        let ctx = ExplainContext::from_cli("line", false, None, &[]);
        assert_eq!(ex_ctx("{ print }", &ctx), "pass through");
    }

    #[test]
    fn env_idiom_with_context() {
        let ctx = ExplainContext::from_cli("csv", true, None, &["data.csv".into()]);
        assert_eq!(
            ex_ctx("!seen[$0]++", &ctx),
            "deduplicate by line (CSV, headers, data.csv)",
        );
    }

    #[test]
    fn env_auto_detected_line_mode_no_noise() {
        let ctx = ExplainContext::from_cli("line", false, None, &["data.txt".into()]);
        assert_eq!(
            ex_ctx("{ sum += $1 } END { print sum }", &ctx),
            "sum of column 1 (data.txt)",
        );
    }

    // ── Budget ──────────────────────────────────────────────────

    #[test]
    fn render_budget_truncation() {
        let out = ex(r#"
            { print "histogram of some very long expression name", "where col 7 ~ /^extremely-long-pattern-that-keeps-going$/" }
        "#);
        assert!(out.len() <= 72, "len {} > 72: {out}", out.len());
    }

    #[test]
    fn render_empty() {
        assert_eq!(ex("BEGIN {}"), "");
    }
}
