use super::super::time::TICKS_PER_YEAR;
use super::super::{ItemKind, Simulation, SimulationEventDetails, SimulationEventKind};
use super::support::{entity, grid_from_rows};

fn sharing_simulation(cooperativeness: f32) -> Simulation {
    let mut giver = entity(1, 0, 0, 0.0);
    giver.age_ticks = 25 * TICKS_PER_YEAR;
    giver.personality.cooperativeness = cooperativeness;
    giver.inventory.add(ItemKind::Food, 30);

    let mut recipient = entity(2, 0, 0, 90.0);
    recipient.age_ticks = 25 * TICKS_PER_YEAR;

    Simulation {
        entities: vec![giver, recipient],
        next_entity_id: 3,
        ..Simulation::default()
    }
}

#[test]
fn cooperative_entity_shares_food_without_changing_the_total() {
    let mut world = grid_from_rows(&["P"]);
    let mut simulation = sharing_simulation(1.0);

    simulation.step(&mut world);

    assert_eq!(simulation.entities[0].inventory.amount(ItemKind::Food), 20);
    assert_eq!(simulation.entities[1].inventory.amount(ItemKind::Food), 10);
    assert_eq!(simulation.food_consumed, 0);
    let event = simulation
        .recent_events()
        .find(|event| event.kind == SimulationEventKind::FoodShared)
        .expect("food sharing event");
    assert_eq!(event.actor_id, 1);
    assert_eq!(event.target_id, Some(2));
    assert_eq!(
        event.details,
        SimulationEventDetails::FoodShared { amount: 10 }
    );
}

#[test]
fn uncooperative_entity_refuses_without_moving_food() {
    let mut world = grid_from_rows(&["P"]);
    let mut simulation = sharing_simulation(0.0);

    simulation.step(&mut world);

    assert_eq!(simulation.entities[0].inventory.amount(ItemKind::Food), 30);
    assert_eq!(simulation.entities[1].inventory.amount(ItemKind::Food), 0);
    let event = simulation
        .recent_events()
        .find(|event| event.kind == SimulationEventKind::FoodShareRefused)
        .expect("food sharing refusal event");
    assert_eq!(event.actor_id, 1);
    assert_eq!(event.target_id, Some(2));
    assert_eq!(event.details, SimulationEventDetails::FoodShareRefused);
}

#[test]
fn positive_affinity_can_change_a_borderline_refusal_into_help() {
    assert!(!super::super::food_share_willingness(0.4, 0));
    assert!(super::super::food_share_willingness(0.4, 1_000));
}
