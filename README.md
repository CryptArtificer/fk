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
  error.rs         – source-location-aware diagnostics (Span type)
  repl.rs          – interactive REPL mode
  input/
    mod.rs         – Record struct, RecordReader trait, source orchestration
    line.rs        – default line-oriented reader
    csv.rs         – RFC 4180 CSV/TSV reader (quoted fields, multi-line)
    json.rs        – JSON Lines (NDJSON) reader
    regex_rs.rs    – regex-based record separator reader
  builtins/
    mod.rs         – dispatch table, coercion helpers
    string.rs      – length, substr, index, sub, gsub, …
    math.rs        – sin, cos, sqrt, int, **, …
    time.rs        – systime, strftime, mktime
    printf.rs      – format_printf and spec helpers
    json.rs        – jpath() JSON path access (jq-light)
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
- [x] Extract builtins from `action.rs` into `builtins/` (string, math, printf)
- [x] Extract input logic into `input/` with `RecordReader` trait
- [x] Add `Span` (line/col) to tokens; thread through parser for error locations
- [x] Add `error.rs` with formatted diagnostics

#### 3b — Structured input formats
- [x] CSV input mode (`-i csv`, RFC 4180 compliant: quoted fields, embedded commas/newlines)
- [x] TSV input mode (`-i tsv`)
- [x] Header mode (`-H`): parse first line as column names, populate `HDR` array
- [x] JSON lines input mode (`-i json`): each line is a JSON object, fields by value order
- [x] RS as regex (multi-char RS treated as regex pattern after BEGIN)
- [x] `jpath(json, path)` — lightweight JSON path access (`.key`, `[N]`, `.key[]`, implicit iteration)
- [x] `jpath(json, path, array)` — extract iterated values / arrays / objects into awk arrays

#### 3c — Language additions
- [x] `**` exponentiation operator
- [x] Hex numeric literals (`0x1F`) and `\x` / `\u` escape sequences
- [x] `nextfile` statement
- [x] `delete array` (delete entire array, not just one key)
- [x] `length(array)` (return number of elements)
- [x] Negative field indexes (`$-1` = last field, `$-2` = second-to-last) and `$(expr)` computed fields
- [x] `/dev/stderr` and `/dev/stdout` special files for output redirection
- [x] `fflush()` and `system()` builtins
- [x] Time functions: `systime()`, `strftime()`, `mktime()`

#### 3d — Quality of life
- [x] Error messages with source locations (`line:col`)
- [x] Unicode-aware `length()`, `substr()`, `index()`, field splitting
- [x] REPL / interactive mode (`--repl`)

#### Phase 4 — Benchmarks
- [x] Create `benches/` directory with criterion-based benchmarks
- [x] Field splitting throughput (whitespace, single-char, multi-char FS)
- [x] Lexer + parser throughput on realistic programs
- [x] Record processing throughput (simple print, field access, pattern match)
- [x] Comparison harness: `fk` vs `awk` vs `gawk` vs `mawk` on common tasks
- [x] Large-file benchmark (1M+ lines, CSV-like data)

#### Phase 5 — Tutorial & showcase
- [x] `examples/` directory with annotated scripts
- [x] Basics: field extraction, filtering, summing columns (`01-basics.sh`)
- [x] Text transforms: CSV wrangling, log parsing, frequency counting (`02-text-transforms.sh`)
- [x] Advanced: user-defined functions, associative arrays, multi-file processing (`03-advanced.sh`)
- [x] Showcase fk-only features: `**`, `$-1`, hex literals, `\u` escapes, time functions (`04-fk-features.sh`)
- [x] Structured input showcase: CSV, TSV, JSON, jpath (`05-json-and-csv.sh`)
- [x] One-liner cheat sheet (`docs/cheatsheet.md`)

#### Phase 6 — Hardening & optimisation
- [ ] Fuzz testing (lexer, parser, executor) with `cargo-fuzz`
- [ ] Edge-case audit: empty input, binary data, extremely long lines, deep recursion
- [ ] Reduce allocations in hot paths (field splitting, record loop, string concat)
- [ ] Intern frequently-used variable names (NR, NF, FS, …) to avoid HashMap lookups
- [ ] Profile-guided review of the executor loop
- [ ] CI pipeline (build, test, lint, clippy)
- [ ] Publish to crates.io

## Usage

```sh
# Print second field of every line (tab-separated)
echo -e "a\tb\nc\td" | fk -F'\t' '{ print $2 }'

# Sum a column
fk '{ sum += $1 } END { print sum }' numbers.txt

# Pattern match
fk '/error/ { print NR, $0 }' log.txt

# Exponentiation and hex literals
echo "4" | fk '{ print $1 ** 0.5, 0xFF }'

# Negative field indexes (last field)
echo "a b c d" | fk '{ print $-1 }'

# Time functions
echo "" | fk '{ print strftime("%Y-%m-%d %H:%M:%S", systime()) }'

# Run a shell command
echo "" | fk '{ system("echo hello from system()") }'

# Print to stderr
echo "oops" | fk '{ print $0 > "/dev/stderr" }'

# CSV input (RFC 4180: handles quoted fields, embedded commas)
echo -e 'name,age\n"Alice",30\n"Bob ""B""",25' | fk -i csv '{ print $1, $2 }'

# TSV input
echo -e 'x\t10\ny\t20' | fk -i tsv '{ sum += $2 } END { print sum }'

# Header mode: first line defines column names in HDR array
echo -e 'name,score\nAlice,95\nBob,87' | fk -i csv -H '{ print HDR[1], $1 }'

# JSON lines input (fields are values in insertion order)
echo '{"name":"Alice","age":30}' | fk -i json '{ print $1, $2 }'

# jpath: navigate nested JSON (jq-light)
echo '{"users":[{"name":"Alice"},{"name":"Bob"}]}' | fk '{ print jpath($0, ".users[1].name") }'

# jpath: iterate — .users[].name or .users.name (implicit iteration)
echo '{"users":[{"id":1},{"id":2},{"id":3}]}' | fk '{ print jpath($0, ".users[].id") }'

# jpath: extract iterated values into awk array
echo '{"items":[10,20,30]}' | fk '{ n = jpath($0, ".items", a); for (i=1; i<=n; i++) print a[i] }'

# Multi-char RS as regex (paragraph mode)
printf 'a\nb\n\nc\nd\n' | fk -v 'RS=\n\n' '{ print NR, $0 }'

# Unicode-aware: length, substr, index count characters, not bytes
echo "café" | fk '{ print length($0), substr($0,4,1) }'

# REPL / interactive mode
# fk --repl
# fk> BEGIN { x = 42; print x }
# 42
# fk> :vars
# fk> :q
```

## Building

```sh
cargo build --release
# binary: target/release/fk
```

## License

TBD
