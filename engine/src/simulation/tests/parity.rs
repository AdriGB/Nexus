use super::super::Simulation;
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
        assert_eq!(a.pregnancy, b.pregnancy);
        assert_eq!(a.postpartum_until_tick, b.postpartum_until_tick);
        assert_eq!(a.personality, b.personality);

        // Mind state
        assert_eq!(a.mind.perception_radius, b.mind.perception_radius);
        assert_eq!(a.mind.current_goal, b.mind.current_goal);
        assert_eq!(a.mind.current_plan, b.mind.current_plan);
        assert_eq!(a.mind.plan_index, b.mind.plan_index);
        assert_eq!(a.mind.goal_since_tick, b.mind.goal_since_tick);
        assert_eq!(a.mind.utility_scores.eat, b.mind.utility_scores.eat);
        assert_eq!(a.mind.utility_scores.explore, b.mind.utility_scores.explore);
        assert_eq!(a.mind.utility_scores.rest, b.mind.utility_scores.rest);
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
