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
  main.rs      – entry point, orchestration
  cli.rs       – command-line argument parsing (-F, -v, program, files)
  input.rs     – record-oriented reading (files, stdin, "-")
  field.rs     – field splitting (FS / OFS semantics)
  lexer.rs     – tokeniser for the fk language
  parser.rs    – recursive-descent parser (tokens → AST)
  action.rs    – executor: pattern matching, statements, expressions, builtins
  runtime.rs   – runtime state (variables, fields, associative arrays)
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
- [ ] Better error messages with source locations
- [ ] Unicode-aware field splitting
- [ ] JSON / CSV input modes (module)
- [ ] Plugin / module system for user-defined extensions
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
