# Phase 20 — Array convenience & language constructs

## Goal

Add built-in functions and language constructs that eliminate boilerplate in
data-processing programs. Everything follows existing patterns: array builtins
mutate in place and return name or count, language constructs desugar into
existing mechanisms.

## Staging

### Stage 1: `collect(a, expr)` — per-record append

The foundation for downstream features. Called per-record to build up a
numeric sequence for later analysis.

```awk
{ collect(a, $3) }          # append $3 to a[], auto-keyed 1..N
END { print mean(a) }
```

Semantics:
- Appends `expr` to array `a` with key `length(a)+1`
- Skips NaN and empty-string values (common in real data)
- Returns the new count
- Works like `a[++n] = $3` but shorter, skip-safe, and chainable

Implementation: `builtin_collect` in `builtins_rt.rs`, dispatched from
`eval.rs`. No parser changes.

### Stage 2: `top(a, n)` and `bottom(a, n)` — array selection

Keep only the largest or smallest n values from a 1..N numeric array.

```awk
{ collect(a, $3) }
END { top(a, 5); print join(a, ", ") }   # 5 largest values
```

Semantics:
- Sort values, keep top/bottom n, re-key 1..n
- Returns count (min(n, length(a)))
- Mutates array in place

Implementation: two builtins in `builtins_rt.rs`. No parser changes.

### Stage 3: `runtotal(a)` and `norm(a)` — array transforms

In-place transforms that return the array name for chaining (like `hist()`).

```awk
{ collect(a, $3) }
END { print plotbox(runtotal(a)) }       # running total chart
END { print join(norm(a), " ") }         # values scaled 0..1
```

`runtotal(a)`: replace each value with the cumulative sum up to that point.
Keys preserved, order by key.

`norm(a)`: scale all values to 0..1 (min becomes 0, max becomes 1).
If min == max, all values become 0.

Both return the array name (string) for chaining into `plotbox()`, `join()`,
etc. — same pattern as `hist()`.

### Stage 4: `window(a, n, expr)` — sliding window

Per-record function that maintains a ring buffer of the last n values.

```awk
{ window(w, 5, $3); print mean(w) }     # 5-record moving average
```

Semantics:
- Maintains array `a` as a circular buffer of at most n values
- Each call appends expr (skipping NaN/empty), evicts oldest if full
- Re-keys 1..min(count, n) so stats builtins work
- Returns current window size

Implementation needs a cursor stored alongside the array. Use ArrayMeta for
this (new variant `Window { capacity, cursor }`).

### Stage 5: `every N { block }` — pattern sugar

A pattern that fires every Nth record.

```awk
every 10 { print NR, $0 }              # print every 10th line
```

Desugars to `NR % N == 0`. N is evaluated once at parse time (must be a
numeric literal or constant expression).

Parser change: recognize `every` as a keyword when it appears in pattern
position, followed by an expression and a brace block. Produces
`Pattern::Expression(NR % N == 0)`.

### Stage 6: `last N { block }` — buffered tail

Execute a block against only the last N records of input.

```awk
last 5 { print }                        # print last 5 lines
last 10 { sum += $1 } END { print sum } # sum last 10 values
```

Semantics:
- During the main loop, buffer the last N records in a ring buffer
- After the main loop (before END), replay the buffered records through the
  block, re-splitting fields and updating NR/NF/$0 for each
- N is evaluated once (literal or constant expression)

Parser change: `last` keyword in pattern position, produces a new
`Pattern::Last(expr)` variant. Executor stores a ring buffer per `last`
rule and replays at end-of-input.

### Stage 7: Sorted for-in — `@sort` / `@val` modifiers

Deterministic iteration order for `for (k in arr)`.

```awk
END { for (k in freq) @rsort(val) { print k, freq[k] } }
```

Modifiers (applied after `)`):
- `@sort` — keys in ascending alpha order
- `@rsort` — keys in descending alpha order
- `@nsort` — keys in ascending numeric order (smart_sort_keys)
- `@rnsort` — keys in descending numeric order
- `@val` — keys sorted by ascending value (numeric)
- `@rval` — keys sorted by descending value (numeric)

Parser change: after parsing `for (k in arr)`, check for `@ident` token.
New AST variant `ForInSorted(var, array, sort_mode, block)` or add a
`sort_mode: Option<SortMode>` field to `ForIn`.

Executor: sort keys before iterating.

## Order

```
Stage 1 (collect) → commit
Stage 2 (top/bottom) → commit
Stage 3 (runtotal/norm) → commit
Stage 4 (window) → commit
Stage 5 (every) → commit
Stage 6 (last) → commit
Stage 7 (sorted for-in) → commit
```

Each stage: implement, test, clippy, update docs, commit.
