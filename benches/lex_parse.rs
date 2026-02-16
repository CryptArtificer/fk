use criterion::{black_box, criterion_group, criterion_main, Criterion};
use fk::lexer::Lexer;
use fk::parser::Parser;

const SIMPLE_PRINT: &str = r#"{ print $1 }"#;

const FIELD_MATH: &str = r#"{ sum += $3; count++ } END { print sum / count }"#;

const REALISTIC: &str = r#"
BEGIN { FS = "," }
/error/ {
    errors++
    if ($3 > 100) {
        printf "CRITICAL: %s at line %d\n", $2, NR
        critical++
    } else {
        printf "warning: %s\n", $2
    }
}
$1 == "INFO" {
    info++
}
END {
    printf "errors=%d critical=%d info=%d\n", errors, critical, info
}
"#;

const FUNCTIONS: &str = r#"
function abs(x) { return x < 0 ? -x : x }
function max(a, b) { return a > b ? a : b }
function clamp(x, lo, hi) {
    if (x < lo) return lo
    if (x > hi) return hi
    return x
}
BEGIN { print max(abs(-5), clamp(10, 0, 7)) }
"#;

const PHASE7_CONTROL: &str = r#"
BEGIN { FS = "," }
{
    i = 1
    do {
        if ($i ~ /^[0-9]+$/ && $i + 0 > 500) {
            a[$i]++
            if ($i + 0 > 9000) break
        }
        i++
    } while (i <= NF)
}
/^quit/ { exit 0 }
END {
    for (k in a) { n++; total += k }
    printf "keys=%d total=%d\n", n, total
}
"#;

fn bench_lex(c: &mut Criterion) {
    let mut group = c.benchmark_group("lexer");
    for (name, src) in [
        ("simple_print", SIMPLE_PRINT),
        ("field_math", FIELD_MATH),
        ("realistic", REALISTIC),
        ("functions", FUNCTIONS),
        ("phase7_control", PHASE7_CONTROL),
    ] {
        group.bench_function(name, |b| {
            b.iter(|| {
                let mut lex = Lexer::new(black_box(src));
                lex.tokenize().unwrap()
            })
        });
    }
    group.finish();
}

fn bench_parse(c: &mut Criterion) {
    let mut group = c.benchmark_group("parser");
    for (name, src) in [
        ("simple_print", SIMPLE_PRINT),
        ("field_math", FIELD_MATH),
        ("realistic", REALISTIC),
        ("functions", FUNCTIONS),
        ("phase7_control", PHASE7_CONTROL),
    ] {
        let mut lex = Lexer::new(src);
        let tokens = lex.tokenize().unwrap();
        group.bench_function(name, |b| {
            b.iter(|| {
                let mut par = Parser::new(black_box(tokens.clone()));
                par.parse().unwrap()
            })
        });
    }
    group.finish();
}

fn bench_lex_and_parse(c: &mut Criterion) {
    let mut group = c.benchmark_group("lex+parse");
    for (name, src) in [
        ("simple_print", SIMPLE_PRINT),
        ("field_math", FIELD_MATH),
        ("realistic", REALISTIC),
        ("functions", FUNCTIONS),
        ("phase7_control", PHASE7_CONTROL),
    ] {
        group.bench_function(name, |b| {
            b.iter(|| {
                let mut lex = Lexer::new(black_box(src));
                let tokens = lex.tokenize().unwrap();
                let mut par = Parser::new(tokens);
                par.parse().unwrap()
            })
        });
    }
    group.finish();
}

criterion_group!(benches, bench_lex, bench_parse, bench_lex_and_parse);
criterion_main!(benches);
