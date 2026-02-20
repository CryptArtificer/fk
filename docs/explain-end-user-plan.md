# Explain: end-user language plan

**Principle.** Explain output is for the **end user** — someone reading a short description of what the program does. They may not know awk or fk. Every phrase must be understandable without knowing builtin names or implementation details.

**Scope.** This document is the single source of truth for end-user wording. Implementation must follow it exactly. No ad-hoc string choices; all wording is either defined here or derived from data (e.g. column numbers, paths) via documented rules.

---

## 1. Audit: where user-visible text is produced

| Source | Op / path | Current wording | Owner |
|--------|-----------|-----------------|--------|
| reduce | Transform | `replace {pat} → {repl}` | Done (replaces gensub/gsub/sub) |
| reduce | Extract | `regex extract`, `JSON extract (.path)` | §2 |
| reduce | Join | `join on {k}`, `semi-join on {k}`, `anti-join on {k}` | §3 |
| reduce | Dedup | `deduplicate by {key}` (humanize) | OK |
| reduce | Count | `count lines`, `count {n}` | OK |
| reduce | Freq | `freq of {key}` → render: "frequency of …" | OK |
| reduce | Sum | `sum of {source}` (humanize) | OK |
| reduce | Agg | humanize(text) | OK |
| reduce | Stats | `stats of {source}` → render: "statistics of …" | OK |
| reduce | Histogram | `histogram of {source}` | OK |
| reduce | Select | field list from lower (slot names) | §4 |
| render | Where | `where {text}` (to_title_columns) | OK |
| render | Select | format_field_list(fields) | §4 |
| render | NumberLines, Rewrite, Collect, Generate, Timed, Reformat | fixed strings | OK |

---

## 2. Extract phrases (reduce)

**Current:** `regex extract`, `regex extract + format`, `JSON extract`, `JSON extract (.path)`, `JSON extract + format`, `JSON extract (.path) + format`.

**Rule:** Use **pattern** instead of **regex** when describing the operation. "Regex" is jargon; "pattern" is widely understood.

**New wording:**

- `regex extract` → **pattern extract**
- `regex extract + format` → **pattern extract + format**
- `JSON extract` → unchanged
- `JSON extract (.path)` → unchanged
- `JSON extract + format` → unchanged
- `JSON extract (.path) + format` → unchanged

**Implementation:** In `reduce_extract`, replace the literal string `"regex extract"` with `"pattern extract"` and `"regex extract + format"` with `"pattern extract + format"`. No other logic change.

---

## 3. Join phrases (reduce)

**Current:** Join op carries `(kind, text)` where `text` is `"{kind} on {k}"` or just `kind`; `kind` is one of `join`, `semi-join`, `anti-join`. Render uses `text` only.

**Rule:** "Semi-join" and "anti-join" are SQL/relational jargon. Use plain language that describes what the user sees.

**New wording:**

- **join** → unchanged: `join on {k}` or `join`
- **semi-join** → **matching rows**: `matching rows on {k}` or `matching rows`
- **anti-join** → **rows without a match**: `rows without a match on {k}` or `rows without a match`

**Implementation:** In `try_join`, after computing `kind` (`"join"` | `"semi-join"` | `"anti-join"`), compute a **display phrase**:

- `join` → `"join"`
- `semi-join` → `"matching rows"`
- `anti-join` → `"rows without a match"`

Then set `text = format!("{} on {}", display_phrase, k)` when `key.is_some()`, else `text = display_phrase.to_string()`. Push `Op::Join(kind.into(), text)` as today (kind is still the internal tag; only the displayed `text` changes). Render already uses `text`; no render change.

---

## 4. Output slot names in Select (render + one table)

**Current:** When the program prints the result of a function call (e.g. `printf "%s", hex(i)`), we use the **function name** as the slot name. So we get "range 33..126: i, hex, chr" or "range: c, ord, hex". Names like `ord`, `chr`, `hex` are programmer-facing.

**Rule:** When a slot name is a known builtin that has a clearer end-user term, substitute that term. All other slot names (variables, field refs, jpath paths, other builtins) stay unchanged.

**Builtin display table** (only these are substituted; everything else is identity):

| Slot name (from program) | Display name (for end user) |
|--------------------------|-----------------------------|
| ord                      | code point                  |
| chr                      | character                   |
| hex                      | hex value                   |

No other builtins are mapped. So: `length` stays "length", `mean` stays "mean", `substr` stays "substr", jpath paths (e.g. "method", "ms") stay as-is, variable names stay as-is.

**Implementation:**

1. **Location:** In `render.rs`, add a single function that maps a slot name to its display form:
   - `fn slot_display_name(name: &str) -> &str`
   - Static table (e.g. `match name { "ord" => "code point", "chr" => "character", "hex" => "hex value", _ => name }`).
2. **Use:** In `format_field_list`, in the non-numeric branch (the `else` that currently does `fields.join(", ")`), map each field: `fields.iter().map(|f| slot_display_name(f)).collect::<Vec<_>>().join(", ")`.
3. **Select from JSON:** When we format `from JSON: {}`, we use `fields.join(", ")` in render (in the `looks_like_json_paths` branch). Those fields are jpath path segments (e.g. "method", "path"); they are not builtin names, so we do **not** apply the display table there. Only apply the table in `format_field_list` (the general field list case). So the JSON branch stays `fields.join(", ")` with no mapping.
4. **Redundant-Select suppression (render):** We currently suppress when the single Select token equals the transform verb or is sub/gsub/gensub. The transform verb is now "replace". So we still suppress when Select is "replace" or "sub"/"gsub"/"gensub". After adding the display table, the Select phrase might show "code point" or "character" instead of "ord"/"chr". So the slot name we compare for redundancy is still the **raw** slot (we compare before we've built the phrase text, or we compare phrase.text). Actually we compare phrase.text — and phrase.text for Select is the result of format_field_list, which will now contain "code point" not "ord". So we're not suppressing "ord" as a Select when the transform is "replace"; we're suppressing when the Select is a single token that equals "replace" or sub/gsub/gensub. So no change needed to the suppression logic; it doesn't depend on ord/chr/hex.

**Tests:** Update any test that asserts exact output containing "ord", "chr", or "hex" in a Select phrase to expect "code point", "character", "hex value" instead.

---

## 5. No change (already end-user)

- **Where:** "where" + condition (humanized). Left as-is.
- **Dedup, Count, Freq, Sum, Agg, Stats, Histogram:** Wording uses "deduplication", "line count", "frequency of", "sum of", "statistics of", "histogram of". No change.
- **NumberLines, Rewrite, Collect, Generate, Timed, Reformat:** "numbered lines", "rewritten fields", "collected lines", "output", "timed", "reformat output". No change.
- **humanize / to_title_columns:** Continue to use for $N → "col N" / "column N" and cleaning of expressions. No change.

---

## 6. Implementation order and testing

1. **§2 Extract:** In `reduce.rs`, `reduce_extract`, replace "regex extract" with "pattern extract" and "regex extract + format" with "pattern extract + format". Run explain tests; update tests that expect "regex extract" to expect "pattern extract".
2. **§3 Join:** In `reduce.rs`, `try_join`, introduce `display_phrase` and build `text` from it. Run explain tests; update tests that expect "semi-join" or "anti-join" to expect the new phrases.
3. **§4 Slot names:** In `render.rs`, add `slot_display_name` and use it in `format_field_list` in the non-numeric branch only. Run explain tests; update tests that expect "ord", "chr", or "hex" in Select to expect "code point", "character", "hex value".

After each step, run full explain test suite and clippy. Update `.cursorrules` and `docs/progress.md` in the same commit as the code.

---

## 7. Sign-off

This plan is complete for the scope above. Implementation follows the order in §6. No shortcuts: one table, one place per rule, tests updated to match.
