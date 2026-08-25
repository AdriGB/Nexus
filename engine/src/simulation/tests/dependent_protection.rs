use super::super::autonomy::{
    Action, Goal, DEPENDENT_PROTECTION_TRIGGER_DISTANCE, DEPENDENT_REUNION_RADIUS,
    URGENT_HUNGER_THRESHOLD,
};
use super::super::time::TICKS_PER_YEAR;
use super::super::{ItemKind, Simulation};
use super::support::{entity, plain_grid};

const CAREGIVER_ID: u32 = 1;

fn caregiver(id: u32, x: u32, y: u32) -> super::super::entity::Entity {
    let mut caregiver = entity(id, x, y, 0.0);
    caregiver.age_ticks = 25 * TICKS_PER_YEAR;
    caregiver.personality.curiosity = 0.0;
    caregiver
}

fn child(id: u32, x: u32, y: u32, caregiver_id: Option<u32>) -> super::super::entity::Entity {
    let mut child = entity(id, x, y, 0.0);
    child.age_ticks = 8 * TICKS_PER_YEAR;
    child.caregiver_id = caregiver_id;
    child
}

fn simulation_with_child(distance: u32) -> Simulation {
    Simulation {
        entities: vec![
            caregiver(CAREGIVER_ID, 1, 1),
            child(2, 1 + distance, 1, Some(CAREGIVER_ID)),
        ],
        next_entity_id: 3,
        ..Simulation::default()
    }
}

fn target(simulation: &Simulation) -> Option<u32> {
    simulation.entities[0]
        .mind
        .current_action()
        .and_then(Action::target_entity_id)
}

#[test]
fn visible_separated_child_triggers_protection() {
    let mut simulation = simulation_with_child(5);
    simulation.step(&mut plain_grid(12, 3));
    assert_eq!(
        simulation.entities[0].mind.current_goal,
        Some(Goal::ProtectDependent)
    );
    assert_eq!(target(&simulation), Some(2));
    assert_eq!(
        simulation.entities[0]
            .mind
            .decision_explanation
            .expect("protection explanation")
            .reason
            .label(),
        "dependent_protection"
    );
}

#[test]
fn child_at_trigger_distance_does_not_trigger() {
    let mut simulation = simulation_with_child(DEPENDENT_PROTECTION_TRIGGER_DISTANCE);
    simulation.step(&mut plain_grid(12, 3));
    assert_ne!(
        simulation.entities[0].mind.current_goal,
        Some(Goal::ProtectDependent)
    );
}

#[test]
fn child_beyond_trigger_distance_does_trigger() {
    let mut simulation = simulation_with_child(DEPENDENT_PROTECTION_TRIGGER_DISTANCE + 1);
    simulation.step(&mut plain_grid(12, 3));
    assert_eq!(
        simulation.entities[0].mind.current_goal,
        Some(Goal::ProtectDependent)
    );
}

#[test]
fn protection_targets_farthest_assigned_child() {
    let mut simulation = Simulation {
        entities: vec![
            caregiver(1, 1, 1),
            child(2, 6, 1, Some(1)),
            child(3, 7, 1, Some(1)),
        ],
        next_entity_id: 4,
        ..Simulation::default()
    };
    simulation.step(&mut plain_grid(12, 3));
    assert_eq!(target(&simulation), Some(3));
}

#[test]
fn equal_distance_uses_lower_child_id() {
    let mut simulation = Simulation {
        entities: vec![
            caregiver(1, 6, 2),
            child(3, 1, 2, Some(1)),
            child(2, 11, 2, Some(1)),
        ],
        next_entity_id: 4,
        ..Simulation::default()
    };
    simulation.step(&mut plain_grid(13, 5));
    assert_eq!(target(&simulation), Some(2));
}

#[test]
fn infant_does_not_trigger_protection() {
    let mut infant = child(2, 6, 1, Some(1));
    infant.age_ticks = 0;
    let mut simulation = Simulation {
        entities: vec![caregiver(1, 1, 1), infant],
        next_entity_id: 3,
        ..Simulation::default()
    };
    simulation.step(&mut plain_grid(12, 3));
    assert_ne!(
        simulation.entities[0].mind.current_goal,
        Some(Goal::ProtectDependent)
    );
}

#[test]
fn adolescent_does_not_trigger_protection() {
    let mut adolescent = child(3, 7, 1, Some(1));
    adolescent.age_ticks = 14 * TICKS_PER_YEAR;
    let mut simulation = Simulation {
        entities: vec![caregiver(1, 1, 1), adolescent],
        next_entity_id: 4,
        ..Simulation::default()
    };
    simulation.step(&mut plain_grid(12, 3));
    assert_ne!(
        simulation.entities[0].mind.current_goal,
        Some(Goal::ProtectDependent)
    );
}

#[test]
fn unassigned_child_does_not_trigger_protection() {
    let mut simulation = Simulation {
        entities: vec![caregiver(1, 1, 1), child(2, 6, 1, None)],
        next_entity_id: 3,
        ..Simulation::default()
    };
    simulation.step(&mut plain_grid(12, 3));
    assert_ne!(
        simulation.entities[0].mind.current_goal,
        Some(Goal::ProtectDependent)
    );
}

#[test]
fn child_assigned_to_other_caregiver_does_not_trigger() {
    let mut simulation = Simulation {
        entities: vec![
            caregiver(1, 1, 1),
            child(2, 6, 1, Some(3)),
            caregiver(3, 7, 1),
        ],
        next_entity_id: 4,
        ..Simulation::default()
    };
    simulation.step(&mut plain_grid(12, 3));
    assert_ne!(
        simulation.entities[0].mind.current_goal,
        Some(Goal::ProtectDependent)
    );
}

#[test]
fn biological_child_without_caregiver_assignment_does_not_trigger() {
    let mut biological_child = child(2, 6, 1, None);
    biological_child.mother_id = Some(1);
    let mut simulation = Simulation {
        entities: vec![caregiver(1, 1, 1), biological_child],
        next_entity_id: 3,
        ..Simulation::default()
    };
    simulation.step(&mut plain_grid(12, 3));
    assert_ne!(
        simulation.entities[0].mind.current_goal,
        Some(Goal::ProtectDependent)
    );
}

#[test]
fn invisible_child_does_not_start_protection() {
    let mut simulation = simulation_with_child(7);
    simulation.step(&mut plain_grid(12, 3));
    assert_ne!(
        simulation.entities[0].mind.current_goal,
        Some(Goal::ProtectDependent)
    );
}

#[test]
fn adult_caregiver_can_protect() {
    let mut simulation = simulation_with_child(5);
    simulation.step(&mut plain_grid(12, 3));
    assert_eq!(
        simulation.entities[0].mind.current_goal,
        Some(Goal::ProtectDependent)
    );
}

#[test]
fn elder_caregiver_can_protect() {
    let mut simulation = simulation_with_child(5);
    simulation.entities[0].age_ticks = 70 * TICKS_PER_YEAR;
    simulation.step(&mut plain_grid(12, 3));
    assert_eq!(
        simulation.entities[0].mind.current_goal,
        Some(Goal::ProtectDependent)
    );
}

#[test]
fn dependent_provisioning_outranks_protection() {
    let mut simulation = simulation_with_child(5);
    simulation.entities[1].hunger = 80.0;
    simulation.entities[0].inventory.add(ItemKind::Food, 10);
    simulation.step(&mut plain_grid(12, 3));
    assert_eq!(
        simulation.entities[0].mind.current_goal,
        Some(Goal::ShareFood)
    );
    assert_eq!(
        simulation.entities[0]
            .mind
            .decision_explanation
            .expect("provisioning explanation")
            .reason
            .label(),
        "dependent_provisioning"
    );
}

#[test]
fn urgent_caregiver_hunger_outranks_active_protection() {
    let mut simulation = simulation_with_child(5);
    simulation.entities[0].mind.set_plan(
        Goal::ProtectDependent,
        vec![Action::ApproachEntity(2)],
        0,
    );
    simulation.entities[0].hunger = URGENT_HUNGER_THRESHOLD;
    simulation.entities[0].inventory.add(ItemKind::Food, 10);
    simulation.step(&mut plain_grid(12, 3));
    assert_eq!(simulation.entities[0].mind.current_goal, Some(Goal::Eat));
}

#[test]
fn protection_interrupts_explore() {
    let mut simulation = simulation_with_child(5);
    simulation.entities[0]
        .mind
        .set_plan(Goal::Explore, vec![Action::Wait], 0);
    simulation.step(&mut plain_grid(12, 3));
    assert_eq!(
        simulation.entities[0].mind.current_goal,
        Some(Goal::ProtectDependent)
    );
}

#[test]
fn protection_interrupts_socialize() {
    let mut simulation = simulation_with_child(5);
    simulation.entities[0]
        .mind
        .set_plan(Goal::Socialize, vec![Action::Wait], 0);
    simulation.step(&mut plain_grid(12, 3));
    assert_eq!(
        simulation.entities[0].mind.current_goal,
        Some(Goal::ProtectDependent)
    );
}

#[test]
fn active_protection_uses_reunion_hysteresis() {
    let mut simulation = simulation_with_child(4);
    simulation.entities[0].mind.set_plan(
        Goal::ProtectDependent,
        vec![Action::ApproachEntity(2)],
        0,
    );
    simulation.step(&mut plain_grid(12, 3));
    assert_eq!(
        simulation.entities[0].mind.current_goal,
        Some(Goal::ProtectDependent)
    );
    for _ in 0..10 {
        simulation.step(&mut plain_grid(12, 3));
        if simulation.entities[0].mind.current_goal != Some(Goal::ProtectDependent) {
            break;
        }
    }
    assert_ne!(
        simulation.entities[0].mind.current_goal,
        Some(Goal::ProtectDependent)
    );
    assert!(
        simulation.entities[0].x.abs_diff(simulation.entities[1].x)
            + simulation.entities[0].y.abs_diff(simulation.entities[1].y)
            <= DEPENDENT_REUNION_RADIUS
    );
}

#[test]
fn caregiver_and_child_reunite_without_teleporting() {
    let mut simulation = simulation_with_child(5);
    let initial_caregiver = (simulation.entities[0].x, simulation.entities[0].y);
    simulation.step(&mut plain_grid(12, 3));
    assert!(simulation.entities[0].x.abs_diff(initial_caregiver.0) <= 1);
    assert_eq!(simulation.entities[1].mind.current_goal, Some(Goal::Follow));
    for _ in 0..12 {
        simulation.step(&mut plain_grid(12, 3));
    }
    let distance = simulation.entities[0].x.abs_diff(simulation.entities[1].x)
        + simulation.entities[0].y.abs_diff(simulation.entities[1].y);
    assert!(distance <= DEPENDENT_REUNION_RADIUS);
    assert_eq!(simulation.entities[1].caregiver_id, Some(CAREGIVER_ID));
}

#[test]
fn hidden_target_uses_last_seen_position_and_does_not_pollute_social_cooldown() {
    let mut simulation = simulation_with_child(5);
    let mut world = plain_grid(24, 3);
    simulation.step(&mut world);
    let remembered_position = simulation.entities[0]
        .mind
        .memory
        .known_entities
        .iter()
        .find(|known| known.id == 2)
        .map(|known| (known.last_seen_x, known.last_seen_y))
        .expect("visible child should be remembered");
    let retry_before = simulation.entities[0]
        .mind
        .memory
        .known_entities
        .iter()
        .find(|known| known.id == 2)
        .expect("child memory")
        .seek_retry_after_tick;

    simulation.entities[1].x = 20;
    simulation.entities[1]
        .mind
        .set_plan(Goal::Follow, vec![Action::Wait], simulation.tick);
    simulation.step(&mut world);

    assert_ne!(simulation.entities[0].path.last().copied(), Some((20, 1)));
    assert!(simulation.entities[0]
        .path
        .last()
        .copied()
        .is_none_or(|target| target == remembered_position));

    for _ in 0..10 {
        simulation.step(&mut world);
        if simulation.entities[0].mind.current_goal != Some(Goal::ProtectDependent) {
            break;
        }
    }
    assert_ne!(
        simulation.entities[0].mind.current_goal,
        Some(Goal::ProtectDependent)
    );
    let retry_after = simulation.entities[0]
        .mind
        .memory
        .known_entities
        .iter()
        .find(|known| known.id == 2)
        .expect("child memory remains")
        .seek_retry_after_tick;
    assert_eq!(retry_after, retry_before);
}

#[test]
fn identical_simulations_produce_identical_protection() {
    let mut a = simulation_with_child(5);
    let mut b = a.clone();
    a.step(&mut plain_grid(12, 3));
    b.step(&mut plain_grid(12, 3));
    let state = |simulation: &Simulation| {
        simulation
            .entities()
            .iter()
            .map(|entity| {
                (
                    entity.id,
                    entity.x,
                    entity.y,
                    entity.mind.current_goal,
                    entity.mind.current_action(),
                    entity.path.clone(),
                    entity.caregiver_id,
                    entity.household_id,
                )
            })
            .collect::<Vec<_>>()
    };
    assert_eq!(state(&a), state(&b));
}

#[test]
fn dependent_protection_matches_normal_and_profiled_paths() {
    let mut normal = simulation_with_child(5);
    let mut profiled = normal.clone();
    let mut autonomy_profiled = normal.clone();
    normal.step(&mut plain_grid(12, 3));
    profiled.profile_step(&mut plain_grid(12, 3));
    autonomy_profiled.profile_autonomy_step(&mut plain_grid(12, 3));
    let state = |simulation: &Simulation| {
        simulation
            .entities()
            .iter()
            .map(|entity| {
                (
                    entity.x,
                    entity.y,
                    entity.mind.current_goal,
                    entity.mind.current_action(),
                    entity.path.clone(),
                    entity.caregiver_id,
                    entity.household_id,
                )
            })
            .collect::<Vec<_>>()
    };
    assert_eq!(state(&normal), state(&profiled));
    assert_eq!(state(&normal), state(&autonomy_profiled));
}
