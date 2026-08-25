use criterion::{criterion_group, criterion_main, BenchmarkId, Criterion};
use nexus_engine::benchmarking::BenchmarkSpatialFixture;
use std::hint::black_box;

fn spatial(c: &mut Criterion) {
    let mut rebuild = c.benchmark_group("spatial/rebuild");
    for population in [100, 1_000, 10_000] {
        let mut fixture = BenchmarkSpatialFixture::new(population).unwrap();
        rebuild.bench_with_input(
            BenchmarkId::from_parameter(population),
            &population,
            |b, _| {
                b.iter(|| black_box(fixture.rebuild()));
            },
        );
    }
    rebuild.finish();

    let mut query = c.benchmark_group("spatial/query");
    for population in [100, 1_000, 10_000] {
        let fixture = BenchmarkSpatialFixture::new(population).unwrap();
        query.bench_with_input(
            BenchmarkId::from_parameter(population),
            &population,
            |b, _| {
                b.iter(|| black_box(fixture.query_local_population()));
            },
        );
    }
    query.finish();
}

criterion_group!(benches, spatial);
criterion_main!(benches);
