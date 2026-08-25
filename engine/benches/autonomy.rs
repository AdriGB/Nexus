use criterion::{criterion_group, criterion_main, BatchSize, BenchmarkId, Criterion};
use nexus_engine::benchmarking::BenchmarkAutonomyFixture;
use std::hint::black_box;

fn autonomy(c: &mut Criterion) {
    let mut group = c.benchmark_group("autonomy");
    for population in [100, 1_000] {
        let fixture = BenchmarkAutonomyFixture::new(population).unwrap();
        group.bench_with_input(
            BenchmarkId::from_parameter(population),
            &population,
            |b, _| {
                b.iter_batched(
                    || fixture.prepare(),
                    |mut run| black_box(run.run_once()),
                    BatchSize::SmallInput,
                );
            },
        );
    }
    group.finish();
}

criterion_group!(benches, autonomy);
criterion_main!(benches);
