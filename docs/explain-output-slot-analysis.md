# Explain output: problem analysis and plan

**Status:** Part A (output slots from literals + function names, multi-slot per expr) implemented. Part B (stats + JSON order/redundancy/"statistics of source") to do.

## What’s wrong (three cases)

### 1. ASCII table — `range 33..126: i`

**Program:** In a loop over `i`, we `printf` three values: `i`, `hex(i)`, `chr(i)` (decimal, hex, character), 6 per line.

**Current explanation:** `range 33..126: i` — only the loop variable is mentioned.

**Problem:** We only name *refs* (variables/fields). For `hex(i)` and `chr(i)` we recurse into the args and only see `i`, so every slot collapses to `"i"`. The fact that we’re printing *three* different things (the number, its hex, its character) is lost. The description should reflect that we output **i, hex, chr** (or equivalent), not just **i**.

---

### 2. Per-character — `range: c`

**Program:** In a loop we `printf` `c`, `ord(c)`, `hex(ord(c))` — character, code point, hex.

**Current explanation:** `range: c` — only the loop variable.

**Problem:** Same as case 1. We only collect refs; for `ord(c)` and `hex(ord(c))` we only see `c`, so we end up with a single slot. The real output shape is **c, ord, hex** (character, code point, hex). The explanation should show that.

---

### 3. Stats + JSON — `statistics, JSON extract (JSON, api.jsonl)`

**Program:** Rule does `jpath($0, ".ms")` and stores in `lat[NR]`; END prints `mean(lat)`, `median(lat)`, `p(lat,95)`, etc. (latency stats).

**Current explanation:** `statistics, JSON extract (JSON, api.jsonl)`.

**Problems (one or more of):**

- **Order:** Logically we “extract from JSON, then compute statistics”. Putting “statistics” first suggests the opposite.
- **Redundancy:** “JSON extract” and “(JSON, api.jsonl)” both say “JSON”; the env suffix can make the phrase feel repetitive.
- **What is summarized:** We may not be tying “statistics” to the extracted quantity (e.g. “statistics of .ms” or “latency”) so it’s clear what the stats are about.

---

## Root cause (cases 1 and 2)

**Output ref collection** today:

- For each value expr we use either:
  - a preceding string literal (simplified) as the slot name, or
  - the *refs* from that expr (variables, fields, jpath paths).
- For a **function call** like `hex(i)` or `chr(i)` we don’t treat the call itself as a slot; we only recurse into args and collect `i`. So multiple calls that share the same argument collapse to one ref.

So the pipeline never sees “we’re printing hex(i) and chr(i)” — it only sees “we’re printing something that involves i”. That’s why we get “range 33..126: i” and “range: c” instead of “range 33..126: i, hex, chr” and “range: c, ord, hex”.

---

## Plan (no special cases)

### A. Cases 1 and 2 — slot name from top-level call

**Rule:** For each *value* position in a print/printf, the slot name is (in order of precedence):

1. **Preceding string literal** (simplified) — already implemented; labels outweigh variable names.
2. **If the value expr is a function call** — use the **function name** as the slot name (e.g. `hex`, `chr`, `ord`). One slot per value; do not recurse into args for the purpose of that slot.
3. **Else** — use the ref(s) from the expr as today.

So:

- `printf "  %3d  %4s  %s", i, hex(i), chr(i)` → slots `["i", "hex", "chr"]`.
- `printf "  %s → %d → %s\n", c, ord(c), hex(ord(c))` → slots `["c", "ord", "hex"]`.

**Implementation:** In `collect_output_refs` (lower), when we process an expr that is not a string literal:

- If there is a preceding label, use it (unchanged).
- Else if the expr is `Expr::FuncCall(name, _)` (top-level value), push `name.clone()` as the slot for that position; do not add refs from the args for that slot (so we get one slot per call).
- Else call `collect_expr_refs` and use the refs as today.

Result: “range 33..126: i, hex, chr” and “range: c, ord, hex” emerge from the same generic rule (literal → label; call → function name; else → refs). No special case for “dec/hex/chr” or “ord/hex”.

### B. Case 3 — stats + JSON

**Clarify and fix in order:**

1. **Phrase order**  
   Ensure “JSON extract” (or equivalent) is ordered before “statistics” when the program flow is “extract in rules, then stats in END”. That may mean adjusting reduce/merge order or phrase priority so that rule-body extract is described before END stats.

2. **Redundancy with env**  
   If the base explanation already says “JSON extract” and the env suffix is “(JSON, api.jsonl)”, consider:
   - Dropping “JSON” from the extract phrase when env already says “JSON”, or
   - Making the env suffix not repeat “JSON” when the base already implies it.  
   Prefer a single generic rule (e.g. “env adds format + file; base phrases describe what the program does”) so we don’t special-case “JSON”.

3. **What is summarized**  
   Ensure `resolve_source` (or equivalent) can still see the array/value that feeds stats (e.g. `lat` / `.ms`) so the Stats phrase can be “statistics of &lt;source&gt;” (e.g. “statistics of ms” or “statistics of column”). If reduce removes array ops before stats are built, we may need to derive the source earlier or from a different op so the stats phrase stays descriptive without special cases.

Implement after A; use the actual code paths (reduce order, render, env merge) to decide the minimal change for order, redundancy, and “statistics of X”.

---

## Summary

| Case | Problem | Fix (generic) |
|------|--------|----------------|
| ASCII table | Only “i” shown; we print i, hex(i), chr(i) | Use function name as slot when value expr is a call → “i, hex, chr” |
| Per-char | Only “c” shown; we print c, ord(c), hex(ord(c)) | Same rule → “c, ord, hex” |
| Stats + JSON | Order, redundancy, or missing “what” for stats | Phrase order, env vs base wording, and “statistics of &lt;source&gt;” from existing data |

All fixes stay data-driven from the AST/reductions; no special-case strings for “dec”, “hex”, “chr”, “ord”, or “latency”.
