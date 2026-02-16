use crate::{action, lexer, parser, runtime};

/// Helper: parse and run a program, return the runtime state for inspection.
fn eval(program_text: &str, input_lines: &[&str]) -> runtime::Runtime {
    let mut lex = lexer::Lexer::new(program_text);
    let tokens = lex.tokenize().expect("lexer error");
    let mut par = parser::Parser::new(tokens);
    let program = par.parse().expect("parse error");

    let mut rt = runtime::Runtime::new();
    let mut exec = action::Executor::new(&program, &mut rt);

    exec.run_begin();
    for line in input_lines {
        exec.run_record(line);
    }
    exec.run_end();

    rt
}

// ── User-defined functions: parsing ──────────────────────────────

#[test]
fn parse_simple_function_definition() {
    let src = "function greet(name) { print name }";
    let mut lex = lexer::Lexer::new(src);
    let tokens = lex.tokenize().unwrap();
    let mut par = parser::Parser::new(tokens);
    let prog = par.parse().unwrap();

    assert_eq!(prog.functions.len(), 1);
    assert_eq!(prog.functions[0].name, "greet");
    assert_eq!(prog.functions[0].params, vec!["name"]);
}

#[test]
fn parse_function_with_multiple_params() {
    let src = "function add(a, b, c) { return a + b + c }";
    let mut lex = lexer::Lexer::new(src);
    let tokens = lex.tokenize().unwrap();
    let mut par = parser::Parser::new(tokens);
    let prog = par.parse().unwrap();

    assert_eq!(prog.functions[0].params, vec!["a", "b", "c"]);
}

#[test]
fn parse_function_with_no_params() {
    let src = "function hello() { print \"hi\" }";
    let mut lex = lexer::Lexer::new(src);
    let tokens = lex.tokenize().unwrap();
    let mut par = parser::Parser::new(tokens);
    let prog = par.parse().unwrap();

    assert_eq!(prog.functions[0].name, "hello");
    assert!(prog.functions[0].params.is_empty());
}

#[test]
fn parse_function_alongside_rules() {
    let src = "function double(x) { return x * 2 } { print double($1) }";
    let mut lex = lexer::Lexer::new(src);
    let tokens = lex.tokenize().unwrap();
    let mut par = parser::Parser::new(tokens);
    let prog = par.parse().unwrap();

    assert_eq!(prog.functions.len(), 1);
    assert_eq!(prog.rules.len(), 1);
}

#[test]
fn parse_return_without_value() {
    let src = "function noop() { return }";
    let mut lex = lexer::Lexer::new(src);
    let tokens = lex.tokenize().unwrap();
    let mut par = parser::Parser::new(tokens);
    let prog = par.parse().unwrap();

    assert_eq!(prog.functions[0].name, "noop");
}

// ── User-defined functions: execution ────────────────────────────

#[test]
fn function_returns_computed_value() {
    let rt = eval(
        "function double(x) { return x * 2 } { result = double($1) }",
        &["5"],
    );
    assert_eq!(rt.get_var("result"), "10");
}

#[test]
fn function_with_multiple_args() {
    let rt = eval(
        "function add(a, b) { return a + b } { result = add($1, $2) }",
        &["3 7"],
    );
    assert_eq!(rt.get_var("result"), "10");
}

#[test]
fn function_params_are_local_and_restored() {
    let rt = eval(
        "BEGIN { x = 99 } function f(x) { return x + 1 } { result = f(5) }",
        &["ignored"],
    );
    // The function should not clobber the global x
    assert_eq!(rt.get_var("x"), "99");
    assert_eq!(rt.get_var("result"), "6");
}

#[test]
fn function_with_no_return_gives_empty_string() {
    let rt = eval(
        "function noop() { x = 1 } { result = noop() }",
        &["anything"],
    );
    assert_eq!(rt.get_var("result"), "");
}

#[test]
fn function_early_return_skips_remaining_body() {
    let rt = eval(
        "function f(x) { if (x > 0) return \"pos\"; return \"non-pos\" } { result = f($1) }",
        &["5"],
    );
    assert_eq!(rt.get_var("result"), "pos");
}

#[test]
fn function_early_return_non_positive() {
    let rt = eval(
        "function f(x) { if (x > 0) return \"pos\"; return \"non-pos\" } { result = f($1) }",
        &["-3"],
    );
    assert_eq!(rt.get_var("result"), "non-pos");
}

#[test]
fn recursive_function_factorial() {
    let rt = eval(
        "function fact(n) { if (n <= 1) return 1; return n * fact(n - 1) } { result = fact($1) }",
        &["6"],
    );
    assert_eq!(rt.get_var("result"), "720");
}

#[test]
fn function_can_access_global_variables() {
    let rt = eval(
        "function f() { return g } BEGIN { g = 42 } { result = f() }",
        &["x"],
    );
    assert_eq!(rt.get_var("result"), "42");
}

#[test]
fn function_can_modify_global_variables() {
    let rt = eval(
        "function bump() { total += 10 } { bump() }",
        &["a", "b", "c"],
    );
    // bump() called 3 times, each adding 10
    assert_eq!(rt.get_var("total"), "30");
}

#[test]
fn function_missing_args_default_to_empty() {
    let rt = eval(
        "function f(a, b) { return a + b } { result = f(5) }",
        &["x"],
    );
    // b defaults to "" which coerces to 0
    assert_eq!(rt.get_var("result"), "5");
}

#[test]
fn multiple_functions_defined() {
    let rt = eval(
        "function sq(x) { return x*x } function cube(x) { return x*x*x } { s = sq($1); c = cube($1) }",
        &["3"],
    );
    assert_eq!(rt.get_var("s"), "9");
    assert_eq!(rt.get_var("c"), "27");
}
