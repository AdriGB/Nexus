//! Kinship query costs measured against a lineage that actually has one.
//!
//! Every scenario in the suite reports `genealogy_links = 0`: `GESTATION_TICKS`
//! is 6,720 and the short scenarios run 124 ticks, so no entity is ever born.
//! `long-run-1000` does produce births, but only one generation, and only after
//! its social graph has decayed tenfold — measured in #200, where kinship turned
//! out to be 0.94% of the step. That leaves the kinship paths that return
//! *something* completely unmeasured, which is what this bench is for. See #199.
//!
//! `children_scan` is kept deliberately: it is the linear filter that #198
//! replaced, preserved so the index can be quantified against it with real
//! children in the tree. The scenario A/B could not do that, because there both
//! implementations returned nothing and the cost was zero either way.

use criterion::{criterion_group, criterion_main, BenchmarkId, Criterion};
use nexus_engine::benchmarking::BenchmarkKinshipFixture;
use std::hint::black_box;
use std::time::Duration;

/// Must be nonzero multiples of `KINSHIP_FIXTURE_GENERATIONS` (10), so that the
/// generations are all the same size.
const POPULATIONS: [u32; 3] = [100, 1_000, 10_000];

fn children(c: &mut Criterion) {
    let mut index = c.benchmark_group("kinship/children_index");
    for population in POPULATIONS {
        let fixture = BenchmarkKinshipFixture::new(population).unwrap();
        index.bench_with_input(
            BenchmarkId::from_parameter(population),
            &population,
            |b, _| {
                b.iter(|| black_box(fixture.children_of_index()));
            },
        );
    }
    index.finish();

    // Identical lookups without the per-call `Vec` that `children_of` copies
    // into. The difference against `children_index` is what #198's leftover
    // `.to_vec()` costs.
    let mut no_copy = c.benchmark_group("kinship/children_index_no_copy");
    for population in POPULATIONS {
        let fixture = BenchmarkKinshipFixture::new(population).unwrap();
        no_copy.bench_with_input(
            BenchmarkId::from_parameter(population),
            &population,
            |b, _| {
                b.iter(|| black_box(fixture.children_of_index_without_copy()));
            },
        );
    }
    no_copy.finish();

    let mut scan = c.benchmark_group("kinship/children_scan");
    for population in POPULATIONS {
        let fixture = BenchmarkKinshipFixture::new(population).unwrap();
        scan.bench_with_input(
            BenchmarkId::from_parameter(population),
            &population,
            |b, _| {
                b.iter(|| black_box(fixture.children_of_scan()));
            },
        );
    }
    scan.finish();
}

fn traversal(c: &mut Criterion) {
    let mut descendants = c.benchmark_group("kinship/descendants");
    for population in POPULATIONS {
        let fixture = BenchmarkKinshipFixture::new(population).unwrap();
        descendants.bench_with_input(
            BenchmarkId::from_parameter(population),
            &population,
            |b, _| {
                b.iter(|| black_box(fixture.descendants()));
            },
        );
    }
    descendants.finish();

    // `relationship_between` ends in a nested scan over both ancestor sets, so
    // this is by far the most expensive routine here: at 10,000 each ancestor
    // set saturates near the full generation size. Criterion's default 100
    // samples would make the run take minutes, so the group is told to take
    // fewer, longer samples instead.
    let mut relationships = c.benchmark_group("kinship/relationships");
    relationships.warm_up_time(Duration::from_secs(1));
    relationships.measurement_time(Duration::from_secs(5));
    relationships.sample_size(10);
    for population in POPULATIONS {
        let fixture = BenchmarkKinshipFixture::new(population).unwrap();
        relationships.bench_with_input(
            BenchmarkId::from_parameter(population),
            &population,
            |b, _| {
                b.iter(|| black_box(fixture.relationships()));
            },
        );
    }
    relationships.finish();
}

criterion_group!(benches, children, traversal);
criterion_main!(benches);
