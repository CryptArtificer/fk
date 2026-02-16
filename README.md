# fk — filter-kernel

A modernized, modular awk clone built in Rust.

## Intent

`fk` aims to replicate the core text-processing model of awk — read input, split into records and fields, match patterns, execute actions — while providing a cleaner foundation that is easy to extend. Each category of functionality (I/O, field splitting, pattern matching, expressions, built-in functions, etc.) lives in its own module so new capabilities can be added without touching unrelated code.

### Design principles

- **Modular** — every concern is a separate module behind a clear interface.
- **Minimal dependencies** — lean on the Rust standard library; pull in crates only when they genuinely earn their keep.
- **Incremental** — built in deliberate steps, each one leaving a usable tool.

## Architecture (planned)

```
src/
  main.rs          – entry point, arg parsing, orchestration
  cli.rs           – command-line argument definitions
  input.rs         – record-oriented reading (files, stdin)
  field.rs         – field splitting (FS / OFS)
  pattern.rs       – pattern matching (string, regex, ranges)
  action.rs        – action execution (print, assignment, …)
  expr.rs          – expression evaluation
  program.rs       – parsed program representation (rules, BEGIN/END)
  lexer.rs         – tokeniser for the fk language
  parser.rs        – parser (tokens → program AST)
  runtime.rs       – runtime state (variables, NR, NF, etc.)
  builtins/        – built-in function modules (math, string, I/O, …)
```

## Progress

### Phase 0 — Skeleton & essentials
- [ ] Argument handling (`-F`, `-v`, `'program'`, file operands)
- [ ] Read lines from stdin and file arguments
- [ ] Split records into fields (`$0`, `$1` … `$NF`)
- [ ] Print action (bare `{ print }` / `{ print $N }`)
- [ ] Field separator (`-F` flag and `FS` variable)
- [ ] Basic pattern matching (string equality, `/regex/`)
- [ ] `BEGIN` and `END` blocks
- [ ] Built-in variables: `NR`, `NF`, `FS`, `OFS`, `RS`, `ORS`

### Phase 1 — Language core
- [ ] Arithmetic & string expressions
- [ ] Variable assignment
- [ ] `if` / `else`
- [ ] `while`, `for`, `for-in`
- [ ] User-defined variables and associative arrays
- [ ] `printf` / `sprintf`

### Phase 2 — Full awk compatibility
- [ ] User-defined functions
- [ ] Getline variants
- [ ] Output redirection (`>`, `>>`, `|`)
- [ ] All POSIX awk built-in functions (substr, index, split, sub, gsub, match, length, tolower, toupper, …)
- [ ] Multiple rule support, pattern ranges (`/start/,/stop/`)
- [ ] Uninitialized variable semantics (0 / "")
- [ ] Concatenation as implicit operator
- [ ] Coercion rules (string ↔ number)

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
