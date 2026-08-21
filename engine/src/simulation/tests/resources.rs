use super::super::config::{FOOD_SEARCH_THRESHOLD, MAX_HUNGER, STARVATION_DAMAGE_PER_TICK};
use super::super::time::TICKS_PER_YEAR;
use super::super::{ItemKind, Simulation};
use super::super::{SimulationEventCause, SimulationEventDetails, SimulationEventKind};
use super::support::*;
use crate::world::{ResourceDeposit, ResourceKind};

#[test]
fn renewable_food_regenerates_only_on_daily_boundaries() {
    let mut world = grid_from_rows(&["F"]);
    world.resources[0].as_mut().unwrap().amount = 10;
    let mut simulation = Simulation::default();

    for _ in 0..23 {
        simulation.step(&mut world);
    }
    assert_eq!(world.resources[0].unwrap().amount, 10);
    assert_eq!(simulation.world_revision(), 0);

    simulation.step(&mut world);
    assert_eq!(world.resources[0].unwrap().amount, 11);
    assert_eq!(simulation.world_revision(), 1);
}

#[test]
fn exhausted_renewable_food_deposit_reappears() {
    let mut world = grid_from_rows(&["F"]);
    world.resources[0] = None;
    let mut simulation = Simulation::default();

    for _ in 0..24 {
        simulation.step(&mut world);
    }

    assert_eq!(
        world.resources[0],
        Some(ResourceDeposit {
            kind: ResourceKind::Food,
            amount: 1,
        })
    );
    assert_eq!(simulation.world_revision(), 1);
}

#[test]
fn renewal_stops_at_capacity_and_ignores_unregistered_resources() {
    let mut full_world = grid_from_rows(&["F"]);
    let mut full_simulation = Simulation::default();
    for _ in 0..48 {
        full_simulation.step(&mut full_world);
    }
    assert_eq!(full_world.resources[0].unwrap().amount, 20);
    assert_eq!(full_simulation.world_revision(), 0);

    let mut stone_world = grid_from_rows(&["P"]);
    stone_world.resources[0] = Some(ResourceDeposit {
        kind: ResourceKind::Stone,
        amount: 5,
    });
    let mut stone_simulation = Simulation::default();
    for _ in 0..48 {
        stone_simulation.step(&mut stone_world);
    }
    assert_eq!(stone_world.resources[0].unwrap().amount, 5);
    assert_eq!(stone_simulation.world_revision(), 0);
}

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
    for _ in 0..11 {
        simulation.step(&mut world);
    }

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
    let discovery_events: Vec<_> = simulation
        .recent_events()
        .filter(|event| event.kind == SimulationEventKind::Discovery)
        .collect();
    assert_eq!(discovery_events.len(), 2);
    assert_eq!(discovery_events[0].actor_id, 1);
    assert!(discovery_events.iter().all(|event| {
        event.cause == SimulationEventCause::ResourceFound
            && event.location.x == 0
            && event.location.y == 0
            && event.details
                == SimulationEventDetails::ResourceDiscovery {
                    kind: crate::world::ResourceKind::Food,
                    amount: 10,
                }
    }));
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
    for _ in 0..11 {
        simulation.step(&mut world);
    }
    let stats = simulation.population_stats();
    assert_eq!(stats.population, 1);
    assert_eq!(stats.food_consumed, 10);
    assert!(stats.average_hunger < FOOD_SEARCH_THRESHOLD);
    assert_eq!(
        simulation.entities()[0].mind.memory.known_resources[0].estimated_amount,
        10
    );
}

#[test]
fn gathering_takes_ten_ticks_before_moving_food_into_inventory() {
    let mut world = grid_from_rows(&["F"]);
    world.resources[0].as_mut().unwrap().amount = 20;
    let mut simulation = simulation_with_entity(0, 0, 60.0);
    simulation.entities[0].age_ticks = 25 * TICKS_PER_YEAR;

    for _ in 0..9 {
        simulation.step(&mut world);
    }
    assert_eq!(world.resources[0].unwrap().amount, 20);
    assert_eq!(simulation.entities()[0].inventory.amount(ItemKind::Food), 0);
    assert_eq!(simulation.food_consumed, 0);

    simulation.step(&mut world);
    assert_eq!(world.resources[0].unwrap().amount, 10);
    assert_eq!(
        simulation.entities()[0].inventory.amount(ItemKind::Food),
        10
    );
    assert_eq!(simulation.food_consumed, 0);
    assert_eq!(simulation.world_revision(), 1);

    simulation.step(&mut world);
    assert_eq!(simulation.entities()[0].inventory.amount(ItemKind::Food), 0);
    assert_eq!(simulation.food_consumed, 10);
    assert_eq!(simulation.world_revision(), 1);
}

#[test]
fn gathering_respects_remaining_inventory_capacity() {
    let mut world = grid_from_rows(&["F"]);
    world.resources[0].as_mut().unwrap().amount = 20;
    let mut simulation = simulation_with_entity(0, 0, 60.0);
    simulation.entities[0].age_ticks = 25 * TICKS_PER_YEAR;
    simulation.entities[0].inventory.add(ItemKind::Stone, 45);

    for _ in 0..10 {
        simulation.step(&mut world);
    }

    assert_eq!(world.resources[0].unwrap().amount, 15);
    assert_eq!(simulation.entities()[0].inventory.amount(ItemKind::Food), 5);
    assert_eq!(simulation.entities()[0].inventory.used_capacity(), 50);
    assert_eq!(simulation.food_consumed, 0);
}
