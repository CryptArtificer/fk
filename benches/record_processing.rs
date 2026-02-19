use criterion::{BenchmarkId, Criterion, black_box, criterion_group, criterion_main};
use fk::action::Executor;
use fk::input::Record;
use fk::lexer::Lexer;
use fk::parser::Parser;
use fk::runtime::Runtime;

fn compile(src: &str) -> fk::parser::Program {
    let mut lex = Lexer::new(src);
    let tokens = lex.tokenize().unwrap();
    let mut par = Parser::new(tokens);
    par.parse().unwrap()
}

fn make_lines(n: usize) -> Vec<String> {
    (0..n)
        .map(|i| format!("{} field_{} {} extra", i, i % 100, i * 7))
        .collect()
}

fn bench_simple_print(c: &mut Criterion) {
    let program = compile("{ print $2 }");
    let lines = make_lines(1000);
    c.bench_function("record/simple_print_1k", |b| {
        b.iter(|| {
            let mut rt = Runtime::new();
            let mut exec = Executor::new(&program, &mut rt);
            exec.run_begin();
            for line in &lines {
                let rec = Record {
                    text: line.clone(),
                    fields: None,
                };
                exec.run_record(black_box(&rec));
            }
            exec.run_end();
        })
    });
}

fn bench_field_access(c: &mut Criterion) {
    let program = compile("{ x = $1 + $3 }");
    let lines = make_lines(1000);
    c.bench_function("record/field_access_1k", |b| {
        b.iter(|| {
            let mut rt = Runtime::new();
            let mut exec = Executor::new(&program, &mut rt);
            exec.run_begin();
            for line in &lines {
                let rec = Record {
                    text: line.clone(),
                    fields: None,
                };
                exec.run_record(black_box(&rec));
            }
            exec.run_end();
        })
    });
}

fn bench_pattern_match(c: &mut Criterion) {
    let program = compile("/field_42/ { count++ }");
    let lines = make_lines(1000);
    c.bench_function("record/pattern_match_1k", |b| {
        b.iter(|| {
            let mut rt = Runtime::new();
            let mut exec = Executor::new(&program, &mut rt);
            exec.run_begin();
            for line in &lines {
                let rec = Record {
                    text: line.clone(),
                    fields: None,
                };
                exec.run_record(black_box(&rec));
            }
            exec.run_end();
        })
    });
}

fn bench_accumulate(c: &mut Criterion) {
    let program = compile("{ sum += $1; count++ } END { avg = sum / count }");
    let mut group = c.benchmark_group("record/accumulate");
    for &n in &[100, 1_000, 10_000] {
        let lines = make_lines(n);
        group.bench_with_input(BenchmarkId::from_parameter(n), &lines, |b, lines| {
            b.iter(|| {
                let mut rt = Runtime::new();
                let mut exec = Executor::new(&program, &mut rt);
                exec.run_begin();
                for line in lines {
                    let rec = Record {
                        text: line.clone(),
                        fields: None,
                    };
                    exec.run_record(black_box(&rec));
                }
                exec.run_end();
            })
        });
    }
    group.finish();
}

fn bench_computed_regex(c: &mut Criterion) {
    let program = compile(r#"BEGIN { pat = "field_4[0-9]" } $0 ~ pat { count++ }"#);
    let lines = make_lines(1000);
    c.bench_function("record/computed_regex_1k", |b| {
        b.iter(|| {
            let mut rt = Runtime::new();
            let mut exec = Executor::new(&program, &mut rt);
            exec.run_begin();
            for line in &lines {
                let rec = Record {
                    text: line.clone(),
                    fields: None,
                };
                exec.run_record(black_box(&rec));
            }
            exec.run_end();
        })
    });
}

fn bench_do_while_break(c: &mut Criterion) {
    let program = compile(r#"{ i=1; do { if ($1+0 > 500) break; i++ } while (i <= 10) }"#);
    let lines = make_lines(1000);
    c.bench_function("record/do_while_break_1k", |b| {
        b.iter(|| {
            let mut rt = Runtime::new();
            let mut exec = Executor::new(&program, &mut rt);
            exec.run_begin();
            for line in &lines {
                let rec = Record {
                    text: line.clone(),
                    fields: None,
                };
                exec.run_record(black_box(&rec));
            }
            exec.run_end();
        })
    });
}

fn bench_multidim_array(c: &mut Criterion) {
    let program = compile(r#"{ a[$1 % 10, $3 % 10]++ } END { for (k in a) n++; print n }"#);
    let lines = make_lines(1000);
    c.bench_function("record/multidim_array_1k", |b| {
        b.iter(|| {
            let mut rt = Runtime::new();
            let mut exec = Executor::new(&program, &mut rt);
            exec.run_begin();
            for line in &lines {
                let rec = Record {
                    text: line.clone(),
                    fields: None,
                };
                exec.run_record(black_box(&rec));
            }
            exec.run_end();
        })
    });
}

fn bench_match_capture(c: &mut Criterion) {
    let program = compile(r#"{ match($0, "([0-9]+) (field_[0-9]+) ([0-9]+)", cap) }"#);
    let lines = make_lines(1000);
    c.bench_function("record/match_capture_1k", |b| {
        b.iter(|| {
            let mut rt = Runtime::new();
            let mut exec = Executor::new(&program, &mut rt);
            exec.run_begin();
            for line in &lines {
                let rec = Record {
                    text: line.clone(),
                    fields: None,
                };
                exec.run_record(black_box(&rec));
            }
            exec.run_end();
        })
    });
}

fn bench_string_builtins(c: &mut Criterion) {
    let program =
        compile(r#"{ x = trim("  " $2 "  "); y = reverse(x); z = startswith(x, "field") }"#);
    let lines = make_lines(1000);
    c.bench_function("record/string_builtins_1k", |b| {
        b.iter(|| {
            let mut rt = Runtime::new();
            let mut exec = Executor::new(&program, &mut rt);
            exec.run_begin();
            for line in &lines {
                let rec = Record {
                    text: line.clone(),
                    fields: None,
                };
                exec.run_record(black_box(&rec));
            }
            exec.run_end();
        })
    });
}

fn bench_math_builtins(c: &mut Criterion) {
    let program = compile(r#"{ x = abs($3 - 5000); y = ceil(x / 7); z = min(y, 100) }"#);
    let lines = make_lines(1000);
    c.bench_function("record/math_builtins_1k", |b| {
        b.iter(|| {
            let mut rt = Runtime::new();
            let mut exec = Executor::new(&program, &mut rt);
            exec.run_begin();
            for line in &lines {
                let rec = Record {
                    text: line.clone(),
                    fields: None,
                };
                exec.run_record(black_box(&rec));
            }
            exec.run_end();
        })
    });
}

criterion_group!(
    benches,
    bench_simple_print,
    bench_field_access,
    bench_pattern_match,
    bench_accumulate,
    bench_computed_regex,
    bench_do_while_break,
    bench_multidim_array,
    bench_match_capture,
    bench_string_builtins,
    bench_math_builtins
);
criterion_main!(benches);
