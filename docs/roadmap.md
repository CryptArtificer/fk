# fk roadmap — performance & completeness

## Phase A — Skip unnecessary work ✓

**A1. `needs_fields`** ✓ — analyze.rs walks the AST; when the program
never accesses `$1`…`$N`, `field::split()` is skipped entirely.
Pattern matching went from 3.2× to 4.3× faster than awk.

**A2. `needs_nf`** ✓ — when NF is never read, the NF counting path
is also skipped (combined with A1 in the nosplit branch).

**A3. `max_field_hint`** ✓ — Runtime now stores `record_text` alongside
fields with a `fields_dirty` flag. `$0` served from record_text when no
field was modified; capped split is safe. `{ print $2 }` on a 4-field
CSV splits only 2 fields (0.15s → 0.13s).

## Phase B — Resource lifecycle ✓

**B1. Persistent output files** ✓ — was already done (`output_files` HashMap).

**B2. Persistent getline file handles** ✓ — `input_files: HashMap<String, BufReader<File>>`
on Executor. `getline < "file"` in a loop reads successive lines.

**B3. Persistent getline pipes** ✓ — `"cmd" | getline var` spawns the
command once and reads successive lines from its stdout.

**B4. `close()` for input handles** ✓ — `close(name)` now closes input
file handles and input pipes in addition to output handles.

## Phase C — Awk compat

**C1. CONVFMT** ✓ — interned variable, wired into concatenation path.
OFMT also wired into print path (was stored but unused).

**C2. Dynamic printf width** ✓ — `%*d`, `%.*f`, `%*.*f` all consume
extra arguments. Negative dynamic width triggers left-alignment.

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

    A1-A3 ✓ → B1-B4 ✓ → D1 ✓ → C1-C2 ✓
      → E3 → C3-C5 → D2 (if profiling justifies) → E1-E2

Remaining: E3 (lazy field storage), C3-C5 (compat), D2/E1-E2 (advanced).
