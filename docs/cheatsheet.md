# fk cheat sheet

## Invocation

```sh
fk [options] 'program' [file ...]
fk --repl                          # interactive mode
```

| Flag | Description |
|------|-------------|
| `-F sep` | Set field separator |
| `-v var=val` | Set variable before execution |
| `-i csv` | CSV input mode (RFC 4180) |
| `-i tsv` | TSV input mode |
| `-i json` | JSON lines input mode |
| `-H` | Header mode (first line → `HDR` array) |
| `--repl` | Interactive REPL |

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
| `FS` | Input field separator |
| `OFS` | Output field separator |
| `RS` | Record separator (multi-char = regex) |
| `ORS` | Output record separator |

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
for (init; cond; step) { ... }
for (key in array) { ... }
break / continue
nextfile                # skip to next input file (fk)
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
| `match(s, pat)` | Regex match, sets RSTART/RLENGTH |
| `split(s, arr [, sep])` | Split into array, return count |

### Math
| Function | Description |
|----------|-------------|
| `int(x)` | Truncate to integer |
| `sqrt(x)` | Square root |
| `sin(x)` / `cos(x)` | Trigonometry |
| `log(x)` / `exp(x)` | Natural log / e^x |

### Time (fk extensions)
| Function | Description |
|----------|-------------|
| `systime()` | Current epoch (seconds) |
| `strftime(fmt, epoch)` | Format epoch as string |
| `mktime("Y M D H M S")` | Date string → epoch |

### I/O
| Function | Description |
|----------|-------------|
| `system(cmd)` | Run shell command, return exit status |
| `fflush()` | Flush stdout |

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
```

## REPL commands

| Command | Description |
|---------|-------------|
| `:q` / `:quit` | Exit |
| `:reset` | Clear all state |
| `:vars` | Show all variables |
