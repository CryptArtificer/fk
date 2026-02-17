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

Phases 0–8 complete. See [docs/progress.md](docs/progress.md) for the full checklist.

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
fk is faster than awk on every benchmark (1.1–3.2× faster):

| Benchmark | fk | awk | Speedup |
|---|---|---|---|
| `print $2` | 0.48 s | 0.58 s | 1.2× |
| Sum column | 0.24 s | 0.60 s | 2.5× |
| `/active/` count | 0.24 s | 0.76 s | 3.2× |
| Field arithmetic | 0.25 s | 0.62 s | 2.5× |
| Associative array | 0.28 s | 0.67 s | 2.4× |
| Computed regex | 0.34 s | 0.65 s | 1.9× |
| Tight loop (3×) | 0.70 s | 0.75 s | 1.1× |

Parquet support reads 1M rows, auto-extracts column names, and runs
pattern-action programs with named field access — no other awk can do this.

Measured on Apple M3 Pro, 36 GB RAM, macOS 26.2.
awk version 20200816 (macOS system awk). `fk` built with `--release`.

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
traces back to Aho's work on finite automata. And the entire premise of `fk` —
a small, composable text filter that reads from stdin and writes to stdout —
is the Unix philosophy that Kernighan helped articulate.

Standing on the shoulders of giants, writing a not-so-small toy. 😄

## License

MIT
