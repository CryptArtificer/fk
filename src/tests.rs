use crate::{action, input, lexer, parser, runtime};
use crate::input::RecordReader;

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
        let rec = input::Record { text: line.to_string(), fields: None };
        exec.run_record(&rec);
    }
    exec.run_end();

    rt
}

/// Helper: parse and run with header mode (-H). First line is header.
fn eval_with_header(program_text: &str, fs: &str, input_lines: &[&str]) -> runtime::Runtime {
    let mut lex = lexer::Lexer::new(program_text);
    let tokens = lex.tokenize().expect("lexer error");
    let mut par = parser::Parser::new(tokens);
    let program = par.parse().expect("parse error");

    let mut rt = runtime::Runtime::new();
    rt.set_var("FS", fs);
    let mut exec = action::Executor::new(&program, &mut rt);

    exec.run_begin();
    let mut lines = input_lines.iter();
    if let Some(header) = lines.next() {
        exec.set_header_from_text(header);
    }
    for line in lines {
        let rec = input::Record { text: line.to_string(), fields: None };
        exec.run_record(&rec);
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

// ── delete array (whole array) ───────────────────────────────────

#[test]
fn delete_entire_array() {
    let rt = eval(
        "{ a[1]=\"x\"; a[2]=\"y\"; delete a; result = length(a) }",
        &["z"],
    );
    assert_eq!(rt.get_var("result"), "0");
}

#[test]
fn delete_entire_array_then_rebuild() {
    let rt = eval(
        "{ a[1]=\"old\"; delete a; a[1]=\"new\"; result = a[1] }",
        &["z"],
    );
    assert_eq!(rt.get_var("result"), "new");
}

// ── length(array) ────────────────────────────────────────────────

#[test]
fn length_of_array() {
    let rt = eval(
        "{ a[\"x\"]=1; a[\"y\"]=2; a[\"z\"]=3; result = length(a) }",
        &["w"],
    );
    assert_eq!(rt.get_var("result"), "3");
}

#[test]
fn length_of_empty_array() {
    let rt = eval(
        "{ split(\"\", a); result = length(a) }",
        &["w"],
    );
    assert_eq!(rt.get_var("result"), "0");
}

#[test]
fn length_of_string_still_works() {
    let rt = eval("{ result = length(\"hello\") }", &["x"]);
    assert_eq!(rt.get_var("result"), "5");
}

// ── Negative field indexes ───────────────────────────────────────

#[test]
fn negative_field_last() {
    // $-1 should be the last field
    let rt = eval("{ result = $-1 }", &["a b c"]);
    assert_eq!(rt.get_var("result"), "c");
}

#[test]
fn negative_field_second_to_last() {
    let rt = eval("{ result = $-2 }", &["a b c d"]);
    assert_eq!(rt.get_var("result"), "c");
}

#[test]
fn negative_field_beyond_start_gives_whole_record() {
    // $-99 on a 3-field record should resolve to $0
    let rt = eval("{ result = $-99 }", &["a b c"]);
    assert_eq!(rt.get_var("result"), "a b c");
}

// ── IO builtins ──────────────────────────────────────────────────

#[test]
fn system_returns_exit_status() {
    let rt = eval("{ result = system(\"true\") }", &["x"]);
    assert_eq!(rt.get_var("result"), "0");
}

#[test]
fn system_returns_nonzero_on_failure() {
    let rt = eval("{ result = system(\"false\") }", &["x"]);
    assert_eq!(rt.get_var("result"), "1");
}

#[test]
fn fflush_returns_zero() {
    let rt = eval("{ result = fflush() }", &["x"]);
    assert_eq!(rt.get_var("result"), "0");
}

// ── Time functions ───────────────────────────────────────────────

#[test]
fn systime_returns_positive_integer() {
    let rt = eval("{ result = systime() }", &["x"]);
    let ts: f64 = rt.get_var("result").parse().unwrap();
    assert!(ts > 1_000_000_000.0); // after 2001
}

#[test]
fn strftime_formats_known_epoch() {
    // Epoch 0 = 1970-01-01 00:00:00 UTC
    let rt = eval(
        r#"{ result = strftime("%Y-%m-%d %H:%M:%S", 0) }"#,
        &["x"],
    );
    assert_eq!(rt.get_var("result"), "1970-01-01 00:00:00");
}

#[test]
fn strftime_formats_known_date() {
    // 1234567890 = 2009-02-13 23:31:30 UTC
    let rt = eval(
        r#"{ result = strftime("%Y-%m-%d", 1234567890) }"#,
        &["x"],
    );
    assert_eq!(rt.get_var("result"), "2009-02-13");
}

#[test]
fn strftime_weekday_and_month_names() {
    // Epoch 0 = Thursday, January
    let rt = eval(
        r#"{ result = strftime("%A %B", 0) }"#,
        &["x"],
    );
    assert_eq!(rt.get_var("result"), "Thursday January");
}

#[test]
fn mktime_converts_to_epoch() {
    let rt = eval(
        r#"{ result = mktime("1970 1 1 0 0 0") }"#,
        &["x"],
    );
    assert_eq!(rt.get_var("result"), "0");
}

#[test]
fn mktime_roundtrips_with_strftime() {
    let rt = eval(
        r#"{ ts = mktime("2009 2 13 23 31 30"); result = strftime("%Y-%m-%d %H:%M:%S", ts) }"#,
        &["x"],
    );
    assert_eq!(rt.get_var("result"), "2009-02-13 23:31:30");
}

// ── nextfile ─────────────────────────────────────────────────────

#[test]
fn nextfile_stops_remaining_rules_for_record() {
    // Two rules: first sets x and calls nextfile, second should not run
    let rt = eval(
        "{ x++ ; nextfile } { y++ }",
        &["a", "b", "c"],
    );
    // nextfile on single source: first record processed, rest skipped
    assert_eq!(rt.get_var("x"), "1");
    assert_eq!(rt.get_var("y"), "");
}

#[test]
fn nextfile_parses_as_statement() {
    let mut lex = lexer::Lexer::new("{ nextfile }");
    let tokens = lex.tokenize().unwrap();
    let mut par = parser::Parser::new(tokens);
    let prog = par.parse().unwrap();
    assert_eq!(prog.rules.len(), 1);
}

// ── Unicode-aware operations ─────────────────────────────────────

#[test]
fn unicode_length_counts_chars() {
    let rt = eval(
        "{ n = length($0) }",
        &["café"],
    );
    assert_eq!(rt.get_var("n"), "4");
}

#[test]
fn unicode_substr_indexes_by_char() {
    let rt = eval(
        "{ s = substr($0, 4, 1) }",
        &["café"],
    );
    assert_eq!(rt.get_var("s"), "é");
}

#[test]
fn unicode_index_returns_char_position() {
    let rt = eval(
        "{ p = index($0, \"é\") }",
        &["café"],
    );
    assert_eq!(rt.get_var("p"), "4");
}

// ── jpath (JSON path) ───────────────────────────────────────────

#[test]
fn jpath_flat_key() {
    let rt = eval(
        r#"{ x = jpath($0, ".name") }"#,
        &[r#"{"name":"Alice","age":30}"#],
    );
    assert_eq!(rt.get_var("x"), "Alice");
}

#[test]
fn jpath_nested_array() {
    let rt = eval(
        r#"{ x = jpath($0, ".users[1].name") }"#,
        &[r#"{"users":[{"name":"Alice"},{"name":"Bob"}]}"#],
    );
    assert_eq!(rt.get_var("x"), "Bob");
}

#[test]
fn jpath_missing_returns_empty() {
    let rt = eval(
        r#"{ x = jpath($0, ".nope") }"#,
        &[r#"{"a":1}"#],
    );
    assert_eq!(rt.get_var("x"), "");
}

#[test]
fn jpath_extract_array_into_awk_array() {
    let rt = eval(
        r#"{ n = jpath($0, ".items", arr); x = arr[1]; y = arr[2]; z = arr[3] }"#,
        &[r#"{"items":[10,20,30]}"#],
    );
    assert_eq!(rt.get_var("n"), "3");
    assert_eq!(rt.get_array("arr", "1"), "10");
    assert_eq!(rt.get_array("arr", "2"), "20");
    assert_eq!(rt.get_array("arr", "3"), "30");
}

#[test]
fn jpath_extract_object_into_awk_array() {
    let rt = eval(
        r#"{ n = jpath($0, ".", arr) }"#,
        &[r#"{"name":"Alice","age":30}"#],
    );
    assert_eq!(rt.get_var("n"), "2");
    assert_eq!(rt.get_array("arr", "name"), "Alice");
    assert_eq!(rt.get_array("arr", "age"), "30");
}

#[test]
fn jpath_extract_scalar_gives_single_element() {
    let rt = eval(
        r#"{ n = jpath($0, ".name", arr) }"#,
        &[r#"{"name":"Bob"}"#],
    );
    assert_eq!(rt.get_var("n"), "1");
    assert_eq!(rt.get_array("arr", "0"), "Bob");
}

#[test]
fn jpath_iterate_and_project() {
    // .users[].id or .users.id → extract all ids into array
    let rt = eval(
        r#"{ n = jpath($0, ".users[].id", ids) }"#,
        &[r#"{"users":[{"id":10},{"id":20},{"id":30}]}"#],
    );
    assert_eq!(rt.get_var("n"), "3");
    assert_eq!(rt.get_array("ids", "1"), "10");
    assert_eq!(rt.get_array("ids", "2"), "20");
    assert_eq!(rt.get_array("ids", "3"), "30");
}

#[test]
fn jpath_implicit_iteration() {
    // .users.name without [] — implicitly iterates
    let rt = eval(
        r#"{ x = jpath($0, ".users.name") }"#,
        &[r#"{"users":[{"name":"Alice"},{"name":"Bob"}]}"#],
    );
    assert_eq!(rt.get_var("x"), "Alice\nBob");
}

// ── Edge-case audit ─────────────────────────────────────────────

#[test]
fn empty_input_produces_no_records() {
    let rt = eval("{ count++ } END { x = count }", &[]);
    assert_eq!(rt.get_var("NR"), "0");
    assert_eq!(rt.get_var("x"), "");
}

#[test]
fn empty_line_gives_nf_zero() {
    let rt = eval("{ nf = NF }", &[""]);
    assert_eq!(rt.get_var("NR"), "1");
    assert_eq!(rt.get_var("nf"), "0");
}

#[test]
fn multiple_empty_lines() {
    let rt = eval("{ total += NF } END { x = total }", &["", "", ""]);
    assert_eq!(rt.get_var("NR"), "3");
    assert_eq!(rt.get_var("x"), "0");
}

#[test]
fn begin_end_with_no_input() {
    let rt = eval("BEGIN { x = 1 } END { y = 2 }", &[]);
    assert_eq!(rt.get_var("x"), "1");
    assert_eq!(rt.get_var("y"), "2");
}

#[test]
fn nul_byte_in_input() {
    let rt = eval("{ nf = NF; len = length($0) }", &["a\x00b"]);
    assert_eq!(rt.get_var("nf"), "1");
    assert_eq!(rt.get_var("len"), "3");
}

#[test]
fn high_field_read_returns_empty() {
    let rt = eval("{ x = $100 }", &["a b c"]);
    assert_eq!(rt.get_var("x"), "");
}

#[test]
fn high_field_write_extends_nf() {
    let rt = eval("{ $500 = \"x\"; nf = NF }", &["a"]);
    assert_eq!(rt.get_var("nf"), "500");
}

#[test]
fn trailing_separator_produces_empty_fields() {
    let rt2 = eval(
        "BEGIN { FS = \",\" } { nf = NF; f2 = $2; f3 = $3 }",
        &["a,,"],
    );
    assert_eq!(rt2.get_var("nf"), "3");
    assert_eq!(rt2.get_var("f2"), "");
    assert_eq!(rt2.get_var("f3"), "");
}

#[test]
fn long_line_1mb() {
    let line = "x".repeat(1_000_000);
    let rt = eval("{ len = length($0) }", &[&line]);
    assert_eq!(rt.get_var("len"), "1000000");
}

#[test]
fn many_fields() {
    let line = (1..=1000).map(|i| i.to_string()).collect::<Vec<_>>().join(" ");
    let rt = eval("{ nf = NF; last = $NF }", &[&line]);
    assert_eq!(rt.get_var("nf"), "1000");
    assert_eq!(rt.get_var("last"), "1000");
}

#[test]
fn deep_recursion_does_not_crash() {
    // Should hit the call depth limit (200) and return gracefully, not stack overflow
    let rt = eval(
        "function f(n) { if (n <= 0) return 0; return f(n-1) } BEGIN { x = f(500) }",
        &[],
    );
    // x will be "" because the depth limit is hit before n reaches 0
    let x = rt.get_var("x");
    assert!(x == "0" || x == "", "unexpected result: {}", x);
}

#[test]
fn moderate_recursion_works() {
    let rt = eval(
        "function f(n) { if (n <= 1) return 1; return n * f(n-1) } BEGIN { x = f(10) }",
        &[],
    );
    assert_eq!(rt.get_var("x"), "3628800");
}

#[test]
fn field_zero_reconstructed_with_ofs() {
    let rt = eval(
        "BEGIN { OFS = \"-\" } { $1 = $1; x = $0 }",
        &["a b c"],
    );
    assert_eq!(rt.get_var("x"), "a-b-c");
}

#[test]
fn json_mode_preserves_raw_record_text_for_jpath() {
    let program_text = "{ x = jpath($0, \".a\") }";
    let mut lex = lexer::Lexer::new(program_text);
    let tokens = lex.tokenize().expect("lexer error");
    let mut par = parser::Parser::new(tokens);
    let program = par.parse().expect("parse error");

    let mut rt = runtime::Runtime::new();
    let mut exec = action::Executor::new(&program, &mut rt);

    exec.run_begin();
    let rec = input::Record {
        text: r#"{"a":1}"#.to_string(),
        fields: Some(vec!["1".to_string()]),
    };
    exec.run_record(&rec);
    exec.run_end();

    assert_eq!(rt.get_var("x"), "1");
}

#[test]
fn assign_field_zero_re_splits() {
    let rt = eval("{ $0 = \"x y z\"; nf = NF; f2 = $2 }", &["a"]);
    assert_eq!(rt.get_var("nf"), "3");
    assert_eq!(rt.get_var("f2"), "y");
}

#[test]
fn uninitialized_array_length_is_zero() {
    let rt = eval("BEGIN { x = length(arr) }", &[]);
    // length of non-existent array: should not crash
    assert_eq!(rt.get_var("x"), "0");
}

// ── break ────────────────────────────────────────────────────────

#[test]
fn break_in_while() {
    let rt = eval("BEGIN { i=0; while (1) { i++; if (i==3) break } x=i }", &[]);
    assert_eq!(rt.get_var("x"), "3");
}

#[test]
fn break_in_for() {
    let rt = eval("BEGIN { for (i=0; i<10; i++) { if (i==5) break } x=i }", &[]);
    assert_eq!(rt.get_var("x"), "5");
}

#[test]
fn break_in_for_in() {
    let rt = eval(
        "BEGIN { a[1]=1; a[2]=2; a[3]=3; for (k in a) { n++; break } x=n }",
        &[],
    );
    assert_eq!(rt.get_var("x"), "1");
}

#[test]
fn break_in_do_while() {
    let rt = eval("BEGIN { i=0; do { i++; if (i==4) break } while (1); x=i }", &[]);
    assert_eq!(rt.get_var("x"), "4");
}

// ── continue ─────────────────────────────────────────────────────

#[test]
fn continue_in_while() {
    let rt = eval(
        "BEGIN { i=0; while (i<5) { i++; if (i==3) continue; s+=i } x=s }",
        &[],
    );
    // sum of 1+2+4+5 = 12
    assert_eq!(rt.get_var("x"), "12");
}

#[test]
fn continue_in_for() {
    let rt = eval(
        "BEGIN { for (i=1; i<=5; i++) { if (i==3) continue; s+=i } x=s }",
        &[],
    );
    // sum of 1+2+4+5 = 12
    assert_eq!(rt.get_var("x"), "12");
}

#[test]
fn continue_in_for_in() {
    // Count keys that are not "b"
    let rt = eval(
        r#"BEGIN { a["a"]=1; a["b"]=2; a["c"]=3; for (k in a) { if (k=="b") continue; n++ } x=n }"#,
        &[],
    );
    assert_eq!(rt.get_var("x"), "2");
}

#[test]
fn continue_in_do_while() {
    let rt = eval(
        "BEGIN { i=0; do { i++; if (i==3) continue; s+=i } while (i<5); x=s }",
        &[],
    );
    // sum of 1+2+4+5 = 12
    assert_eq!(rt.get_var("x"), "12");
}

// ── do-while ─────────────────────────────────────────────────────

#[test]
fn do_while_basic() {
    let rt = eval("BEGIN { i=0; do { i++ } while (i<3); x=i }", &[]);
    assert_eq!(rt.get_var("x"), "3");
}

#[test]
fn do_while_runs_at_least_once() {
    let rt = eval("BEGIN { i=0; do { i++ } while (0); x=i }", &[]);
    assert_eq!(rt.get_var("x"), "1");
}

#[test]
fn do_while_with_body_block() {
    let rt = eval(
        "BEGIN { s=0; i=1; do { s+=i; i++ } while (i<=5); x=s }",
        &[],
    );
    assert_eq!(rt.get_var("x"), "15");
}

// ── exit ─────────────────────────────────────────────────────────

#[test]
fn exit_stops_processing_records() {
    let rt = eval("{ n++; if (n==2) exit } END { x=n }", &["a", "b", "c", "d"]);
    assert_eq!(rt.get_var("x"), "2");
}

#[test]
fn exit_runs_end_block() {
    let rt = eval("BEGIN { exit } END { x=42 }", &[]);
    assert_eq!(rt.get_var("x"), "42");
}

#[test]
fn exit_with_code() {
    // We can't easily check the process exit code from the test helper,
    // but we can verify that exit(code) stops processing and runs END.
    let rt = eval("{ n++; exit(2) } END { x=n }", &["a", "b", "c"]);
    assert_eq!(rt.get_var("x"), "1");
}

#[test]
fn exit_from_begin() {
    let rt = eval("BEGIN { x=1; exit } { x=99 } END { y=x }", &[]);
    assert_eq!(rt.get_var("y"), "1");
}

// ── computed regex ───────────────────────────────────────────────

#[test]
fn computed_regex_match() {
    let rt = eval(
        r#"{ pat = "hel"; result = ($0 ~ pat) }"#,
        &["hello"],
    );
    assert_eq!(rt.get_var("result"), "1");
}

#[test]
fn computed_regex_not_match() {
    let rt = eval(
        r#"{ pat = "xyz"; result = ($0 !~ pat) }"#,
        &["hello"],
    );
    assert_eq!(rt.get_var("result"), "1");
}

#[test]
fn computed_regex_with_special_chars() {
    let rt = eval(
        r#"{ pat = "^[0-9]+$"; result = ($0 ~ pat) }"#,
        &["12345"],
    );
    assert_eq!(rt.get_var("result"), "1");
}

#[test]
fn computed_regex_no_match() {
    let rt = eval(
        r#"{ pat = "^[0-9]+$"; result = ($0 ~ pat) }"#,
        &["abc"],
    );
    assert_eq!(rt.get_var("result"), "0");
}

#[test]
fn regex_pattern_uses_real_regex() {
    // Verify that /regex/ patterns use real regex, not string contains
    let rt = eval(r#"/^hello$/ { x = 1 }"#, &["hello", "hello world"]);
    assert_eq!(rt.get_var("x"), "1");
}

// ── gensub ───────────────────────────────────────────────────────

#[test]
fn gensub_basic_first() {
    let rt = eval(
        r#"{ x = gensub("o", "0", 1) }"#,
        &["foobar"],
    );
    assert_eq!(rt.get_var("x"), "f0obar");
}

#[test]
fn gensub_global() {
    let rt = eval(
        r#"{ x = gensub("o", "0", "g") }"#,
        &["foobar"],
    );
    assert_eq!(rt.get_var("x"), "f00bar");
}

#[test]
fn gensub_nth_occurrence() {
    let rt = eval(
        r#"{ x = gensub("o", "0", 2) }"#,
        &["foobar"],
    );
    assert_eq!(rt.get_var("x"), "fo0bar");
}

#[test]
fn gensub_does_not_modify_original() {
    let rt = eval(
        r#"{ x = gensub("o", "0", "g"); y = $0 }"#,
        &["foobar"],
    );
    assert_eq!(rt.get_var("x"), "f00bar");
    assert_eq!(rt.get_var("y"), "foobar");
}

#[test]
fn gensub_with_explicit_target() {
    let rt = eval(
        r#"{ s = "hello"; x = gensub("l", "L", "g", s) }"#,
        &["ignored"],
    );
    assert_eq!(rt.get_var("x"), "heLLo");
}

#[test]
fn gensub_regex_pattern() {
    let rt = eval(
        r#"{ x = gensub("[0-9]+", "NUM", "g") }"#,
        &["abc123def456"],
    );
    assert_eq!(rt.get_var("x"), "abcNUMdefNUM");
}

// ── SUBSEP and multi-dimensional arrays ──────────────────────────

#[test]
fn subsep_multi_dim_array() {
    let rt = eval(
        r#"BEGIN { a[1,2] = "x"; a[3,4] = "y"; x = a[1,2]; y = a[3,4] }"#,
        &[],
    );
    assert_eq!(rt.get_var("x"), "x");
    assert_eq!(rt.get_var("y"), "y");
}

#[test]
fn subsep_default_value() {
    // SUBSEP defaults to \x1c (ASCII 28)
    let rt = eval(
        r#"BEGIN { a[1,2] = "v" ; for (k in a) x = k }"#,
        &[],
    );
    assert_eq!(rt.get_var("x"), "1\x1c2");
}

// ── OFMT default ─────────────────────────────────────────────────

#[test]
fn ofmt_default_value() {
    let rt = eval(r#"BEGIN { x = OFMT }"#, &[]);
    assert_eq!(rt.get_var("x"), "%.6g");
}

#[test]
fn ofmt_is_settable() {
    let rt = eval(r#"BEGIN { OFMT = "%.2f"; x = OFMT }"#, &[]);
    assert_eq!(rt.get_var("x"), "%.2f");
}

// ── SUBSEP is settable ──────────────────────────────────────────

#[test]
fn subsep_custom_value() {
    let rt = eval(
        r#"BEGIN { SUBSEP = ":"; a[1,2] = "v"; for (k in a) x = k }"#,
        &[],
    );
    assert_eq!(rt.get_var("x"), "1:2");
}

// ── FNR tracks per-record within eval helper ─────────────────────

#[test]
fn fnr_increments_with_nr_in_single_source() {
    // In the test helper (single source), FNR is not incremented because
    // that's done by main.rs. But NR still works. FNR stays 0.
    // This test documents the current behavior with the eval() helper.
    let rt = eval("{ last_nr = NR }", &["a", "b", "c"]);
    assert_eq!(rt.get_var("last_nr"), "3");
}

// ── ENVIRON access ───────────────────────────────────────────────

#[test]
fn environ_is_accessible_as_array() {
    // ENVIRON is populated in main.rs, not in the test helper.
    // But we can test that the array mechanism works.
    let rt = eval(
        r#"BEGIN { ENVIRON["TEST_KEY"] = "test_val"; x = ENVIRON["TEST_KEY"] }"#,
        &[],
    );
    assert_eq!(rt.get_var("x"), "test_val");
}

// ── ARGC / ARGV ─────────────────────────────────────────────────

#[test]
fn argc_argv_accessible_as_vars() {
    // ARGC/ARGV are populated in main.rs. Test that the mechanism works.
    let rt = eval(
        r#"BEGIN { ARGC = 3; ARGV[0] = "fk"; ARGV[1] = "prog"; x = ARGC; y = ARGV[1] }"#,
        &[],
    );
    assert_eq!(rt.get_var("x"), "3");
    assert_eq!(rt.get_array("ARGV", "1"), "prog");
}

// ── exit wiring with exit code ───────────────────────────────────

#[test]
fn exit_code_is_stored() {
    let prog = "BEGIN { exit(42) }";
    let mut lex = crate::lexer::Lexer::new(prog);
    let tokens = lex.tokenize().unwrap();
    let mut par = crate::parser::Parser::new(tokens);
    let program = par.parse().unwrap();
    let mut rt = crate::runtime::Runtime::new();
    let mut exec = crate::action::Executor::new(&program, &mut rt);
    exec.run_begin();
    assert_eq!(exec.should_exit(), Some(42));
}

#[test]
fn exit_zero_is_stored() {
    let prog = "BEGIN { exit }";
    let mut lex = crate::lexer::Lexer::new(prog);
    let tokens = lex.tokenize().unwrap();
    let mut par = crate::parser::Parser::new(tokens);
    let program = par.parse().unwrap();
    let mut rt = crate::runtime::Runtime::new();
    let mut exec = crate::action::Executor::new(&program, &mut rt);
    exec.run_begin();
    assert_eq!(exec.should_exit(), Some(0));
}

// ── FILENAME default ─────────────────────────────────────────────

#[test]
fn filename_default_is_empty() {
    let rt = eval(r#"BEGIN { x = FILENAME }"#, &[]);
    assert_eq!(rt.get_var("x"), "");
}

// ── next ─────────────────────────────────────────────────────────

#[test]
fn next_skips_remaining_rules() {
    let rt = eval(
        "{ x++; next } { y++ }",
        &["a", "b", "c"],
    );
    assert_eq!(rt.get_var("x"), "3");
    assert_eq!(rt.get_var("y"), "");
}

#[test]
fn next_in_conditional() {
    let rt = eval(
        r#"$0 == "skip" { next } { n++ }"#,
        &["a", "skip", "b", "skip", "c"],
    );
    assert_eq!(rt.get_var("n"), "3");
}

// ── regex backslash preservation ─────────────────────────────────

#[test]
fn regex_pattern_with_backslash_escape() {
    let rt = eval(
        r#"/\[data\]/ { x++ }"#,
        &["[data]", "other", "[data] more"],
    );
    assert_eq!(rt.get_var("x"), "2");
}

#[test]
fn regex_dot_is_not_literal() {
    // . in regex should match any char, not just literal dot
    let rt = eval(
        r#"/a.c/ { x++ }"#,
        &["abc", "aXc", "ac", "a.c"],
    );
    assert_eq!(rt.get_var("x"), "3");
}

#[test]
fn regex_caret_anchors() {
    let rt = eval(
        r#"/^hello$/ { x++ }"#,
        &["hello", "hello world", "say hello"],
    );
    assert_eq!(rt.get_var("x"), "1");
}

// ── Header name field access ────────────────────────────────────

#[test]
fn header_names_as_field_accessors() {
    let rt = eval_with_header(
        r#"{ result = $name " is " $age }"#,
        ",",
        &["name,age,city", "Alice,30,NYC", "Bob,25,LA"],
    );
    assert_eq!(rt.get_var("result"), "Bob is 25");
}

#[test]
fn header_names_with_filter() {
    let rt = eval_with_header(
        r#"$age > 28 { count++ }"#,
        ",",
        &["name,age,city", "Alice,30,NYC", "Bob,25,LA", "Carol,35,Chicago"],
    );
    assert_eq!(rt.get_var("count"), "2");
}

// ── Math builtins ───────────────────────────────────────────────

#[test]
fn math_abs() {
    let rt = eval(r#"BEGIN { x = abs(-5.5) }"#, &[]);
    assert_eq!(rt.get_var("x"), "5.5");
}

#[test]
fn math_ceil_floor_round() {
    let rt = eval(r#"BEGIN { a = ceil(2.3); b = floor(2.7); c = round(2.5) }"#, &[]);
    assert_eq!(rt.get_var("a"), "3");
    assert_eq!(rt.get_var("b"), "2");
    assert_eq!(rt.get_var("c"), "3");
}

#[test]
fn math_min_max() {
    let rt = eval(r#"BEGIN { a = min(3, 7); b = max(3, 7) }"#, &[]);
    assert_eq!(rt.get_var("a"), "3");
    assert_eq!(rt.get_var("b"), "7");
}

#[test]
fn math_atan2() {
    let rt = eval(r#"BEGIN { x = atan2(1, 1) }"#, &[]);
    assert_eq!(rt.get_var("x"), "0.785398");
}

#[test]
fn math_log2_log10() {
    let rt = eval(r#"BEGIN { a = log2(8); b = log10(1000) }"#, &[]);
    assert_eq!(rt.get_var("a"), "3.000000");
    assert_eq!(rt.get_var("b"), "3.000000");
}

#[test]
fn math_rand_returns_0_to_1() {
    let rt = eval(r#"BEGIN { srand(42); x = rand() }"#, &[]);
    let x: f64 = rt.get_var("x").parse().unwrap();
    assert!(x >= 0.0 && x < 1.0);
}

#[test]
fn math_srand_makes_deterministic() {
    let rt1 = eval(r#"BEGIN { srand(42); x = rand() }"#, &[]);
    let rt2 = eval(r#"BEGIN { srand(42); x = rand() }"#, &[]);
    assert_eq!(rt1.get_var("x"), rt2.get_var("x"));
}

// ── String builtins ─────────────────────────────────────────────

#[test]
fn string_trim() {
    let rt = eval(r#"BEGIN { x = trim("  hello  ") }"#, &[]);
    assert_eq!(rt.get_var("x"), "hello");
}

#[test]
fn string_ltrim_rtrim() {
    let rt = eval(r#"BEGIN { a = ltrim("  hi"); b = rtrim("hi  ") }"#, &[]);
    assert_eq!(rt.get_var("a"), "hi");
    assert_eq!(rt.get_var("b"), "hi");
}

#[test]
fn string_startswith_endswith() {
    let rt = eval(r#"BEGIN { a = startswith("hello", "hel"); b = endswith("hello", "lo"); c = startswith("hello", "xyz") }"#, &[]);
    assert_eq!(rt.get_var("a"), "1");
    assert_eq!(rt.get_var("b"), "1");
    assert_eq!(rt.get_var("c"), "0");
}

#[test]
fn string_repeat() {
    let rt = eval(r#"BEGIN { x = repeat("ab", 3) }"#, &[]);
    assert_eq!(rt.get_var("x"), "ababab");
}

#[test]
fn string_reverse() {
    let rt = eval(r#"BEGIN { x = reverse("hello") }"#, &[]);
    assert_eq!(rt.get_var("x"), "olleh");
}

#[test]
fn string_chr_ord() {
    let rt = eval(r#"BEGIN { a = chr(65); b = ord("A") }"#, &[]);
    assert_eq!(rt.get_var("a"), "A");
    assert_eq!(rt.get_var("b"), "65");
}

#[test]
fn string_hex() {
    let rt = eval(r#"BEGIN { x = hex(255) }"#, &[]);
    assert_eq!(rt.get_var("x"), "0xff");
}

// ── Date builtins ───────────────────────────────────────────────

#[test]
fn date_parsedate_basic() {
    let rt = eval(r#"BEGIN { x = parsedate("2025-01-15 10:30:00", "%Y-%m-%d %H:%M:%S") }"#, &[]);
    assert_eq!(rt.get_var("x"), "1736937000");
}

#[test]
fn date_parsedate_roundtrip() {
    let rt = eval(
        r#"BEGIN { ts = mktime("2024 06 15 12 30 45"); s = strftime("%Y-%m-%d %H:%M:%S", ts); ts2 = parsedate(s, "%Y-%m-%d %H:%M:%S"); x = (ts == ts2) }"#,
        &[],
    );
    assert_eq!(rt.get_var("x"), "1");
}

// ── Bitwise operations ──────────────────────────────────────────

#[test]
fn bitwise_and_or_xor() {
    let rt = eval(r#"BEGIN { a = and(12, 10); b = or(12, 10); c = xor(12, 10) }"#, &[]);
    assert_eq!(rt.get_var("a"), "8");
    assert_eq!(rt.get_var("b"), "14");
    assert_eq!(rt.get_var("c"), "6");
}

#[test]
fn bitwise_shift() {
    let rt = eval(r#"BEGIN { a = lshift(1, 8); b = rshift(256, 4) }"#, &[]);
    assert_eq!(rt.get_var("a"), "256");
    assert_eq!(rt.get_var("b"), "16");
}

#[test]
fn bitwise_compl() {
    let rt = eval(r#"BEGIN { x = and(compl(0), 0xFF) }"#, &[]);
    assert_eq!(rt.get_var("x"), "255");
}

// ── join, typeof ────────────────────────────────────────────────

#[test]
fn join_array() {
    let rt = eval(
        r#"BEGIN { a[1]="x"; a[2]="y"; a[3]="z"; x = join(a, ",") }"#,
        &[],
    );
    assert_eq!(rt.get_var("x"), "x,y,z");
}

#[test]
fn typeof_values() {
    let rt = eval(
        r#"BEGIN { a = 42; b = "hi"; c[1]=1; ta = typeof(a); tb = typeof(b); tc = typeof(c); tu = typeof(unknown) }"#,
        &[],
    );
    assert_eq!(rt.get_var("ta"), "number");
    assert_eq!(rt.get_var("tb"), "string");
    assert_eq!(rt.get_var("tc"), "array");
    assert_eq!(rt.get_var("tu"), "uninitialized");
}

// ── asort / asorti ──────────────────────────────────────────────

#[test]
fn asort_by_values() {
    let rt = eval(
        r#"BEGIN { a["x"]="3"; a["y"]="1"; a["z"]="2"; n = asort(a); r = a[1] "," a[2] "," a[3] }"#,
        &[],
    );
    assert_eq!(rt.get_var("n"), "3");
    assert_eq!(rt.get_var("r"), "1,2,3");
}

#[test]
fn asorti_by_keys() {
    let rt = eval(
        r#"BEGIN { a["c"]=1; a["a"]=2; a["b"]=3; n = asorti(a); r = a[1] "," a[2] "," a[3] }"#,
        &[],
    );
    assert_eq!(rt.get_var("n"), "3");
    assert_eq!(rt.get_var("r"), "a,b,c");
}

// ── match with captures ─────────────────────────────────────────

#[test]
fn match_with_capture_groups() {
    let rt = eval(
        r#"BEGIN { s = "2025-01-15"; match(s, "([0-9]+)-([0-9]+)-([0-9]+)", cap); y = cap[1]; m = cap[2]; d = cap[3] }"#,
        &[],
    );
    assert_eq!(rt.get_var("y"), "2025");
    assert_eq!(rt.get_var("m"), "01");
    assert_eq!(rt.get_var("d"), "15");
}

#[test]
fn match_captures_full_match() {
    let rt = eval(
        r#"BEGIN { match("hello world", "(hell)o (w)", cap); x = cap[0] }"#,
        &[],
    );
    assert_eq!(rt.get_var("x"), "hello w");
}

// ── Quoted/string field access ($"name") ────────────────────────

#[test]
fn quoted_field_access_by_name() {
    let rt = eval_with_header(
        r#"{ result = $"name" " is " $"age" }"#,
        ",",
        &["name,age,city", "Alice,30,NYC"],
    );
    assert_eq!(rt.get_var("result"), "Alice is 30");
}

#[test]
fn variable_field_access_by_name() {
    let rt = eval_with_header(
        r#"BEGIN { col = "city" } { result = $col }"#,
        ",",
        &["name,age,city", "Alice,30,NYC"],
    );
    assert_eq!(rt.get_var("result"), "NYC");
}

#[test]
fn quoted_field_with_special_chars() {
    let rt = eval_with_header(
        r#"{ result = $"user-name" }"#,
        ",",
        &["user-name,user-age", "Alice,30"],
    );
    assert_eq!(rt.get_var("result"), "Alice");
}

#[test]
fn quoted_field_filter() {
    let rt = eval_with_header(
        r#"$"score" > 90 { count++ }"#,
        ",",
        &["name,score", "Alice,95", "Bob,80", "Carol,92"],
    );
    assert_eq!(rt.get_var("count"), "2");
}

#[test]
fn numeric_string_field_still_works() {
    let rt = eval(
        r#"{ x = $"2" }"#,
        &["hello world"],
    );
    assert_eq!(rt.get_var("x"), "world");
}

// ── printf alignment and formatting ─────────────────────────────

#[test]
fn printf_right_align_int() {
    let r = crate::builtins::format_printf("%8d", &["42".into()]);
    assert_eq!(r, "      42");
}

#[test]
fn printf_left_align_int() {
    let r = crate::builtins::format_printf("%-8d", &["42".into()]);
    assert_eq!(r, "42      ");
}

#[test]
fn printf_zero_pad_int() {
    let r = crate::builtins::format_printf("%08d", &["42".into()]);
    assert_eq!(r, "00000042");
}

#[test]
fn printf_zero_pad_negative() {
    let r = crate::builtins::format_printf("%08d", &["-42".into()]);
    assert_eq!(r, "-0000042");
}

#[test]
fn printf_force_sign() {
    let r = crate::builtins::format_printf("%+d", &["42".into()]);
    assert_eq!(r, "+42");
}

#[test]
fn printf_force_sign_with_width() {
    let r = crate::builtins::format_printf("%+8d", &["42".into()]);
    assert_eq!(r, "     +42");
}

#[test]
fn printf_zero_pad_with_sign() {
    let r = crate::builtins::format_printf("%+08d", &["42".into()]);
    assert_eq!(r, "+0000042");
}

#[test]
fn printf_space_sign() {
    let r = crate::builtins::format_printf("% d", &["42".into()]);
    assert_eq!(r, " 42");
}

#[test]
fn printf_zero_pad_float() {
    let r = crate::builtins::format_printf("%012.2f", &["3.14".into()]);
    assert_eq!(r, "000000003.14");
}

#[test]
fn printf_force_sign_float() {
    let r = crate::builtins::format_printf("%+.2f", &["3.14".into()]);
    assert_eq!(r, "+3.14");
}

#[test]
fn printf_hex_zero_pad() {
    let r = crate::builtins::format_printf("%08x", &["255".into()]);
    assert_eq!(r, "000000ff");
}

#[test]
fn printf_octal_zero_pad() {
    let r = crate::builtins::format_printf("%08o", &["255".into()]);
    assert_eq!(r, "00000377");
}

#[test]
fn printf_string_precision() {
    let r = crate::builtins::format_printf("%.5s", &["hello world".into()]);
    assert_eq!(r, "hello");
}

// ── Statistical builtins ────────────────────────────────────────

#[test]
fn stats_sum() {
    let rt = eval(
        r#"{ a[NR] = $1 } END { result = sum(a) }"#,
        &["10", "20", "30", "40", "50"],
    );
    assert_eq!(rt.get_var("result"), "150");
}

#[test]
fn stats_mean() {
    let rt = eval(
        r#"{ a[NR] = $1 } END { result = mean(a) }"#,
        &["10", "20", "30", "40", "50"],
    );
    assert_eq!(rt.get_var("result"), "30");
}

#[test]
fn stats_median_odd() {
    let rt = eval(
        r#"{ a[NR] = $1 } END { result = median(a) }"#,
        &["3", "1", "5", "2", "4"],
    );
    assert_eq!(rt.get_var("result"), "3");
}

#[test]
fn stats_median_even() {
    let rt = eval(
        r#"{ a[NR] = $1 } END { result = median(a) }"#,
        &["1", "2", "3", "4"],
    );
    assert_eq!(rt.get_var("result"), "2.5");
}

#[test]
fn stats_stddev() {
    let rt = eval(
        r#"{ a[NR] = $1 } END { result = stddev(a) }"#,
        &["2", "4", "4", "4", "5", "5", "7", "9"],
    );
    let v: f64 = rt.get_var("result").parse().unwrap();
    assert!((v - 2.0).abs() < 0.01, "stddev should be ~2.0, got {}", v);
}

#[test]
fn stats_variance() {
    let rt = eval(
        r#"{ a[NR] = $1 } END { result = variance(a) }"#,
        &["2", "4", "4", "4", "5", "5", "7", "9"],
    );
    let v: f64 = rt.get_var("result").parse().unwrap();
    assert!((v - 4.0).abs() < 0.01, "variance should be ~4.0, got {}", v);
}

#[test]
fn stats_percentile_p50() {
    let rt = eval(
        r#"{ a[NR] = $1 } END { result = p(a, 50) }"#,
        &["10", "20", "30", "40", "50"],
    );
    assert_eq!(rt.get_var("result"), "30");
}

#[test]
fn stats_percentile_p95() {
    let rt = eval(
        r#"{ a[NR] = $1 } END { result = p(a, 95) }"#,
        &["10", "20", "30", "40", "50", "60", "70", "80", "90", "100"],
    );
    let v: f64 = rt.get_var("result").parse().unwrap();
    assert!((v - 95.5).abs() < 0.01, "p95 should be ~95.5, got {}", v);
}

#[test]
fn stats_percentile_long_form() {
    let rt = eval(
        r#"{ a[NR] = $1 } END { result = percentile(a, 75) }"#,
        &["10", "20", "30", "40", "50"],
    );
    assert_eq!(rt.get_var("result"), "40");
}

#[test]
fn stats_quantile() {
    let rt = eval(
        r#"{ a[NR] = $1 } END { result = quantile(a, 0.75) }"#,
        &["10", "20", "30", "40", "50"],
    );
    assert_eq!(rt.get_var("result"), "40");
}

#[test]
fn stats_iqm() {
    let rt = eval(
        r#"{ a[NR] = $1 } END { result = iqm(a) }"#,
        &["10", "20", "30", "40", "50", "60", "70", "80", "90", "100"],
    );
    let v: f64 = rt.get_var("result").parse().unwrap();
    assert!(v > 30.0 && v < 80.0, "iqm should be in the middle range, got {}", v);
}

#[test]
fn stats_min_array() {
    let rt = eval(
        r#"{ a[NR] = $1 } END { result = min(a) }"#,
        &["30", "10", "50", "20"],
    );
    assert_eq!(rt.get_var("result"), "10");
}

#[test]
fn stats_max_array() {
    let rt = eval(
        r#"{ a[NR] = $1 } END { result = max(a) }"#,
        &["30", "10", "50", "20"],
    );
    assert_eq!(rt.get_var("result"), "50");
}

#[test]
fn stats_scalar_min_max_still_works() {
    let rt = eval(
        r#"BEGIN { a = min(3, 7); b = max(3, 7) }"#,
        &[],
    );
    assert_eq!(rt.get_var("a"), "3");
    assert_eq!(rt.get_var("b"), "7");
}

#[test]
fn stats_empty_array() {
    let rt = eval(
        r#"END { result = sum(a) }"#,
        &["ignored"],
    );
    assert_eq!(rt.get_var("result"), "0");
}

#[test]
fn stats_single_element() {
    let rt = eval(
        r#"{ a[1] = 42 } END { s = sum(a); m = mean(a); d = median(a); sd = stddev(a) }"#,
        &["x"],
    );
    assert_eq!(rt.get_var("s"), "42");
    assert_eq!(rt.get_var("m"), "42");
    assert_eq!(rt.get_var("d"), "42");
    assert_eq!(rt.get_var("sd"), "0");
}

#[test]
fn multiple_begin_blocks() {
    let rt = eval(
        r#"BEGIN { a = 1 } BEGIN { b = 2 } END { c = a + b }"#,
        &[],
    );
    assert_eq!(rt.get_var("a"), "1");
    assert_eq!(rt.get_var("b"), "2");
    assert_eq!(rt.get_var("c"), "3");
}

#[test]
fn multiple_end_blocks() {
    let rt = eval(
        r#"{ sum += $1 } END { a = sum } END { b = a * 2 }"#,
        &["10", "20"],
    );
    assert_eq!(rt.get_var("a"), "30");
    assert_eq!(rt.get_var("b"), "60");
}

// --- describe / sniffer tests ---

#[test]
fn sniff_csv() {
    let data = "name,age,score\nalice,30,95.5\nbob,25,82.0\n";
    let mut reader = std::io::BufReader::new(data.as_bytes());
    let schema = crate::describe::sniff(&mut reader);
    assert_eq!(schema.format, crate::describe::Format::Csv);
    assert!(schema.has_header);
    assert_eq!(schema.columns, vec!["name", "age", "score"]);
    assert_eq!(schema.types[0], crate::describe::ColType::String);
    assert_eq!(schema.types[1], crate::describe::ColType::Int);
    assert_eq!(schema.types[2], crate::describe::ColType::Float);
}

#[test]
fn sniff_tsv() {
    let data = "host\tstatus\tlatency\nweb1\t200\t12.5\nweb2\t500\t45.1\n";
    let mut reader = std::io::BufReader::new(data.as_bytes());
    let schema = crate::describe::sniff(&mut reader);
    assert_eq!(schema.format, crate::describe::Format::Tsv);
    assert!(schema.has_header);
    assert_eq!(schema.columns.len(), 3);
}

#[test]
fn sniff_json() {
    let data = r#"{"user":"alice","score":95}
{"user":"bob","score":82}
"#;
    let mut reader = std::io::BufReader::new(data.as_bytes());
    let schema = crate::describe::sniff(&mut reader);
    assert_eq!(schema.format, crate::describe::Format::Json);
    assert!(!schema.has_header);
    assert_eq!(schema.columns, vec!["user", "score"]);
    assert_eq!(schema.types[0], crate::describe::ColType::String);
    assert_eq!(schema.types[1], crate::describe::ColType::Int);
}

#[test]
fn sniff_whitespace() {
    let data = "1234 root 2.5\n5678 www 15.3\n";
    let mut reader = std::io::BufReader::new(data.as_bytes());
    let schema = crate::describe::sniff(&mut reader);
    assert_eq!(schema.format, crate::describe::Format::Space);
    assert!(!schema.has_header);
}

#[test]
fn sniff_csv_no_header() {
    let data = "10,20,30\n40,50,60\n70,80,90\n";
    let mut reader = std::io::BufReader::new(data.as_bytes());
    let schema = crate::describe::sniff(&mut reader);
    assert_eq!(schema.format, crate::describe::Format::Csv);
    assert!(!schema.has_header);
    assert_eq!(schema.types, vec![
        crate::describe::ColType::Int,
        crate::describe::ColType::Int,
        crate::describe::ColType::Int,
    ]);
}

#[test]
fn format_from_extension_works() {
    assert_eq!(crate::describe::format_from_extension("data.csv"), Some(crate::describe::Format::Csv));
    assert_eq!(crate::describe::format_from_extension("data.csv.gz"), Some(crate::describe::Format::Csv));
    assert_eq!(crate::describe::format_from_extension("data.tsv.zst"), Some(crate::describe::Format::Tsv));
    assert_eq!(crate::describe::format_from_extension("data.json"), Some(crate::describe::Format::Json));
    assert_eq!(crate::describe::format_from_extension("data.jsonl.bz2"), Some(crate::describe::Format::Json));
    assert_eq!(crate::describe::format_from_extension("data.parquet"), Some(crate::describe::Format::Parquet));
    assert_eq!(crate::describe::format_from_extension("data.txt"), None);
    assert_eq!(crate::describe::format_from_extension("data.log.gz"), None);
}

#[test]
fn compressed_extension_detection() {
    assert!(crate::describe::is_compressed("file.csv.gz"));
    assert!(crate::describe::is_compressed("file.zst"));
    assert!(crate::describe::is_compressed("file.bz2"));
    assert!(crate::describe::is_compressed("file.xz"));
    assert!(!crate::describe::is_compressed("file.csv"));
    assert!(!crate::describe::is_compressed("file.txt"));
}

// ── Compressed CSV integration test ─────────────────────────────

#[test]
fn compressed_csv_gz_reads_correctly() {
    let fixture = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests/data")
        .join("problematic.csv.gz");
    if !fixture.exists() {
        panic!("tests/data/problematic.csv.gz not found");
    }

    let path = fixture.to_str().unwrap();
    let reader_box = crate::describe::open_maybe_compressed(path)
        .expect("failed to decompress .csv.gz");
    let mut buf = std::io::BufReader::new(reader_box);
    let mut csv = crate::input::csv::CsvReader::comma();

    let mut records: Vec<Vec<String>> = Vec::new();
    while let Some(rec) = csv.next_record(&mut buf).expect("read error") {
        records.push(rec.fields.unwrap());
    }

    // Header row present
    assert_eq!(records[0], vec!["id", "name", "comment"]);

    // Spot-check well-formed rows
    let row1 = records.iter().find(|r| r[0] == "1").unwrap();
    assert_eq!(row1[1], "John Doe");
    assert_eq!(row1[2], "Simple entry");

    let row2 = records.iter().find(|r| r[0] == "2").unwrap();
    assert_eq!(row2[1], "Jane, A.");

    let row7 = records.iter().find(|r| r[0] == "7").unwrap();
    assert_eq!(row7[1], "Élodie");

    // Multi-line field survived decompression
    let row9 = records.iter().find(|r| r[0] == "9").unwrap();
    assert_eq!(row9[1], "Multi\nLine");
    assert_eq!(row9[2], "Comment with\nembedded newline");

    // Escaped quotes survived decompression
    let row11 = records.iter().find(|r| r[0] == "11").unwrap();
    assert_eq!(row11[1], "Escaped \"quote\" test");

    // Rows after the malformed unclosed-quote row are intact
    let row15 = records.iter().find(|r| r[0] == "15").unwrap();
    assert_eq!(row15[1], "NULL");
    assert_eq!(row15[2], "Literal NULL string");
}

#[test]
fn compressed_csv_gz_auto_detects_format() {
    let fixture = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests/data")
        .join("problematic.csv.gz");
    if !fixture.exists() {
        return;
    }
    let path = fixture.to_str().unwrap();
    let fmt = crate::describe::format_from_extension(path);
    assert_eq!(fmt, Some(crate::describe::Format::Csv));
    assert!(crate::describe::is_compressed(path));
}

#[test]
fn edge_csv_file_reads_correctly() {
    let fixture = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests/data")
        .join("edge_cases.csv");
    if !fixture.exists() {
        panic!("tests/data/edge_cases.csv not found");
    }

    let path = fixture.to_str().unwrap();
    let file = std::fs::File::open(path).expect("open edge_cases.csv");
    let mut buf = std::io::BufReader::new(file);
    let mut csv = crate::input::csv::CsvReader::comma();

    let mut records: Vec<Vec<String>> = Vec::new();
    while let Some(rec) = csv.next_record(&mut buf).expect("read error") {
        records.push(rec.fields.unwrap());
    }

    assert_eq!(records[0], vec!["id", "name", "comment"]);

    let ids: Vec<&str> = records.iter().map(|r| r[0].as_str()).collect();
    assert!(ids.contains(&"id"));
    assert!(ids.contains(&"1"));
    assert!(ids.contains(&"2"));
    // Row 3 merges with row 4 (unclosed quote), so "4" won't be a standalone id
    assert!(ids.contains(&"5"));
    assert!(ids.contains(&"9"));
    assert!(ids.contains(&"15"));
}

// --- keys(), vals(), print arr ---

#[test]
fn keys_returns_sorted_keys() {
    let rt = eval(
        r#"BEGIN { a["cherry"]=1; a["apple"]=2; a["banana"]=3; result = keys(a) }"#,
        &[],
    );
    assert_eq!(rt.get_var("result"), "apple\nbanana\ncherry");
}

#[test]
fn keys_numeric_keys_sorted_numerically() {
    let rt = eval(
        r#"BEGIN { a[10]=1; a[2]=1; a[1]=1; result = keys(a) }"#,
        &[],
    );
    assert_eq!(rt.get_var("result"), "1\n2\n10");
}

#[test]
fn vals_returns_values_sorted_by_key() {
    let rt = eval(
        r#"BEGIN { a[1]="x"; a[2]="y"; a[3]="z"; result = vals(a) }"#,
        &[],
    );
    assert_eq!(rt.get_var("result"), "x\ny\nz");
}

#[test]
fn vals_associative_keys_sorted_alphabetically() {
    let rt = eval(
        r#"BEGIN { a["b"]="two"; a["a"]="one"; a["c"]="three"; result = vals(a) }"#,
        &[],
    );
    assert_eq!(rt.get_var("result"), "one\ntwo\nthree");
}

#[test]
fn negated_bare_regex_pattern() {
    let rt = eval(
        r#"!/^#/ { result = result $0 "," }"#,
        &["# comment", "root", "nobody"],
    );
    assert_eq!(rt.get_var("result"), "root,nobody,");
}

#[test]
fn bare_regex_in_expression_context() {
    let rt = eval(
        r#"{ if (/^ok/) result = result $0 "," }"#,
        &["ok fine", "nope", "ok great"],
    );
    assert_eq!(rt.get_var("result"), "ok fine,ok great,");
}

// --- uniq, invert, compact ---

#[test]
fn uniq_deduplicates_values() {
    let rt = eval(
        r#"{ a[NR]=$1 } END { n=uniq(a); result=n ":" a[1] "," a[2] "," a[3] }"#,
        &["x", "y", "x", "z", "y"],
    );
    assert_eq!(rt.get_var("result"), "3:x,y,z");
}

#[test]
fn invert_swaps_keys_values() {
    let rt = eval(
        r#"BEGIN { a["k1"]="v1"; a["k2"]="v2"; inv(a); result=a["v1"] "," a["v2"] }"#,
        &[],
    );
    assert_eq!(rt.get_var("result"), "k1,k2");
}

#[test]
fn compact_removes_falsy() {
    let rt = eval(
        r#"BEGIN { a[1]="hi"; a[2]=""; a[3]=0; a[4]="ok"; n=tidy(a); result=n }"#,
        &[],
    );
    assert_eq!(rt.get_var("result"), "2");
}

// --- set operations ---

#[test]
fn diff_removes_common_keys() {
    let rt = eval(
        r#"BEGIN { a["x"]=1; a["y"]=1; a["z"]=1; b["y"]=1; diff(a,b); result=keys(a) }"#,
        &[],
    );
    assert_eq!(rt.get_var("result"), "x\nz");
}

#[test]
fn inter_keeps_common_keys() {
    let rt = eval(
        r#"BEGIN { a["x"]=1; a["y"]=1; a["z"]=1; b["y"]=1; b["z"]=1; inter(a,b); result=keys(a) }"#,
        &[],
    );
    assert_eq!(rt.get_var("result"), "y\nz");
}

#[test]
fn union_merges_keys() {
    let rt = eval(
        r#"BEGIN { a["x"]=1; b["y"]=2; b["z"]=3; union(a,b); result=keys(a) }"#,
        &[],
    );
    assert_eq!(rt.get_var("result"), "x\ny\nz");
}

// --- seq, sample ---

#[test]
fn seq_fills_range() {
    let rt = eval(
        r#"BEGIN { n=seq(a,3,7); result=n ":" join(a,",") }"#,
        &[],
    );
    assert_eq!(rt.get_var("result"), "5:3,4,5,6,7");
}

#[test]
fn seq_reverse_range() {
    let rt = eval(
        r#"BEGIN { seq(a,5,1); result=join(a,",") }"#,
        &[],
    );
    assert_eq!(rt.get_var("result"), "5,4,3,2,1");
}

#[test]
fn sample_reduces_array() {
    let rt = eval(
        r#"BEGIN { srand(42); seq(a,1,100); n=samp(a,5); result=n }"#,
        &[],
    );
    assert_eq!(rt.get_var("result"), "5");
}

// --- lpad, rpad ---

#[test]
fn lpad_pads_left() {
    let rt = eval(
        r#"BEGIN { result = lpad("hi", 6, ".") }"#,
        &[],
    );
    assert_eq!(rt.get_var("result"), "....hi");
}

#[test]
fn rpad_pads_right() {
    let rt = eval(
        r#"BEGIN { result = rpad("hi", 6) }"#,
        &[],
    );
    assert_eq!(rt.get_var("result"), "hi    ");
}

#[test]
fn lpad_no_truncate() {
    let rt = eval(
        r#"BEGIN { result = lpad("hello", 3) }"#,
        &[],
    );
    assert_eq!(rt.get_var("result"), "hello");
}

// --- shuffle ---

#[test]
fn shuffle_preserves_elements() {
    let rt = eval(
        r#"BEGIN { srand(1); a[1]="a"; a[2]="b"; a[3]="c"; shuf(a); result=length(a) }"#,
        &[],
    );
    assert_eq!(rt.get_var("result"), "3");
}

// --- slurp ---

#[test]
fn slurp_into_string() {
    let rt = eval(
        r#"BEGIN { s = slurp("/etc/shells"); result = (length(s) > 0) ? "ok" : "empty" }"#,
        &[],
    );
    assert_eq!(rt.get_var("result"), "ok");
}

#[test]
fn slurp_into_array() {
    let rt = eval(
        r#"BEGIN { n = slurp("/etc/shells", a); result = (n > 0) ? "ok" : "empty" }"#,
        &[],
    );
    assert_eq!(rt.get_var("result"), "ok");
}

// --- join defaults to OFS ---

#[test]
fn join_defaults_to_ofs() {
    let rt = eval(
        r#"BEGIN { OFS=","; a[1]="x"; a[2]="y"; a[3]="z"; result=join(a) }"#,
        &[],
    );
    assert_eq!(rt.get_var("result"), "x,y,z");
}

// --- diagnostics: clock, start, elapsed ---

#[test]
fn clk_returns_positive() {
    let rt = eval(r#"BEGIN { result = (clk() >= 0) ? "ok" : "bad" }"#, &[]);
    assert_eq!(rt.get_var("result"), "ok");
}

#[test]
fn tic_toc_basic() {
    let rt = eval(
        r#"BEGIN { tic("t"); for(i=0;i<10000;i++){} result = (toc("t") >= 0) ? "ok" : "bad" }"#,
        &[],
    );
    assert_eq!(rt.get_var("result"), "ok");
}

#[test]
fn toc_without_tic_uses_epoch() {
    let rt = eval(
        r#"BEGIN { result = (toc() >= 0) ? "ok" : "bad" }"#,
        &[],
    );
    assert_eq!(rt.get_var("result"), "ok");
}

#[test]
fn multiple_timers() {
    let rt = eval(
        r#"BEGIN {
            tic("a"); tic("b")
            for(i=0;i<10000;i++){}
            ea = toc("a"); eb = toc("b")
            result = (ea >= 0 && eb >= 0) ? "ok" : "bad"
        }"#,
        &[],
    );
    assert_eq!(rt.get_var("result"), "ok");
}

// --- diagnostics: dump ---

#[test]
fn dump_returns_one() {
    let rt = eval(r#"BEGIN { x = "hello"; result = dump(x) }"#, &[]);
    assert_eq!(rt.get_var("result"), "1");
}

#[test]
fn dump_array_returns_one() {
    let rt = eval(
        r#"BEGIN { a[1]="x"; a[2]="y"; result = dump(a) }"#,
        &[],
    );
    assert_eq!(rt.get_var("result"), "1");
}

#[test]
fn dump_to_file() {
    let rt = eval(
        r#"BEGIN { x = 42; dump(x, "/tmp/fk_test_dump.txt"); result = "ok" }"#,
        &[],
    );
    assert_eq!(rt.get_var("result"), "ok");
    let content = std::fs::read_to_string("/tmp/fk_test_dump.txt").unwrap_or_default();
    assert!(content.contains("dump:"), "dump file should contain output");
    let _ = std::fs::remove_file("/tmp/fk_test_dump.txt");
}

// --- in operator ---

#[test]
fn in_operator_with_field() {
    let rt = eval(r#"BEGIN{a["x"]=1} {if($0 in a) result="yes"}"#, &["x", "y"]);
    assert_eq!(rt.get_var("result"), "yes");
}

#[test]
fn in_operator_negated() {
    let rt = eval(r#"BEGIN{a["x"]=1} !($0 in a) {result=result $0}"#, &["x", "y"]);
    assert_eq!(rt.get_var("result"), "y");
}

#[test]
fn multidim_in_operator() {
    let rt = eval(
        r#"BEGIN{a["x",1]=1} {if(($1,$2) in a) result="yes"; else result="no"}"#,
        &["x 1"],
    );
    assert_eq!(rt.get_var("result"), "yes");
}

#[test]
fn multidim_in_operator_miss() {
    let rt = eval(
        r#"BEGIN{a["x",1]=1} {if(($1,$2) in a) result="yes"; else result="no"}"#,
        &["y 2"],
    );
    assert_eq!(rt.get_var("result"), "no");
}

// --- regex literal in sub/gsub ---

#[test]
fn sub_regex_literal() {
    let rt = eval(r#"{sub(/foo/,"bar"); result=$0}"#, &["foo baz foo"]);
    assert_eq!(rt.get_var("result"), "bar baz foo");
}

#[test]
fn gsub_regex_literal() {
    let rt = eval(r#"{gsub(/foo/,"bar"); result=$0}"#, &["foo baz foo"]);
    assert_eq!(rt.get_var("result"), "bar baz bar");
}

#[test]
fn match_regex_literal() {
    let rt = eval(r#"{match($0, /(\w+)=(\d+)/, m); result=m[1] ":" m[2]}"#, &["key=42"]);
    assert_eq!(rt.get_var("result"), "key:42");
}

// --- printf %c ---

#[test]
fn printf_percent_c_numeric() {
    let rt = eval(r#"BEGIN{result=sprintf("%c%c%c", 72, 105, 33)}"#, &[]);
    assert_eq!(rt.get_var("result"), "Hi!");
}

#[test]
fn printf_percent_c_string() {
    let rt = eval(r#"BEGIN{result=sprintf("%c", "A")}"#, &[]);
    assert_eq!(rt.get_var("result"), "A");
}

// --- length bare / length() ---

#[test]
fn length_bare() {
    let rt = eval(r#"{result=length}"#, &["hello"]);
    assert_eq!(rt.get_var("result"), "5");
}

#[test]
fn length_empty_parens() {
    let rt = eval(r#"{result=length()}"#, &["hello"]);
    assert_eq!(rt.get_var("result"), "5");
}
