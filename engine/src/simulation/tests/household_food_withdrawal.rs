use super::super::autonomy::{Action, Goal, HouseholdWithdrawAttempt, KnownResource};
use super::super::households::Household;
use super::super::time::TICKS_PER_YEAR;
use super::super::{Inventory, ItemKind, Simulation};
use super::support::{entity, grid_from_rows, plain_grid};
use crate::world::{ResourceDeposit, ResourceKind};

fn hungry_adult(id: u32, position: (u32, u32)) -> super::super::Entity {
    let mut adult = entity(id, position.0, position.1, 70.0);
    adult.age_ticks = 25 * TICKS_PER_YEAR;
    adult.health = 100.0;
    adult.household_id = Some(1);
    adult.personality.curiosity = 0.0;
    adult
}

fn household(food: u16) -> Household {
    let mut storage = Inventory::new(200);
    storage.add(ItemKind::Food, food);
    Household {
        id: 1,
        formed_tick: 0,
        dissolved_tick: None,
        inheritance: None,
        migration: None,
        residence_x: 2,
        residence_y: 0,
        storage,
    }
}

fn simulation(position: (u32, u32), food: u16) -> Simulation {
    Simulation {
        entities: vec![hungry_adult(1, position)],
        next_entity_id: 2,
        households: vec![household(food)],
        next_household_id: 2,
        ..Simulation::default()
    }
}

fn remember_world_food(simulation: &mut Simulation, position: (u32, u32), amount: u16) {
    simulation.entities[0]
        .mind
        .memory
        .known_resources
        .push(KnownResource {
            x: position.0,
            y: position.1,
            kind: ResourceKind::Food,
            last_seen_tick: 0,
            estimated_amount: amount,
            failed_attempts: 0,
            avoid_until_tick: 0,
        });
}

#[test]
fn adult_prefers_household_food_over_world_food() {
    let mut simulation = simulation((0, 0), 20);
    remember_world_food(&mut simulation, (4, 0), 50);
    simulation.step(&mut plain_grid(5, 1));
    assert_eq!(
        simulation.entities[0].mind.current_plan,
        vec![Action::MoveTo(2, 0), Action::WithdrawHouseholdFood(10)]
    );
}

#[test]
fn adult_with_household_food_plans_withdrawal() {
    let mut simulation = simulation((0, 0), 20);
    simulation.step(&mut plain_grid(3, 1));
    assert_eq!(
        simulation.entities[0].mind.current_goal,
        Some(Goal::AcquireResource)
    );
    assert!(simulation.entities[0]
        .mind
        .current_plan
        .contains(&Action::WithdrawHouseholdFood(10)));
}

#[test]
fn adult_already_home_withdraws_without_move() {
    let mut simulation = simulation((2, 0), 20);
    simulation.step(&mut plain_grid(3, 1));
    assert_eq!(
        simulation.entities[0].mind.current_plan,
        vec![Action::WithdrawHouseholdFood(10)]
    );
    assert_eq!(simulation.entities[0].inventory.amount(ItemKind::Food), 10);
}

#[test]
fn adult_away_from_home_moves_before_withdrawal() {
    let mut simulation = simulation((0, 0), 20);
    let mut world = plain_grid(3, 1);
    simulation.step(&mut world);
    assert_eq!((simulation.entities[0].x, simulation.entities[0].y), (1, 0));
    assert_eq!(simulation.entities[0].inventory.amount(ItemKind::Food), 0);
    simulation.step(&mut world);
    simulation.step(&mut world);
    assert_eq!(simulation.entities[0].inventory.amount(ItemKind::Food), 10);
}

#[test]
fn withdrawal_requests_one_meal() {
    let mut simulation = simulation((2, 0), 100);
    simulation.step(&mut plain_grid(3, 1));
    assert_eq!(simulation.entities[0].inventory.amount(ItemKind::Food), 10);
    assert_eq!(simulation.households[0].storage.amount(ItemKind::Food), 90);
}

#[test]
fn withdrawal_respects_available_household_food() {
    let mut simulation = simulation((2, 0), 4);
    simulation.step(&mut plain_grid(3, 1));
    assert_eq!(simulation.entities[0].inventory.amount(ItemKind::Food), 4);
    assert_eq!(simulation.households[0].storage.amount(ItemKind::Food), 0);
}

#[test]
fn withdrawal_respects_personal_inventory_capacity() {
    let mut simulation = simulation((2, 0), 20);
    simulation.entities[0].inventory.add(ItemKind::Stone, 48);
    simulation.step(&mut plain_grid(3, 1));
    assert_eq!(simulation.entities[0].inventory.amount(ItemKind::Food), 2);
    assert_eq!(simulation.households[0].storage.amount(ItemKind::Food), 18);
}

#[test]
fn empty_household_storage_falls_back_to_world_food() {
    let mut simulation = simulation((0, 0), 0);
    remember_world_food(&mut simulation, (2, 0), 10);
    let mut world = plain_grid(3, 1);
    world.resources[2] = Some(ResourceDeposit {
        kind: ResourceKind::Food,
        amount: 10,
    });
    simulation.step(&mut world);
    assert_eq!(
        simulation.entities[0].mind.current_plan.last(),
        Some(&Action::Gather(ResourceKind::Food))
    );
}

#[test]
fn unreachable_household_residence_falls_back_to_world_food() {
    let mut simulation = simulation((0, 0), 20);
    remember_world_food(&mut simulation, (0, 0), 10);
    let mut world = grid_from_rows(&["P#P"]);
    world.resources[0] = Some(ResourceDeposit {
        kind: ResourceKind::Food,
        amount: 10,
    });
    simulation.step(&mut world);
    assert_eq!(
        simulation.entities[0].mind.current_plan,
        vec![Action::Gather(ResourceKind::Food)]
    );
}

#[test]
fn entity_without_household_uses_existing_acquisition() {
    let mut simulation = simulation((0, 0), 20);
    simulation.entities[0].household_id = None;
    remember_world_food(&mut simulation, (2, 0), 10);
    let mut world = plain_grid(3, 1);
    world.resources[2] = Some(ResourceDeposit {
        kind: ResourceKind::Food,
        amount: 10,
    });
    simulation.step(&mut world);
    assert_eq!(
        simulation.entities[0].mind.current_plan.last(),
        Some(&Action::Gather(ResourceKind::Food))
    );
}

#[test]
fn child_behavior_is_unchanged() {
    let caregiver = hungry_adult(1, (2, 0));
    let mut child = entity(2, 0, 0, 0.0);
    child.age_ticks = 8 * TICKS_PER_YEAR;
    child.caregiver_id = Some(1);
    child.household_id = Some(1);
    let mut simulation = Simulation {
        entities: vec![caregiver, child],
        next_entity_id: 3,
        households: vec![household(20)],
        next_household_id: 2,
        ..Simulation::default()
    };
    simulation.step(&mut plain_grid(3, 1));
    assert_eq!(simulation.entities[1].mind.current_goal, Some(Goal::Follow));
}

#[test]
fn withdraw_action_uses_deferred_attempt() {
    let mut simulation = simulation((2, 0), 20);
    let mut world = plain_grid(3, 1);
    simulation.rebuild_population_index(&world);
    let (_, _, _, _, _, _, _, _, attempts, _) = simulation.run_autonomy(&mut world, None);
    assert_eq!(simulation.entities[0].inventory.amount(ItemKind::Food), 0);
    assert_eq!(simulation.households[0].storage.amount(ItemKind::Food), 20);
    assert_eq!(
        attempts,
        vec![HouseholdWithdrawAttempt {
            actor_id: 1,
            amount: 10,
            actor_location: (2, 0)
        }]
    );
}

#[test]
fn successful_withdrawal_moves_food_to_personal_inventory() {
    let mut simulation = simulation((2, 0), 20);
    simulation.process_household_withdraw_attempts(vec![HouseholdWithdrawAttempt {
        actor_id: 1,
        amount: 10,
        actor_location: (2, 0),
    }]);
    assert_eq!(simulation.entities[0].inventory.amount(ItemKind::Food), 10);
    assert_eq!(simulation.households[0].storage.amount(ItemKind::Food), 10);
}

#[test]
fn failed_withdrawal_does_not_mutate_state() {
    let mut simulation = simulation((2, 0), 20);
    simulation.entities[0].household_id = None;
    simulation.process_household_withdraw_attempts(vec![HouseholdWithdrawAttempt {
        actor_id: 1,
        amount: 10,
        actor_location: (2, 0),
    }]);
    assert_eq!(simulation.entities[0].inventory.amount(ItemKind::Food), 0);
    assert_eq!(simulation.households[0].storage.amount(ItemKind::Food), 20);
}

#[test]
fn household_acquire_plan_is_not_invalidated_as_stale_world_food() {
    let mut simulation = simulation((0, 0), 20);
    let mut world = plain_grid(3, 1);
    simulation.step(&mut world);
    simulation.step(&mut world);
    assert!(simulation.entities[0]
        .mind
        .current_plan
        .contains(&Action::WithdrawHouseholdFood(10)));
}

#[test]
fn withdrawal_then_naturally_selects_eat() {
    let mut simulation = simulation((2, 0), 20);
    let mut world = plain_grid(3, 1);
    simulation.step(&mut world);
    simulation.step(&mut world);
    assert_eq!(simulation.entities[0].mind.current_goal, Some(Goal::Eat));
    assert_eq!(simulation.entities[0].inventory.amount(ItemKind::Food), 0);
}

#[test]
fn competing_withdrawals_never_duplicate_food() {
    let mut simulation = Simulation {
        entities: vec![hungry_adult(1, (2, 0)), hungry_adult(2, (2, 0))],
        next_entity_id: 3,
        households: vec![household(10)],
        next_household_id: 2,
        ..Simulation::default()
    };
    simulation.step(&mut plain_grid(3, 1));
    let personal = simulation
        .entities
        .iter()
        .map(|entity| entity.inventory.amount(ItemKind::Food))
        .sum::<u16>();
    assert_eq!(personal, 10);
    assert_eq!(simulation.households[0].storage.amount(ItemKind::Food), 0);
}

#[test]
fn identical_simulations_produce_identical_withdrawal_behavior() {
    let mut first = simulation((0, 0), 20);
    let mut second = first.clone();
    let mut first_world = plain_grid(3, 1);
    let mut second_world = plain_grid(3, 1);
    for _ in 0..4 {
        first.step(&mut first_world);
        second.step(&mut second_world);
    }
    assert_eq!(first.entities[0].inventory, second.entities[0].inventory);
    assert_eq!(first.households, second.households);
}

#[test]
fn normal_and_profiled_paths_match() {
    let mut normal = simulation((0, 0), 20);
    let mut profiled = normal.clone();
    let mut normal_world = plain_grid(3, 1);
    let mut profiled_world = plain_grid(3, 1);
    for _ in 0..4 {
        normal.step(&mut normal_world);
        profiled.profile_autonomy_step(&mut profiled_world);
    }
    assert_eq!(normal.entities[0].inventory, profiled.entities[0].inventory);
    assert_eq!(normal.households, profiled.households);
}

#[test]
fn household_economy_deposit_withdraw_and_eat_conserves_food() {
    let mut contributor = hungry_adult(1, (2, 0));
    contributor.hunger = 0.0;
    contributor.health = 1.0;
    contributor.personality.caution = 1.0;
    contributor.inventory.add(ItemKind::Food, 35);
    let consumer = hungry_adult(2, (2, 0));
    let mut simulation = Simulation {
        entities: vec![contributor, consumer],
        next_entity_id: 3,
        households: vec![household(0)],
        next_household_id: 2,
        ..Simulation::default()
    };
    let mut world = plain_grid(3, 1);
    let initial_total = 35;
    simulation.step(&mut world);
    simulation.entities[1].mind.clear_goal();
    simulation.step(&mut world);
    assert_eq!(simulation.entities[0].inventory.amount(ItemKind::Food), 20);
    assert_eq!(
        simulation.entities[1].mind.current_goal,
        Some(Goal::AcquireResource)
    );
    simulation.step(&mut world);
    assert_eq!(simulation.entities[1].mind.current_goal, Some(Goal::Eat));
    let remaining = simulation
        .entities
        .iter()
        .map(|entity| entity.inventory.amount(ItemKind::Food))
        .sum::<u16>()
        + simulation.households[0].storage.amount(ItemKind::Food);
    assert_eq!(remaining + 10, initial_total);
}
