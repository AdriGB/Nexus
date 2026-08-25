use super::super::autonomy::{
    best_household_conflict_candidate, Action, Goal, HouseholdConflictAttempt, KnownEntity,
};
use super::super::households::Household;
use super::super::time::{TICKS_PER_DAY, TICKS_PER_YEAR};
use super::super::{
    Inventory, ItemKind, Simulation, SimulationEventCause, SimulationEventDetails,
    SimulationEventKind,
};
use super::support::{entity, plain_grid};

fn known(id: u32, affinity: i16) -> KnownEntity {
    KnownEntity {
        id,
        first_seen_tick: 0,
        last_seen_tick: 0,
        last_seen_x: id,
        last_seen_y: 1,
        observed_ticks: 10,
        affinity,
        last_interaction_tick: 0,
        interaction_count: 1,
        seek_retry_after_tick: None,
    }
}

fn adult(id: u32, affinity: Option<(u32, i16)>) -> super::super::Entity {
    let mut entity = entity(id, id, 1, 40.0);
    entity.age_ticks = 30 * TICKS_PER_YEAR;
    entity.household_id = Some(1);
    entity.personality.cooperativeness = 0.0;
    if let Some((target, affinity)) = affinity {
        entity
            .mind
            .memory
            .known_entities
            .push(known(target, affinity));
    }
    entity
}

fn household() -> Household {
    Household {
        id: 1,
        formed_tick: 0,
        dissolved_tick: None,
        inheritance: None,
        migration: None,
        residence_x: 1,
        residence_y: 1,
        storage: Inventory::new(200),
    }
}

fn simulation(affinity: i16) -> Simulation {
    Simulation {
        entities: vec![adult(1, Some((2, affinity))), adult(2, Some((1, -150)))],
        next_entity_id: 3,
        households: vec![household()],
        next_household_id: 2,
        ..Simulation::default()
    }
}

fn prepare(simulation: &mut Simulation) {
    simulation.rebuild_population_index(&plain_grid(12, 4));
    for entity in &mut simulation.entities {
        entity.mind.visible_entities = simulation
            .population_cache
            .iter()
            .filter(|snapshot| snapshot.id != entity.id)
            .map(|snapshot| snapshot.id)
            .collect();
    }
}

#[test]
fn hostile_visible_household_member_is_conflict_candidate() {
    let mut simulation = simulation(-500);
    prepare(&mut simulation);
    assert_eq!(
        best_household_conflict_candidate(&simulation.entities[0], &simulation.population_cache, 1)
            .unwrap()
            .target_id,
        2
    );
}

#[test]
fn positive_outsider_hidden_and_former_members_are_not_candidates() {
    let mut positive = simulation(100);
    prepare(&mut positive);
    assert!(best_household_conflict_candidate(
        &positive.entities[0],
        &positive.population_cache,
        1
    )
    .is_none());

    let mut outsider = simulation(-500);
    outsider.entities[1].household_id = None;
    prepare(&mut outsider);
    assert!(best_household_conflict_candidate(
        &outsider.entities[0],
        &outsider.population_cache,
        1
    )
    .is_none());

    let mut hidden = simulation(-500);
    prepare(&mut hidden);
    hidden.entities[0].mind.visible_entities.clear();
    assert!(
        best_household_conflict_candidate(&hidden.entities[0], &hidden.population_cache, 1)
            .is_none()
    );
}

#[test]
fn child_and_infant_are_neither_initiators_nor_targets() {
    for age in [1, 8] {
        let mut simulation = simulation(-500);
        simulation.entities[1].age_ticks = age * TICKS_PER_YEAR;
        prepare(&mut simulation);
        assert!(best_household_conflict_candidate(
            &simulation.entities[0],
            &simulation.population_cache,
            1
        )
        .is_none());
        simulation.entities[0].age_ticks = age * TICKS_PER_YEAR;
        simulation.entities[1].age_ticks = 30 * TICKS_PER_YEAR;
        prepare(&mut simulation);
        assert!(best_household_conflict_candidate(
            &simulation.entities[0],
            &simulation.population_cache,
            1
        )
        .is_none());
    }
}

#[test]
fn adolescent_adult_and_elder_can_initiate_conflict() {
    for age in [15, 30, 70] {
        let mut simulation = simulation(-500);
        simulation.entities[0].age_ticks = age * TICKS_PER_YEAR;
        prepare(&mut simulation);
        assert!(best_household_conflict_candidate(
            &simulation.entities[0],
            &simulation.population_cache,
            1
        )
        .is_some());
    }
}

#[test]
fn hostility_hunger_and_low_cooperativeness_raise_score_but_cannot_create_eligibility() {
    let mut base = simulation(-500);
    prepare(&mut base);
    let base_score =
        best_household_conflict_candidate(&base.entities[0], &base.population_cache, 1)
            .unwrap()
            .score;
    base.entities[0].hunger = 80.0;
    let hungry = best_household_conflict_candidate(&base.entities[0], &base.population_cache, 1)
        .unwrap()
        .score;
    assert!(hungry > base_score);
    base.entities[0].mind.memory.known_entities[0].affinity = 100;
    assert!(
        best_household_conflict_candidate(&base.entities[0], &base.population_cache, 1).is_none()
    );
}

#[test]
fn more_hostile_and_lower_id_targets_win_deterministically() {
    let mut actor = adult(1, None);
    actor.mind.memory.known_entities = vec![known(2, -300), known(3, -600)];
    let mut simulation = Simulation {
        entities: vec![actor, adult(2, None), adult(3, None)],
        next_entity_id: 4,
        households: vec![household()],
        next_household_id: 2,
        ..Simulation::default()
    };
    prepare(&mut simulation);
    assert_eq!(
        best_household_conflict_candidate(&simulation.entities[0], &simulation.population_cache, 1)
            .unwrap()
            .target_id,
        3
    );
    simulation.entities[0].mind.memory.known_entities[0].affinity = -600;
    assert_eq!(
        best_household_conflict_candidate(&simulation.entities[0], &simulation.population_cache, 1)
            .unwrap()
            .target_id,
        2
    );
}

#[test]
fn strong_tension_chooses_confront_without_teleporting() {
    let mut simulation = simulation(-900);
    let before = (simulation.entities[0].x, simulation.entities[0].y);
    simulation.step(&mut plain_grid(12, 4));
    assert_eq!(
        simulation.entities[0].mind.current_goal,
        Some(Goal::ConfrontHouseholdMember)
    );
    assert_eq!((simulation.entities[0].x, simulation.entities[0].y), before);
}

#[test]
fn household_conflict_emits_once_reduces_affinity_and_skips_generic_contact() {
    let mut simulation = simulation(-900);
    simulation.step(&mut plain_grid(12, 4));
    let conflicts: Vec<_> = simulation
        .recent_events()
        .filter(|event| event.kind == SimulationEventKind::HouseholdConflict)
        .collect();
    assert_eq!(conflicts.len(), 1);
    assert_eq!(
        simulation
            .recent_events()
            .filter(|event| event.kind == SimulationEventKind::Interaction)
            .count(),
        0
    );
    assert!(simulation.entities[0].mind.memory.affinity_to(2).unwrap() < -900);
    assert!(simulation.entities[1].mind.memory.affinity_to(1).unwrap() < -150);
    assert!(matches!(
        conflicts[0].details,
        SimulationEventDetails::HouseholdConflict {
            household_id: 1,
            ..
        }
    ));
    assert!(simulation.entities[0]
        .mind
        .memory
        .conflict_on_cooldown(2, simulation.tick));
    assert!(simulation.entities[1]
        .mind
        .memory
        .conflict_on_cooldown(1, simulation.tick));
}

#[test]
fn reciprocal_conflict_attempts_produce_one_conflict() {
    let mut simulation = simulation(-900);
    simulation.step(&mut plain_grid(12, 4));
    assert_eq!(
        simulation
            .recent_events()
            .filter(|event| event.kind == SimulationEventKind::HouseholdConflict)
            .count(),
        1
    );
}

#[test]
fn more_hostile_reciprocal_initiator_wins() {
    let mut simulation = simulation(-700);
    simulation.entities[1].mind.memory.known_entities[0].affinity = -900;
    simulation.step(&mut plain_grid(12, 4));
    let conflict = simulation
        .recent_events()
        .find(|event| event.kind == SimulationEventKind::HouseholdConflict)
        .unwrap();
    assert_eq!(conflict.actor_id, 2);
    assert_eq!(conflict.target_id, Some(1));
}

#[test]
fn conflict_affinity_changes_reference_conflict_event() {
    let mut simulation = simulation(-190);
    simulation.entities[1].mind.memory.known_entities[0].affinity = -190;
    simulation.process_household_conflict_attempts(vec![HouseholdConflictAttempt {
        actor_id: 1,
        target_id: 2,
        actor_location: (1, 1),
    }]);
    let conflict_id = simulation
        .recent_events()
        .find(|event| event.kind == SimulationEventKind::HouseholdConflict)
        .unwrap()
        .id;
    let affinity_changes: Vec<_> = simulation
        .recent_events()
        .filter(|event| event.kind == SimulationEventKind::AffinityChange)
        .collect();
    assert_eq!(affinity_changes.len(), 2);
    assert!(affinity_changes.iter().all(|event| {
        event.cause == SimulationEventCause::HouseholdConflict
            && event.caused_by_event_id == Some(conflict_id)
    }));
}

#[test]
fn conflict_caused_partnership_dissolution_references_conflict_event() {
    let mut simulation = simulation(-190);
    simulation.entities[1].mind.memory.known_entities[0].affinity = -190;
    simulation.entities[0].partner_id = Some(2);
    simulation.entities[1].partner_id = Some(1);
    simulation.process_household_conflict_attempts(vec![HouseholdConflictAttempt {
        actor_id: 1,
        target_id: 2,
        actor_location: (1, 1),
    }]);
    let conflict_id = simulation
        .recent_events()
        .find(|event| event.kind == SimulationEventKind::HouseholdConflict)
        .unwrap()
        .id;
    let dissolution = simulation
        .recent_events()
        .find(|event| event.kind == SimulationEventKind::PartnershipDissolved)
        .unwrap();
    assert_eq!(dissolution.cause, SimulationEventCause::HouseholdConflict);
    assert_eq!(dissolution.caused_by_event_id, Some(conflict_id));
}

#[test]
fn visible_target_leaving_household_cancels_active_conflict_pursuit() {
    let mut simulation = simulation(-500);
    simulation.entities[0].x = 0;
    simulation.entities[1].x = 4;
    simulation.entities[0].mind.set_plan(
        Goal::ConfrontHouseholdMember,
        vec![Action::ApproachEntity(2), Action::Interact(2)],
        0,
    );
    simulation.entities[0].path = vec![(1, 1), (2, 1)];
    simulation.entities[0].path_index = 1;
    simulation.entities[0].action_tick = 7;
    simulation.entities[1].household_id = None;

    simulation.step(&mut plain_grid(12, 4));

    assert_eq!(simulation.entities[0].mind.current_goal, None);
    assert!(simulation.entities[0].path.is_empty());
    assert_eq!(simulation.entities[0].path_index, 0);
    assert_eq!(simulation.entities[0].action_tick, 0);
    assert!(!simulation
        .recent_events()
        .any(|event| event.kind == SimulationEventKind::HouseholdConflict));
}

#[test]
fn dead_household_member_cannot_receive_household_conflict() {
    let mut simulation = simulation(-900);
    let original_affinity = simulation.entities[0].mind.memory.affinity_to(2);
    simulation.entities[1].age_ticks = simulation.entities[1].lifespan_ticks - 1;

    simulation.step(&mut plain_grid(12, 4));

    assert_eq!(simulation.entities.len(), 1);
    assert_eq!(
        simulation.entities[0].mind.memory.affinity_to(2),
        original_affinity
    );
    assert!(!simulation
        .recent_events()
        .any(|event| event.kind == SimulationEventKind::HouseholdConflict));
}

#[test]
fn severe_conflict_fragments_membership_without_moving_resources_or_people() {
    let mut simulation = simulation(-900);
    simulation.entities[0].inventory.add(ItemKind::Food, 7);
    simulation.households[0].storage.add(ItemKind::Stone, 11);
    let position = (simulation.entities[0].x, simulation.entities[0].y);
    simulation.step(&mut plain_grid(12, 4));
    assert_eq!(simulation.entities[0].household_id, None);
    assert_eq!(simulation.entities[1].household_id, Some(1));
    assert_eq!(
        (simulation.entities[0].x, simulation.entities[0].y),
        position
    );
    assert_eq!(simulation.entities[0].inventory.amount(ItemKind::Food), 7);
    assert_eq!(simulation.households[0].storage.amount(ItemKind::Stone), 11);
    assert_eq!(
        (
            simulation.households[0].residence_x,
            simulation.households[0].residence_y
        ),
        (1, 1)
    );
    assert!(simulation.households[0].is_active());
}

#[test]
fn departing_caregivers_dependents_follow_existing_membership_sync() {
    let mut simulation = simulation(-900);
    let mut child = entity(3, 1, 1, 0.0);
    child.age_ticks = 8 * TICKS_PER_YEAR;
    child.caregiver_id = Some(1);
    child.household_id = Some(1);
    simulation.entities.push(child);
    simulation.next_entity_id = 4;
    simulation.step(&mut plain_grid(12, 4));
    assert_eq!(simulation.entities[0].household_id, None);
    assert_eq!(simulation.entities[2].household_id, None);
    assert_eq!(simulation.entities[2].caregiver_id, Some(1));
}

#[test]
fn conflict_cooldown_is_pair_specific_and_expires_after_a_day() {
    let mut simulation = simulation(-300);
    prepare(&mut simulation);
    simulation.entities[0].mind.memory.mark_conflict(2, 1);
    assert!(best_household_conflict_candidate(
        &simulation.entities[0],
        &simulation.population_cache,
        2
    )
    .is_none());
    assert!(best_household_conflict_candidate(
        &simulation.entities[0],
        &simulation.population_cache,
        1 + TICKS_PER_DAY
    )
    .is_some());
}

#[test]
fn household_conflicts_match_normal_and_profiled_paths() {
    let mut normal = simulation(-300);
    let mut profiled = normal.clone();
    let mut autonomy_profiled = normal.clone();
    normal.step(&mut plain_grid(12, 4));
    profiled.profile_step(&mut plain_grid(12, 4));
    autonomy_profiled.profile_autonomy_step(&mut plain_grid(12, 4));
    let state = |simulation: &Simulation| {
        simulation
            .entities
            .iter()
            .map(|entity| {
                (
                    entity.id,
                    entity.household_id,
                    entity.mind.current_goal,
                    entity
                        .mind
                        .memory
                        .affinity_to(if entity.id == 1 { 2 } else { 1 }),
                    entity
                        .mind
                        .memory
                        .conflict_on_cooldown(if entity.id == 1 { 2 } else { 1 }, simulation.tick),
                )
            })
            .collect::<Vec<_>>()
    };
    assert_eq!(state(&normal), state(&profiled));
    assert_eq!(state(&normal), state(&autonomy_profiled));
}
