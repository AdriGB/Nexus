use super::super::autonomy::{Action, Goal, URGENT_HUNGER_THRESHOLD};
use super::super::households::Household;
use super::super::time::TICKS_PER_YEAR;
use super::super::{Inventory, ItemKind, Simulation};
use super::support::{entity, grid_from_rows, plain_grid};
use crate::world::ResourceKind;

const CAREGIVER_ID: u32 = 1;
const DEPENDENT_ID: u32 = 2;

#[derive(Clone, Copy)]
enum DependentStage {
    Infant,
    Child,
}

fn household(food: u16) -> Household {
    let mut storage = Inventory::new(200);
    storage.add(ItemKind::Food, food);
    Household {
        id: 1,
        formed_tick: 0,
        dissolved_tick: None,
        inheritance: None,
        residence_x: 0,
        residence_y: 0,
        storage,
    }
}

fn provisioning_simulation(stage: DependentStage, household_food: u16) -> Simulation {
    let mut caregiver = entity(CAREGIVER_ID, 0, 0, 0.0);
    caregiver.age_ticks = 25 * TICKS_PER_YEAR;
    caregiver.household_id = Some(1);
    caregiver.personality.curiosity = 0.0;

    let mut dependent = entity(DEPENDENT_ID, 0, 0, 80.0);
    dependent.age_ticks = match stage {
        DependentStage::Infant => 0,
        DependentStage::Child => 8 * TICKS_PER_YEAR,
    };
    dependent.caregiver_id = Some(CAREGIVER_ID);
    dependent.household_id = Some(1);

    Simulation {
        entities: vec![caregiver, dependent],
        next_entity_id: 3,
        households: vec![household(household_food)],
        next_household_id: 2,
        ..Simulation::default()
    }
}

fn decision(simulation: &Simulation) -> (Goal, &'static str) {
    let explanation = simulation.entities[0]
        .mind
        .decision_explanation
        .expect("caregiver decision explanation");
    (explanation.chosen_goal, explanation.reason.label())
}

#[test]
fn hungry_child_causes_caregiver_provisioning() {
    let mut simulation = provisioning_simulation(DependentStage::Child, 0);
    simulation.step(&mut plain_grid(4, 1));
    assert_eq!(
        decision(&simulation),
        (Goal::AcquireResource, "dependent_provisioning")
    );
}

#[test]
fn caregiver_with_food_prefers_share_for_hungry_child() {
    let mut simulation = provisioning_simulation(DependentStage::Child, 0);
    simulation.entities[0].inventory.add(ItemKind::Food, 10);
    simulation.step(&mut plain_grid(1, 1));
    assert_eq!(
        decision(&simulation),
        (Goal::ShareFood, "dependent_provisioning")
    );
    assert_eq!(simulation.entities[1].inventory.amount(ItemKind::Food), 10);
}

#[test]
fn caregiver_without_food_acquires_for_hungry_child() {
    let mut simulation = provisioning_simulation(DependentStage::Child, 0);
    simulation.step(&mut grid_from_rows(&["PF"]));
    assert_eq!(
        decision(&simulation),
        (Goal::AcquireResource, "dependent_provisioning")
    );
    assert!(simulation.entities[0]
        .mind
        .current_plan
        .contains(&Action::Gather(ResourceKind::Food)));
}

#[test]
fn caregiver_uses_household_food_for_hungry_child() {
    let mut simulation = provisioning_simulation(DependentStage::Child, 30);
    simulation.step(&mut plain_grid(1, 1));
    assert_eq!(simulation.entities[0].inventory.amount(ItemKind::Food), 10);
    assert_eq!(simulation.households[0].storage.amount(ItemKind::Food), 20);
}

#[test]
fn partial_personal_food_withdraws_only_missing_amount() {
    let mut simulation = provisioning_simulation(DependentStage::Child, 30);
    simulation.entities[0].inventory.add(ItemKind::Food, 4);
    simulation.step(&mut plain_grid(1, 1));
    assert_eq!(simulation.entities[0].inventory.amount(ItemKind::Food), 10);
    assert_eq!(simulation.households[0].storage.amount(ItemKind::Food), 24);
}

#[test]
fn empty_household_storage_falls_back_to_world_food() {
    let mut simulation = provisioning_simulation(DependentStage::Child, 0);
    simulation.step(&mut grid_from_rows(&["F"]));
    assert!(simulation.entities[0]
        .mind
        .current_plan
        .contains(&Action::Gather(ResourceKind::Food)));
}

#[test]
fn caregiver_explores_when_no_food_source_is_known() {
    let mut simulation = provisioning_simulation(DependentStage::Child, 0);
    simulation.step(&mut plain_grid(32, 32));
    assert_eq!(
        decision(&simulation),
        (Goal::AcquireResource, "dependent_provisioning")
    );
    assert_eq!(
        simulation.entities[0].mind.current_goal,
        Some(Goal::Explore)
    );
}

#[test]
fn optional_non_dependent_sharing_behavior_is_unchanged() {
    let mut simulation = provisioning_simulation(DependentStage::Child, 0);
    simulation.entities[1].caregiver_id = None;
    simulation.entities[0].inventory.add(ItemKind::Food, 10);
    simulation.step(&mut plain_grid(1, 1));
    assert_ne!(
        simulation.entities[0].mind.current_goal,
        Some(Goal::ShareFood)
    );
    assert_eq!(simulation.entities[1].inventory.amount(ItemKind::Food), 0);
}

#[test]
fn hungry_infant_causes_caregiver_provisioning() {
    let mut simulation = provisioning_simulation(DependentStage::Infant, 0);
    simulation.step(&mut plain_grid(1, 1));
    assert_eq!(
        decision(&simulation),
        (Goal::AcquireResource, "dependent_provisioning")
    );
}

#[test]
fn caregiver_with_meal_eats_to_feed_hungry_infant() {
    let mut simulation = provisioning_simulation(DependentStage::Infant, 0);
    simulation.entities[0].inventory.add(ItemKind::Food, 10);
    simulation.step(&mut plain_grid(1, 1));
    assert_eq!(decision(&simulation), (Goal::Eat, "dependent_provisioning"));
    assert!(simulation.entities[1].hunger < 80.0);
    assert_eq!(simulation.entities[1].inventory.amount(ItemKind::Food), 0);
}

#[test]
fn caregiver_without_meal_acquires_for_hungry_infant() {
    let mut simulation = provisioning_simulation(DependentStage::Infant, 0);
    simulation.entities[0].inventory.add(ItemKind::Food, 9);
    simulation.step(&mut grid_from_rows(&["F"]));
    assert_eq!(
        decision(&simulation),
        (Goal::AcquireResource, "dependent_provisioning")
    );
}

#[test]
fn household_food_can_supply_infant_caregiver() {
    let mut simulation = provisioning_simulation(DependentStage::Infant, 1);
    simulation.entities[0].inventory.add(ItemKind::Food, 8);
    simulation.step(&mut plain_grid(1, 1));
    assert_eq!(simulation.entities[0].inventory.amount(ItemKind::Food), 9);
    assert_eq!(simulation.households[0].storage.amount(ItemKind::Food), 0);
    simulation.step(&mut plain_grid(1, 1));
    assert_eq!(
        decision(&simulation),
        (Goal::AcquireResource, "dependent_provisioning")
    );
}

#[test]
fn caregiver_consumption_reduces_infant_hunger_through_existing_pipeline() {
    let mut simulation = provisioning_simulation(DependentStage::Infant, 0);
    simulation.entities[0].inventory.add(ItemKind::Food, 10);
    let before = simulation.entities[1].hunger;
    simulation.step(&mut plain_grid(1, 1));
    assert!(simulation.entities[1].hunger < before);
    assert_eq!(simulation.entities[1].inventory.amount(ItemKind::Food), 0);
}

#[test]
fn infant_is_prioritized_over_child() {
    let mut simulation = provisioning_simulation(DependentStage::Infant, 0);
    let mut child = entity(3, 0, 0, 90.0);
    child.age_ticks = 8 * TICKS_PER_YEAR;
    child.caregiver_id = Some(CAREGIVER_ID);
    simulation.entities.push(child);
    simulation.next_entity_id = 4;
    simulation.entities[0].inventory.add(ItemKind::Food, 10);
    simulation.step(&mut plain_grid(1, 1));
    assert_eq!(decision(&simulation), (Goal::Eat, "dependent_provisioning"));
    assert_eq!(simulation.entities[2].inventory.amount(ItemKind::Food), 0);
}

#[test]
fn child_is_provisioned_after_infant_need_is_satisfied() {
    let mut simulation = provisioning_simulation(DependentStage::Infant, 0);
    let mut child = entity(3, 0, 0, 90.0);
    child.age_ticks = 8 * TICKS_PER_YEAR;
    child.caregiver_id = Some(CAREGIVER_ID);
    simulation.entities.push(child);
    simulation.next_entity_id = 4;
    simulation.entities[0].inventory.add(ItemKind::Food, 20);
    let mut world = plain_grid(1, 1);
    simulation.step(&mut world);
    simulation.step(&mut world);
    assert_eq!(simulation.entities[2].inventory.amount(ItemKind::Food), 10);
}

#[test]
fn urgent_caregiver_hunger_takes_priority_over_dependent() {
    let mut simulation = provisioning_simulation(DependentStage::Child, 0);
    simulation.entities[0].hunger = URGENT_HUNGER_THRESHOLD;
    simulation.entities[0].inventory.add(ItemKind::Food, 10);
    simulation.step(&mut plain_grid(1, 1));
    assert_eq!(simulation.entities[0].mind.current_goal, Some(Goal::Eat));
    assert_ne!(decision(&simulation).1, "dependent_provisioning");
    assert_eq!(simulation.entities[1].inventory.amount(ItemKind::Food), 0);
}

#[test]
fn urgent_hunger_interrupts_an_active_dependent_share_plan() {
    let mut simulation = provisioning_simulation(DependentStage::Child, 0);
    simulation.entities[0].hunger = URGENT_HUNGER_THRESHOLD;
    simulation.entities[0].inventory.add(ItemKind::Food, 10);
    simulation.entities[0]
        .mind
        .set_plan(Goal::ShareFood, vec![Action::ShareFood(DEPENDENT_ID)], 0);
    simulation.step(&mut plain_grid(1, 1));
    assert_eq!(simulation.entities[0].mind.current_goal, Some(Goal::Eat));
    assert_eq!(simulation.entities[1].inventory.amount(ItemKind::Food), 0);
}

#[test]
fn caregiver_resumes_dependent_provisioning_after_self_feeding() {
    let mut simulation = provisioning_simulation(DependentStage::Child, 0);
    simulation.entities[0].hunger = URGENT_HUNGER_THRESHOLD;
    simulation.entities[0].inventory.add(ItemKind::Food, 20);
    let mut world = plain_grid(1, 1);
    simulation.step(&mut world);
    simulation.step(&mut world);
    assert_eq!(simulation.entities[1].inventory.amount(ItemKind::Food), 10);
}

#[test]
fn infant_provisioning_does_not_require_visibility() {
    let mut simulation = provisioning_simulation(DependentStage::Infant, 0);
    simulation.entities[1].x = 20;
    simulation.entities[1].y = 20;
    simulation.step(&mut plain_grid(32, 32));
    assert_eq!(
        decision(&simulation),
        (Goal::AcquireResource, "dependent_provisioning")
    );
}

#[test]
fn child_provisioning_preserves_visibility_requirement() {
    let mut simulation = provisioning_simulation(DependentStage::Child, 0);
    simulation.entities[1].x = 20;
    simulation.entities[1].y = 20;
    simulation.step(&mut plain_grid(32, 32));
    assert_ne!(decision(&simulation).1, "dependent_provisioning");
}

#[test]
fn elder_caregiver_can_provision_dependent() {
    let mut simulation = provisioning_simulation(DependentStage::Child, 10);
    simulation.entities[0].age_ticks = 70 * TICKS_PER_YEAR;
    simulation.step(&mut plain_grid(1, 1));
    assert_eq!(simulation.entities[0].inventory.amount(ItemKind::Food), 10);
    assert_eq!(decision(&simulation).1, "dependent_provisioning");
}

#[test]
fn child_follow_behavior_is_unchanged() {
    let mut simulation = provisioning_simulation(DependentStage::Child, 0);
    simulation.entities[1].hunger = 0.0;
    simulation.step(&mut plain_grid(2, 1));
    assert_eq!(simulation.entities[1].mind.current_goal, Some(Goal::Follow));
}

#[test]
fn provisioning_does_not_change_household_membership() {
    let mut simulation = provisioning_simulation(DependentStage::Child, 10);
    let before: Vec<_> = simulation
        .entities
        .iter()
        .map(|entity| entity.household_id)
        .collect();
    simulation.step(&mut plain_grid(1, 1));
    simulation.step(&mut plain_grid(1, 1));
    let after: Vec<_> = simulation
        .entities
        .iter()
        .map(|entity| entity.household_id)
        .collect();
    assert_eq!(after, before);
}

#[test]
fn child_household_provisioning_is_end_to_end_and_conserves_food() {
    let mut simulation = provisioning_simulation(DependentStage::Child, 20);
    let mut world = plain_grid(1, 1);
    simulation.step(&mut world);
    assert_eq!(decision(&simulation).0, Goal::AcquireResource);
    simulation.step(&mut world);
    assert_eq!(decision(&simulation).0, Goal::ShareFood);
    assert_eq!(simulation.households[0].storage.amount(ItemKind::Food), 10);
    assert_eq!(simulation.entities[0].inventory.amount(ItemKind::Food), 0);
    assert_eq!(simulation.entities[1].inventory.amount(ItemKind::Food), 10);
}

#[test]
fn infant_household_provisioning_is_end_to_end_and_conserves_food() {
    let mut simulation = provisioning_simulation(DependentStage::Infant, 20);
    let before_hunger = simulation.entities[1].hunger;
    let mut world = plain_grid(1, 1);
    simulation.step(&mut world);
    assert_eq!(decision(&simulation).0, Goal::AcquireResource);
    simulation.step(&mut world);
    assert_eq!(decision(&simulation).0, Goal::Eat);
    assert_eq!(simulation.households[0].storage.amount(ItemKind::Food), 10);
    assert_eq!(simulation.entities[0].inventory.amount(ItemKind::Food), 0);
    assert_eq!(simulation.entities[1].inventory.amount(ItemKind::Food), 0);
    assert!(simulation.entities[1].hunger < before_hunger);
}

#[test]
fn identical_simulations_produce_identical_care_behavior() {
    let mut first = provisioning_simulation(DependentStage::Child, 20);
    let mut second = first.clone();
    let mut first_world = plain_grid(1, 1);
    let mut second_world = plain_grid(1, 1);
    for _ in 0..3 {
        first.step(&mut first_world);
        second.step(&mut second_world);
    }
    assert_eq!(
        first.entities[0].mind.current_goal,
        second.entities[0].mind.current_goal
    );
    assert_eq!(first.entities[0].inventory, second.entities[0].inventory);
    assert_eq!(first.entities[1].inventory, second.entities[1].inventory);
    assert_eq!(first.households, second.households);
}

#[test]
fn normal_and_profiled_paths_match() {
    let mut normal = provisioning_simulation(DependentStage::Infant, 20);
    let mut profiled = normal.clone();
    let mut normal_world = plain_grid(1, 1);
    let mut profiled_world = plain_grid(1, 1);
    for _ in 0..3 {
        normal.step(&mut normal_world);
        profiled.profile_step(&mut profiled_world);
    }
    assert_eq!(
        normal.entities[0].mind.current_goal,
        profiled.entities[0].mind.current_goal
    );
    assert_eq!(normal.entities[0].inventory, profiled.entities[0].inventory);
    assert_eq!(normal.entities[1].hunger, profiled.entities[1].hunger);
    assert_eq!(normal.households, profiled.households);
}
