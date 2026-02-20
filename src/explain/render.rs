use std::path::Path;

use super::lower::{Desc, Op};

/// Render a reduced Desc into a terse human-readable string.
///
/// Collects all high-level ops, sorts by priority, and joins
/// them into a budget-limited string.
pub fn render(desc: &Desc, budget: usize) -> String {
    let mut phrases: Vec<Phrase> = Vec::new();

    collect_phrases(&desc.begin, &mut phrases);
    for rule in &desc.rules {
        collect_phrases(&rule.body, &mut phrases);
    }
    collect_phrases(&desc.end, &mut phrases);

    if phrases.is_empty() {
        return String::new();
    }

    // Sort by priority (highest first), stable
    phrases.sort_by_key(|p| std::cmp::Reverse(p.priority));

    // Subsumption: remove phrases that are made redundant by others
    let tags: Vec<Tag> = phrases.iter().map(|p| p.tag).collect();
    phrases.retain(|p| !p.tag.subsumed_by().iter().any(|t| tags.contains(t)));

    // Special merge: Agg + Freq → relabel to aggregation, drop freq
    if tags.contains(&Tag::Agg) && tags.contains(&Tag::Freq) {
        for p in &mut phrases {
            if p.tag == Tag::Agg {
                p.text = p.text.replacen("sum of ", "aggregation of ", 1);
            }
        }
        phrases.retain(|p| p.tag != Tag::Freq);
    }

    // Merge "slurped from X" into the preceding Select (e.g. "from JSON: ...") so it stays in one phrase
    if let Some(slurp_text) = phrases.iter().find(|p| p.tag == Tag::Slurp).map(|p| p.text.clone()) {
        if let Some(p) = phrases.iter_mut().find(|p| p.tag == Tag::Select) {
            p.text.push_str(", ");
            p.text.push_str(&slurp_text);
        }
        phrases.retain(|p| p.tag != Tag::Slurp);
    }

    // Render with budget
    render_phrases(&phrases, budget)
}

// ── Phrase: tagged text with significance weight ─────────────────
//
// All explanation ordering is driven by a single significance table in
// Tag::priority() below. Higher weight = more significant = appears first.
// So e.g. "filter on match() capture" (CaptureFilter) comes before "regex
// extract" (Extract); pattern/where (Filter) is low so it appears last.

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Tag {
    Chart,
    Stats,
    Agg,
    Freq,
    Sum,
    Count,
    Collect,
    Transform,
    Extract,
    CaptureFilter,
    Filter,
    Rewrite,
    Select,
    Generate,
    Number,
    Slurp,
}

impl Tag {
    /// Single source of truth for phrase order in every explanation.
    /// Higher = more significant = first. No other code should reorder phrases.
    fn priority(self) -> u8 {
        match self {
            Self::Chart => 90,
            Self::Stats => 85,
            Self::Agg => 80,
            Self::Freq => 75,
            Self::Sum => 70,
            Self::Count | Self::Collect => 65,
            Self::CaptureFilter => 66, // filter on c[N] ≥ X — most significant when present
            Self::Transform => 60,
            Self::Extract => 55,
            Self::Rewrite => 35,
            Self::Select | Self::Generate => 30,
            Self::Number => 25,
            Self::Filter => 5,  // pattern/where — qualifier last
            Self::Slurp => 4,
        }
    }

    fn subsumed_by(self) -> &'static [Tag] {
        match self {
            Tag::Stats => &[Tag::Chart],
            Tag::Agg | Tag::Sum => &[Tag::Stats],
            Tag::Collect => &[Tag::Chart, Tag::Stats, Tag::Agg, Tag::Freq],
            Tag::Number => &[Tag::Select, Tag::Count],
            _ => &[],
        }
    }
}

struct Phrase {
    text: String,
    tag: Tag,
    priority: u8,
}

impl Phrase {
    fn new(text: impl Into<String>, tag: Tag) -> Self {
        Self {
            text: text.into(),
            priority: tag.priority(),
            tag,
        }
    }
}

// ── Collecting phrases from ops ─────────────────────────────────

fn collect_phrases(ops: &[Op], out: &mut Vec<Phrase>) {
    let (range_bounds, range_over) = ops.iter().find_map(|o| match o {
        Op::Range(b, k) => Some((b.clone(), k.clone())),
        _ => None,
    }).unwrap_or((None, None));
    let has_range = ops.iter().any(|o| matches!(o, Op::Range(_, _)));
    let transform_first_word = ops.iter().find_map(|o| match o {
        Op::Transform(t) => t.split_whitespace().next().map(String::from),
        _ => None,
    });
    for op in ops {
        if matches!(op, Op::Range(_, _)) {
            continue;
        }
        if let Some(mut phrase) = op_to_phrase(op) {
            if has_range && phrase.tag == Tag::Select {
                phrase.text = match (&range_over, &range_bounds) {
                    (Some(k), _) => format!("for all {}: {}", k, phrase.text),
                    (_, Some(b)) => format!("range {}: {}", b.replace('–', ".."), phrase.text),
                    _ => format!("range: {}", phrase.text),
                };
            }
            // Don't repeat the transform verb as the only output (e.g. "replace ..., gensub" → "replace ...")
            let redundant = phrase.tag == Tag::Select
                && !phrase.text.contains(',')
                && !phrase.text.contains("column")
                && (transform_first_word.as_deref() == Some(phrase.text.as_str())
                    || (transform_first_word.as_deref() == Some("replace")
                        && matches!(phrase.text.as_str(), "sub" | "gsub" | "gensub")));
            if redundant {
                continue;
            }
            out.push(phrase);
        }
    }
}

fn op_to_phrase(op: &Op) -> Option<Phrase> {
    match op {
        Op::Where(text) => Some(Phrase::new(
            format!("where {}", to_title_columns(text)),
            Tag::Filter,
        )),
        Op::Select(fields) => {
            if fields.len() == 1 && fields[0].ends_with("fields") {
                Some(Phrase::new(
                    fields[0].replace("fields", "columns"),
                    Tag::Select,
                ))
            } else if looks_like_json_paths(fields) {
                Some(Phrase::new(
                    format!("from JSON: {}", fields.join(", ")),
                    Tag::Select,
                ))
            } else {
                Some(Phrase::new(format_field_list(fields), Tag::Select))
            }
        }
        Op::Freq(text) => {
            let rest = text.trim_start_matches("freq of ");
            let title = if rest.is_empty() || rest == "frequency" {
                "frequency".into()
            } else {
                format!("frequency of {}", to_title_columns(rest))
            };
            Some(Phrase::new(title, Tag::Freq))
        }
        Op::Sum(text) => {
            let rest = text.trim_start_matches("sum ");
            Some(Phrase::new(
                format!("sum of {}", to_title_columns(rest)),
                Tag::Sum,
            ))
        }
        Op::Agg(_, text) => {
            let t = to_title_columns(text);
            let title = if t.starts_with("sum column") {
                t.replacen("sum column", "sum of column", 1)
            } else if t.starts_with("sum ") {
                t.replacen("sum ", "sum of ", 1)
            } else if t.starts_with("agg column") {
                t.replacen("agg column", "aggregation of column", 1)
            } else if t.starts_with("agg ") {
                t.replacen("agg ", "aggregation of ", 1)
            } else {
                t
            };
            Some(Phrase::new(title, Tag::Agg))
        }
        Op::Histogram(text) => {
            let s = text.trim_start_matches("histogram of ");
            let title = if s.is_empty() || s == "histogram" || s == "chart" {
                "histogram".into()
            } else {
                format!("histogram of {}", to_title_columns(s))
            };
            Some(Phrase::new(title, Tag::Chart))
        }
        Op::Stats(text) => {
            let s = text.trim_start_matches("stats of ");
            let title = if s.is_empty() || s == "stats" {
                "statistics".into()
            } else {
                format!("statistics of {}", to_title_columns(s))
            };
            Some(Phrase::new(title, Tag::Stats))
        }
        Op::Count(Some(text)) => {
            let t = text.clone();
            let title = if t == "count lines" {
                "line count".into()
            } else if t.starts_with("count ") {
                t
            } else {
                "line count".into()
            };
            Some(Phrase::new(title, Tag::Count))
        }
        Op::Count(None) => Some(Phrase::new("line count", Tag::Count)),
        Op::Dedup(text) => {
            let by = text.trim_start_matches("deduplicate by ");
            let by_title = if by.is_empty() || by == "deduplication" {
                "key".into()
            } else if by == "$0" {
                "line".into()
            } else {
                to_title_columns(by)
            };
            Some(Phrase::new(format!("deduplication by {by_title}"), Tag::Filter))
        }
        Op::Join(_, text) => Some(Phrase::new(to_title_columns(text), Tag::Filter)),
        Op::Transform(text) => Some(Phrase::new(text.clone(), Tag::Transform)),
        Op::Extract(text) => Some(Phrase::new(text.clone(), Tag::Extract)),
        Op::NumberLines => Some(Phrase::new("numbered lines", Tag::Number)),
        Op::Rewrite => Some(Phrase::new("rewritten fields", Tag::Rewrite)),
        Op::Collect => Some(Phrase::new("collected lines", Tag::Collect)),
        Op::Generate => Some(Phrase::new("output", Tag::Generate)),
        Op::Timed => Some(Phrase::new("timed", Tag::Number)),
        Op::Reformat(text) => Some(Phrase::new(text.clone(), Tag::Rewrite)),
        Op::CaptureFilter(text) => Some(Phrase::new(text.clone(), Tag::CaptureFilter)),
        Op::Slurp(path) => {
            let base = Path::new(path)
                .file_name()
                .and_then(|p| p.to_str())
                .unwrap_or(path);
            Some(Phrase::new(format!("slurped from {base}"), Tag::Slurp))
        }
        _ => None,
    }
}

// ── Field list formatting ───────────────────────────────────────

/// True when the output list looks like jpath keys (dotted paths) rather than column names.
fn looks_like_json_paths(fields: &[String]) -> bool {
    if fields.is_empty() {
        return false;
    }
    let any_dotted = fields.iter().any(|f| f.contains('.'));
    let none_numeric = fields.iter().all(|f| f.parse::<usize>().is_err());
    any_dotted && none_numeric
}

/// Format a field list as title-style: "column N" / "columns N–M".
///   ["1"]         → "column 1"
///   ["1","2","3"] → "columns 1–3"
///   ["1","3"]     → "columns 1, 3"
///   ["host","cpu"]→ "host, cpu"
fn format_field_list(fields: &[String]) -> String {
    let nums: Option<Vec<usize>> = fields.iter().map(|f| f.parse::<usize>().ok()).collect();
    if let Some(ref idx) = nums {
        let consecutive = idx.len() > 1 && idx.windows(2).all(|w| w[1] == w[0] + 1);
        let (prefix, body): (String, String) = if idx.len() == 1 {
            ("column ".into(), idx[0].to_string())
        } else if consecutive {
            ("columns ".into(), format!("{}–{}", idx[0], idx.last().unwrap()))
        } else {
            (
                "columns ".into(),
                idx.iter()
                    .map(|n| n.to_string())
                    .collect::<Vec<_>>()
                    .join(", "),
            )
        };
        format!("{prefix}{body}")
    } else {
        fields.join(", ")
    }
}

/// Turn reduce-layer text into title-style: "col N" → "column N", "col 1–3" → "columns 1–3".
fn to_title_columns(s: &str) -> String {
    let s = s.replace("col ", "column ");
    if s.contains("column ") && s.contains('–') {
        s.replacen("column ", "columns ", 1)
    } else {
        s
    }
}

// ── Budget-aware rendering ──────────────────────────────────────

fn render_phrases(phrases: &[Phrase], budget: usize) -> String {
    if phrases.is_empty() {
        return String::new();
    }

    for take in (1..=phrases.len()).rev() {
        let parts: Vec<&str> = phrases[..take].iter().map(|p| p.text.as_str()).collect();
        let mut text = parts.join(", ");
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
