use criterion::{black_box, criterion_group, criterion_main, Criterion, BenchmarkId};
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
                let rec = Record { text: line.clone(), fields: None };
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
                let rec = Record { text: line.clone(), fields: None };
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
                let rec = Record { text: line.clone(), fields: None };
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
                    let rec = Record { text: line.clone(), fields: None };
                    exec.run_record(black_box(&rec));
                }
                exec.run_end();
            })
        });
    }
    group.finish();
}

criterion_group!(benches, bench_simple_print, bench_field_access, bench_pattern_match, bench_accumulate);
criterion_main!(benches);
