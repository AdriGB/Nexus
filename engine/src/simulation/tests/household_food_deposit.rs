use super::super::autonomy::{Action, Goal, HouseholdDepositAttempt};
use super::super::households::{Household, DEFAULT_HOUSEHOLD_STORAGE_CAPACITY};
use super::super::time::TICKS_PER_YEAR;
use super::super::{Inventory, ItemKind, Simulation};
use super::support::{entity, plain_grid};

fn adult(id: u32, position: (u32, u32), food: u16) -> super::super::Entity {
    let mut entity = entity(id, position.0, position.1, 0.0);
    entity.age_ticks = 25 * TICKS_PER_YEAR;
    entity.health = 1.0;
    entity.household_id = Some(1);
    entity.personality.curiosity = 0.0;
    entity.personality.caution = 1.0;
    entity.inventory.add(ItemKind::Food, food);
    entity
}

fn household(capacity: u16, used: u16) -> Household {
    let mut storage = Inventory::new(capacity);
    storage.add(ItemKind::Stone, used);
    Household {
        id: 1,
        formed_tick: 0,
        dissolved_tick: None,
        residence_x: 2,
        residence_y: 0,
        storage,
    }
}

fn simulation(position: (u32, u32), food: u16, storage_used: u16) -> Simulation {
    Simulation {
        entities: vec![adult(1, position, food)],
        next_entity_id: 2,
        households: vec![household(DEFAULT_HOUSEHOLD_STORAGE_CAPACITY, storage_used)],
        next_household_id: 2,
        ..Simulation::default()
    }
}

#[test]
fn adult_with_surplus_food_plans_household_deposit() {
    let mut simulation = simulation((0, 0), 35, 0);
    simulation.step(&mut plain_grid(3, 1));
    assert_eq!(
        simulation.entities[0].mind.current_plan,
        vec![
            Action::MoveTo(2, 0),
            Action::DepositHouseholdFood(15),
            Action::Wait
        ]
    );
}

#[test]
fn adult_keeps_personal_food_reserve() {
    let mut simulation = simulation((2, 0), 50, 0);
    simulation.step(&mut plain_grid(3, 1));
    assert_eq!(simulation.entities[0].inventory.amount(ItemKind::Food), 20);
    assert_eq!(simulation.households[0].storage.amount(ItemKind::Food), 30);
}

#[test]
fn adult_with_exact_reserve_does_not_deposit() {
    let mut simulation = simulation((2, 0), 20, 0);
    simulation.step(&mut plain_grid(3, 1));
    assert_eq!(simulation.entities[0].mind.current_plan, vec![Action::Wait]);
}

#[test]
fn adult_below_reserve_does_not_deposit() {
    let mut simulation = simulation((2, 0), 10, 0);
    simulation.step(&mut plain_grid(3, 1));
    assert_eq!(simulation.entities[0].mind.current_plan, vec![Action::Wait]);
}

#[test]
fn storage_capacity_limits_planned_deposit() {
    let mut simulation = simulation((2, 0), 50, 195);
    simulation.step(&mut plain_grid(3, 1));
    assert_eq!(
        simulation.entities[0].mind.current_plan,
        vec![Action::DepositHouseholdFood(5), Action::Wait]
    );
    assert_eq!(simulation.entities[0].inventory.amount(ItemKind::Food), 45);
}

#[test]
fn full_household_storage_skips_deposit() {
    let mut simulation = simulation((2, 0), 50, 200);
    simulation.step(&mut plain_grid(3, 1));
    assert_eq!(simulation.entities[0].mind.current_plan, vec![Action::Wait]);
}

#[test]
fn member_away_from_home_moves_then_deposits() {
    let mut simulation = simulation((0, 0), 35, 0);
    let mut world = plain_grid(3, 1);
    for _ in 0..3 {
        simulation.step(&mut world);
    }
    assert_eq!((simulation.entities[0].x, simulation.entities[0].y), (2, 0));
    assert_eq!(simulation.entities[0].inventory.amount(ItemKind::Food), 20);
    assert_eq!(simulation.households[0].storage.amount(ItemKind::Food), 15);
}

#[test]
fn member_already_home_deposits_before_waiting() {
    let mut simulation = simulation((2, 0), 35, 0);
    simulation.step(&mut plain_grid(3, 1));
    assert_eq!(
        simulation.entities[0].mind.current_plan,
        vec![Action::DepositHouseholdFood(15), Action::Wait]
    );
    assert_eq!(simulation.entities[0].mind.plan_index, 1);
}

#[test]
fn entity_without_household_never_deposits() {
    let mut simulation = simulation((2, 0), 50, 0);
    simulation.entities[0].household_id = None;
    simulation.step(&mut plain_grid(3, 1));
    assert_eq!(simulation.entities[0].mind.current_plan, vec![Action::Wait]);
    assert_eq!(simulation.households[0].storage.used_capacity(), 0);
}

#[test]
fn child_behavior_does_not_gain_household_deposit() {
    let caregiver = adult(1, (2, 0), 0);
    let mut child = entity(2, 0, 0, 0.0);
    child.age_ticks = 8 * TICKS_PER_YEAR;
    child.caregiver_id = Some(1);
    child.household_id = Some(1);
    child.inventory.add(ItemKind::Food, 50);
    let mut simulation = Simulation {
        entities: vec![caregiver, child],
        next_entity_id: 3,
        households: vec![household(200, 0)],
        next_household_id: 2,
        ..Simulation::default()
    };
    simulation.step(&mut plain_grid(3, 1));
    assert_eq!(simulation.entities[1].mind.current_goal, Some(Goal::Follow));
    assert!(!simulation.entities[1]
        .mind
        .current_plan
        .iter()
        .any(|action| matches!(action, Action::DepositHouseholdFood(_))));
}

#[test]
fn deposit_action_uses_deferred_attempt() {
    let mut simulation = simulation((2, 0), 35, 0);
    let mut world = plain_grid(3, 1);
    simulation.rebuild_population_index(&world);
    let (_, _, _, _, _, _, _, attempts, _) = simulation.run_autonomy(&mut world);
    assert_eq!(simulation.entities[0].inventory.amount(ItemKind::Food), 35);
    assert_eq!(simulation.households[0].storage.amount(ItemKind::Food), 0);
    assert_eq!(attempts.len(), 1);
    assert_eq!(attempts[0].amount, 15);
}

#[test]
fn successful_attempt_moves_food_to_household_storage() {
    let mut simulation = simulation((2, 0), 35, 0);
    simulation.process_household_deposit_attempts(vec![HouseholdDepositAttempt {
        actor_id: 1,
        amount: 15,
        actor_location: (2, 0),
    }]);
    assert_eq!(simulation.entities[0].inventory.amount(ItemKind::Food), 20);
    assert_eq!(simulation.households[0].storage.amount(ItemKind::Food), 15);
}

#[test]
fn failed_attempt_does_not_mutate_inventory_or_storage() {
    let mut simulation = simulation((2, 0), 35, 0);
    simulation.entities[0].household_id = None;
    simulation.process_household_deposit_attempts(vec![HouseholdDepositAttempt {
        actor_id: 1,
        amount: 15,
        actor_location: (2, 0),
    }]);
    assert_eq!(simulation.entities[0].inventory.amount(ItemKind::Food), 35);
    assert_eq!(simulation.households[0].storage.amount(ItemKind::Food), 0);
}

#[test]
fn identical_simulations_produce_identical_deposit_behavior() {
    let mut first = simulation((0, 0), 50, 0);
    let mut second = first.clone();
    let mut first_world = plain_grid(3, 1);
    let mut second_world = plain_grid(3, 1);
    for _ in 0..4 {
        first.step(&mut first_world);
        second.step(&mut second_world);
    }
    assert_eq!(first.entities[0].inventory, second.entities[0].inventory);
    assert_eq!(first.households, second.households);
    assert_eq!(
        first.entities[0].mind.current_plan,
        second.entities[0].mind.current_plan
    );
}

#[test]
fn normal_and_profiled_paths_match() {
    let mut normal = simulation((0, 0), 50, 0);
    let mut profiled = normal.clone();
    let mut normal_world = plain_grid(3, 1);
    let mut profiled_world = plain_grid(3, 1);
    for _ in 0..4 {
        normal.step(&mut normal_world);
        profiled.profile_autonomy_step(&mut profiled_world);
    }
    assert_eq!(normal.entities[0].inventory, profiled.entities[0].inventory);
    assert_eq!(normal.households, profiled.households);
    assert_eq!(
        (normal.entities[0].x, normal.entities[0].y),
        (profiled.entities[0].x, profiled.entities[0].y)
    );
}

#[test]
fn two_adults_independently_return_and_deposit_combined_surplus() {
    let mut simulation = Simulation {
        entities: vec![adult(1, (0, 0), 35), adult(2, (4, 0), 50)],
        next_entity_id: 3,
        households: vec![household(200, 0)],
        next_household_id: 2,
        ..Simulation::default()
    };
    let mut world = plain_grid(5, 1);
    for _ in 0..4 {
        simulation.step(&mut world);
    }
    assert_eq!((simulation.entities[0].x, simulation.entities[0].y), (2, 0));
    assert_eq!((simulation.entities[1].x, simulation.entities[1].y), (2, 0));
    assert_eq!(simulation.entities[0].inventory.amount(ItemKind::Food), 20);
    assert_eq!(simulation.entities[1].inventory.amount(ItemKind::Food), 20);
    assert_eq!(simulation.households[0].storage.amount(ItemKind::Food), 45);
}
