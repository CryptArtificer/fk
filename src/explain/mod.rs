//! # Explain pipeline: design
//!
//! **Model.** An explanation is a short summary of what a program does. We produce it in three
//! stages; each stage has a single responsibility and no special-case branches for specific
//! programs.
//!
//! 1. **Lower (AST → Desc).** For every AST node we either emit one or more ops or explicitly
//!    ignore. No silent fallthrough: every `Statement` and every effectful `Expr` variant is
//!    handled. Output refs (what gets printed) are collected from print/printf args by
//!    recursing and resolving vars; we never hard-code variable names or idioms.
//!
//! 2. **Reduce (Desc → Desc).** Pattern rewrites only. Each pass matches low-level op patterns
//!    and replaces them with high-level ops. All pattern data (builtin names, tag sets) lives
//!    in const tables; pass order is fixed and documented. No pass emits a phrase that depends
//!    on specific variable names or literal values (e.g. no "if fields == ['i'] then …").
//!
//! 3. **Render (Desc → String).** Collect ops → phrases, sort by significance (one table),
//!    apply subsumption and a small set of merge rules, then truncate to budget. Phrase order
//!    and wording come from the significance table and from op payloads (e.g. Transform(text));
//!    no ad-hoc string overrides for particular programs.
//!
//! **Invariants.** (a) Changing a weight in the significance table is the only way to change
//! phrase order. (b) Adding a new “idiom” means adding a reduce rule that matches op patterns
//! and emits a high-level op with a generic description, not a branch on var names. (c) Tests
//! may assert exact strings for specific programs; production code must not branch on those
//! programs.

mod lower;
mod reduce;
mod render;

use std::path::Path;

use crate::analyze::analyze;
use crate::parser::Program;

pub use lower::{Desc, Flags, Op, RuleDesc};
pub use render::render as render_desc;
use lower::lower;
use reduce::reduce;
use render::render;

const BUDGET: usize = 72;

/// Runtime environment context for explain().
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

/// Produce a terse, human-readable explanation of a program.
///
/// Pipeline: AST → lower (flat ops) → reduce (normalize) → render (text).
pub fn explain(program: &Program, ctx: Option<&ExplainContext>) -> String {
    let info = analyze(program);
    let env = ctx.and_then(|c| c.suffix());

    let mut desc = lower(program, &info);
    reduce(&mut desc, &info);
    let base = render(&desc, BUDGET);

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

#[cfg(test)]
mod tests;
