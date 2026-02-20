# Explain module: simplification plan

## Problem

The `--explain` feature produces a terse one-line program summary. It exists
primarily for the `show` helper in examples (showcase subtitles) and as a
developer convenience. The current implementation is 3,700 lines across a
three-stage pipeline (lower → reduce → render) in `src/explain/` — wildly
disproportionate to the need.

### Issues in the current implementation

1. **Dead code** — `reduce_chart_stats` pushes then immediately removes Stats
   when chart is present
2. **Duplication** — `unwrap_coercion` is identical in `lower.rs` and
   `analyze.rs`; `expr_text` (90 lines) in `lower.rs` duplicates
   `expr_to_source` / `fmt_expr` in `analyze.rs`
3. **Special-case violation** — `describe_capture_filter` hardcodes array name
   `"c"` despite the stated "no special cases" principle
4. **Redundant work** — `scan_expr_fns` duplicates `Op::Fn` entries already
   pushed by `lower_effect`
5. **Implicit ordering** — reduce passes have undocumented ordering
   dependencies
6. **Incomplete coverage** — `lower_stmt` catch-all silently drops several
   Statement variants despite claiming all are handled
7. **Overweight** — `lower.rs` alone is 1,059 lines mixing four concerns

## Scope of explain

**Primary use case:** plotbox chart subtitles — already handled by
`build_array_description` in `analyze.rs` (~70 lines). Does NOT use the
explain module.

**Secondary use case:** `--explain` CLI flag, called by the `show` helper in
example scripts to print a `→ description` line under each demo command. This
is developer-facing; not end-user documentation.

## Plan

Replace `src/explain/` (directory with 5 files, 3,013 lines) with a single
flat `src/explain.rs` (~200–300 lines). Same `--explain` flag, same
`ExplainContext`, same output for common patterns.

### What stays

- `ExplainContext` and its `from_cli` / `suffix` logic (environment labels)
- `explain()` public API: `fn explain(program: &Program, ctx: Option<&ExplainContext>) -> String`
- `detect_format_from_ext`, `detect_compression` helpers
- Budget constant (72 chars)

### What changes

**Before:** AST → lower (flat Op enum, 27 variants) → reduce (12 ordered
passes) → render (significance tags, subsumption, budget truncation).

**After:** Single-pass pattern detection directly on the AST. One function
walks the program and checks for known idioms in priority order:

1. **Dedup** — single rule, `!seen[key]++` pattern
2. **Join/semi/anti** — NR==FNR first rule with next, second rule
3. **Count** — `n++` in rules + END print (no array accumulation)
4. **Histogram/Stats** — array collect in rules + chart/stat builtins in END
5. **Frequency** — `arr[key]++` in rules + for-in in END
6. **Aggregation** — `arr[key] += val` + `arr2[key]++` in rules + for-in in END
7. **Sum** — `var += $field` in rules + END print
8. **Transform** — sub/gsub/gensub in rule body
9. **Select** — print/printf field refs (collapse consecutive, humanize)
10. **Filter** — pattern on rule
11. **Misc** — number lines, rewrite fields, collect, reformat, iterate fields
12. **Fallback** — "output"

Each detector is a simple function that inspects the AST directly — no
intermediate representation, no multi-pass reduction. If a high-priority
pattern matches, lower-priority ones are skipped or appended as qualifiers.

### Deduplication

- **Delete** `unwrap_coercion` from explain; make `analyze::unwrap_coercion`
  `pub(crate)` and import it
- **Delete** `expr_text` from explain; use `analyze::expr_to_source` where
  needed
- **Delete** `humanize` — rewrite as a simpler function using the same
  `$N → column N` logic but without byte-level manipulation

### Files deleted

- `src/explain/mod.rs` (178 lines)
- `src/explain/lower.rs` (1,059 lines)
- `src/explain/reduce.rs` (758 lines)
- `src/explain/render.rs` (368 lines)
- `src/explain/tests.rs` (650 lines)
- `docs/explain-end-user-plan.md` (122 lines)
- `docs/explain-output-slot-analysis.md` (106 lines)

### Files created

- `src/explain.rs` (~200–300 lines, includes tests)

### Files modified

- `src/lib.rs` — module declaration stays `pub mod explain;` (file vs dir is transparent)
- `src/main.rs` — no change (already uses `explain::explain` and `explain::ExplainContext`)
- `src/analyze.rs` — make `unwrap_coercion` `pub(crate)`
- `.cursorrules` — update explain description
- `docs/progress.md` — note the simplification

### Test preservation

All 67 existing test assertions will be ported to `#[cfg(test)] mod tests` in
the new `src/explain.rs`. The tests define the contract; the implementation
changes but the outputs don't.

### What we're NOT changing

- `build_array_description` in `analyze.rs` — the plotbox subtitle mechanism
  is independent and already good
- `ExplainContext::from_cli` API
- The `--explain` CLI flag behavior
- The `show` helper in examples
