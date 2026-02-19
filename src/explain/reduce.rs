use crate::analyze::ProgramInfo;

use super::lower::{Desc, Op};

const STAT_BUILTINS: &[&str] = &[
    "mean", "median", "stddev", "variance", "sum", "min", "max",
    "p", "percentile", "quantile", "iqm",
];
const CHART_BUILTINS: &[&str] = &["hist", "plotbox", "plot"];
const TRANSFORM_BUILTINS: &[&str] = &[
    "gsub", "sub", "gensub", "trim", "ltrim", "rtrim",
    "reverse", "toupper", "tolower",
];

/// Apply reduction passes until stable.
///
/// Each pass scans the op lists and replaces low-level patterns with
/// high-level semantic ops.  Passes run in priority order so that
/// the most specific patterns match first.
pub(crate) fn reduce(desc: &mut Desc, info: &ProgramInfo) {
    // Idiom detection (whole-program patterns)
    if try_dedup(desc) { return; }
    if try_join(desc) { return; }
    if try_count_match(desc) { return; }

    // Per-section reductions
    reduce_chart_stats(desc);
    reduce_accumulation(desc);
    reduce_count(desc);
    reduce_collect(desc);
    reduce_transforms(desc);
    reduce_extract(desc);
    reduce_rewrite(desc);
    reduce_select(desc, info);
    reduce_filters(desc);
    reduce_iter_fields(desc);
    reduce_number_lines(desc);
    reduce_reformat(desc);
    reduce_timing(desc);
    reduce_fallback(desc);
}

// ── Whole-program idiom detection ───────────────────────────────

fn try_dedup(desc: &mut Desc) -> bool {
    if desc.rules.len() == 1
        && desc.begin.is_empty()
        && desc.end.is_empty()
        && let Some(Op::PatternDedup(key)) = &desc.rules[0].filter
    {
        let text = humanize(&format!("deduplicate by {key}"));
        desc.rules[0].filter = None;
        desc.rules[0].body = vec![Op::Dedup(text)];
        true
    } else {
        false
    }
}

fn try_join(desc: &mut Desc) -> bool {
    if desc.rules.len() < 2 {
        return false;
    }
    let first = &desc.rules[0];
    if !matches!(&first.filter, Some(Op::PatternNrFnr)) {
        return false;
    }
    if !first.body.iter().any(|op| matches!(op, Op::Next)) {
        return false;
    }

    let key = first.body.iter().find_map(|op| {
        if let Op::ArrayPut { key, .. } | Op::ArrayInc { key, .. } = op {
            Some(humanize(key))
        } else {
            None
        }
    });

    let second = &desc.rules[1];
    let second_pat = second.filter.as_ref().map(|f| match f {
        Op::Filter(t) => t.as_str(),
        _ => "",
    });
    let kind = match second_pat {
        Some(p) if p.contains('!') && p.contains("in") => "anti-join",
        Some(p) if p.contains("in") => "semi-join",
        _ => "join",
    };

    let text = match key {
        Some(k) => format!("{kind} on {k}"),
        None => kind.to_string(),
    };
    desc.begin.clear();
    desc.rules.clear();
    desc.end.clear();
    desc.rules.push(super::lower::RuleDesc {
        filter: None,
        body: vec![Op::Join(kind.into(), text)],
    });
    true
}

fn try_count_match(desc: &mut Desc) -> bool {
    if desc.end.is_empty() {
        return false;
    }
    for rule in &desc.rules {
        let has_inc = rule.body.iter().any(|op| matches!(op, Op::Inc(_)));
        if has_inc {
            let pat = rule.filter.as_ref().and_then(|f| match f {
                Op::Filter(t) => Some(humanize(t)),
                _ => None,
            });
            let text = match &pat {
                Some(p) => format!("count {p}"),
                None => "count lines".into(),
            };
            desc.begin.clear();
            desc.rules.clear();
            desc.end.clear();
            desc.rules.push(super::lower::RuleDesc {
                filter: None,
                body: vec![Op::Count(pat.map(|_| text.clone()).or(Some(text)))],
            });
            return true;
        }
    }
    false
}

// ── Section-level reductions ────────────────────────────────────

fn reduce_chart_stats(desc: &mut Desc) {
    let all_fns = collect_all_fns(desc);
    let has_chart = all_fns.iter().any(|f| CHART_BUILTINS.contains(&f.as_str()));
    let has_stats = all_fns.iter().any(|f| STAT_BUILTINS.contains(&f.as_str()));

    if !has_chart && !has_stats {
        return;
    }

    let source = resolve_source(desc);

    if has_chart {
        let op_name = if all_fns.iter().any(|f| f == "hist") {
            "histogram"
        } else {
            "chart"
        };
        let text = match &source {
            Some(s) => format!("{op_name} of {}", humanize(s)),
            None => op_name.into(),
        };
        desc.end.retain(|op| !matches!(op, Op::Fn(_) | Op::Emit(_) | Op::EmitFmt(_) | Op::EmitCounter));
        desc.end.push(Op::Histogram(text));

        if has_stats {
            let text = match &source {
                Some(s) => format!("stats of {}", humanize(s)),
                None => "stats".into(),
            };
            desc.end.push(Op::Stats(text));
        }

        // Chart subsumes stats
        desc.end.retain(|op| !matches!(op, Op::Stats(_)));
    } else if has_stats {
        let text = match &source {
            Some(s) => format!("stats of {}", humanize(s)),
            None => "stats".into(),
        };
        desc.end.retain(|op| !matches!(op, Op::Fn(_) | Op::Emit(_) | Op::EmitFmt(_) | Op::EmitCounter));
        desc.end.push(Op::Stats(text));
    }

    // If we have chart/stats, array puts in rules are part of the pattern — remove
    for rule in &mut desc.rules {
        rule.body.retain(|op| !matches!(op, Op::ArrayPut { .. }));
    }
}

fn reduce_accumulation(desc: &mut Desc) {
    // Aggregate: arr[key] += val  combined with  arr[key2]++ (frequency)
    let mut accum_arr: Option<(String, String, String)> = None; // arr, key, val
    let mut freq_arr: Option<(String, String)> = None; // arr, key

    for rule in &desc.rules {
        for op in &rule.body {
            match op {
                Op::ArrayAccum { arr, key, val } if accum_arr.is_none() => {
                    accum_arr = Some((arr.clone(), key.clone(), val.clone()));
                }
                Op::ArrayInc { arr, key } if freq_arr.is_none() => {
                    freq_arr = Some((arr.clone(), key.clone()));
                }
                _ => {}
            }
        }
    }

    // If we have both accum and freq on same key → aggregate
    let has_end_iter = desc.end.iter().any(|op| matches!(op, Op::ForIn(_)));
    if let (Some((_, ak, av)), Some((_, fk))) = (&accum_arr, &freq_arr)
        && ak == fk
        && has_end_iter
    {
        let text = format!("agg {} by {}", humanize(av), humanize(ak));
        clear_rules_and_end(desc);
        desc.rules.push(super::lower::RuleDesc {
            filter: None,
            body: vec![Op::Agg(humanize(av), text)],
        });
        return;
    }

    // Frequency: arr[key]++ + for(k in arr) in END
    if let Some((_, key)) = &freq_arr
        && has_end_iter
    {
        let text = format!("freq of {}", humanize(key));
        clear_rules_and_end(desc);
        desc.rules.push(super::lower::RuleDesc {
            filter: None,
            body: vec![Op::Freq(text)],
        });
        return;
    }

    // Accumulation: arr[key] += val → reduce to Agg in END
    if let Some((_, key, val)) = &accum_arr
        && has_end_iter
    {
        let text = format!("sum {} by {}", humanize(val), humanize(key));
        clear_rules_and_end(desc);
        desc.rules.push(super::lower::RuleDesc {
            filter: None,
            body: vec![Op::Agg(humanize(val), text)],
        });
        return;
    }

    // Simple sum: var += field
    let has_end = !desc.end.is_empty();
    for rule in &mut desc.rules {
        let accum_source = rule.body.iter().find_map(|op| {
            if let Op::Accum { source, .. } = op
                && is_data_ref(source)
            {
                Some(source.clone())
            } else {
                None
            }
        });
        if let Some(source) = accum_source {
            let text = format!("sum {}", humanize(&source));
            if has_end {
                clear_rules_and_end(desc);
                desc.rules.push(super::lower::RuleDesc {
                    filter: None,
                    body: vec![Op::Sum(text)],
                });
            } else {
                rule.body.retain(|op| !matches!(op, Op::Accum { .. }));
                rule.body.push(Op::Sum(text));
            }
            return;
        }
    }
}

fn reduce_count(desc: &mut Desc) {
    if desc.end.is_empty() {
        return;
    }
    let has_high = desc
        .end
        .iter()
        .any(|op| matches!(op, Op::Histogram(_) | Op::Stats(_)));
    if has_high {
        return;
    }

    // END-only: EmitCounter or bare Emit → count lines
    if desc.begin.is_empty()
        && desc.rules.is_empty()
        && desc.end.iter().any(|op| {
            matches!(op, Op::EmitCounter | Op::Emit(_) | Op::EmitFmt(_))
        })
    {
        desc.end.clear();
        desc.end.push(Op::Count(Some("count lines".into())));
    }
}

fn reduce_collect(desc: &mut Desc) {
    // arr[NR] = $0 in rules + emit in END → collect + emit
    let has_collect = desc.rules.iter().any(|r| {
        r.body.iter().any(|op| {
            matches!(op, Op::ArrayPut { key, val, .. } if key == "NR" && val == "$0")
        })
    });
    let has_end_emit = desc
        .end
        .iter()
        .any(|op| matches!(op, Op::Emit(_) | Op::EmitFmt(_) | Op::EmitCounter));

    if has_collect && has_end_emit {
        clear_rules_and_end(desc);
        desc.rules.push(super::lower::RuleDesc {
            filter: None,
            body: vec![Op::Collect],
        });
    }
}

fn reduce_transforms(desc: &mut Desc) {
    for rule in &mut desc.rules {
        let mut transform_desc = None;
        for op in &rule.body {
            match op {
                Op::SubGsub { kind, pat, repl } => {
                    transform_desc = Some(format!("{kind} {pat} → {repl}"));
                }
                Op::Fn(name) if TRANSFORM_BUILTINS.contains(&name.as_str()) => {
                    if transform_desc.is_none() {
                        transform_desc = Some("transform".into());
                    }
                }
                _ => {}
            }
        }
        if let Some(desc_text) = transform_desc {
            rule.body
                .retain(|op| !matches!(op, Op::SubGsub { .. }));
            rule.body.push(Op::Transform(desc_text));
        }
    }
}

fn reduce_extract(desc: &mut Desc) {
    for rule in &mut desc.rules {
        let has_match = rule.body.iter().any(|op| matches!(op, Op::MatchCall));
        let has_jpath = rule.body.iter().any(|op| matches!(op, Op::Jpath(_)));
        let has_fmt = rule
            .body
            .iter()
            .any(|op| matches!(op, Op::EmitFmt(_)));

        // If jpath resolved into select fields, don't add separate extract
        let jpath_in_select = rule.body.iter().any(|op| {
            if let Op::Emit(fields) | Op::EmitFmt(fields) = op {
                // Fields from jpath are non-numeric names
                fields.iter().any(|f| f.parse::<usize>().is_err() && !f.is_empty())
            } else {
                false
            }
        });

        let extract = match (has_match, has_jpath && !jpath_in_select, has_fmt) {
            (true, _, true) => Some("regex extract + format"),
            (true, _, false) => Some("regex extract"),
            (false, true, true) => Some("JSON extract + format"),
            (false, true, false) => Some("JSON extract"),
            _ => None,
        };

        if let Some(text) = extract {
            rule.body
                .retain(|op| !matches!(op, Op::MatchCall | Op::Jpath(_)));
            rule.body.push(Op::Extract(text.into()));
        }
    }
}

fn reduce_rewrite(desc: &mut Desc) {
    for rule in &mut desc.rules {
        if rule.body.iter().any(|op| matches!(op, Op::AssignField(_))) {
            rule.body
                .retain(|op| !matches!(op, Op::AssignField(_)));
            rule.body.push(Op::Rewrite);
        }
    }
}

fn reduce_select(desc: &mut Desc, _info: &ProgramInfo) {
    // Convert Emit/EmitFmt with field refs into Select
    for rule in &mut desc.rules {
        let mut all_fields: Vec<String> = Vec::new();
        let mut has_emit = false;

        for op in &rule.body {
            match op {
                Op::Emit(fields) | Op::EmitFmt(fields) => {
                    has_emit = true;
                    for f in fields {
                        if !all_fields.contains(f) {
                            all_fields.push(f.clone());
                        }
                    }
                }
                _ => {}
            }
        }

        if !has_emit {
            continue;
        }

        // Check for NR/FNR in output
        // (These would be in the original exprs but we lose them in field collection.
        //  Detect via the collected refs — counters are filtered out by field_display.)
        // We handle this by checking if the Emit had zero useful fields but
        // the original print had expressions.

        if !all_fields.is_empty() {
            rule.body
                .retain(|op| !matches!(op, Op::Emit(_) | Op::EmitFmt(_)));
            if all_fields.len() > 5 {
                rule.body.push(Op::Select(vec![format!("{} fields", all_fields.len())]));
            } else {
                rule.body.push(Op::Select(all_fields));
            }
        }
    }

    // Also check BEGIN/END for standalone emits
    reduce_emit_to_select(&mut desc.begin);
    reduce_emit_to_select(&mut desc.end);
}

fn reduce_emit_to_select(ops: &mut Vec<Op>) {
    let fields: Vec<String> = ops
        .iter()
        .filter_map(|op| match op {
            Op::Emit(f) | Op::EmitFmt(f) => Some(f.clone()),
            _ => None,
        })
        .flatten()
        .collect();

    if !fields.is_empty() {
        ops.retain(|op| !matches!(op, Op::Emit(_) | Op::EmitFmt(_)));
        if fields.len() > 5 {
            ops.push(Op::Select(vec![format!("{} fields", fields.len())]));
        } else {
            ops.push(Op::Select(fields));
        }
    }
}

fn reduce_filters(desc: &mut Desc) {
    for rule in &mut desc.rules {
        if let Some(Op::Filter(text)) = &rule.filter {
            let h = humanize(text);
            if h != "1" {
                rule.body.insert(0, Op::Where(h));
            }
            rule.filter = None;
        }
    }
}

fn reduce_iter_fields(desc: &mut Desc) {
    for rule in &mut desc.rules {
        if rule.body.iter().any(|op| matches!(op, Op::IterNF)) {
            rule.body.retain(|op| {
                !matches!(op, Op::IterNF | Op::Select(_) | Op::Emit(_) | Op::EmitFmt(_))
            });
            rule.body.push(Op::Transform("iterate fields".into()));
        }
    }
}

fn reduce_number_lines(desc: &mut Desc) {
    // EmitCounter in END → count lines (handled by reduce_count)
    // EmitCounter in rules → number lines
    for rule in &mut desc.rules {
        if rule.body.iter().any(|op| matches!(op, Op::EmitCounter)) {
            rule.body.retain(|op| !matches!(op, Op::EmitCounter));
            rule.body.push(Op::NumberLines);
        }
    }
}

fn reduce_reformat(desc: &mut Desc) {
    for rule in &mut desc.rules {
        if rule
            .body
            .iter()
            .any(|op| matches!(op, Op::Reformat(_)))
        {
            rule.body.retain(|op| !matches!(op, Op::Reformat(_)));
            // Only push if there's nothing else
            if rule.body.is_empty() {
                rule.body.push(Op::Reformat("reformat output".into()));
            }
        }
    }
    // Also in begin
    if desc
        .begin
        .iter()
        .any(|op| matches!(op, Op::Reformat(_)))
    {
        desc.begin.retain(|op| !matches!(op, Op::Reformat(_)));
        desc.begin.push(Op::Reformat("reformat output".into()));
    }
}

fn reduce_timing(desc: &mut Desc) {
    if desc.flags.has_timing {
        // Add timed annotation to wherever it makes sense
        let target = if !desc.begin.is_empty() {
            &mut desc.begin
        } else if !desc.end.is_empty() {
            &mut desc.end
        } else if let Some(rule) = desc.rules.first_mut() {
            &mut rule.body
        } else {
            return;
        };
        target.push(Op::Timed);
    }
}

fn reduce_fallback(desc: &mut Desc) {
    // If we have output but no high-level ops describe it, add Generate
    let has_any_high = has_high_level_ops(desc);
    let has_output = desc.rules.iter().any(|r| {
        r.body.iter().any(|op| {
            matches!(
                op,
                Op::Emit(_) | Op::EmitFmt(_) | Op::EmitCounter | Op::Select(_)
            )
        })
    }) || desc.begin.iter().any(|op| {
        matches!(
            op,
            Op::Emit(_) | Op::EmitFmt(_) | Op::EmitCounter | Op::Select(_)
        )
    }) || desc.end.iter().any(|op| {
        matches!(
            op,
            Op::Emit(_) | Op::EmitFmt(_) | Op::EmitCounter | Op::Select(_)
        )
    });

    // Any remaining raw Emit without fields → Generate
    let mut any_output = has_output;
    for rule in &mut desc.rules {
        for op in &mut rule.body {
            if let Op::Emit(fields) = op
                && fields.is_empty()
            {
                any_output = true;
            }
        }
    }

    if !has_any_high && any_output {
        // Check if there's already a Select
        let has_select = desc.rules.iter().any(|r| {
            r.body.iter().any(|op| matches!(op, Op::Select(_)))
        });
        if !has_select {
            // Find the right place to put Generate
            if !desc.begin.is_empty() && desc.rules.is_empty() && desc.end.is_empty() {
                desc.begin.push(Op::Generate);
            } else if let Some(rule) = desc.rules.first_mut() {
                rule.body.push(Op::Generate);
            }
        }
    }
}

// ── Helpers ─────────────────────────────────────────────────────

fn collect_all_fns(desc: &Desc) -> Vec<String> {
    let mut fns = Vec::new();
    let collect = |ops: &[Op], fns: &mut Vec<String>| {
        for op in ops {
            if let Op::Fn(name) = op
                && !fns.contains(name)
            {
                fns.push(name.clone());
            }
        }
    };
    collect(&desc.begin, &mut fns);
    for rule in &desc.rules {
        collect(&rule.body, &mut fns);
    }
    collect(&desc.end, &mut fns);
    fns
}

fn resolve_source(desc: &Desc) -> Option<String> {
    for rule in &desc.rules {
        for op in &rule.body {
            if let Op::ArrayPut { val, .. } | Op::ArrayAccum { val, .. } = op
                && is_data_ref(val)
            {
                return Some(val.clone());
            }
        }
    }
    None
}

fn is_data_ref(s: &str) -> bool {
    s.starts_with('$') || matches!(s, "NF" | "NR" | "FNR" | "FILENAME")
}

fn has_high_level_ops(desc: &Desc) -> bool {
    let is_high = |op: &Op| {
        matches!(
            op,
            Op::Where(_)
                | Op::Select(_)
                | Op::Freq(_)
                | Op::Sum(_)
                | Op::Agg(_, _)
                | Op::Histogram(_)
                | Op::Stats(_)
                | Op::Count(_)
                | Op::Dedup(_)
                | Op::Join(_, _)
                | Op::Transform(_)
                | Op::Extract(_)
                | Op::NumberLines
                | Op::Rewrite
                | Op::Collect
                | Op::Generate
                | Op::Reformat(_)
        )
    };
    desc.begin.iter().any(is_high)
        || desc.rules.iter().any(|r| r.body.iter().any(is_high))
        || desc.end.iter().any(is_high)
}

fn clear_rules_and_end(desc: &mut Desc) {
    desc.rules.clear();
    desc.end.clear();
}

/// Replace `$N` (N>0) with `col N`, strip `+ 0` coercions, clean SUBSEP.
pub(super) fn humanize(s: &str) -> String {
    let s = s.replace(" + 0", "").replace("0 + ", "");
    let s = s.replace(" $SUBSEP ", ", ");
    let mut out = String::with_capacity(s.len());
    let b = s.as_bytes();
    let mut i = 0;
    while i < b.len() {
        if b[i] == b'$' {
            let start = i + 1;
            // Check for quoted name: $"name"
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
                // $0 → keep as $0
                out.push('$');
                out.push_str(&s[start..end]);
                i = end;
                continue;
            }
            // Check for $varname — keep $name intact
            let mut end = start;
            while end < b.len() && (b[end].is_ascii_alphanumeric() || b[end] == b'_') {
                end += 1;
            }
            if end > start {
                out.push('$');
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
