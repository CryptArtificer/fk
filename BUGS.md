# Known bugs

## `&&` / `||` between patterns in rule condition

```sh
fk '/foo/ && /bar/' file        # parse error: unexpected token: And
fk '/foo/ || /bar/' file        # same
fk '/foo/ && !/bar/' file       # same
```

awk supports boolean operators between pattern expressions in the condition position.
fk's parser rejects them.

Likely related: `!/regex/` (negated match) in compound pattern expressions.

### Repro

```sh
echo "hello world" | fk '/hello/ && /world/'
# expected: hello world
# actual:   fk: parse error: 1:9: unexpected token: And
```

### Workaround

Use `if` inside the action block:

```sh
fk '/hello/ { if (/world/) print }' file
```

## Verify regex escape sequences

`\t`, `\n`, `\s`, `\d` etc. inside regex character classes — do they all work?
Quick test passed for `\t` in `/^[^\t ]/` but no systematic coverage exists.
Add tests for common escapes in regex patterns, especially inside `[]` character classes.

---

# POSIX AWK compatibility gaps

Verified 2026-04-15 against `target/release/fk`. Items marked ✅ already work.

## Regex handling

| # | Gap | Severity | Status |
|---|-----|----------|--------|
| 1 | `sub()`/`gsub()` treat string patterns as literal, not regex — `sub("[0-9]+", "NUM")` matches the literal text `[0-9]+`. `gensub()` uses regex correctly. | Critical | Open |
| 2 | `&` in sub/gsub replacement is literal — `gsub("b", "[&]")` produces `a[&]c` not `a[b]c`. POSIX requires `&` to mean the matched text. | Critical | Open |
| 3 | ~~Regex literals in function arguments~~ | — | ✅ Works — `sub(/t/, "T")` parses and runs correctly |
| 4 | `split()` separator is literal, not ERE — `split("a1b2c", a, "[0-9]")` returns 1 element | Critical | Open |
| 5 | Multi-char FS is literal, not ERE — `-F'[,;]'` treats pattern as 4-char literal | Critical | Open |

## Record and field semantics

| # | Gap | Severity | Status |
|---|-----|----------|--------|
| 6 | Single-char RS other than `\n` not implemented — `-v RS=:` does not split on `:` | Critical | Open |
| 7 | `RS=""` paragraph mode not supported — fk still splits on `\n` | High | Open |
| 8 | Assigning to NF doesn't rebuild `$0` — `NF=5` on a 3-field record should extend with empty fields | High | Open |
| 9 | Arrays can't be passed to user functions by reference — `function f(a) { a[1]="x" }` doesn't modify caller's array. POSIX requires pass-by-reference for arrays. | Critical | Open |

## Operators and literals

| # | Gap | Severity | Status |
|---|-----|----------|--------|
| 10 | `^` exponentiation not supported — POSIX specifies `^`, fk only has `**` | High | Open |
| 11 | `^=` compound assignment not supported | Medium | Open |
| 12 | Unary `+` not supported — `print +$1` is a parse error | Medium | Open |
| 13 | Scientific notation `1.5e3` parsed as `1.5` then identifier `e3` | Medium | Open |
| 14 | Octal escape `\NNN` in strings not supported — `"\101"` is literal | Medium | Open |
| 15 | String literals missing `\r`, `\a`, `\b`, `\f`, `\v` escapes | Medium | Open |
| 16 | printf missing `\r`, `\a`, `\b`, `\f`, `\v` in format strings | Medium | Open |
| 17 | printf missing `%X`, `%E`, `%G` uppercase format specifiers | Medium | Open |

## getline semantics

| # | Gap | Severity | Status |
|---|-----|----------|--------|
| 18 | ~~`getline < file` incorrectly increments NR~~ | — | ✅ Works correctly |
| 19 | ~~`cmd \| getline` incorrectly increments NR~~ | — | Needs retest |
| 20 | getline from current input during record processing — needs investigation | Medium | Open |

## Other

| # | Gap | Severity | Status |
|---|-----|----------|--------|
| 21 | Variable assignments between file arguments — `fk 'prog' var=val file` treats `var=val` as filename | Medium | Open |
| 22 | ~~`(expr)` in array context~~ — `print (1 in arr)` | — | ✅ Works |
| 23 | ~~CONVFMT not applied during coercion~~ | — | ✅ Works |
| 24 | OFMT not applied during `print` of computed numbers | Low | Open |

## Priority order

Fix first (standard awk programs break in fk):
1. sub/gsub must use regex + support `&` in replacement
2. Array pass-by-reference in user functions
3. Multi-char FS as ERE + split() with regex separator
4. Single-char RS ≠ `\n` + RS="" paragraph mode
5. `^` operator (alias for `**`)

Fix next (broad compatibility):
6. String/printf escape sequences (`\r`, `\a`, `\b`, `\f`, `\v`, `\NNN`)
7. Scientific notation in number literals
8. NF assignment rebuilding `$0`
9. Printf uppercase format specifiers (`%X`, `%E`, `%G`)
10. Unary plus, `^=`
11. Variable assignments between file arguments

---

# gawk extension gaps

Not POSIX-required. Implement if fk users ask for them.

| Gap | Detail | Demand |
|-----|--------|--------|
| `**=` compound power assignment | `x **= 2` is a parse error | Moderate |
| `\|&` two-way pipes (coprocess) | Syntax error | Moderate |
| `patsplit(string, array, fieldpat)` | Not implemented | Moderate |
| FPAT (field pattern) | fk has native CSV which covers the main use case | Low |
| FIELDWIDTHS | Fixed-width field splitting | Low |
| PROCINFO array | Process info | Low |
| `@include` directive | Source file inclusion | Low |
| `isarray(x)` | fk has `typeof()` which covers this | Low |
| Networking (`/inet/tcp/...`) | TCP/IP sockets | Low |

---

# Architecture: binary size and startup

fk statically links parquet/arrow/zstd, which adds ~5MB to the binary (7.6MB vs 2.5MB core-only). The OS pages in the full binary on exec, costing ~10ms of startup overhead vs awk/gawk. Compute performance is on par.

**Proposed:** split into `fk` (core, ~2.5MB) and `fk-full` or a dynamically loaded plugin for parquet/arrow. Core binary matches awk startup. Build with `cargo build --no-default-features` to test core-only today.
