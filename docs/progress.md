# Progress

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
- [x] Apache Parquet input (`-i parquet`, enabled by default)
- [x] `match()` with capture groups (3rd argument: array)
- [x] `asort(arr)` / `asorti(arr)` — sort arrays by value / key
- [x] `join(arr, sep)` — join array values into string
- [x] `typeof(x)` — return `"number"`, `"string"`, `"array"`, or `"uninitialized"`
- [x] Bitwise: `and()`, `or()`, `xor()`, `lshift()`, `rshift()`, `compl()`
- [x] Math: `rand()`, `srand()`, `atan2()`, `abs()`, `ceil()`, `floor()`, `round()`, `min()`, `max()`, `log2()`, `log10()`
- [x] String: `trim()`, `ltrim()`, `rtrim()`, `startswith()`, `endswith()`, `repeat()`, `reverse()`, `chr()`, `ord()`, `hex()`
- [x] Date: `parsedate(str, fmt)` — parse dates back to epoch
- [x] Richer `strftime()` specifiers (`%j %u %w %e %C %y %p %I`)
