use criterion::{black_box, criterion_group, criterion_main, Criterion};
use taiko::parser::{parse_tja_file, tja_file_bench};

pub fn tja_benchmark(c: &mut Criterion) {
    let ready_to = include_str!("../benches/Ready to.tja");
    c.bench_function("tja test: \"Ready to\"", |b| {
        b.iter(|| tja_file_bench(black_box(ready_to)))
    });
}

pub fn full_tja_benchmark(c: &mut Criterion) {
    let ready_to = include_str!("../benches/Ready to.tja");

    c.bench_function("tja test: \"Ready to\", full parse", |b| {
        b.iter(|| parse_tja_file(black_box(ready_to)).unwrap());
    });
}

criterion_group!(
    benches,
    tja_benchmark,
    full_tja_benchmark
);
criterion_main!(benches);
