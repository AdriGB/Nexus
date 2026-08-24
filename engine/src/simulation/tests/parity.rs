use super::super::autonomy::KnownEntity;
use super::super::entity::Personality;
use super::super::time::TICKS_PER_YEAR;
use super::super::{ItemKind, Simulation, SimulationEventKind};
use super::support::*;
use crate::world::Grid;

const TICK_COUNT: u64 = 100;
const POPULATION: u32 = 10;

fn run_parity<F>(step_fn: F)
where
    F: Fn(&mut Simulation, &mut Grid),
{
    // 4 food sources (one F per row), 10 entities, 100 ticks.
    // Covers: exploration → perception → hunger → food search →
    // pathfinding → movement → consumption → memory → planning → rest.
    // Does NOT cover births (gestation = 40 weeks ≈ 6720 ticks).
    let rows = ["PPPFPPPPPP", "PPPPPPFPPP", "PFPPPPPPPP", "PPPPFPPPPP"];
    let mut world_a = grid_from_rows(&rows);
    let mut world_b = grid_from_rows(&rows);
    let mut sim_a = Simulation::with_population(42, &world_a, POPULATION);
    let mut sim_b = Simulation::with_population(42, &world_b, POPULATION);

    for _ in 0..TICK_COUNT {
        sim_a.step(&mut world_a);
        step_fn(&mut sim_b, &mut world_b);
    }

    assert_equivalent(&sim_a, &world_a, &sim_b, &world_b);
}

fn assert_equivalent(sim_a: &Simulation, world_a: &Grid, sim_b: &Simulation, world_b: &Grid) {
    // Simulation-level state
    assert_eq!(sim_a.tick(), sim_b.tick());
    assert_eq!(sim_a.entities().len(), sim_b.entities().len());
    assert_eq!(sim_a.food_consumed, sim_b.food_consumed);
    assert_eq!(sim_a.world_revision(), sim_b.world_revision());
    assert_eq!(sim_a.recent_events.next_id(), sim_b.recent_events.next_id());
    assert_eq!(
        sim_a.recent_events().collect::<Vec<_>>(),
        sim_b.recent_events().collect::<Vec<_>>()
    );

    // Per-entity state
    for (a, b) in sim_a.entities().iter().zip(sim_b.entities()) {
        assert_eq!(a.id, b.id);
        assert_eq!((a.x, a.y), (b.x, b.y));
        assert_eq!(a.sex, b.sex);
        assert_eq!(a.lifespan_ticks, b.lifespan_ticks);
        assert_eq!(a.hunger, b.hunger);
        assert_eq!(a.health, b.health);
        assert_eq!(a.age_ticks, b.age_ticks);
        assert_eq!(a.activity, b.activity);
        assert_eq!(a.path, b.path);
        assert_eq!(a.path_index, b.path_index);
        assert_eq!(a.movement_credit, b.movement_credit);
        assert_eq!(a.caregiver_id, b.caregiver_id);
        assert_eq!(a.partner_id, b.partner_id);
        assert_eq!(a.mother_id, b.mother_id);
        assert_eq!(a.father_id, b.father_id);
        assert_eq!(a.pregnancy, b.pregnancy);
        assert_eq!(a.postpartum_until_tick, b.postpartum_until_tick);
        assert_eq!(a.personality, b.personality);
        assert_eq!(a.inventory, b.inventory);

        // Mind state
        assert_eq!(a.mind.perception_radius, b.mind.perception_radius);
        assert_eq!(a.mind.current_goal, b.mind.current_goal);
        assert_eq!(a.mind.current_plan, b.mind.current_plan);
        assert_eq!(a.mind.plan_index, b.mind.plan_index);
        assert_eq!(a.mind.goal_since_tick, b.mind.goal_since_tick);
        assert_eq!(a.mind.utility_scores.eat, b.mind.utility_scores.eat);
        assert_eq!(
            a.mind.utility_scores.acquire_resource,
            b.mind.utility_scores.acquire_resource
        );
        assert_eq!(a.mind.utility_scores.explore, b.mind.utility_scores.explore);
        assert_eq!(a.mind.utility_scores.rest, b.mind.utility_scores.rest);
        assert_eq!(
            a.mind.utility_scores.socialize,
            b.mind.utility_scores.socialize
        );
        assert_eq!(
            a.mind.utility_scores.share_food,
            b.mind.utility_scores.share_food
        );
        assert_eq!(a.mind.visible_entities, b.mind.visible_entities);

        // Memory state
        assert_eq!(a.mind.memory.known_resources, b.mind.memory.known_resources);
        assert_eq!(a.mind.memory.known_entities, b.mind.memory.known_entities);
        assert_eq!(
            a.mind.memory.known_chunk_count(),
            b.mind.memory.known_chunk_count()
        );
        for y in 0..world_a.height {
            for x in 0..world_a.width {
                assert_eq!(
                    a.mind.memory.remembers_chunk(world_a, x, y),
                    b.mind.memory.remembers_chunk(world_b, x, y),
                );
            }
        }
    }

    // World resources
    assert_eq!(world_a.resources.len(), world_b.resources.len());
    for (slot_a, slot_b) in world_a.resources.iter().zip(world_b.resources.iter()) {
        assert_eq!(slot_a.is_some(), slot_b.is_some());
        if let (Some(a), Some(b)) = (slot_a, slot_b) {
            assert_eq!(a.kind, b.kind);
            assert_eq!(a.amount, b.amount);
        }
    }

    // Population stats
    let stats_a = sim_a.population_stats();
    let stats_b = sim_b.population_stats();
    assert_eq!(stats_a.population, stats_b.population);
    assert_eq!(stats_a.births, stats_b.births);
    assert_eq!(stats_a.deaths, stats_b.deaths);
    assert_eq!(stats_a.females, stats_b.females);
    assert_eq!(stats_a.males, stats_b.males);
    assert_eq!(stats_a.pregnant, stats_b.pregnant);
    assert_eq!(stats_a.food_consumed, stats_b.food_consumed);
}

#[test]
fn profile_step_matches_step() {
    run_parity(|simulation, world| {
        simulation.profile_step(world);
    });
}

#[test]
fn profile_autonomy_step_matches_step() {
    run_parity(|simulation, world| {
        simulation.profile_autonomy_step(world);
    });
}

#[test]
fn profile_step_matches_food_sharing_relationship_effects() {
    fn sharing_simulation() -> Simulation {
        let mut giver = entity(1, 0, 0, 0.0);
        giver.age_ticks = 25 * TICKS_PER_YEAR;
        giver.personality.cooperativeness = 1.0;
        giver.inventory.add(ItemKind::Food, 30);
        let mut recipient = entity(2, 0, 0, 90.0);
        recipient.age_ticks = 25 * TICKS_PER_YEAR;
        Simulation {
            entities: vec![giver, recipient],
            next_entity_id: 3,
            ..Simulation::default()
        }
    }

    let mut normal_world = grid_from_rows(&["P"]);
    let mut profiled_world = grid_from_rows(&["P"]);
    let mut normal = sharing_simulation();
    let mut profiled = sharing_simulation();

    normal.step(&mut normal_world);
    profiled.profile_step(&mut profiled_world);

    assert_equivalent(&normal, &normal_world, &profiled, &profiled_world);
}

#[test]
fn profile_step_matches_dependent_feeding() {
    fn dependent_simulation() -> Simulation {
        let mut caregiver = entity(1, 0, 0, 0.0);
        caregiver.age_ticks = 25 * TICKS_PER_YEAR;
        caregiver.personality.cooperativeness = 0.0;
        caregiver.inventory.add(ItemKind::Food, 30);
        let mut child = entity(2, 0, 0, 80.0);
        child.age_ticks = 8 * TICKS_PER_YEAR;
        child.caregiver_id = Some(1);
        Simulation {
            entities: vec![caregiver, child],
            next_entity_id: 3,
            ..Simulation::default()
        }
    }

    let mut normal_world = grid_from_rows(&["P"]);
    let mut profiled_world = grid_from_rows(&["P"]);
    let mut normal = dependent_simulation();
    let mut profiled = dependent_simulation();

    normal.step(&mut normal_world);
    profiled.profile_step(&mut profiled_world);

    assert_equivalent(&normal, &normal_world, &profiled, &profiled_world);
    assert_eq!(normal.entities[1].inventory.amount(ItemKind::Food), 10);
}

#[test]
fn profile_step_matches_partnership_formation() {
    fn partnership_simulation() -> Simulation {
        let known = |id| KnownEntity {
            id,
            first_seen_tick: 0,
            last_seen_tick: 0,
            last_seen_x: 0,
            last_seen_y: 0,
            observed_ticks: 1,
            affinity: 210,
            last_interaction_tick: 0,
            interaction_count: 2,
            seek_retry_after_tick: None,
        };
        let mut first = entity(1, 0, 0, 0.0);
        let mut second = entity(2, 0, 0, 0.0);
        for entity in [&mut first, &mut second] {
            entity.age_ticks = 25 * TICKS_PER_YEAR;
        }
        second.personality = first.personality;
        first.mind.memory.known_entities.push(known(2));
        second.mind.memory.known_entities.push(known(1));
        Simulation {
            entities: vec![first, second],
            next_entity_id: 3,
            ..Simulation::default()
        }
    }

    let mut normal_world = grid_from_rows(&["P"]);
    let mut profiled_world = grid_from_rows(&["P"]);
    let mut normal = partnership_simulation();
    let mut profiled = partnership_simulation();

    normal.step(&mut normal_world);
    profiled.profile_step(&mut profiled_world);

    assert_equivalent(&normal, &normal_world, &profiled, &profiled_world);
    assert_eq!(normal.entities[0].partner_id, Some(2));
    assert_eq!(normal.entities[1].partner_id, Some(1));
}

#[test]
fn profiled_pipelines_match_renewable_resource_regeneration() {
    let mut normal_world = grid_from_rows(&["F"]);
    let mut profiled_world = grid_from_rows(&["F"]);
    let mut autonomy_profiled_world = grid_from_rows(&["F"]);
    normal_world.resources[0] = None;
    profiled_world.resources[0] = None;
    autonomy_profiled_world.resources[0] = None;
    let mut normal = Simulation::default();
    let mut profiled = Simulation::default();
    let mut autonomy_profiled = Simulation::default();

    for _ in 0..24 {
        normal.step(&mut normal_world);
        profiled.profile_step(&mut profiled_world);
        autonomy_profiled.profile_autonomy_step(&mut autonomy_profiled_world);
    }

    assert_equivalent(&normal, &normal_world, &profiled, &profiled_world);
    assert_equivalent(
        &normal,
        &normal_world,
        &autonomy_profiled,
        &autonomy_profiled_world,
    );
    assert_eq!(normal_world.resources[0].unwrap().amount, 1);
}

#[test]
fn profile_step_matches_affinity_change_events_from_step() {
    let relationship = |id, x, y| KnownEntity {
        id,
        first_seen_tick: 0,
        last_seen_tick: 0,
        last_seen_x: x,
        last_seen_y: y,
        observed_ticks: 1,
        affinity: 99,
        last_interaction_tick: 0,
        interaction_count: 1,
        seek_retry_after_tick: None,
    };
    let mut a = entity(1, 2, 2, 0.0);
    let mut b = entity(2, 2, 3, 0.0);
    for entity in [&mut a, &mut b] {
        entity.age_ticks = 25 * TICKS_PER_YEAR;
        entity.personality = Personality {
            curiosity: 0.5,
            sociability: 0.5,
            cooperativeness: 0.5,
            caution: 0.5,
            persistence: 0.5,
        };
    }
    a.mind.memory.known_entities.push(relationship(2, 2, 3));
    b.mind.memory.known_entities.push(relationship(1, 2, 2));
    let mut normal = Simulation {
        entities: vec![a.clone(), b.clone()],
        next_entity_id: 3,
        ..Simulation::default()
    };
    let mut profiled = Simulation {
        entities: vec![a, b],
        next_entity_id: 3,
        ..Simulation::default()
    };
    let mut normal_world = plain_grid(8, 8);
    let mut profiled_world = plain_grid(8, 8);

    normal.step(&mut normal_world);
    profiled.profile_step(&mut profiled_world);

    let normal_events: Vec<_> = normal.recent_events().collect();
    let profiled_events: Vec<_> = profiled.recent_events().collect();
    assert_eq!(
        normal_events
            .iter()
            .filter(|event| event.kind == SimulationEventKind::AffinityChange)
            .count(),
        2
    );
    assert_eq!(normal_events, profiled_events);
    assert_eq!(
        normal.recent_events.next_id(),
        profiled.recent_events.next_id()
    );
}
