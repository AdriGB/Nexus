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

#[test]
fn unsupported_hash_version_returns_unsupported_version_error() {
    let world = sample_grid();
    let sim = Simulation::with_population(42, &world, POPULATION);
    let mut snapshot = sim.to_snapshot(&world, None, None);
    snapshot.header.hash_version = 999;

    let Err(err) = snapshot.restore() else {
        panic!("expected UnsupportedVersion error");
    };
    match err {
        SnapshotError::UnsupportedVersion(msg) => {
            assert!(msg.contains("hash version"));
            assert!(msg.contains("999"));
        }
        other => panic!("expected UnsupportedVersion, got {:?}", other),
    }
}

#[test]
fn unsupported_engine_version_returns_unsupported_version_error() {
    let world = sample_grid();
    let sim = Simulation::with_population(42, &world, POPULATION);
    let mut snapshot = sim.to_snapshot(&world, None, None);
    snapshot.header.engine_version = "99.0.0".to_string();

    let Err(err) = snapshot.restore() else {
        panic!("expected UnsupportedVersion error");
    };
    match err {
        SnapshotError::UnsupportedVersion(msg) => {
            assert!(msg.contains("engine version"));
            assert!(msg.contains("99.0.0"));
        }
        other => panic!("expected UnsupportedVersion, got {:?}", other),
    }
}

#[test]
fn test_all_enum_variants_roundtrip_and_no_collision() {
    use crate::simulation::autonomy::{Action, Goal};
    use crate::simulation::entity::EntityActivity;
    use crate::simulation::events::{SimulationEventCause, SimulationEventKind};
    use crate::simulation::inventory::ItemKind;
    use crate::simulation::snapshot::*;
    use crate::world::{ResourceKind, Terrain};
    use std::collections::HashSet;

    // 1. Goal
    let all_goals = [
        Goal::Eat,
        Goal::AcquireResource,
        Goal::ConfrontHouseholdMember,
        Goal::Explore,
        Goal::Follow,
        Goal::Grieve,
        Goal::MigrateHousehold,
        Goal::ProtectDependent,
        Goal::Rest,
        Goal::Socialize,
        Goal::ShareFood,
    ];
    // Compile-time exhaustiveness check:
    for &goal in &all_goals {
        match goal {
            Goal::Eat => {}
            Goal::AcquireResource => {}
            Goal::ConfrontHouseholdMember => {}
            Goal::Explore => {}
            Goal::Follow => {}
            Goal::Grieve => {}
            Goal::MigrateHousehold => {}
            Goal::ProtectDependent => {}
            Goal::Rest => {}
            Goal::Socialize => {}
            Goal::ShareFood => {}
        }
    }
    let mut goal_codes = HashSet::new();
    for &goal in &all_goals {
        let code = goal_to_u32(goal);
        assert!(goal_codes.insert(code), "Goal collision for code {code}");
        assert_eq!(u32_to_goal(code).unwrap(), goal);
        assert_eq!(Goal::try_from(code).unwrap(), goal);
    }
    assert_eq!(goal_codes.len(), all_goals.len());

    // 2. Action
    let all_actions = [
        Action::MoveTo(10, 20),
        Action::Gather(ResourceKind::Food),
        Action::Consume(ResourceKind::Food),
        Action::ExploreArea(5, 15),
        Action::Wait,
        Action::ApproachEntity(42),
        Action::Interact(42),
        Action::ShareFood(42),
        Action::DepositHouseholdFood(100),
        Action::WithdrawHouseholdFood(50),
    ];
    // Compile-time exhaustiveness check:
    for &action in &all_actions {
        match action {
            Action::MoveTo(_, _) => {}
            Action::Gather(_) => {}
            Action::Consume(_) => {}
            Action::ExploreArea(_, _) => {}
            Action::Wait => {}
            Action::ApproachEntity(_) => {}
            Action::Interact(_) => {}
            Action::ShareFood(_) => {}
            Action::DepositHouseholdFood(_) => {}
            Action::WithdrawHouseholdFood(_) => {}
        }
    }
    for &action in &all_actions {
        let dto = action_to_dto(&action);
        let from_dto: ActionSnapshotV1 = action.into();
        assert_eq!(dto, from_dto);
        assert_eq!(dto_to_action(&dto).unwrap(), action);
        assert_eq!(Action::try_from(dto).unwrap(), action);
    }

    // 3. SimulationEventKind
    let all_event_kinds = [
        SimulationEventKind::Interaction,
        SimulationEventKind::Birth,
        SimulationEventKind::Death,
        SimulationEventKind::Discovery,
        SimulationEventKind::Consumption,
        SimulationEventKind::Encounter,
        SimulationEventKind::AffinityChange,
        SimulationEventKind::PartnershipFormed,
        SimulationEventKind::PartnershipDissolved,
        SimulationEventKind::FoodShared,
        SimulationEventKind::FoodShareRefused,
        SimulationEventKind::HouseholdConflict,
    ];
    for &kind in &all_event_kinds {
        match kind {
            SimulationEventKind::Interaction => {}
            SimulationEventKind::Birth => {}
            SimulationEventKind::Death => {}
            SimulationEventKind::Discovery => {}
            SimulationEventKind::Consumption => {}
            SimulationEventKind::Encounter => {}
            SimulationEventKind::AffinityChange => {}
            SimulationEventKind::PartnershipFormed => {}
            SimulationEventKind::PartnershipDissolved => {}
            SimulationEventKind::FoodShared => {}
            SimulationEventKind::FoodShareRefused => {}
            SimulationEventKind::HouseholdConflict => {}
        }
    }
    let mut event_kind_codes = HashSet::new();
    for &kind in &all_event_kinds {
        let code = event_kind_to_u32(kind);
        assert!(
            event_kind_codes.insert(code),
            "EventKind collision for code {code}"
        );
        assert_eq!(u32_to_event_kind(code).unwrap(), kind);
        assert_eq!(SimulationEventKind::try_from(code).unwrap(), kind);
    }
    assert_eq!(event_kind_codes.len(), all_event_kinds.len());

    // 4. SimulationEventCause
    let all_event_causes = [
        SimulationEventCause::MutualSocialContact,
        SimulationEventCause::Born,
        SimulationEventCause::Starvation,
        SimulationEventCause::NaturalDeath,
        SimulationEventCause::AteFood,
        SimulationEventCause::ResourceFound,
        SimulationEventCause::FirstEncounter,
        SimulationEventCause::RelationshipDecay,
        SimulationEventCause::FoodShared,
        SimulationEventCause::FoodShareRefused,
        SimulationEventCause::MutualCommitment,
        SimulationEventCause::HouseholdConflict,
    ];
    for &cause in &all_event_causes {
        match cause {
            SimulationEventCause::MutualSocialContact => {}
            SimulationEventCause::Born => {}
            SimulationEventCause::Starvation => {}
            SimulationEventCause::NaturalDeath => {}
            SimulationEventCause::AteFood => {}
            SimulationEventCause::ResourceFound => {}
            SimulationEventCause::FirstEncounter => {}
            SimulationEventCause::RelationshipDecay => {}
            SimulationEventCause::FoodShared => {}
            SimulationEventCause::FoodShareRefused => {}
            SimulationEventCause::MutualCommitment => {}
            SimulationEventCause::HouseholdConflict => {}
        }
    }
    let mut event_cause_codes = HashSet::new();
    for &cause in &all_event_causes {
        let code = event_cause_to_u32(cause);
        assert!(
            event_cause_codes.insert(code),
            "EventCause collision for code {code}"
        );
        assert_eq!(u32_to_event_cause(code).unwrap(), cause);
        assert_eq!(SimulationEventCause::try_from(code).unwrap(), cause);
    }
    assert_eq!(event_cause_codes.len(), all_event_causes.len());

    // 5. ItemKind
    let all_item_kinds = [
        ItemKind::Food,
        ItemKind::Timber,
        ItemKind::Stone,
        ItemKind::Iron,
    ];
    for &item in &all_item_kinds {
        match item {
            ItemKind::Food => {}
            ItemKind::Timber => {}
            ItemKind::Stone => {}
            ItemKind::Iron => {}
        }
    }
    let mut item_kind_codes = HashSet::new();
    for &item in &all_item_kinds {
        let code = item_kind_to_u8(item);
        assert!(
            item_kind_codes.insert(code),
            "ItemKind collision for code {code}"
        );
        assert_eq!(u8_to_item_kind(code).unwrap(), item);
        assert_eq!(ItemKind::try_from(code).unwrap(), item);
    }
    assert_eq!(item_kind_codes.len(), all_item_kinds.len());

    // 6. Terrain
    let all_terrains = [
        Terrain::DeepWater,
        Terrain::ShallowWater,
        Terrain::Beach,
        Terrain::Plains,
        Terrain::Grassland,
        Terrain::Forest,
        Terrain::DenseForest,
        Terrain::Hills,
        Terrain::Mountain,
        Terrain::SnowPeak,
        Terrain::Desert,
        Terrain::Swamp,
        Terrain::Tundra,
    ];
    for &terrain in &all_terrains {
        match terrain {
            Terrain::DeepWater => {}
            Terrain::ShallowWater => {}
            Terrain::Beach => {}
            Terrain::Plains => {}
            Terrain::Grassland => {}
            Terrain::Forest => {}
            Terrain::DenseForest => {}
            Terrain::Hills => {}
            Terrain::Mountain => {}
            Terrain::SnowPeak => {}
            Terrain::Desert => {}
            Terrain::Swamp => {}
            Terrain::Tundra => {}
        }
    }
    let mut terrain_codes = HashSet::new();
    for &terrain in &all_terrains {
        let code = terrain_to_u8(terrain);
        assert!(
            terrain_codes.insert(code),
            "Terrain collision for code {code}"
        );
        assert_eq!(u8_to_terrain(code).unwrap(), terrain);
        assert_eq!(Terrain::try_from(code).unwrap(), terrain);
    }
    assert_eq!(terrain_codes.len(), all_terrains.len());

    // 7. ResourceKind
    let all_resource_kinds = [
        ResourceKind::Food,
        ResourceKind::Timber,
        ResourceKind::Stone,
        ResourceKind::Iron,
    ];
    for &rk in &all_resource_kinds {
        match rk {
            ResourceKind::Food => {}
            ResourceKind::Timber => {}
            ResourceKind::Stone => {}
            ResourceKind::Iron => {}
        }
    }
    let mut rk_codes = HashSet::new();
    for &rk in &all_resource_kinds {
        let code = resource_kind_to_u8(rk);
        assert!(
            rk_codes.insert(code),
            "ResourceKind collision for code {code}"
        );
        assert_eq!(u8_to_resource_kind(code).unwrap(), rk);
        assert_eq!(ResourceKind::try_from(code).unwrap(), rk);
    }
    assert_eq!(rk_codes.len(), all_resource_kinds.len());

    // 8. EntityActivity
    let all_activities = [
        EntityActivity::Idle,
        EntityActivity::SeekingFood,
        EntityActivity::Moving,
        EntityActivity::Starving,
        EntityActivity::Exploring,
        EntityActivity::Resting,
        EntityActivity::Socializing,
    ];
    for &activity in &all_activities {
        match activity {
            EntityActivity::Idle => {}
            EntityActivity::SeekingFood => {}
            EntityActivity::Moving => {}
            EntityActivity::Starving => {}
            EntityActivity::Exploring => {}
            EntityActivity::Resting => {}
            EntityActivity::Socializing => {}
        }
    }
    let mut activity_codes = HashSet::new();
    for &activity in &all_activities {
        let code = activity_to_u32(activity);
        assert!(
            activity_codes.insert(code),
            "EntityActivity collision for code {code}"
        );
        assert_eq!(u32_to_activity(code).unwrap(), activity);
        assert_eq!(EntityActivity::try_from(code).unwrap(), activity);
    }
    assert_eq!(activity_codes.len(), all_activities.len());
}

#[cfg(feature = "benchmarks")]
#[test]
fn snapshot_roundtrip_lineage_1000_preserves_golden_hash() {
    let scenario = crate::benchmarking::find_scenario("lineage-1000")
        .expect("lineage-1000 benchmark scenario must exist");
    let (mut world, mut sim) =
        crate::benchmarking::prepare_scenario(scenario).expect("prepare_scenario should succeed");

    // 1. Run warmup ticks (24)
    for _ in 0..scenario.warmup_ticks {
        sim.step(&mut world);
    }

    // 2. Run measured ticks (100)
    for _ in 0..scenario.measured_ticks {
        sim.step(&mut world);
    }

    // Verify state hash matches golden baseline exactly
    let golden_hash = sim.state_hash(&world).to_string();
    assert_eq!(
        golden_hash, "2a87c12bc48937a2",
        "state hash after measured run must exactly match lineage-1000 golden baseline"
    );

    // 3. Save snapshot
    let json = sim
        .save_snapshot(&world, Some(scenario.seed), Some(scenario.world.sea_level))
        .expect("save_snapshot should succeed");

    // 4. Restore snapshot
    let (mut restored_sim, mut restored_world) =
        Simulation::load_snapshot(&json).expect("load_snapshot should succeed");

    assert_eq!(
        restored_sim.state_hash(&restored_world).to_string(),
        golden_hash,
        "restored simulation state hash must exactly match golden baseline"
    );

    // 5. Advance both by 25 ticks and verify parity at every step
    for tick in 1..=25 {
        sim.step(&mut world);
        restored_sim.step(&mut restored_world);

        let hash_sim = sim.state_hash(&world);
        let hash_restored = restored_sim.state_hash(&restored_world);
        assert_eq!(
            hash_sim, hash_restored,
            "hash diverged at tick {} post-restore: {} vs {}",
            tick, hash_sim, hash_restored
        );
    }
}
