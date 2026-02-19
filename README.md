# fk — filter-kernel

A slightly modernized awk clone built in Rust.

> **Note** — This is a personal project built as a learning exercise. It is not
> intended as a production tool and may not be actively maintained. You are
> welcome to explore the code, open issues, or fork it, but please set your
> expectations accordingly.

## Intent

`fk` aims to replicate the core text-processing model of awk — read input, split into records and fields, match patterns, execute actions — while providing a cleaner foundation that is easy to extend. Each category of functionality (I/O, field splitting, pattern matching, expressions, built-in functions, etc.) lives in its own module so new capabilities can be added without touching unrelated code.

### Design principles

- **Modular** — every concern is a separate module behind a clear interface.
- **Lean core** — the base binary depends only on `regex`. Parquet/Arrow support is an optional feature that can be disabled at build time.
- **Incremental** — built in deliberate steps, each one leaving a usable tool.

## What's different from awk

The pattern-action model is the same. Everything below is new.

- **Structured input** — native CSV, TSV, JSON Lines, and Apache Parquet readers (`-i csv`, `-i json`, `-i parquet`), so you don't need to pre-process with other tools.
- **Named columns** — in header mode (`-H`), access fields by name: `$name`, `$"user-name"`, `$col`. Works with CSV, TSV, JSON, and Parquet.
- **JSON navigation** — `jpath()` gives you jq-like path access from within a pattern-action program.
- **Statistical builtins** — `sum`, `mean`, `median`, `stddev`, `variance`, `hist`, `percentile`, `quantile`, `iqm` on arrays.
- **Array builtins** — `asort`, `asorti`, `join`, `keys`, `vals`, `uniq`, `inv`, `tidy`, `shuf`, `diff`, `inter`, `union`, `seq`, `samp`.
- **Diagnostics** — `dump(x)` inspects any variable or array to stderr. `clk()`, `tic(id)`, `toc(id)` for timing.
- **Unicode-aware** — `length`, `substr`, `index`, and all string builtins count characters, not bytes.
- **Transparent decompression** — gzip, zstd, bzip2, xz, and lz4 files are decompressed on the fly. No need to pipe through `zcat` or `zstdcat` first.
- **Auto-detection** — file extension determines both the decompression method and the input format. `fk '{ print $2 }' data.tsv.gz` just works: it decompresses with zlib and parses as TSV, no flags needed.
- **Schema discovery** — `--describe` sniffs a file, detects its format and compression, infers column names and types, and suggests programs you can run on it.
- **Capture groups in match()** — `match($0, /(\d+)-(\d+)/, cap)` extracts groups into an array. Standard awk can't do this.
- **Better errors** — source-location-aware diagnostics with line and column numbers.
- **Negative field indexes** — `$-1` is the last field, `$-2` is second-to-last.
- **REPL** — interactive mode for exploration (`--repl`).
- **Format & highlight** — `--highlight` prints a syntax-highlighted program (keywords, literals, built-in vars distinct); `--format` pretty-prints with indentation and line breaks. Examples and `--suggest` output use highlighting when available.

Some of these — especially the built-in format readers and decompression —
go against the classic Unix ideal of small, single-purpose tools composed with
pipes. That's a deliberate trade-off: in practice, shelling out to `csvcut` or
`jq` just to feed awk is slow and awkward. Keeping the format awareness inside
the tool means one process, one pass, and column names that survive the whole
pipeline.

## Architecture

```
src/
  main.rs              – entry point, orchestration
  cli.rs               – command-line argument parsing
  describe.rs          – format sniffer, schema inference, suggestions, decompression
  lexer.rs             – tokeniser
  parser.rs            – recursive-descent parser (tokens → AST)
  runtime.rs           – runtime state (variables, fields, arrays, Value type)
  field.rs             – field splitting (FS / OFS semantics)
  error.rs             – source-location-aware diagnostics (Span type)
  format/              – syntax-highlight (theme, segments) and pretty-print (AST → indented source)
  repl.rs              – interactive REPL mode
  action/
    mod.rs             – executor core, public API, pattern matching
    eval.rs            – expression evaluation, field access, assignment
    stmt.rs            – statement execution, control flow, output
    builtins_rt.rs     – builtins needing runtime (sub, gsub, match, split, stats, …)
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

Phases 0–16 complete. See [docs/progress.md](docs/progress.md) for the full
checklist and [docs/roadmap.md](docs/roadmap.md) for the performance and completeness plan.

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

# ── Diagnostics & timing ──

# Inspect a variable or array (output to stderr)
echo "hello" | fk '{ dump($0) }'

# Time a section of your program
seq 1 100000 | fk 'BEGIN{tic("sum")} {s+=$1} END{printf "sum=%d in %.3fs\n", s, toc("sum")}'

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
For more reliable numbers, use `make suite-perf-strict` which warms up,
runs multiple trials, and reports median/p90 into `bench_data/`.
See `docs/perf-baseline.md` for the latest strict baseline snapshot.

Performance varies by workload and machine. In general, fk is faster on
multi-field arithmetic and pattern-heavy workloads, while simple Unix tools
(`wc`, `head`) can remain faster for single-purpose tasks. Use the strict
perf report for current, apples-to-apples numbers.

Parquet support reads 1M rows, auto-extracts column names, and runs
pattern-action programs with named field access — no other awk can do this.

See the strict baseline report for exact timings and environment details.

## Building

```sh
# Default build (includes Parquet support)
cargo build --release

# Without Parquet (lighter binary, no arrow/parquet deps)
cargo build --release --no-default-features

# binary: target/release/fk
```

## Acknowledgments

`fk` is a love letter to **awk**, created in 1977 by Alfred Aho, Peter
Weinberger, and Brian Kernighan at Bell Labs. Their design — records, fields,
patterns, actions — remains one of the most elegant ideas in computing.

This project also leans on other things they gave us. The `printf` format
strings implemented here come straight from C, co-authored by Kernighan and
Dennis Ritchie. The regular expression theory underpinning the `regex` crate
traces back to Aho's work on finite automata. And the entire premise of `fk` — a composable text filter that reads from
stdin and writes to stdout — is the Unix philosophy that Kernighan helped
articulate. `fk` bends that philosophy a little by absorbing format readers
that purists would keep as separate tools, but the core loop is still the
same one Aho, Weinberger, and Kernighan designed almost fifty years ago.

Standing on the shoulders of giants, writing a not-so-small toy. 😄

*Yes, the name came first. "filter-kernel" is a backronym. No disrespect intended to awk or its creators — just a two-letter command that's easy to type and hard to forget.* 😁

## License

MIT
