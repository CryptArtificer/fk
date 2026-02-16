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

// ── Ternary operator ─────────────────────────────────────────────

#[test]
fn ternary_selects_true_branch() {
    let rt = eval("{ result = (1 > 0) ? \"yes\" : \"no\" }", &["x"]);
    assert_eq!(rt.get_var("result"), "yes");
}

#[test]
fn ternary_selects_false_branch() {
    let rt = eval("{ result = (0 > 1) ? \"yes\" : \"no\" }", &["x"]);
    assert_eq!(rt.get_var("result"), "no");
}

#[test]
fn ternary_with_field_values() {
    let rt = eval("{ result = ($1 > 10) ? \"big\" : \"small\" }", &["25"]);
    assert_eq!(rt.get_var("result"), "big");
}

#[test]
fn ternary_nested() {
    let rt = eval(
        "{ result = ($1 > 0) ? \"pos\" : ($1 == 0) ? \"zero\" : \"neg\" }",
        &["-5"],
    );
    assert_eq!(rt.get_var("result"), "neg");
}

#[test]
fn ternary_as_function_argument() {
    let rt = eval(
        "function f(x) { return x } { result = f($1 > 0 ? 1 : -1) }",
        &["42"],
    );
    assert_eq!(rt.get_var("result"), "1");
}

// ── Coercion rules ───────────────────────────────────────────────

#[test]
fn uninitialized_var_is_zero_in_arithmetic() {
    let rt = eval("{ result = x + 5 }", &["ignored"]);
    assert_eq!(rt.get_var("result"), "5");
}

#[test]
fn uninitialized_var_is_empty_string_in_print_context() {
    let rt = eval("{ result = x }", &["ignored"]);
    assert_eq!(rt.get_var("result"), "");
}

#[test]
fn numeric_strings_compared_as_numbers() {
    // "10" > "9" numerically (but "10" < "9" lexicographically)
    let rt = eval("{ result = (\"10\" > \"9\") }", &["x"]);
    assert_eq!(rt.get_var("result"), "1");
}

#[test]
fn non_numeric_strings_compared_lexicographically() {
    let rt = eval("{ result = (\"banana\" > \"apple\") }", &["x"]);
    assert_eq!(rt.get_var("result"), "1");
}

#[test]
fn mixed_comparison_is_string_based() {
    // "abc" is not numeric, so compare as strings
    let rt = eval("{ result = (\"abc\" > \"10\") }", &["x"]);
    assert_eq!(rt.get_var("result"), "1"); // "abc" > "10" lexicographically
}

#[test]
fn string_coerces_to_zero_in_arithmetic() {
    let rt = eval("{ result = \"hello\" + 5 }", &["x"]);
    assert_eq!(rt.get_var("result"), "5");
}

#[test]
fn leading_number_prefix_parsed() {
    let rt = eval("{ result = \"123abc\" + 0 }", &["x"]);
    assert_eq!(rt.get_var("result"), "123");
}

#[test]
fn equality_of_same_numeric_different_format() {
    // "1.0" == "1" should be true (both numeric)
    let rt = eval("{ result = (\"1.0\" == \"1\") }", &["x"]);
    assert_eq!(rt.get_var("result"), "1");
}

#[test]
fn inequality_of_different_strings() {
    let rt = eval("{ result = (\"foo\" != \"bar\") }", &["x"]);
    assert_eq!(rt.get_var("result"), "1");
}

#[test]
fn empty_string_is_falsy() {
    let rt = eval("{ result = (\"\" ? \"truthy\" : \"falsy\") }", &["x"]);
    assert_eq!(rt.get_var("result"), "falsy");
}

#[test]
fn zero_string_is_falsy() {
    let rt = eval("{ result = (\"0\" ? \"truthy\" : \"falsy\") }", &["x"]);
    assert_eq!(rt.get_var("result"), "falsy");
}

#[test]
fn nonzero_number_string_is_truthy() {
    let rt = eval("{ result = (\"1\" ? \"truthy\" : \"falsy\") }", &["x"]);
    assert_eq!(rt.get_var("result"), "truthy");
}

// ── Built-in functions: split ────────────────────────────────────

#[test]
fn split_basic_whitespace() {
    let rt = eval(
        "{ n = split($0, a); result = n; first = a[1]; second = a[2] }",
        &["hello world"],
    );
    assert_eq!(rt.get_var("result"), "2");
    assert_eq!(rt.get_array("a", "1"), "hello");
    assert_eq!(rt.get_array("a", "2"), "world");
}

#[test]
fn split_with_custom_separator() {
    let rt = eval(
        "{ n = split($0, a, \":\"); result = n; second = a[2] }",
        &["one:two:three"],
    );
    assert_eq!(rt.get_var("result"), "3");
    assert_eq!(rt.get_array("a", "1"), "one");
    assert_eq!(rt.get_array("a", "2"), "two");
    assert_eq!(rt.get_array("a", "3"), "three");
}

#[test]
fn split_clears_previous_array_contents() {
    let rt = eval(
        "{ split(\"a:b:c\", arr, \":\"); split(\"x:y\", arr, \":\"); result = arr[3] }",
        &["x"],
    );
    // After second split, arr[3] should be empty (cleared)
    assert_eq!(rt.get_var("result"), "");
}

// ── Built-in functions: sub / gsub ───────────────────────────────

#[test]
fn sub_replaces_first_occurrence_in_dollar_zero() {
    let rt = eval("{ sub(\"world\", \"earth\"); result = $0 }", &["hello world world"]);
    assert_eq!(rt.get_var("result"), "hello earth world");
}

#[test]
fn gsub_replaces_all_occurrences_in_dollar_zero() {
    let rt = eval("{ gsub(\"o\", \"0\"); result = $0 }", &["foo boo"]);
    assert_eq!(rt.get_var("result"), "f00 b00");
}

#[test]
fn sub_on_named_variable() {
    let rt = eval("{ x = \"aabbcc\"; sub(\"bb\", \"BB\", x); result = x }", &["z"]);
    assert_eq!(rt.get_var("result"), "aaBBcc");
}

#[test]
fn gsub_returns_replacement_count() {
    let rt = eval("{ n = gsub(\"a\", \"x\"); result = n }", &["banana"]);
    assert_eq!(rt.get_var("result"), "3");
}

#[test]
fn sub_no_match_returns_zero() {
    let rt = eval("{ n = sub(\"xyz\", \"!\"); result = n }", &["hello"]);
    assert_eq!(rt.get_var("result"), "0");
}

// ── Built-in functions: match ────────────────────────────────────

#[test]
fn match_finds_pattern_sets_rstart_rlength() {
    let rt = eval("{ match($0, \"wor\"); rs = RSTART; rl = RLENGTH }", &["hello world"]);
    assert_eq!(rt.get_var("rs"), "7");
    assert_eq!(rt.get_var("rl"), "3");
}

#[test]
fn match_no_match_returns_zero() {
    let rt = eval("{ result = match($0, \"xyz\"); rl = RLENGTH }", &["hello"]);
    assert_eq!(rt.get_var("result"), "0");
    assert_eq!(rt.get_var("rl"), "-1");
}

#[test]
fn match_at_start_of_string() {
    let rt = eval("{ result = match($0, \"hel\") }", &["hello"]);
    assert_eq!(rt.get_var("result"), "1");
}

// ── Pattern ranges ───────────────────────────────────────────────

#[test]
fn range_pattern_includes_start_and_stop_lines() {
    let rt = eval(
        "/START/,/STOP/ { count++ }",
        &["before", "START", "middle", "STOP", "after"],
    );
    assert_eq!(rt.get_var("count"), "3"); // START, middle, STOP
}

#[test]
fn range_pattern_not_active_before_start() {
    let rt = eval(
        "/BEGIN_RANGE/,/END_RANGE/ { count++ }",
        &["nothing", "here", "BEGIN_RANGE", "inside", "END_RANGE", "outside"],
    );
    assert_eq!(rt.get_var("count"), "3"); // BEGIN_RANGE, inside, END_RANGE
}

#[test]
fn range_pattern_can_reactivate() {
    let rt = eval(
        "/ON/,/OFF/ { count++ }",
        &["x", "ON", "a", "OFF", "x", "ON", "b", "OFF", "x"],
    );
    assert_eq!(rt.get_var("count"), "6"); // ON,a,OFF + ON,b,OFF
}

#[test]
fn range_stays_active_if_stop_never_seen() {
    let rt = eval(
        "/START/,/STOP/ { count++ }",
        &["START", "a", "b", "c"],
    );
    assert_eq!(rt.get_var("count"), "4"); // all lines from START onward
}

// ── Output redirection: parsing ──────────────────────────────────

#[test]
fn parse_print_with_overwrite_redirect() {
    let mut lex = lexer::Lexer::new("{ print $0 > \"out.txt\" }");
    let tokens = lex.tokenize().unwrap();
    let mut par = parser::Parser::new(tokens);
    let prog = par.parse().unwrap();
    assert_eq!(prog.rules.len(), 1);
    // Just verify it parses without error
}

#[test]
fn parse_print_with_append_redirect() {
    let mut lex = lexer::Lexer::new("{ print $0 >> \"out.txt\" }");
    let tokens = lex.tokenize().unwrap();
    let mut par = parser::Parser::new(tokens);
    let prog = par.parse().unwrap();
    assert_eq!(prog.rules.len(), 1);
}

#[test]
fn parse_print_with_pipe_redirect() {
    let mut lex = lexer::Lexer::new("{ print $0 | \"sort\" }");
    let tokens = lex.tokenize().unwrap();
    let mut par = parser::Parser::new(tokens);
    let prog = par.parse().unwrap();
    assert_eq!(prog.rules.len(), 1);
}

#[test]
fn parse_printf_with_redirect() {
    let mut lex = lexer::Lexer::new("{ printf \"%s\\n\", $1 > \"out.txt\" }");
    let tokens = lex.tokenize().unwrap();
    let mut par = parser::Parser::new(tokens);
    let prog = par.parse().unwrap();
    assert_eq!(prog.rules.len(), 1);
}

// ── Getline: parsing ─────────────────────────────────────────────

#[test]
fn parse_getline_simple() {
    let mut lex = lexer::Lexer::new("{ getline }");
    let tokens = lex.tokenize().unwrap();
    let mut par = parser::Parser::new(tokens);
    let prog = par.parse().unwrap();
    assert_eq!(prog.rules.len(), 1);
}

#[test]
fn parse_getline_into_var() {
    let mut lex = lexer::Lexer::new("{ getline line }");
    let tokens = lex.tokenize().unwrap();
    let mut par = parser::Parser::new(tokens);
    let prog = par.parse().unwrap();
    assert_eq!(prog.rules.len(), 1);
}

#[test]
fn parse_getline_from_file() {
    let mut lex = lexer::Lexer::new("{ getline < \"data.txt\" }");
    let tokens = lex.tokenize().unwrap();
    let mut par = parser::Parser::new(tokens);
    let prog = par.parse().unwrap();
    assert_eq!(prog.rules.len(), 1);
}

#[test]
fn parse_getline_var_from_file() {
    let mut lex = lexer::Lexer::new("{ getline line < \"data.txt\" }");
    let tokens = lex.tokenize().unwrap();
    let mut par = parser::Parser::new(tokens);
    let prog = par.parse().unwrap();
    assert_eq!(prog.rules.len(), 1);
}

#[test]
fn parse_cmd_pipe_getline() {
    let mut lex = lexer::Lexer::new("{ \"date\" | getline d }");
    let tokens = lex.tokenize().unwrap();
    let mut par = parser::Parser::new(tokens);
    let prog = par.parse().unwrap();
    assert_eq!(prog.rules.len(), 1);
}

// ── Exponentiation operator ──────────────────────────────────────

#[test]
fn exponentiation_basic() {
    let rt = eval("{ result = 2 ** 10 }", &["x"]);
    assert_eq!(rt.get_var("result"), "1024");
}

#[test]
fn exponentiation_right_associative() {
    // 2 ** 3 ** 2 should be 2 ** (3 ** 2) = 2 ** 9 = 512
    let rt = eval("{ result = 2 ** 3 ** 2 }", &["x"]);
    assert_eq!(rt.get_var("result"), "512");
}

#[test]
fn exponentiation_higher_than_multiplication() {
    // 3 * 2 ** 3 should be 3 * 8 = 24
    let rt = eval("{ result = 3 * 2 ** 3 }", &["x"]);
    assert_eq!(rt.get_var("result"), "24");
}

#[test]
fn exponentiation_fractional() {
    // 9 ** 0.5 = 3
    let rt = eval("{ result = 9 ** 0.5 }", &["x"]);
    assert_eq!(rt.get_var("result"), "3");
}

// ── Hex literals ─────────────────────────────────────────────────

#[test]
fn hex_literal_basic() {
    let rt = eval("{ result = 0xFF }", &["x"]);
    assert_eq!(rt.get_var("result"), "255");
}

#[test]
fn hex_literal_in_arithmetic() {
    let rt = eval("{ result = 0x10 + 1 }", &["x"]);
    assert_eq!(rt.get_var("result"), "17");
}

#[test]
fn hex_literal_uppercase_x() {
    let rt = eval("{ result = 0X1F }", &["x"]);
    assert_eq!(rt.get_var("result"), "31");
}

// ── String escape sequences ──────────────────────────────────────

#[test]
fn hex_escape_in_string() {
    let rt = eval(r#"{ result = "\x41\x42\x43" }"#, &["x"]);
    assert_eq!(rt.get_var("result"), "ABC");
}

#[test]
fn unicode_escape_in_string() {
    let rt = eval(r#"{ result = "\u00e9" }"#, &["x"]);
    assert_eq!(rt.get_var("result"), "é");
}

#[test]
fn unicode_escape_emoji() {
    let rt = eval(r#"{ result = "\u2764" }"#, &["x"]);
    assert_eq!(rt.get_var("result"), "❤");
}
