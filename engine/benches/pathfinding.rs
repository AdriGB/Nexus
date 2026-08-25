use criterion::{criterion_group, criterion_main, Criterion};
use nexus_engine::benchmarking::BenchmarkPathfindingFixture;
use std::hint::black_box;

fn pathfinding(c: &mut Criterion) {
    let mut group = c.benchmark_group("pathfinding");
    for (name, mut fixture) in [
        ("short", BenchmarkPathfindingFixture::short()),
        ("long", BenchmarkPathfindingFixture::long()),
        (
            "mixed-terrain",
            BenchmarkPathfindingFixture::mixed_terrain(),
        ),
    ] {
        group.bench_function(name, |b| b.iter(|| black_box(fixture.run())));
    }
    group.finish();
}

criterion_group!(benches, pathfinding);
criterion_main!(benches);
