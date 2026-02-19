# fk cheat sheet

## Invocation

```sh
fk [options] 'program' [file ...]
fk [options] file ...              # defaults to { print }
fk -f progfile [file ...]          # read program from file
fk --describe [file ...]           # sniff format, show schema & examples
fk --suggest  [file ...]           # schema + smart tailored programs
fk --repl                          # interactive mode
fk --highlight 'program'           # syntax-highlighted program and exit
fk --format    'program'           # pretty-print program and exit
fk --help / fk --version
```

| Flag | Description |
|------|-------------|
| `-F sep` | Set field separator |
| `-f file` | Read program from file |
| `-v var=val` | Set variable before execution |
| `-i csv` | CSV input mode (RFC 4180) |
| `-i tsv` | TSV input mode |
| `-i json` | JSON lines input mode |
| `-i parquet` | Apache Parquet input |
| `-H` | Header mode (first line → `HDR` array + named columns) |
| `-d`, `--describe` | Describe mode: detect format, infer schema, suggest programs |
| `-S`, `--suggest` | Suggest mode: schema + smart copy-pasteable programs |
| `--repl` | Interactive REPL |
| `--highlight` | Syntax-highlight program and exit |
| `--format` | Pretty-print program and exit |

## Program structure

```
BEGIN { ... }          # runs once before input
/pattern/ { ... }      # runs for matching lines
{ ... }                # runs for every line
END { ... }            # runs once after input
```

## Fields and variables

| Expression | Meaning |
|------------|---------|
| `$0` | Entire record |
| `$1`, `$2`, ... | Fields by position |
| `$NF` | Last field |
| `$-1` | Last field (fk extension) |
| `$-2` | Second-to-last |
| `$(expr)` | Computed field index |
| `NR` | Record number (across all files) |
| `NF` | Number of fields in current record |
| `FNR` | Record number in current file |
| `FILENAME` | Current input file name |
| `FS` | Input field separator |
| `OFS` | Output field separator |
| `RS` | Record separator (multi-char = regex) |
| `ORS` | Output record separator |
| `SUBSEP` | Subscript separator (default `\x1c`) |
| `OFMT` | Number output format (default `"%.6g"`) |
| `ENVIRON` | Array of environment variables |
| `ARGC` / `ARGV` | Argument count and values |

## Patterns

```
/regex/                # regex match on $0
$2 == "x"              # expression
$1 > 0 && $2 ~ /pat/  # compound
/start/,/stop/         # range (inclusive)
```

## Operators

| Op | Description |
|----|-------------|
| `+` `-` `*` `/` `%` | Arithmetic |
| `**` | Exponentiation (fk) |
| `++` `--` | Increment / decrement |
| `=` `+=` `-=` `*=` `/=` `%=` | Assignment |
| `==` `!=` `<` `>` `<=` `>=` | Comparison |
| `~` `!~` | Regex match / not match |
| `&&` `\|\|` `!` | Logical |
| `? :` | Ternary |
| `string string` | Concatenation (implicit) |

## Control flow

```
if (cond) { ... } else { ... }
while (cond) { ... }
do { ... } while (cond)
for (init; cond; step) { ... }
for (key in array) { ... }
break / continue
next                           # skip to next record
exit / exit(code)              # run END block, then exit
nextfile                       # skip to next input file (fk)
```

## Output

```
print expr, expr        # print with OFS, ends with ORS
printf fmt, args        # formatted (no trailing newline)
sprintf(fmt, args)      # formatted → string

print ... > "file"      # overwrite file
print ... >> "file"     # append to file
print ... | "cmd"       # pipe to command
print ... > "/dev/stderr"   # write to stderr
```

## Built-in functions

### Strings
| Function | Description |
|----------|-------------|
| `length(s)` | Character count (unicode-aware) |
| `substr(s, start [, len])` | Substring (1-indexed, unicode-aware) |
| `index(s, target)` | Position of target in s (unicode-aware) |
| `tolower(s)` / `toupper(s)` | Case conversion |
| `sub(pat, repl [, target])` | Replace first match |
| `gsub(pat, repl [, target])` | Replace all matches |
| `match(s, pat [, arr])` | Regex match, sets RSTART/RLENGTH. With `arr`: capture groups |
| `split(s, arr [, sep])` | Split into array, return count |
| `gensub(re, repl, how [, target])` | Like gsub but returns result (doesn't modify target) |
| `trim(s)` | Strip leading and trailing whitespace |
| `ltrim(s)` / `rtrim(s)` | Strip leading / trailing whitespace |
| `startswith(s, prefix)` | Returns 1 if s starts with prefix |
| `endswith(s, suffix)` | Returns 1 if s ends with suffix |
| `repeat(s, n)` | Repeat string n times |
| `reverse(s)` | Reverse a string (unicode-aware) |
| `chr(n)` / `ord(s)` | Character ↔ codepoint |
| `hex(n)` | Format number as hexadecimal (0x...) |
| `lpad(s, width [, char])` | Left-pad to width (default: space) |
| `rpad(s, width [, char])` | Right-pad to width (default: space) |

### Math
| Function | Description |
|----------|-------------|
| `int(x)` | Truncate to integer |
| `sqrt(x)` | Square root |
| `sin(x)` / `cos(x)` | Trigonometry |
| `log(x)` / `exp(x)` | Natural log / e^x |
| `atan2(y, x)` | Arc tangent |
| `abs(x)` | Absolute value |
| `ceil(x)` / `floor(x)` / `round(x)` | Rounding |
| `min(a, b)` / `max(a, b)` | Minimum / maximum |
| `log2(x)` / `log10(x)` | Base-2 / base-10 logarithm |
| `rand()` | Random number 0..1 |
| `srand([seed])` | Seed the RNG |

### Time (fk extensions)
| Function | Description |
|----------|-------------|
| `systime()` | Current epoch (seconds) |
| `strftime(fmt, epoch)` | Format epoch as string |
| `mktime("Y M D H M S")` | Date string → epoch |
| `parsedate(str, fmt)` | Parse date string → epoch |

### I/O
| Function | Description |
|----------|-------------|
| `system(cmd)` | Run shell command, return exit status |
| `fflush()` | Flush stdout |
| `close(name)` | Close an output file or pipe |
| `slurp(file)` | Read entire file into string |
| `slurp(file, arr)` | Read file lines into array, return count |

### Arrays (fk extensions)
| Function | Description |
|----------|-------------|
| `print arr` | Smart print: values (sequential) or keys (associative) |
| `keys(arr)` | Sorted keys as string (joined by ORS) |
| `vals(arr)` | Values sorted by key as string (joined by ORS) |
| `asort(arr)` | Sort by values, re-key 1..N |
| `asorti(arr)` | Sort by keys, store as values 1..N |
| `join(arr [, sep])` | Join array values into string (default: OFS) |
| `uniq(arr)` | Deduplicate values, re-key 1..N |
| `inv(arr)` | Swap keys ↔ values |
| `tidy(arr)` | Remove empty/zero entries |
| `shuf(arr)` | Randomize order, re-key 1..N |
| `diff(a, b)` | Set difference: remove from `a` keys in `b` |
| `inter(a, b)` | Set intersection: keep in `a` only keys also in `b` |
| `union(a, b)` | Set union: merge keys from `b` into `a` |
| `seq(arr, from, to)` | Fill with integer range, re-key 1..N |
| `samp(arr, n)` | Random n elements, re-key 1..n |

### Statistics (fk extensions)
| Function | Description |
|----------|-------------|
| `sum(arr)` | Sum of all values |
| `mean(arr)` | Arithmetic mean |
| `median(arr)` | Median (50th percentile) |
| `stddev(arr)` | Population standard deviation |
| `variance(arr)` | Population variance |
| `hist(arr, bins [, out [, min [, max]]])` | Histogram counts (writes to `out`, returns bin count) |
| `p(arr, n)` / `percentile(arr, n)` | nth percentile (0–100) |
| `quantile(arr, q)` | Quantile (0–1, e.g. 0.95 = p95) |
| `iqm(arr)` | Interquartile mean (robust to outliers) |
| `min(arr)` / `max(arr)` | Min / max of array values |

### Utility (fk extensions)
| Function | Description |
|----------|-------------|
| `typeof(x)` | `"number"`, `"string"`, `"array"`, or `"uninitialized"` |

### Bitwise (fk extensions)
| Function | Description |
|----------|-------------|
| `and(a, b)` / `or(a, b)` / `xor(a, b)` | Bitwise AND, OR, XOR |
| `lshift(a, n)` / `rshift(a, n)` | Bit shift |
| `compl(a)` | Bitwise complement |

### JSON (fk extensions)
| Function | Description |
|----------|-------------|
| `jpath(json, path)` | Extract value at path |
| `jpath(json, path, arr)` | Extract into array, return count |

**jpath paths:** `.key`, `[N]`, `.key.sub`, `.arr[]`, `.arr.key` (implicit iteration)

## Arrays

```
arr[key] = value        # set
x = arr[key]            # get
delete arr[key]         # delete element
delete arr              # delete entire array (fk)
length(arr)             # element count (fk)
for (k in arr) { ... }  # iterate keys
if (key in arr) { ... } # membership test
a[i,j] = value          # multi-dimensional (uses SUBSEP)
```

## User-defined functions

```
function name(params) {
    ...
    return value
}
```

## Numeric literals

```
42                      # decimal
3.14                    # float
0xFF                    # hex (fk extension)
```

## String escapes

```
\n \t \r \\  \"         # standard
\xHH                    # hex byte (fk)
\uHHHH                  # unicode codepoint (fk)
```

## One-liner recipes

```sh
# Print second column
fk '{ print $2 }' file

# Sum a column
fk '{ s += $1 } END { print s }' file

# Count lines matching pattern
fk '/error/ { c++ } END { print c }' log.txt

# Unique values in column 3
fk '!seen[$3]++ { print $3 }' file

# Frequency count
fk '{ a[$1]++ } END { for (k in a) print a[k], k }' file

# CSV to TSV
fk -i csv -v 'OFS=\t' '{ print $1, $2, $3 }' data.csv

# Top spenders from JSON logs
fk -i json '$1 == "purchase" { t[$2] += $3 } END { for (u in t) print u, t[u] }' events.jsonl

# Navigate nested JSON
fk '{ print jpath($0, ".results[].name") }' response.json

# Last field of every line
fk '{ print $-1 }' file

# Square every number
fk '{ print $1 ** 2 }' numbers.txt

# Read program from file
fk -f script.awk data.txt

# Print FILENAME and FNR per file
fk '{ print FILENAME, FNR, $0 }' f1.txt f2.txt

# Access environment
fk 'BEGIN { print ENVIRON["HOME"] }'

# gensub — non-destructive replacement
fk '{ print gensub("[0-9]+", "NUM", "g") }' file

# do-while loop
echo 5 | fk '{ n=$1; do { print n-- } while (n>0) }'

# exit early with code
fk '{ if ($0 == "STOP") exit(1); print }' file

# Two-file join using FNR == NR
fk 'NR==FNR { a[$1]=$2; next } ($1 in a) { print $1, a[$1], $2 }' ref.txt data.txt

# ── Parquet and named columns ──

# Query parquet by column name
fk -i parquet '$age > 30 { print $name }' users.parquet

# Quoted column names (special characters, hyphens, dots)
fk -i parquet '{ print $"user-name", $"total.revenue" }' data.parquet

# Column name in a variable
fk -i parquet 'BEGIN { col="user-name" } { print $col }' data.parquet

# Aggregate parquet data
fk -i parquet '{ dept[$department] += $revenue } END { for (d in dept) print d, dept[d] }' sales.parquet

# CSV with named columns
fk -F, -H '$status == "active" { print $email }' users.csv

# ── Phase 8 signatures ──

# Capture groups
echo "2025-01-15" | fk '{ match($0, "([0-9]+)-([0-9]+)-([0-9]+)", c); print c[1] }'

# Sort + join
fk '{ a[NR]=$1 } END { asort(a); print join(a, ",") }' file

# Bitwise flags
fk '{ if (and($1, 0x04)) print "flag set:", $0 }' file

# Parse dates
fk -F, -H '{ ts = parsedate($created, "%Y-%m-%d"); if (ts > 1700000000) print $name }' data.csv

# ── Statistics ──

# Quick summary stats
fk '{ a[NR]=$1 } END { printf "n=%d mean=%.2f median=%.2f stddev=%.2f\n", length(a), mean(a), median(a), stddev(a) }' data.txt

# p95 latency
fk '{ a[NR]=$3 } END { print "p95:", p(a, 95) }' latency.log

# Interquartile mean (outlier-robust average)
fk '{ a[NR]=$1 } END { print "iqm:", iqm(a) }' measurements.txt

# Zero-padded output
fk '{ printf "%08d\n", $1 }' ids.txt

# ── Array operations (fk-only) ──

# Quick view of a CSV
fk data.csv
fk -H -v 'OFS=\t' data.csv

# Sorted unique values
fk '{ u[$1]++ } END { print u }' file

# Set difference — users in a.txt but not b.txt
fk 'NR==FNR{a[$1];next}{b[$1]} END { diff(a,b); print a }' a.txt b.txt

# Deduplicate array values
fk '{ a[NR]=$1 } END { uniq(a); print a }' file

# Random sample of 10 lines
fk '{ a[NR]=$0 } END { samp(a, 10); print a }' file

# Generate a sequence and shuffle
fk 'BEGIN { seq(a, 1, 52); shuf(a); print a }'

# Read a lookup file, then enrich
fk 'BEGIN { slurp("lookup.csv", lu) } { print $0, lu[$1] }' data.txt

# Left-padded table
fk '{ print lpad($1, 12), rpad($2, 20), $3 }' report.txt

# Invert a mapping
fk 'BEGIN { a["US"]="United States"; a["UK"]="United Kingdom"; inv(a); print a }'
```

## REPL commands

| Command | Description |
|---------|-------------|
| `:q` / `:quit` | Exit |
| `:reset` | Clear all state |
| `:vars` | Show all variables |
