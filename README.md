# fk — filter-kernel

A modernized, modular awk clone built in Rust.

## Intent

`fk` aims to replicate the core text-processing model of awk — read input, split into records and fields, match patterns, execute actions — while providing a cleaner foundation that is easy to extend. Each category of functionality (I/O, field splitting, pattern matching, expressions, built-in functions, etc.) lives in its own module so new capabilities can be added without touching unrelated code.

### Design principles

- **Modular** — every concern is a separate module behind a clear interface.
- **Minimal dependencies** — lean on the Rust standard library; pull in crates only when they genuinely earn their keep.
- **Incremental** — built in deliberate steps, each one leaving a usable tool.

## Architecture

```
src/
  main.rs          – entry point, orchestration
  cli.rs           – command-line argument parsing
  lexer.rs         – tokeniser
  parser.rs        – recursive-descent parser (tokens → AST)
  action.rs        – executor core: pattern matching, statements, expressions
  runtime.rs       – runtime state (variables, fields, arrays)
  field.rs         – field splitting (FS / OFS semantics)
  error.rs         – source-location-aware diagnostics        (Phase 3)
  unicode.rs       – char-aware field/string operations       (Phase 3)
  repl.rs          – interactive mode                         (Phase 3)
  input/
    mod.rs         – InputFormat trait, record reader
    line.rs        – default line-oriented input (current behaviour)
    csv.rs         – RFC 4180 CSV/TSV parser                  (Phase 3)
    json.rs        – line-delimited JSON (NDJSON) parser      (Phase 3)
    header.rs      – first-line header → named-field mapping  (Phase 3)
  builtins/
    mod.rs         – dispatch table
    string.rs      – length, substr, index, sub, gsub, …
    math.rs        – sin, cos, sqrt, int, **, …
    time.rs        – systime, strftime, mktime                (Phase 3)
    io.rs          – fflush, system, /dev/stderr              (Phase 3)
```

## Progress

### Phase 0 — Skeleton & essentials
- [x] Argument handling (`-F`, `-v`, `'program'`, file operands)
- [x] Read lines from stdin and file arguments
- [x] Split records into fields (`$0`, `$1` … `$NF`)
- [x] Print action (bare `{ print }` / `{ print $N }`)
- [x] Field separator (`-F` flag and `FS` variable)
- [x] Basic pattern matching (string equality, `/regex/`)
- [x] `BEGIN` and `END` blocks
- [x] Built-in variables: `NR`, `NF`, `FS`, `OFS`, `RS`, `ORS`

### Phase 1 — Language core
- [x] Arithmetic & string expressions
- [x] Variable assignment (including `+=`, `-=`, `*=`, `/=`, `%=`)
- [x] `if` / `else` (including `else if` chains)
- [x] `while`, C-style `for`, `for (k in array)`
- [x] `++` / `--` (pre and post)
- [x] User-defined variables and associative arrays
- [x] `printf` / `sprintf`
- [x] Implicit string concatenation
- [x] Logical operators (`&&`, `||`, `!`)
- [x] Match operators (`~`, `!~`)
- [x] `delete array[key]`
- [x] Built-in functions: `length`, `substr`, `index`, `tolower`, `toupper`, `int`, `sqrt`, `sin`, `cos`, `log`, `exp`

### Phase 2 — Full awk compatibility
- [x] User-defined functions (`function name(params) { ... }`, `return`)
- [x] Getline variants (`getline`, `getline var`, `getline < file`, `"cmd" | getline`)
- [x] Output redirection (`>`, `>>`, `|` with persistent pipes)
- [x] Remaining POSIX builtins (`split`, `sub`, `gsub`, `match`)
- [x] Pattern ranges (`/start/,/stop/`)
- [x] Coercion rules (numeric string comparison, leading-prefix parsing, uninitialized → 0/"")
- [x] Ternary operator (`?:`)

### Phase 3 — Modernisation & extensions

#### 3a — Refactor into modules (no new features, just structure)
- [ ] Extract builtins from `action.rs` into `builtins/` (string, math, io)
- [ ] Extract input logic into `input/` with `InputFormat` trait
- [ ] Add `Span` (line/col) to tokens; thread through parser for error locations
- [ ] Add `error.rs` with formatted diagnostics

#### 3b — Structured input formats
- [ ] CSV input mode (`-i csv`, RFC 4180 compliant: quoted fields, embedded commas/newlines)
- [ ] TSV input mode (`-i tsv`)
- [ ] Header mode (`-H`): parse first line as column names, access via `@"name"` or `$FI["name"]`
- [ ] JSON lines input mode (`-i json`): each line is a JSON object, fields by key
- [ ] RS as regex (multi-char / pattern record separator)

#### 3c — Language additions
- [ ] `**` exponentiation operator
- [ ] Hex numeric literals (`0x1F`) and `\x` / `\u` escape sequences
- [ ] `nextfile` statement
- [ ] `delete array` (delete entire array, not just one key)
- [ ] `length(array)` (return number of elements)
- [ ] Negative field indexes (`$-1` = last field, `$-2` = second-to-last)
- [ ] `/dev/stderr` special file for error output
- [ ] `fflush()` and `system()` builtins
- [ ] Time functions: `systime()`, `strftime()`, `mktime()`

#### 3d — Quality of life
- [ ] Better error messages with source locations and context
- [ ] Unicode-aware `length()`, `substr()`, field splitting
- [ ] REPL / interactive mode

## Usage (goal)

```sh
# Print second field of every line (tab-separated)
echo -e "a\tb\nc\td" | fk -F'\t' '{ print $2 }'

# Sum a column
fk '{ sum += $1 } END { print sum }' numbers.txt

# Pattern match
fk '/error/ { print NR, $0 }' log.txt
```

## Building

```sh
cargo build --release
# binary: target/release/fk
```

## License

TBD
