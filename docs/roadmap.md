# fk roadmap — performance & completeness

## Phase A — Skip unnecessary work

The executor currently field-splits and counts NF on every record, even
when the program never touches fields. A single pre-analysis pass over
the AST can set flags to skip work.

**A1. `needs_fields`** — does the program reference `$1`..`$N`, `NF`,
`split()`, or assign to fields? If not, skip `field::split()` entirely.
Biggest single win for `/pattern/{print}` and `{gsub(...); print}`.

**A2. `needs_nf`** — only count fields when NF is actually read.

**A3. `max_field_hint`** — if only `$1` and `$2` are used, stop splitting
after 2 fields (lazy split with a cap).

Expected impact: 20-40% faster on pattern-match and gsub workloads.

## Phase B — Resource lifecycle

File handles and pipes are currently opened/closed per call. Awk keeps
them open across the program's lifetime until `close()`.

**B1. Persistent output files** — `print > "file"` in a loop should open
once, write many, close at END. The `output_files` HashMap exists but
isn't wired into print's `>` redirect path.

**B2. Persistent getline file handles** — `getline line < "file"` in a
loop should read successive lines, not reopen from the start. Add
`input_files: HashMap<String, BufReader<File>>` to Executor.

**B3. Persistent getline pipes** — `"cmd" | getline var` keeps the pipe
open across calls. Same approach as B2.

**B4. `close()` for input handles** — already works for output; extend to
input file handles and pipes from B2/B3.

## Phase C — Awk compat

**C1. CONVFMT** — separate from OFMT, controls implicit number-to-string
coercion in concatenation context. Small change in `Value::to_string_val()`.

**C2. Dynamic printf width** — `printf "%*d", width, value`. Parse `*` in
the format spec and consume an extra argument for the width.

**C3. Multiple `-f` files** — concatenate program sources before lexing.

**C4. BEGINFILE / ENDFILE** — gawk extension. Two more optional blocks in
Program, fired in the main loop on source transition.

**C5. Uninitialized variable distinction** — awk distinguishes "never
assigned" from "assigned empty". Affects edge cases with `in` and
truthiness checks.

## Phase D — Annotated program representation

### D1. Annotated AST (first step)

Pre-analysis pass produces a `ProgramInfo` struct:

    ProgramInfo {
        needs_fields: bool,
        needs_nf: bool,
        max_field_hint: Option<usize>,
        uses_getline: bool,
        uses_arrays: HashSet<String>,
        uses_regex: Vec<String>,       // pre-compile at startup
        builtin_calls: HashSet<String>,
        output_targets: Vec<String>,   // pre-open files
    }

The executor checks these flags. Zero structural change to the AST.
Regex pre-compilation alone eliminates cold-start penalties.

### D2. Flat instruction stream (evaluate after profiling)

Lower the AST into `Vec<Op>`:

    enum Op {
        LoadField(u16),
        LoadVar(u16),        // index into var table
        LoadConst(u16),      // index into constant pool
        StoreVar(u16),
        BinOp(BinOpKind),
        JumpIfFalse(u32),
        Jump(u32),
        CallBuiltin(BuiltinId, u8),
        Print(u8),
        ...
    }

Benefits: linear memory layout (cache-friendly), variable lookup by
index (not string HashMap), dispatch via jump table. Cost: lowering pass
+ executor rewrite; AST stays for error reporting.

Only pursue if profiling D1 shows the bottleneck is dispatch overhead
rather than I/O or field splitting.

## Phase E — Memory

**E1. String interning** — frequent array keys ("1", "2", ...) are
allocated thousands of times. Small intern pool saves alloc pressure.

**E2. Arena allocation** — per-record arena for Values. Batch-free instead
of individual String drops.

**E3. Lazy field storage** — store `$0` as a single string + split offsets
(byte positions). Only materialize individual field Strings on access.
Combined with A1/A3 this eliminates most allocation in the hot path.

## Execution order

    A1-A3 → B1-B3 → D1 → measure
      → C1-C2 → E3 → B4 → C3-C5
      → D2 (if profiling justifies) → E1-E2

First line is the performance-critical path. Second line is
feature/polish that can interleave. D2 is contingent on profiling
results after D1.
