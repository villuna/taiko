use criterion::{black_box, criterion_group, criterion_main, Criterion};
use taiko::parser::{parse_tja_file, tja_file_bench};

pub fn tja_benchmark(c: &mut Criterion) {
    let ready_to = include_str!("../example-tracks/Ready To/Ready to.tja");
    c.bench_function("tja test: \"Ready to\"", |b| {
        b.iter(|| tja_file_bench(black_box(ready_to)))
    });
}

pub fn tja_benchmark_with_read_str(c: &mut Criterion) {
    c.bench_function("tja test: \"Ready to\", reading file at runtime", |b| {
        b.iter(|| {
            let ready_to =
                black_box(std::fs::read_to_string("example-tracks/Ready To/Ready to.tja").unwrap());
            tja_file_bench(black_box(&ready_to));
        })
    });
}

pub fn full_tja_benchmark(c: &mut Criterion) {
    let ready_to = include_str!("../example-tracks/Ready To/Ready to.tja");

    c.bench_function("tja test: \"Ready to\", full parse", |b| {
        b.iter(|| parse_tja_file(black_box(ready_to)).unwrap());
    });
}

criterion_group!(
    benches,
    tja_benchmark,
    tja_benchmark_with_read_str,
    full_tja_benchmark
);
criterion_main!(benches);
