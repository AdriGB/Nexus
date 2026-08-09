use super::super::entity::Sex;
use super::super::time::{TICKS_PER_DAY, TICKS_PER_HOUR, TICKS_PER_YEAR};
use super::super::Simulation;
use super::support::*;
use std::collections::HashSet;

#[test]
fn simulation_starts_paused_at_tick_zero() {
    let simulation = Simulation::default();
    assert_eq!(simulation.tick(), 0);
    assert!(simulation.is_paused());
}

#[test]
fn spawns_multiple_entities_with_unique_ids_and_positions() {
    let world = plain_grid(10, 10);
    let simulation = Simulation::with_population(42, &world, 10);
    let ids: HashSet<_> = simulation
        .entities()
        .iter()
        .map(|entity| entity.id)
        .collect();
    let positions: HashSet<_> = simulation
        .entities()
        .iter()
        .map(|entity| (entity.x, entity.y))
        .collect();
    assert_eq!(ids.len(), 10);
    assert_eq!(positions.len(), 10);
}

#[test]
fn paused_simulation_does_not_change_entities() {
    let mut world = grid_from_rows(&["PF"]);
    let mut simulation = simulation_with_entity(0, 0, 59.0);
    simulation.advance(10, &mut world);
    assert_eq!(simulation.tick(), 0);
    assert_eq!(simulation.entities()[0].hunger, 59.0);
}

#[test]
fn entity_ids_are_never_reused_after_death() {
    let mut world = plain_grid(3, 1);
    let mut simulation = Simulation::with_population(42, &world, 2);
    simulation.entities[0].health = 0.0;
    simulation.step(&mut world);
    assert_eq!(simulation.spawn_entities(&world, 1), 1);
    let ids: Vec<_> = simulation
        .entities()
        .iter()
        .map(|entity| entity.id)
        .collect();
    assert_eq!(ids, vec![2, 3]);
}

#[test]
fn population_stats_include_biology() {
    use super::super::entity::Pregnancy;
    use super::super::time::GESTATION_TICKS;

    let mut female = fertile_entity(1, Sex::Female, 0, 0);
    female.pregnancy = Some(Pregnancy {
        father_id: 2,
        conceived_tick: 0,
        due_tick: GESTATION_TICKS,
    });
    let simulation = Simulation {
        entities: vec![female, fertile_entity(2, Sex::Male, 1, 0)],
        next_entity_id: 3,
        ..Simulation::default()
    };
    let stats = simulation.population_stats();
    assert_eq!(stats.females, 1);
    assert_eq!(stats.males, 1);
    assert_eq!(stats.pregnant, 1);
}

#[test]
fn handles_10_100_and_1000_entity_populations() {
    for count in [10, 100, 1_000] {
        let mut world = plain_grid(40, 25);
        let mut simulation = Simulation::with_population(42, &world, count);
        assert_eq!(simulation.entities().len(), count as usize);
        simulation.resume();
        simulation.advance(10, &mut world);
        assert_eq!(simulation.entities().len(), count as usize);
        assert_eq!(simulation.tick(), 10);
    }
}

#[test]
fn same_seed_and_steps_are_deterministic() {
    let rows = ["PPPFPPPPPP", "PPPPPPFPPP", "PFPPPPPPPP", "PPPPFPPPPP"];
    let mut world_a = grid_from_rows(&rows);
    let mut world_b = grid_from_rows(&rows);
    let mut simulation_a = Simulation::with_population(42, &world_a, 10);
    let mut simulation_b = Simulation::with_population(42, &world_b, 10);

    for _ in 0..100 {
        simulation_a.step(&mut world_a);
        simulation_b.step(&mut world_b);
    }

    assert_eq!(simulation_a.tick(), simulation_b.tick());
    assert_eq!(simulation_a.entities().len(), simulation_b.entities().len());

    for (entity_a, entity_b) in simulation_a.entities().iter().zip(simulation_b.entities()) {
        assert_eq!(entity_a.id, entity_b.id);
        assert_eq!((entity_a.x, entity_a.y), (entity_b.x, entity_b.y));
        assert_eq!(entity_a.sex, entity_b.sex);
        assert_eq!(entity_a.age_ticks, entity_b.age_ticks);
        assert_eq!(entity_a.hunger, entity_b.hunger);
        assert_eq!(entity_a.health, entity_b.health);
        assert_eq!(entity_a.pregnancy, entity_b.pregnancy);
        assert_eq!(entity_a.personality, entity_b.personality);
        assert_eq!(entity_a.mind.current_goal, entity_b.mind.current_goal);
    }

    assert_eq!(
        simulation_a.population_stats().births,
        simulation_b.population_stats().births
    );
}

#[test]
fn one_tick_represents_one_hour() {
    assert_eq!(TICKS_PER_HOUR, 1);
    assert_eq!(TICKS_PER_DAY, 24);
    assert_eq!(TICKS_PER_YEAR, 8_760);
}
