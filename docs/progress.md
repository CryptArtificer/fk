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
- [x] Strict perf suite with warmup + median/p90 reporting (`tests/suite/perf_strict.sh`)
- [x] Strict perf baseline captured (`docs/perf-baseline.md`)

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
- [ ] CI pipeline (build, test, lint, clippy) — on hold
- [ ] Publish to crates.io
- [x] Format module: syntax-highlight (theme, token segments, comments; BuiltinVar vs Identifier); `--highlight`; `--format` pretty-print (indent, line breaks); suggest output and examples use highlighting

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

#### Phase 9 — Printf & statistics
- [x] Printf enhancements: `%05d` (zero-pad), `%+d` (force sign), `%x` (hex), `%o` (octal), `%c` with numeric arg
- [x] Statistical builtins on arrays: `sum()`, `mean()`, `median()`, `stddev()`, `variance()`, `p()`/`percentile()`, `quantile()`, `iqm()`, `min()`, `max()`

#### Phase 10 — Code quality
- [x] Split `action.rs` into `action/` directory (mod, eval, stmt, builtins_rt)
- [x] Structured `FkError` with Display+Error
- [x] O(1) lexer spans (incremental line/col tracking)
- [x] Multiple BEGIN/END blocks
- [x] Encapsulated Runtime (private arrays/variables, methods only)
- [x] `#[must_use]` on Value; eliminated AST clones in match_rule hot path

#### Phase 11 — Describe & decompression
- [x] `--describe` / `-d` mode: auto-detect format, infer schema, suggest programs
- [x] Transparent decompression (.gz/.zst/.bz2/.xz/.lz4)
- [x] Auto-detect input mode from file extension (.csv → `-i csv`, .tsv.gz → `-i tsv`, etc.)

#### Phase 12 — CSV robustness & CLI polish
- [x] Single-pass RFC 4180 CSV, unclosed quote damage limiting
- [x] `--help`, `--version`, `-F`/`-i` conflict check, file-only default to `{print}`
- [x] `-v` escape interpretation, `-F` prevents auto-detect override

#### Phase 13 — Array & I/O builtins
- [x] `!/regex/` (bare regex in expression context), `print arr` (smart array dump)
- [x] `keys(arr)`, `vals(arr)`, `join(arr)` defaults to OFS
- [x] Lodash-inspired array builtins: `uniq`, `inv`, `tidy`, `shuf`, `diff`, `inter`, `union`, `seq`, `samp`
- [x] String: `lpad`, `rpad`
- [x] I/O: `slurp(file [, arr])`

#### Phase 14 — Awk compatibility fixes
- [x] `in` operator for any expression (`$0 in a`, `($1,$2) in arr`, `!(key in arr)`)
- [x] Regex literals in `sub`/`gsub`/`match`/`split` (first arg)
- [x] `printf "%c"` with numeric arg (char code conversion)
- [x] Bare `length` / `length()` defaults to `length($0)`
- [x] Diagnostics: `dump(x [,file])`, `clk()` / `clock()`, `tic([id])` / `start([id])`, `toc([id])` / `elapsed([id])`

#### Phase 15 — Expression & I/O fixes
- [x] Ternary/logical/in/match operators in print/printf arguments
- [x] BEGIN/END-only programs skip stdin (gawk behaviour)
- [x] `getline` (no source) reads from current input stream, not raw stdin
- [x] `getline var` preserves `$0` while reading into named variable

#### Phase 16 — Performance: AST analysis & resource lifecycle
- [x] `analyze.rs`: static AST walker produces `ProgramInfo` (needs_fields, needs_nf, max_field hint, regex literals)
- [x] Skip field splitting when program never accesses `$1`…`$N` or NF (nosplit path in Runtime)
- [x] Pre-compile all regex patterns from the AST at startup (eliminates cold-start penalty)
- [x] Persistent `getline < "file"` handles: reads successive lines, not line 1 each time
- [x] Persistent `"cmd" | getline` pipes: command spawned once, reads successive lines
- [x] `close()` now closes input file handles and pipes (not just output)
- [x] Capped field split: `{ print $2 }` only splits 2 of N fields; $0 served from record_text
- [x] record_text tracking with fields_dirty flag — $0 preserves original whitespace (awk correctness fix)
- [x] `CONVFMT` variable: controls implicit number-to-string in concatenation (separate from OFMT)
- [x] OFMT wired into print output path (was stored but unused)
- [x] Dynamic printf width/precision: `%*d`, `%.*f`, `%*.*f` consume extra args
- [x] Pattern matching: 3.2× → 4.3× faster than awk (nosplit)
- [x] Print $2: 0.15s → 0.13s (capped split), 4.7× faster than awk (1M lines, M3 Pro)

#### Phase 17 — Awk compat: C3-C5
- [x] Multiple `-f` files: `-f a.fk -f b.fk` concatenates program sources
- [x] BEGINFILE / ENDFILE blocks: fire at start/end of each input file
- [x] `typeof()` returns `"uninitialized"` for unset array elements (not just variables)

#### Phase 18 — Lazy field storage (E3)
- [x] `split_offsets()` / `split_offsets_limit()` in field.rs — byte-range pairs, zero String allocation
- [x] `Runtime` stores `field_offsets: Vec<(usize, usize)>` + `fields_lazy` flag
- [x] `write_field_to()` writes directly from `record_text[start..end]` — zero-copy hot path
- [x] `get_field()` slices record_text on demand; `set_field()` materializes all before modifying
- [x] `set_record()` and `set_record_capped()` now use offset path by default
- [x] print $2: 0.13s → 0.10s (6.4× faster than awk); sum: 0.24s → 0.17s (3.4× faster)
