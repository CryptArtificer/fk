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
  main.rs              – entry point, orchestration
  cli.rs               – command-line argument parsing
  lexer.rs             – tokeniser
  parser.rs            – recursive-descent parser (tokens → AST)
  action.rs            – executor core: pattern matching, statements, expressions
  runtime.rs           – runtime state (variables, fields, arrays, Value type)
  field.rs             – field splitting (FS / OFS semantics)
  error.rs             – source-location-aware diagnostics (Span type)
  repl.rs              – interactive REPL mode
  input/
    mod.rs             – Record struct, RecordReader trait, source orchestration
    line.rs            – default line-oriented reader
    csv.rs             – RFC 4180 CSV/TSV reader (quoted fields, multi-line)
    json.rs            – JSON Lines (NDJSON) reader
    regex_rs.rs        – regex-based record separator reader
    parquet_reader.rs  – Apache Parquet reader (optional feature)
  builtins/
    mod.rs             – dispatch table, coercion helpers
    string.rs          – length, substr, index, trim, reverse, chr, ord, …
    math.rs            – sin, cos, sqrt, abs, ceil, floor, rand, min, max, …
    time.rs            – systime, strftime, mktime, parsedate
    printf.rs          – format_printf and spec helpers
    json.rs            – jpath() JSON path access (jq-light)
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
- [x] Buffer stdout output (`BufWriter` in Executor, flushed at END / fflush / system)
- [x] Intern built-in variable names (NR, NF, FS, OFS, RS, ORS as dedicated fields)
- [x] Reduce allocations in print hot path (direct-write to BufWriter, no intermediate string)
- [x] Edge-case audit: empty input, binary data, long lines, deep recursion (15 new tests)
- [x] Recursion depth guard (limit 200, clean error instead of stack overflow)
- [x] Profile-guided executor review: eliminate field-index round-trip, concat allocation, format overhead
- [ ] CI pipeline (build, test, lint, clippy)
- [ ] Publish to crates.io

#### Phase 7 — Missing POSIX & gawk features
- [x] `break` / `continue` statements (loop control)
- [x] `do { ... } while (cond)` loop
- [x] `exit` / `exit(code)` statement (runs END, then exits with code)
- [x] `-f program.awk` (read program from file)
- [x] `FILENAME` variable (current input file name)
- [x] `FNR` variable (per-file record number, resets each file)
- [x] `close(file)` / `close(cmd)` builtin (close output files and pipes)
- [x] `ENVIRON` array (environment variables)
- [x] `ARGC` / `ARGV` (command-line arguments)
- [x] `SUBSEP` and multi-dimensional arrays (`a[i,j]` → `a[i SUBSEP j]`)
- [x] `OFMT` variable (number output format, default `"%.6g"`)
- [x] Computed regex (`$0 ~ var` where var holds a pattern string)
- [x] `gensub(regex, replacement, how [, target])` (return modified string)
- [x] `next` statement (skip to next record)
- [x] Proper regex semantics for `/pattern/` and `~`/`!~` (use `regex::Regex`)

#### Phase 8 — Signature features & function library
- [x] Header names as field accessors (`$name`, `$"col-name"`, `$var` in `-H` / parquet mode)
- [x] Apache Parquet input (`-i parquet`, optional `--features parquet`)
- [x] `match()` with capture groups (3rd argument: array)
- [x] `asort(arr)` / `asorti(arr)` — sort arrays by value / key
- [x] `join(arr, sep)` — join array values into string
- [x] `typeof(x)` — return `"number"`, `"string"`, `"array"`, or `"uninitialized"`
- [x] Bitwise: `and()`, `or()`, `xor()`, `lshift()`, `rshift()`, `compl()`
- [x] Math: `rand()`, `srand()`, `atan2()`, `abs()`, `ceil()`, `floor()`, `round()`, `min()`, `max()`, `log2()`, `log10()`
- [x] String: `trim()`, `ltrim()`, `rtrim()`, `startswith()`, `endswith()`, `repeat()`, `reverse()`, `chr()`, `ord()`, `hex()`
- [x] Date: `parsedate(str, fmt)` — parse dates back to epoch
- [x] Richer `strftime()` specifiers (`%j %u %w %e %C %y %p %I`)

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

# Read program from file
echo '{ print $2 }' > prog.awk
fk -f prog.awk data.txt

# FILENAME and FNR across multiple files
fk '{ print FILENAME, FNR, $0 }' file1.txt file2.txt

# do-while loop (runs body at least once)
echo 5 | fk '{ i=$1; do { print i; i-- } while (i>0) }'

# break and continue
echo "" | fk 'BEGIN { for (i=1; i<=10; i++) { if (i==5) break; print i } }'

# exit with code
fk '{ if ($0 == "STOP") exit(1); print }' input.txt

# close() — reopen a file for writing
echo "" | fk '{ print "first" > "/tmp/x"; close("/tmp/x"); print "second" > "/tmp/x" }'

# gensub — return modified string without changing $0
echo "hello world" | fk '{ print gensub("o", "0", "g") }'

# Computed regex — match operator with variable patterns
echo -e "hello\n123\nworld" | fk '{ pat="^[0-9]+$"; if ($0 ~ pat) print "number:", $0 }'

# ENVIRON — access environment variables
echo "" | fk 'BEGIN { print ENVIRON["HOME"] }'

# Multi-dimensional arrays
echo "" | fk 'BEGIN { a[1,2]="x"; a[3,4]="y"; for (k in a) print k, a[k] }'

# REPL / interactive mode
# fk --repl
# fk> BEGIN { x = 42; print x }
# 42
# fk> :vars
# fk> :q

# ── Phase 8: Signature features ──

# Parquet files — query by column name
fk -i parquet '$age > 30 { print $name, $city }' data.parquet

# Quoted column names (hyphens, spaces, dots)
fk -i parquet '{ print $"user-name", $"total.revenue" }' data.parquet

# CSV with named columns (header mode)
echo -e 'name,age,city\nAlice,30,NYC\nBob,25,LA' | fk -F, -H '$age > 28 { print $name }'

# match() with capture groups
echo "2025-01-15" | fk '{ match($0, "([0-9]+)-([0-9]+)-([0-9]+)", cap); print cap[1], cap[2], cap[3] }'

# Sort array values and join
echo -e 'c\na\nb' | fk '{ a[NR]=$0 } END { asort(a); print join(a, ",") }'

# typeof() introspection
echo "" | fk 'BEGIN { x=42; y="hi"; z[1]=1; print typeof(x), typeof(y), typeof(z), typeof(w) }'

# Bitwise operations
echo "" | fk 'BEGIN { print and(0xFF, 0x0F), lshift(1, 8) }'

# Math: rand, abs, ceil, floor, round, min, max
echo "" | fk 'BEGIN { srand(42); print rand(), abs(-5), ceil(2.3), floor(2.7), min(3,7), max(3,7) }'

# String: trim, reverse, chr, ord, hex
echo "  hello  " | fk '{ print trim($0), reverse("abc"), chr(65), ord("A"), hex(255) }'

# parsedate — parse date string to epoch
echo "" | fk 'BEGIN { print parsedate("2025-01-15 10:30:00", "%Y-%m-%d %H:%M:%S") }'
```

## Performance

`make bench-compare` runs fk and awk head-to-head on a 1M-line CSV.
fk is faster than awk on most workloads (1.5–2× on compute-heavy tasks):

| Benchmark | fk | awk | Ratio |
|---|---|---|---|
| `print $2` | 0.80 s | 0.59 s | 1.36× |
| Sum column | 0.32 s | 0.60 s | 0.53× |
| `/active/` count | 0.37 s | 0.78 s | 0.47× |
| Field arithmetic | 0.33 s | 0.64 s | 0.52× |
| Associative array | 0.38 s | 0.68 s | 0.56× |
| Computed regex | 0.43 s | 0.65 s | 0.66× |
| Tight loop (3×) | 0.79 s | 0.75 s | 1.05× |

Parquet support reads 1M rows, auto-extracts column names, and runs
pattern-action programs with named field access — no other awk can do this.

Measured on Apple M3 Pro, 36 GB RAM, macOS 26.2.
awk version 20200816 (macOS system awk). `fk` built with `--release`.

## Building

```sh
# Default build (lightweight, no optional deps)
cargo build --release

# With Parquet support (adds arrow + parquet crates)
cargo build --release --features parquet

# binary: target/release/fk
```

## License

TBD
