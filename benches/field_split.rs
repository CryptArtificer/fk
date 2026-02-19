use criterion::{BenchmarkId, Criterion, black_box, criterion_group, criterion_main};
use fk::field;

fn make_whitespace_line(n: usize) -> String {
    (0..n).map(|i| i.to_string()).collect::<Vec<_>>().join("  ")
}

fn make_comma_line(n: usize) -> String {
    (0..n)
        .map(|i| format!("field{}", i))
        .collect::<Vec<_>>()
        .join(",")
}

fn make_multichar_line(n: usize) -> String {
    (0..n)
        .map(|i| format!("val{}", i))
        .collect::<Vec<_>>()
        .join("::")
}

fn bench_whitespace_split(c: &mut Criterion) {
    let mut group = c.benchmark_group("field_split/whitespace");
    for &n in &[10, 100, 1000] {
        let line = make_whitespace_line(n);
        group.bench_with_input(BenchmarkId::from_parameter(n), &line, |b, line| {
            b.iter(|| field::split(black_box(line), " "))
        });
    }
    group.finish();
}

fn bench_single_char_split(c: &mut Criterion) {
    let mut group = c.benchmark_group("field_split/single_char");
    for &n in &[10, 100, 1000] {
        let line = make_comma_line(n);
        group.bench_with_input(BenchmarkId::from_parameter(n), &line, |b, line| {
            b.iter(|| field::split(black_box(line), ","))
        });
    }
    group.finish();
}

fn bench_multichar_split(c: &mut Criterion) {
    let mut group = c.benchmark_group("field_split/multi_char");
    for &n in &[10, 100, 1000] {
        let line = make_multichar_line(n);
        group.bench_with_input(BenchmarkId::from_parameter(n), &line, |b, line| {
            b.iter(|| field::split(black_box(line), "::"))
        });
    }
    group.finish();
}

criterion_group!(
    benches,
    bench_whitespace_split,
    bench_single_char_split,
    bench_multichar_split
);
criterion_main!(benches);
