use super::super::config::{FOOD_SEARCH_THRESHOLD, MAX_HUNGER, STARVATION_DAMAGE_PER_TICK};
use super::super::time::TICKS_PER_YEAR;
use super::super::Simulation;
use super::super::{SimulationEventCause, SimulationEventDetails, SimulationEventKind};
use super::support::*;

#[test]
fn competing_entities_consume_a_finite_deposit_once() {
    let mut world = grid_from_rows(&["F"]);
    world.resources[0].as_mut().unwrap().amount = 10;
    let mut simulation = Simulation {
        entities: vec![entity(1, 0, 0, 60.0), entity(2, 0, 0, 60.0)],
        next_entity_id: 3,
        ..Simulation::default()
    };
    for entity in &mut simulation.entities {
        entity.age_ticks = 25 * TICKS_PER_YEAR;
    }
    simulation.step(&mut world);

    assert!(world.resources[0].is_none());
    assert_eq!(simulation.food_consumed, 10);
    assert!(simulation.entities()[0].hunger < simulation.entities()[1].hunger);
    assert_eq!(simulation.world_revision(), 1);
    let consumption_events: Vec<_> = simulation
        .recent_events()
        .filter(|event| event.kind == SimulationEventKind::Consumption)
        .collect();
    assert_eq!(consumption_events.len(), 1);
    assert_eq!(consumption_events[0].actor_id, 1);
    assert_eq!(consumption_events[0].cause, SimulationEventCause::AteFood);
    assert_eq!(
        consumption_events[0].details,
        SimulationEventDetails::Consumption { amount: 10 }
    );
    assert_eq!(consumption_events[0].location.x, 0);
    assert_eq!(consumption_events[0].location.y, 0);
}

#[test]
fn starving_entity_loses_health_and_dies() {
    let mut world = grid_from_rows(&["P"]);
    let mut starving = entity(1, 0, 0, MAX_HUNGER);
    starving.health = STARVATION_DAMAGE_PER_TICK;
    let mut simulation = Simulation {
        entities: vec![starving],
        next_entity_id: 2,
        ..Simulation::default()
    };
    simulation.step(&mut world);
    assert!(simulation.entities().is_empty());
    assert_eq!(simulation.population_stats().deaths, 1);
}

#[test]
fn population_stats_report_pressure_and_consumption() {
    let mut world = grid_from_rows(&["F"]);
    let mut simulation = simulation_with_entity(0, 0, 60.0);
    simulation.entities[0].age_ticks = 25 * TICKS_PER_YEAR;
    simulation.step(&mut world);
    let stats = simulation.population_stats();
    assert_eq!(stats.population, 1);
    assert_eq!(stats.food_consumed, 10);
    assert!(stats.average_hunger < FOOD_SEARCH_THRESHOLD);
    assert_eq!(
        simulation.entities()[0].mind.memory.known_resources[0].estimated_amount,
        10
    );
}
