use super::super::autonomy::KnownEntity;
use super::super::households::{
    form_for_partnership, members_of, set_member_household, synchronize_dependent_memberships,
    Household,
};
use super::super::time::{CHILD_AGE_END, TICKS_PER_YEAR};
use super::super::{Inventory, ItemKind, Simulation};
use super::support::{entity, plain_grid};

fn household(id: u32, residence: (u32, u32), food: u16) -> Household {
    let mut storage = Inventory::new(200);
    storage.add(ItemKind::Food, food);
    Household {
        id,
        formed_tick: id as u64,
        dissolved_tick: None,
        inheritance: None,
        migration: None,
        residence_x: residence.0,
        residence_y: residence.1,
        storage,
    }
}

fn partnered_entities() -> Vec<super::super::Entity> {
    let mut first = entity(1, 3, 4, 0.0);
    let mut second = entity(2, 8, 9, 0.0);
    first.age_ticks = 25 * TICKS_PER_YEAR;
    second.age_ticks = 25 * TICKS_PER_YEAR;
    first.partner_id = Some(2);
    second.partner_id = Some(1);
    vec![first, second]
}

fn orphan_reassignment_simulation(
    dependent_age: u64,
    replacement_household: Option<u32>,
) -> Simulation {
    let mut dead_caregiver = entity(1, 0, 0, 0.0);
    dead_caregiver.age_ticks = 25 * TICKS_PER_YEAR;
    dead_caregiver.health = 0.0;
    dead_caregiver.household_id = Some(1);

    let mut replacement = entity(2, 0, 0, 0.0);
    replacement.age_ticks = 25 * TICKS_PER_YEAR;
    replacement.household_id = replacement_household;

    let mut dependent = entity(3, 0, 0, 0.0);
    dependent.age_ticks = dependent_age;
    dependent.caregiver_id = Some(1);
    dependent.household_id = Some(1);

    Simulation {
        entities: vec![dead_caregiver, replacement, dependent],
        next_entity_id: 4,
        households: vec![household(1, (0, 0), 30), household(2, (4, 4), 40)],
        next_household_id: 3,
        ..Simulation::default()
    }
}

fn relationship(id: u32) -> KnownEntity {
    KnownEntity {
        id,
        first_seen_tick: 1,
        last_seen_tick: 1,
        last_seen_x: 0,
        last_seen_y: 0,
        observed_ticks: 2,
        affinity: 210,
        last_interaction_tick: 0,
        interaction_count: 2,
        seek_retry_after_tick: None,
    }
}

fn partnership_incorporation_simulation() -> Simulation {
    let mut member = entity(1, 0, 0, 0.0);
    let mut unassigned = entity(2, 0, 0, 0.0);
    for adult in [&mut member, &mut unassigned] {
        adult.age_ticks = 25 * TICKS_PER_YEAR;
    }
    unassigned.personality = member.personality;
    member.household_id = Some(5);
    member.mind.memory.known_entities.push(relationship(2));
    unassigned.mind.memory.known_entities.push(relationship(1));

    let mut child = entity(3, 0, 0, 0.0);
    child.age_ticks = 8 * TICKS_PER_YEAR;
    child.caregiver_id = Some(2);

    Simulation {
        entities: vec![member, unassigned, child],
        next_entity_id: 4,
        households: vec![household(5, (12, 9), 37)],
        next_household_id: 6,
        ..Simulation::default()
    }
}

fn membership_state(simulation: &Simulation) -> Vec<(u32, Option<u32>, Option<u32>, Option<u32>)> {
    simulation
        .entities
        .iter()
        .map(|entity| {
            (
                entity.id,
                entity.household_id,
                entity.caregiver_id,
                entity.partner_id,
            )
        })
        .collect()
}

#[test]
fn partnership_without_households_creates_new_household() {
    let mut entities = partnered_entities();
    let mut households = Vec::new();
    let mut next_id = 1;
    assert_eq!(
        form_for_partnership(&mut entities, &mut households, &mut next_id, 1, 2, 42),
        Some(1)
    );
    assert_eq!(members_of(&entities, 1), vec![1, 2]);
    assert_eq!(households.len(), 1);
}

#[test]
fn unassigned_partner_joins_existing_household() {
    let mut entities = partnered_entities();
    entities[0].household_id = Some(5);
    let mut households = vec![household(5, (12, 9), 37)];
    let mut next_id = 6;
    form_for_partnership(&mut entities, &mut households, &mut next_id, 1, 2, 42);
    assert_eq!(members_of(&entities, 5), vec![1, 2]);
    assert_eq!(households.len(), 1);
    assert_eq!(next_id, 6);
}

#[test]
fn join_is_independent_of_actor_target_order() {
    let join = |first_id, second_id| {
        let mut entities = partnered_entities();
        entities[0].household_id = Some(5);
        let mut households = vec![household(5, (12, 9), 37)];
        let mut next_id = 6;
        form_for_partnership(
            &mut entities,
            &mut households,
            &mut next_id,
            first_id,
            second_id,
            42,
        );
        entities
            .iter()
            .map(|entity| entity.household_id)
            .collect::<Vec<_>>()
    };
    assert_eq!(join(1, 2), join(2, 1));
}

fn joined_household() -> (Vec<super::super::Entity>, Vec<Household>) {
    let mut entities = partnered_entities();
    entities[0].household_id = Some(5);
    let mut households = vec![household(5, (12, 9), 37)];
    let mut next_id = 6;
    form_for_partnership(&mut entities, &mut households, &mut next_id, 1, 2, 42);
    (entities, households)
}

#[test]
fn joining_existing_household_preserves_residence() {
    let (_, households) = joined_household();
    assert_eq!(
        (households[0].residence_x, households[0].residence_y),
        (12, 9)
    );
}

#[test]
fn joining_existing_household_preserves_storage() {
    let (_, households) = joined_household();
    assert_eq!(households[0].storage.amount(ItemKind::Food), 37);
}

#[test]
fn same_household_partnership_creates_no_new_household() {
    let mut entities = partnered_entities();
    entities[0].household_id = Some(5);
    entities[1].household_id = Some(5);
    let mut households = vec![household(5, (12, 9), 37)];
    let before = households.clone();
    let mut next_id = 6;
    form_for_partnership(&mut entities, &mut households, &mut next_id, 1, 2, 42);
    assert_eq!(households, before);
    assert_eq!(next_id, 6);
}

#[test]
fn different_households_do_not_merge_on_partnership() {
    let mut entities = partnered_entities();
    entities[0].household_id = Some(1);
    entities[1].household_id = Some(2);
    let mut households = vec![household(1, (1, 1), 10), household(2, (2, 2), 20)];
    let before = households.clone();
    let mut next_id = 3;
    assert_eq!(
        form_for_partnership(&mut entities, &mut households, &mut next_id, 1, 2, 42),
        None
    );
    assert_eq!(entities[0].household_id, Some(1));
    assert_eq!(entities[1].household_id, Some(2));
    assert_eq!(households, before);
    assert_eq!(next_id, 3);
}

#[test]
fn reassigned_child_joins_new_caregiver_household() {
    let mut simulation = orphan_reassignment_simulation(8 * TICKS_PER_YEAR, Some(2));
    simulation.step(&mut plain_grid(1, 1));
    assert_eq!(simulation.entities.len(), 2);
    assert_eq!(simulation.entities[1].caregiver_id, Some(2));
    assert_eq!(simulation.entities[1].household_id, Some(2));
}

#[test]
fn reassigned_infant_joins_new_caregiver_household() {
    let mut simulation = orphan_reassignment_simulation(0, Some(2));
    simulation.step(&mut plain_grid(1, 1));
    assert_eq!(simulation.entities[1].caregiver_id, Some(2));
    assert_eq!(simulation.entities[1].household_id, Some(2));
}

#[test]
fn dependent_leaves_old_household_when_new_caregiver_has_none() {
    let mut simulation = orphan_reassignment_simulation(8 * TICKS_PER_YEAR, None);
    simulation.step(&mut plain_grid(1, 1));
    assert_eq!(simulation.entities[1].caregiver_id, Some(2));
    assert_eq!(simulation.entities[1].household_id, None);
}

#[test]
fn dependent_without_caregiver_preserves_existing_household() {
    let mut dependent = entity(1, 0, 0, 0.0);
    dependent.age_ticks = 8 * TICKS_PER_YEAR;
    dependent.household_id = Some(1);
    let mut entities = vec![dependent];
    let households = vec![household(1, (0, 0), 10)];
    assert!(synchronize_dependent_memberships(&mut entities, &households).is_empty());
    assert_eq!(entities[0].household_id, Some(1));
}

fn caregiver_with_dependents() -> (Vec<super::super::Entity>, Vec<Household>) {
    let mut caregiver = entity(1, 0, 0, 0.0);
    caregiver.age_ticks = 25 * TICKS_PER_YEAR;
    caregiver.household_id = Some(5);
    let mut child = entity(2, 0, 0, 0.0);
    child.age_ticks = 8 * TICKS_PER_YEAR;
    child.caregiver_id = Some(1);
    let mut infant = entity(3, 0, 0, 0.0);
    infant.caregiver_id = Some(1);
    let entities = vec![caregiver, child, infant];
    let households = vec![household(5, (0, 0), 0)];
    (entities, households)
}

#[test]
fn caregiver_household_change_propagates_to_child() {
    let (mut entities, households) = caregiver_with_dependents();
    let changes = synchronize_dependent_memberships(&mut entities, &households);
    assert_eq!(changes.len(), 2);
    assert_eq!(entities[1].household_id, Some(5));
}

#[test]
fn caregiver_household_change_propagates_to_infant() {
    let (mut entities, households) = caregiver_with_dependents();
    synchronize_dependent_memberships(&mut entities, &households);
    assert_eq!(entities[2].household_id, Some(5));
}

#[test]
fn graduated_child_keeps_household_membership() {
    let mut adult = entity(1, 0, 0, 0.0);
    adult.age_ticks = CHILD_AGE_END;
    adult.caregiver_id = Some(2);
    adult.household_id = Some(5);
    let mut entities = vec![adult];
    super::super::dependents::clear_graduated_caregivers(&mut entities);
    assert_eq!(entities[0].household_id, Some(5));
}

#[test]
fn graduated_child_clears_caregiver_only() {
    let mut adult = entity(1, 0, 0, 0.0);
    adult.age_ticks = CHILD_AGE_END;
    adult.caregiver_id = Some(2);
    adult.household_id = Some(5);
    let mut entities = vec![adult];
    super::super::dependents::clear_graduated_caregivers(&mut entities);
    assert_eq!(entities[0].caregiver_id, None);
    assert_eq!(entities[0].household_id, Some(5));
}

fn transitioned_member() -> (Vec<super::super::Entity>, Vec<Household>, Vec<Household>) {
    let mut member = entity(1, 0, 0, 0.0);
    member.household_id = Some(1);
    member.inventory.add(ItemKind::Food, 7);
    let mut entities = vec![member];
    let households = vec![household(1, (1, 1), 11), household(2, (2, 2), 22)];
    let storages = households.clone();
    let change = set_member_household(&mut entities, &households, 1, Some(2)).unwrap();
    assert_eq!(change.previous_household_id, Some(1));
    assert_eq!(change.new_household_id, Some(2));
    (entities, households, storages)
}

#[test]
fn membership_transition_preserves_personal_inventory() {
    let (entities, _, _) = transitioned_member();
    assert_eq!(entities[0].inventory.amount(ItemKind::Food), 7);
}

#[test]
fn membership_transition_preserves_old_household_storage() {
    let (_, households, before) = transitioned_member();
    assert_eq!(households[0], before[0]);
}

#[test]
fn membership_transition_preserves_new_household_storage() {
    let (_, households, before) = transitioned_member();
    assert_eq!(households[1], before[1]);
}

#[test]
fn invalid_target_household_does_not_change_membership() {
    let mut member = entity(1, 0, 0, 0.0);
    member.household_id = Some(1);
    let mut entities = vec![member];
    let households = vec![household(1, (1, 1), 0)];
    assert!(set_member_household(&mut entities, &households, 1, Some(99)).is_none());
    assert_eq!(entities[0].household_id, Some(1));
}

#[test]
fn last_member_leaving_does_not_delete_household() {
    let mut member = entity(1, 0, 0, 0.0);
    member.household_id = Some(1);
    let mut entities = vec![member];
    let households = vec![household(1, (1, 1), 11), household(2, (2, 2), 22)];
    set_member_household(&mut entities, &households, 1, Some(2));
    assert!(members_of(&entities, 1).is_empty());
    assert_eq!(households.len(), 2);
}

#[test]
fn members_of_reflects_transition_without_extra_bookkeeping() {
    let mut entities = vec![entity(1, 0, 0, 0.0), entity(2, 0, 0, 0.0)];
    entities[0].household_id = Some(1);
    entities[1].household_id = Some(2);
    let households = vec![household(1, (1, 1), 0), household(2, (2, 2), 0)];
    set_member_household(&mut entities, &households, 2, Some(1));
    assert_eq!(members_of(&entities, 1), vec![1, 2]);
    assert!(members_of(&entities, 2).is_empty());
}

#[test]
fn partnership_incorporation_runs_end_to_end_and_propagates_to_child() {
    let mut simulation = partnership_incorporation_simulation();
    let household_before = simulation.households[0].clone();
    simulation.step(&mut plain_grid(1, 1));
    assert_eq!(simulation.entities[0].partner_id, Some(2));
    assert_eq!(simulation.entities[1].partner_id, Some(1));
    assert_eq!(simulation.entities[0].household_id, Some(5));
    assert_eq!(simulation.entities[1].household_id, Some(5));
    assert_eq!(simulation.entities[2].household_id, Some(5));
    assert_eq!(simulation.households, vec![household_before]);
}

#[test]
fn dependent_reassignment_runs_end_to_end_without_moving_resources() {
    let mut simulation = orphan_reassignment_simulation(8 * TICKS_PER_YEAR, Some(2));
    simulation.entities[2].inventory.add(ItemKind::Food, 7);
    let households_before = simulation.households.clone();
    simulation.step(&mut plain_grid(1, 1));
    assert!(simulation.entities.iter().all(|entity| entity.id != 1));
    let child = simulation
        .entities
        .iter()
        .find(|entity| entity.id == 3)
        .unwrap();
    assert_eq!(child.caregiver_id, Some(2));
    assert_eq!(child.household_id, Some(2));
    assert_eq!(child.inventory.amount(ItemKind::Food), 7);
    assert_eq!(simulation.households[0].dissolved_tick, Some(1));
    assert_eq!(simulation.households[1].dissolved_tick, None);
    for (after, before) in simulation.households.iter().zip(households_before) {
        assert_eq!(after.storage, before.storage);
        assert_eq!(after.formed_tick, before.formed_tick);
        assert_eq!(
            (after.residence_x, after.residence_y),
            (before.residence_x, before.residence_y)
        );
    }
}

#[test]
fn identical_simulations_produce_identical_membership_transitions() {
    let mut first = orphan_reassignment_simulation(0, Some(2));
    let mut second = first.clone();
    first.step(&mut plain_grid(1, 1));
    second.step(&mut plain_grid(1, 1));
    assert_eq!(membership_state(&first), membership_state(&second));
    assert_eq!(first.households, second.households);
}

#[test]
fn normal_and_profiled_paths_match() {
    let mut normal = orphan_reassignment_simulation(0, Some(2));
    let mut profiled = normal.clone();
    normal.step(&mut plain_grid(1, 1));
    profiled.profile_step(&mut plain_grid(1, 1));
    assert_eq!(membership_state(&normal), membership_state(&profiled));
    assert_eq!(normal.households, profiled.households);
}
