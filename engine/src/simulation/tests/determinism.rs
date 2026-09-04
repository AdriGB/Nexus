use super::super::Simulation;
use super::support::*;
use crate::world::Grid;

const POPULATION: u32 = 10;
const CHECKPOINT_TICKS: &[u64] = &[1, 10, 24, 50, 100];

fn sample_grid() -> Grid {
    let rows = ["PPPFPPPPPP", "PPPPPPFPPP", "PFPPPPPPPP", "PPPPFPPPPP"];
    grid_from_rows(&rows)
}

#[test]
fn same_seed_produces_identical_state_hash_at_every_checkpoint() {
    let mut world_a = sample_grid();
    let mut world_b = sample_grid();
    let mut sim_a = Simulation::with_population(42, &world_a, POPULATION);
    let mut sim_b = Simulation::with_population(42, &world_b, POPULATION);

    let mut current_tick = 0;
    for &target_tick in CHECKPOINT_TICKS {
        while current_tick < target_tick {
            sim_a.step(&mut world_a);
            sim_b.step(&mut world_b);
            current_tick += 1;
        }

        let hash_a = sim_a.state_hash(&world_a);
        let hash_b = sim_b.state_hash(&world_b);
        assert_eq!(
            hash_a, hash_b,
            "state hash diverged at tick {target_tick}: {hash_a} != {hash_b}"
        );
    }
}

#[test]
fn different_seeds_produce_different_state_hashes() {
    let mut world_a = sample_grid();
    let mut world_b = sample_grid();
    let mut sim_a = Simulation::with_population(42, &world_a, POPULATION);
    let mut sim_b = Simulation::with_population(999, &world_b, POPULATION);

    for _ in 0..20 {
        sim_a.step(&mut world_a);
        sim_b.step(&mut world_b);
    }

    let hash_a = sim_a.state_hash(&world_a);
    let hash_b = sim_b.state_hash(&world_b);
    assert_ne!(
        hash_a, hash_b,
        "different seeds unexpectedly produced identical hash: {hash_a}"
    );
}

#[test]
fn continuous_step_matches_batched_step_state_hash() {
    let mut world_continuous = sample_grid();
    let mut world_batched = sample_grid();
    let mut sim_continuous = Simulation::with_population(42, &world_continuous, POPULATION);
    let mut sim_batched = Simulation::with_population(42, &world_batched, POPULATION);

    // 100 single-tick steps
    for _ in 0..100 {
        sim_continuous.step(&mut world_continuous);
    }

    // 10 blocks of 10 steps via advance (must call resume() first as paused defaults to true)
    sim_batched.resume();
    for _ in 0..10 {
        sim_batched.advance(10, &mut world_batched);
    }

    assert_eq!(sim_continuous.tick(), 100);
    assert_eq!(sim_batched.tick(), 100);

    let hash_c = sim_continuous.state_hash(&world_continuous);
    let hash_b = sim_batched.state_hash(&world_batched);
    assert_eq!(
        hash_c, hash_b,
        "batching altered simulation state hash: {hash_c} != {hash_b}"
    );
}

#[test]
fn profiled_step_produces_identical_state_hash() {
    let mut world_normal = sample_grid();
    let mut world_profiled = sample_grid();
    let mut sim_normal = Simulation::with_population(42, &world_normal, POPULATION);
    let mut sim_profiled = Simulation::with_population(42, &world_profiled, POPULATION);

    for _ in 0..100 {
        sim_normal.step(&mut world_normal);
        let _profile = sim_profiled.profile_step(&mut world_profiled);
    }

    let hash_normal = sim_normal.state_hash(&world_normal);
    let hash_profiled = sim_profiled.state_hash(&world_profiled);
    assert_eq!(
        hash_normal, hash_profiled,
        "profiling altered logical simulation state: normal={hash_normal}, profiled={hash_profiled}"
    );
}

#[test]
fn profile_autonomy_step_produces_identical_state_hash() {
    let mut world_normal = sample_grid();
    let mut world_profiled = sample_grid();
    let mut sim_normal = Simulation::with_population(42, &world_normal, POPULATION);
    let mut sim_profiled = Simulation::with_population(42, &world_profiled, POPULATION);

    for _ in 0..50 {
        sim_normal.step(&mut world_normal);
        let _profile = sim_profiled.profile_autonomy_step(&mut world_profiled);
    }

    let hash_normal = sim_normal.state_hash(&world_normal);
    let hash_profiled = sim_profiled.state_hash(&world_profiled);
    assert_eq!(
        hash_normal, hash_profiled,
        "autonomy profiling altered logical state: normal={hash_normal}, profiled={hash_profiled}"
    );
}

#[test]
fn mutation_tripwire_detects_state_perturbations() {
    let mut world = sample_grid();
    let mut sim = Simulation::with_population(42, &world, POPULATION);
    for _ in 0..10 {
        sim.step(&mut world);
    }

    let base_hash = sim.state_hash(&world);

    // Perturbation 1: modify an entity's health
    let mut sim_mutated = sim.clone();
    sim_mutated.entities_mut()[0].health -= 1.0;
    assert_ne!(
        base_hash,
        sim_mutated.state_hash(&world),
        "health perturbation was not caught by state hash"
    );

    // Perturbation 2: modify an entity's position
    let mut sim_mutated = sim.clone();
    sim_mutated.entities_mut()[0].x += 1;
    assert_ne!(
        base_hash,
        sim_mutated.state_hash(&world),
        "position perturbation was not caught by state hash"
    );

    // Perturbation 3: modify an entity's hunger
    let mut sim_mutated = sim.clone();
    sim_mutated.entities_mut()[0].hunger += 0.5;
    assert_ne!(
        base_hash,
        sim_mutated.state_hash(&world),
        "hunger perturbation was not caught by state hash"
    );

    // Perturbation 4: modify a resource deposit in the world
    let mut world_mutated = world.clone();
    if let Some(deposit) = world_mutated.resources.iter_mut().flatten().next() {
        deposit.amount = deposit.amount.saturating_add(10);
        assert_ne!(
            base_hash,
            sim.state_hash(&world_mutated),
            "world resource perturbation was not caught by state hash"
        );
    }

    // Perturbation 5: modify a tile's terrain in the world
    let mut world_mutated = world.clone();
    let current_terrain = world_mutated.tiles[0].terrain;
    world_mutated.tiles[0].terrain = if current_terrain == crate::world::Terrain::Mountain {
        crate::world::Terrain::Plains
    } else {
        crate::world::Terrain::Mountain
    };
    assert_ne!(
        base_hash,
        sim.state_hash(&world_mutated),
        "tile terrain perturbation was not caught by state hash"
    );
}

#[test]
fn golden_hash_snapshot_pin() {
    let mut world = sample_grid();
    let mut sim = Simulation::with_population(42, &world, POPULATION);
    for _ in 0..100 {
        sim.step(&mut world);
    }
    let hash = sim.state_hash(&world);
    assert_eq!(
        hash.to_string(),
        "be22c30e48d6e2c3",
        "golden state hash drifted! An unapproved change affected deterministic simulation state"
    );
}

#[test]
fn golden_hash_households_pin() {
    let mut world = sample_grid();
    let mut sim = Simulation::with_population(42, &world, POPULATION);
    sim.form_household_for_partnership(1, 2, 0);
    sim.form_household_for_partnership(3, 4, 0);
    for _ in 0..50 {
        sim.step(&mut world);
    }
    let hash = sim.state_hash(&world);
    assert_eq!(
        hash.to_string(),
        "44cd20c95b0b6981",
        "households golden state hash drifted!"
    );
}

#[test]
fn golden_hash_lineage_pin() {
    let mut world = sample_grid();
    let mut sim = Simulation::with_population(42, &world, POPULATION);
    let parents = [
        (None, None),
        (None, None),
        (None, None),
        (None, None),
        (Some(1), Some(2)),
        (Some(1), Some(2)),
        (Some(3), Some(4)),
        (Some(5), Some(7)),
        (Some(5), Some(7)),
        (Some(6), Some(7)),
    ];
    sim.seed_test_lineage(&parents);

    for _ in 0..50 {
        sim.step(&mut world);
    }
    let hash = sim.state_hash(&world);
    assert_eq!(
        hash.to_string(),
        "1000e3ba396acf6a",
        "lineage golden state hash drifted!"
    );
}
