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

    // Special merge: Agg + Freq → relabel agg, drop freq
    if tags.contains(&Tag::Agg) && tags.contains(&Tag::Freq) {
        for p in &mut phrases {
            if p.tag == Tag::Agg {
                p.text = p.text.replacen("sum ", "agg ", 1);
            }
        }
        phrases.retain(|p| p.tag != Tag::Freq);
    }

    // Render with budget
    render_phrases(&phrases, budget)
}

// ── Phrase: tagged text with priority ───────────────────────────

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
    Filter,
    Rewrite,
    Select,
    Generate,
    Number,
}

impl Tag {
    fn priority(self) -> u8 {
        match self {
            Self::Chart => 90,
            Self::Stats => 85,
            Self::Agg => 80,
            Self::Freq => 75,
            Self::Sum => 70,
            Self::Count | Self::Collect => 65,
            Self::Transform => 60,
            Self::Extract => 55,
            Self::Filter => 40,
            Self::Rewrite => 35,
            Self::Select | Self::Generate => 30,
            Self::Number => 25,
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
    for op in ops {
        if let Some(phrase) = op_to_phrase(op) {
            out.push(phrase);
        }
    }
}

fn op_to_phrase(op: &Op) -> Option<Phrase> {
    match op {
        Op::Where(text) => Some(Phrase::new(format!("where {text}"), Tag::Filter)),
        Op::Select(fields) => {
            if fields.len() == 1 && fields[0].ends_with("fields") {
                Some(Phrase::new(format!("use {}", fields[0]), Tag::Select))
            } else {
                Some(Phrase::new(
                    format!("use {}", format_field_list(fields)),
                    Tag::Select,
                ))
            }
        }
        Op::Freq(text) => Some(Phrase::new(text.clone(), Tag::Freq)),
        Op::Sum(text) => Some(Phrase::new(text.clone(), Tag::Sum)),
        Op::Agg(_, text) => Some(Phrase::new(text.clone(), Tag::Agg)),
        Op::Histogram(text) => Some(Phrase::new(text.clone(), Tag::Chart)),
        Op::Stats(text) => Some(Phrase::new(text.clone(), Tag::Stats)),
        Op::Count(Some(text)) => Some(Phrase::new(text.clone(), Tag::Count)),
        Op::Count(None) => Some(Phrase::new("count lines", Tag::Count)),
        Op::Dedup(text) => Some(Phrase::new(text.clone(), Tag::Filter)),
        Op::Join(_, text) => Some(Phrase::new(text.clone(), Tag::Filter)),
        Op::Transform(text) => Some(Phrase::new(text.clone(), Tag::Transform)),
        Op::Extract(text) => Some(Phrase::new(text.clone(), Tag::Extract)),
        Op::NumberLines => Some(Phrase::new("number lines", Tag::Number)),
        Op::Rewrite => Some(Phrase::new("rewrite fields", Tag::Rewrite)),
        Op::Collect => Some(Phrase::new("collect + emit", Tag::Collect)),
        Op::Generate => Some(Phrase::new("generate output", Tag::Generate)),
        Op::Timed => Some(Phrase::new("timed", Tag::Number)),
        Op::Reformat(text) => Some(Phrase::new(text.clone(), Tag::Rewrite)),
        _ => None,
    }
}

// ── Field list formatting ───────────────────────────────────────

/// Format a field list: collapses consecutive numeric ranges.
///   ["1","2","3"] → "col 1–3"
///   ["1","3"]     → "col 1, 3"
///   ["host","cpu"]→ "host, cpu"
fn format_field_list(fields: &[String]) -> String {
    let nums: Option<Vec<usize>> = fields.iter().map(|f| f.parse::<usize>().ok()).collect();
    if let Some(ref idx) = nums {
        let consecutive = idx.len() > 1 && idx.windows(2).all(|w| w[1] == w[0] + 1);
        let body = if consecutive {
            format!("{}–{}", idx[0], idx.last().unwrap())
        } else {
            idx.iter()
                .map(|n| n.to_string())
                .collect::<Vec<_>>()
                .join(", ")
        };
        format!("col {body}")
    } else {
        fields.join(", ")
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
