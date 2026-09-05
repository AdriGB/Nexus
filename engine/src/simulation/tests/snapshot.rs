use super::super::snapshot::SnapshotError;
use super::super::Simulation;
use super::support::*;
use crate::world::Grid;
use crate::WorldBridge;

const POPULATION: u32 = 10;

fn sample_grid() -> Grid {
    let rows = ["PPPFPPPPPP", "PPPPPPFPPP", "PFPPPPPPPP", "PPPPFPPPPP"];
    grid_from_rows(&rows)
}

#[test]
fn snapshot_roundtrip_preserves_exact_state_hash() {
    let mut world = sample_grid();
    let mut sim = Simulation::with_population(42, &world, POPULATION);

    // Advance 30 ticks so actions, events, memories are generated
    for _ in 0..30 {
        sim.step(&mut world);
    }

    let original_hash = sim.state_hash(&world);

    // Capture and serialize
    let snapshot = sim.to_snapshot(&world, Some(42), Some(0.0));
    assert_eq!(snapshot.header.state_hash, original_hash.to_string());
    assert_eq!(snapshot.header.created_at_tick, 30);
    assert_eq!(snapshot.scalars.tick, 30);

    let json = snapshot.to_json().expect("serialization should succeed");

    // Deserialize and restore
    let (restored_sim, restored_world) =
        Simulation::load_snapshot(&json).expect("restoration should succeed");

    let restored_hash = restored_sim.state_hash(&restored_world);
    assert_eq!(
        original_hash, restored_hash,
        "restored simulation state hash must exactly match original"
    );
}

#[test]
fn future_trajectory_deterministic_after_restore() {
    let mut world = sample_grid();
    let mut sim = Simulation::with_population(42, &world, POPULATION);

    for _ in 0..25 {
        sim.step(&mut world);
    }

    let json = sim
        .save_snapshot(&world, Some(42), Some(0.0))
        .expect("save_snapshot should succeed");
    let (mut restored_sim, mut restored_world) =
        Simulation::load_snapshot(&json).expect("load_snapshot should succeed");

    // Advance both for another 50 ticks and check parity at intervals
    for tick in 1..=50 {
        sim.step(&mut world);
        restored_sim.step(&mut restored_world);

        if tick % 10 == 0 || tick == 50 {
            let hash_orig = sim.state_hash(&world);
            let hash_restored = restored_sim.state_hash(&restored_world);
            assert_eq!(
                hash_orig, hash_restored,
                "diverged at future tick {} after restore: {} vs {}",
                tick, hash_orig, hash_restored
            );
        }
    }
}

#[test]
fn snapshot_with_world_bridge_roundtrip() {
    let mut bridge = WorldBridge::new(42, 32, 32, 0.0);

    for _ in 0..20 {
        bridge.simulation_step();
    }

    let original_hash = bridge.state_hash();
    let json = bridge
        .save_snapshot(Some(42), Some(0.0))
        .expect("bridge save_snapshot should succeed");

    let mut loaded_bridge =
        WorldBridge::load_snapshot(&json).expect("bridge load_snapshot should succeed");

    assert_eq!(
        loaded_bridge.state_hash(),
        original_hash,
        "loaded bridge state hash must match original"
    );

    // Advance both and verify future step parity
    for _ in 0..20 {
        bridge.simulation_step();
        loaded_bridge.simulation_step();
    }

    assert_eq!(
        bridge.state_hash(),
        loaded_bridge.state_hash(),
        "future steps on loaded bridge must remain in lockstep with original"
    );
}

#[test]
fn pretty_json_roundtrip_matches_compact() {
    let mut world = sample_grid();
    let mut sim = Simulation::with_population(42, &world, POPULATION);

    for _ in 0..15 {
        sim.step(&mut world);
    }

    let compact = sim.save_snapshot(&world, None, None).unwrap();
    let pretty = sim.save_snapshot_pretty(&world, None, None).unwrap();

    let (sim_c, world_c) = Simulation::load_snapshot(&compact).unwrap();
    let (sim_p, world_p) = Simulation::load_snapshot(&pretty).unwrap();

    assert_eq!(sim_c.state_hash(&world_c), sim_p.state_hash(&world_p));
}

#[test]
fn tampered_state_hash_rejected() {
    let mut world = sample_grid();
    let mut sim = Simulation::with_population(42, &world, POPULATION);
    sim.step(&mut world);

    let mut snapshot = sim.to_snapshot(&world, None, None);
    snapshot.header.state_hash = "nexus:0:0000000000000000".to_string();

    let Err(err) = snapshot.restore() else {
        panic!("expected StateHashMismatch error");
    };
    match err {
        SnapshotError::StateHashMismatch { expected, actual } => {
            assert_eq!(expected, "nexus:0:0000000000000000");
            assert_ne!(actual, expected);
        }
        other => panic!("expected StateHashMismatch, got {:?}", other),
    }
}

#[test]
fn tampered_data_fails_hash_verification() {
    let mut world = sample_grid();
    let mut sim = Simulation::with_population(42, &world, POPULATION);
    sim.step(&mut world);

    let mut snapshot = sim.to_snapshot(&world, None, None);
    // Tamper with entity hunger
    if let Some(first_entity) = snapshot.entities.first_mut() {
        first_entity.hunger += 10.0;
    }

    let Err(err) = snapshot.restore() else {
        panic!("expected StateHashMismatch error after entity tamper");
    };
    match err {
        SnapshotError::StateHashMismatch { .. } => {}
        other => panic!(
            "expected StateHashMismatch after entity tamper, got {:?}",
            other
        ),
    }
}

#[test]
fn invalid_format_rejected() {
    let world = sample_grid();
    let sim = Simulation::with_population(42, &world, POPULATION);
    let mut snapshot = sim.to_snapshot(&world, None, None);
    snapshot.header.format = "unsupported-format/v99".to_string();

    let Err(err) = snapshot.restore() else {
        panic!("expected InvalidFormat error");
    };
    match err {
        SnapshotError::InvalidFormat(msg) => {
            assert!(msg.contains("unsupported-format/v99"));
        }
        other => panic!("expected InvalidFormat, got {:?}", other),
    }
}

#[test]
fn corrupted_household_id_ordering_rejected() {
    let world = sample_grid();
    let sim = Simulation::with_population(42, &world, POPULATION);
    let mut snapshot = sim.to_snapshot(&world, None, None);

    // Synthesize households with decreasing or out-of-order IDs
    snapshot.scalars.next_household_id = 10;
    snapshot
        .households
        .push(crate::simulation::snapshot::HouseholdSnapshotV1 {
            id: 2,
            formed_tick: 1,
            dissolved_tick: None,
            inheritance: None,
            migration: None,
            residence_x: 0,
            residence_y: 0,
            storage_capacity: 100,
            storage_amounts: [0; 4],
        });
    snapshot
        .households
        .push(crate::simulation::snapshot::HouseholdSnapshotV1 {
            id: 1, // out of order: 1 after 2
            formed_tick: 1,
            dissolved_tick: None,
            inheritance: None,
            migration: None,
            residence_x: 0,
            residence_y: 0,
            storage_capacity: 100,
            storage_amounts: [0; 4],
        });

    let Err(err) = snapshot.restore() else {
        panic!("expected CorruptedData error");
    };
    match err {
        SnapshotError::CorruptedData(msg) => {
            assert!(msg.contains("household ids must be strictly ascending"));
        }
        other => panic!("expected CorruptedData, got {:?}", other),
    }
}

#[test]
fn lineage_and_complex_state_survives_roundtrip() {
    let mut world = sample_grid();
    let mut sim = Simulation::with_population(42, &world, POPULATION);

    // Seed some synthetic lineage
    let parents = vec![
        (None, None),
        (Some(1), None),
        (Some(1), Some(2)),
        (None, Some(2)),
        (Some(3), Some(4)),
        (None, None),
        (None, None),
        (None, None),
        (None, None),
        (None, None),
    ];
    sim.seed_test_lineage(&parents);

    // Run for 50 ticks
    for _ in 0..50 {
        sim.step(&mut world);
    }

    let json = sim
        .save_snapshot_pretty(&world, Some(42), Some(0.0))
        .unwrap();
    let (mut restored_sim, mut restored_world) = Simulation::load_snapshot(&json).unwrap();

    assert_eq!(
        sim.state_hash(&world),
        restored_sim.state_hash(&restored_world)
    );

    // Check genealogy records equality
    assert_eq!(sim.genealogy(), restored_sim.genealogy());

    // Step 50 more ticks
    for _ in 0..50 {
        sim.step(&mut world);
        restored_sim.step(&mut restored_world);
    }

    assert_eq!(
        sim.state_hash(&world),
        restored_sim.state_hash(&restored_world)
    );
}
