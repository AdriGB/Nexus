use super::super::autonomy::{Action, Goal, KnownResource};
use super::super::households::Household;
use super::super::time::TICKS_PER_YEAR;
use super::super::Simulation;
use super::support::{entity, grid_from_rows, plain_grid};
use crate::world::ResourceKind;

fn resting_member(id: u32, x: u32, y: u32) -> super::super::Entity {
    let mut member = entity(id, x, y, 0.0);
    member.age_ticks = 25 * TICKS_PER_YEAR;
    member.health = 1.0;
    member.household_id = Some(1);
    member.personality.curiosity = 0.0;
    member.personality.caution = 1.0;
    member
}

fn household(residence: (u32, u32)) -> Household {
    Household {
        id: 1,
        formed_tick: 0,
        dissolved_tick: None,
        inheritance: None,
        residence_x: residence.0,
        residence_y: residence.1,
        storage: super::super::Inventory::new(200),
    }
}

fn simulation_with_member(position: (u32, u32), residence: (u32, u32)) -> Simulation {
    Simulation {
        entities: vec![resting_member(1, position.0, position.1)],
        next_entity_id: 2,
        households: vec![household(residence)],
        next_household_id: 2,
        ..Simulation::default()
    }
}

#[test]
fn household_member_plans_rest_at_residence() {
    let mut simulation = simulation_with_member((0, 0), (4, 0));
    let mut world = plain_grid(5, 1);
    simulation.step(&mut world);

    let member = &simulation.entities[0];
    assert_eq!(member.mind.current_goal, Some(Goal::Rest));
    assert_eq!(
        member.mind.current_plan,
        vec![Action::MoveTo(4, 0), Action::Wait]
    );
    assert_eq!(member.path.last(), Some(&(4, 0)));
}

#[test]
fn household_member_already_home_waits_immediately() {
    let mut simulation = simulation_with_member((2, 0), (2, 0));

    simulation.step(&mut plain_grid(3, 1));

    let member = &simulation.entities[0];
    assert_eq!((member.x, member.y), (2, 0));
    assert_eq!(member.mind.current_goal, Some(Goal::Rest));
    assert_eq!(member.mind.current_plan, vec![Action::Wait]);
    assert!(member.path.is_empty());
}

#[test]
fn entity_without_household_rests_in_place() {
    let mut member = resting_member(1, 1, 0);
    member.household_id = None;
    let mut simulation = Simulation {
        entities: vec![member],
        next_entity_id: 2,
        ..Simulation::default()
    };

    simulation.step(&mut plain_grid(3, 1));

    assert_eq!((simulation.entities[0].x, simulation.entities[0].y), (1, 0));
    assert_eq!(simulation.entities[0].mind.current_plan, vec![Action::Wait]);
}

#[test]
fn unreachable_residence_falls_back_to_local_rest() {
    let mut simulation = simulation_with_member((0, 0), (2, 0));

    simulation.step(&mut grid_from_rows(&["P#P"]));

    assert_eq!((simulation.entities[0].x, simulation.entities[0].y), (0, 0));
    assert_eq!(simulation.entities[0].mind.current_plan, vec![Action::Wait]);
    assert_eq!(simulation.entities[0].household_id, Some(1));
}

#[test]
fn rest_home_path_uses_normal_pathfinding() {
    let mut simulation = simulation_with_member((0, 0), (4, 0));
    let mut world = plain_grid(5, 1);

    simulation.step(&mut world);

    assert_eq!((simulation.entities[0].x, simulation.entities[0].y), (1, 0));
    assert_eq!(
        simulation.entities[0].mind.current_action(),
        Some(Action::MoveTo(4, 0))
    );
}

#[test]
fn urgent_hunger_still_beats_rest() {
    let mut simulation = simulation_with_member((0, 0), (4, 0));
    simulation.entities[0].health = 100.0;
    simulation.entities[0].hunger = 90.0;
    simulation.entities[0]
        .mind
        .memory
        .known_resources
        .push(KnownResource {
            x: 1,
            y: 0,
            kind: ResourceKind::Food,
            last_seen_tick: 0,
            estimated_amount: 20,
            failed_attempts: 0,
            avoid_until_tick: 0,
        });
    let mut world = grid_from_rows(&["PFPPP"]);

    simulation.step(&mut world);

    assert_eq!(
        simulation.entities[0].mind.current_goal,
        Some(Goal::AcquireResource)
    );
}

#[test]
fn child_follow_behavior_is_unchanged() {
    let mut caregiver = resting_member(1, 4, 0);
    caregiver.health = 100.0;
    let mut child = entity(2, 0, 0, 0.0);
    child.age_ticks = 8 * TICKS_PER_YEAR;
    child.caregiver_id = Some(1);
    child.household_id = Some(1);
    let mut simulation = Simulation {
        entities: vec![caregiver, child],
        next_entity_id: 3,
        households: vec![household((2, 0))],
        next_household_id: 2,
        ..Simulation::default()
    };

    simulation.step(&mut plain_grid(5, 1));

    assert_eq!(simulation.entities[1].mind.current_goal, Some(Goal::Follow));
}

#[test]
fn household_residence_does_not_move_when_member_rests() {
    let mut simulation = simulation_with_member((0, 0), (4, 0));
    let residence = simulation.households[0].clone();
    let mut world = plain_grid(5, 1);

    for _ in 0..5 {
        simulation.step(&mut world);
    }

    assert_eq!(simulation.households[0], residence);
}

#[test]
fn identical_simulations_produce_identical_home_rest_behavior() {
    let mut first = simulation_with_member((0, 0), (4, 0));
    let mut second = simulation_with_member((0, 0), (4, 0));
    let mut first_world = plain_grid(5, 1);
    let mut second_world = plain_grid(5, 1);

    for _ in 0..5 {
        first.step(&mut first_world);
        second.step(&mut second_world);
    }

    assert_eq!(
        (first.entities[0].x, first.entities[0].y),
        (second.entities[0].x, second.entities[0].y)
    );
    assert_eq!(
        first.entities[0].mind.current_plan,
        second.entities[0].mind.current_plan
    );
    assert_eq!(
        first.entities[0].mind.plan_index,
        second.entities[0].mind.plan_index
    );
}

#[test]
fn adult_members_independently_rest_at_shared_residence() {
    let first = resting_member(1, 0, 0);
    let second = resting_member(2, 6, 0);
    let mut simulation = Simulation {
        entities: vec![first, second],
        next_entity_id: 3,
        households: vec![household((3, 0))],
        next_household_id: 2,
        ..Simulation::default()
    };
    let mut world = plain_grid(7, 1);

    for _ in 0..5 {
        simulation.step(&mut world);
    }

    assert_eq!((simulation.entities[0].x, simulation.entities[0].y), (3, 0));
    assert_eq!((simulation.entities[1].x, simulation.entities[1].y), (3, 0));
    assert_eq!(simulation.entities[0].mind.current_goal, Some(Goal::Rest));
    assert_eq!(simulation.entities[1].mind.current_goal, Some(Goal::Rest));
}
