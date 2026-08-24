use super::super::autonomy::KnownEntity;
use super::super::households::{
    assign_newborn, dissolve_empty_households, form_for_partnership, members_of,
    set_member_household, Household,
};
use super::super::time::TICKS_PER_YEAR;
use super::super::{Inventory, ItemKind, Simulation};
use super::support::{entity, plain_grid};

fn household(id: u32, formed_tick: u64, residence: (u32, u32), food: u16) -> Household {
    let mut storage = Inventory::new(200);
    storage.add(ItemKind::Food, food);
    Household {
        id,
        formed_tick,
        dissolved_tick: None,
        residence_x: residence.0,
        residence_y: residence.1,
        storage,
    }
}

fn empty_household() -> Household {
    household(1, 3, (12, 9), 30)
}

fn partnered_entities() -> Vec<super::super::Entity> {
    let mut first = entity(1, 0, 0, 0.0);
    let mut second = entity(2, 0, 0, 0.0);
    first.age_ticks = 25 * TICKS_PER_YEAR;
    second.age_ticks = 25 * TICKS_PER_YEAR;
    first.partner_id = Some(2);
    second.partner_id = Some(1);
    vec![first, second]
}

fn last_member_death_simulation() -> Simulation {
    let mut member = entity(1, 0, 0, 100.0);
    member.age_ticks = 25 * TICKS_PER_YEAR;
    member.health = 1.0;
    member.household_id = Some(1);
    Simulation {
        entities: vec![member],
        next_entity_id: 2,
        households: vec![household(1, 3, (5, 5), 30)],
        next_household_id: 2,
        ..Simulation::default()
    }
}

fn reassignment_simulation() -> Simulation {
    let mut dead_caregiver = entity(1, 0, 0, 0.0);
    dead_caregiver.age_ticks = 25 * TICKS_PER_YEAR;
    dead_caregiver.health = 0.0;
    dead_caregiver.household_id = Some(1);

    let mut replacement = entity(2, 0, 0, 0.0);
    replacement.age_ticks = 25 * TICKS_PER_YEAR;
    replacement.household_id = Some(2);

    let mut child = entity(3, 0, 0, 0.0);
    child.age_ticks = 8 * TICKS_PER_YEAR;
    child.caregiver_id = Some(1);
    child.household_id = Some(1);

    Simulation {
        entities: vec![dead_caregiver, replacement, child],
        next_entity_id: 4,
        households: vec![household(1, 1, (0, 0), 30), household(2, 2, (0, 0), 40)],
        next_household_id: 3,
        ..Simulation::default()
    }
}

fn dissolution_state(simulation: &Simulation) -> Vec<(u32, Option<u64>)> {
    simulation
        .households
        .iter()
        .map(|household| (household.id, household.dissolved_tick))
        .collect()
}

#[test]
fn new_household_starts_active() {
    let mut entities = partnered_entities();
    let mut households = Vec::new();
    let mut next_id = 1;
    form_for_partnership(&mut entities, &mut households, &mut next_id, 1, 2, 7);
    assert!(households[0].is_active());
    assert_eq!(households[0].dissolved_tick, None);
}

#[test]
fn empty_household_dissolves() {
    let mut households = vec![empty_household()];
    let result = dissolve_empty_households(&[], &mut households, 10);
    assert_eq!(result.len(), 1);
    assert_eq!(result[0].household_id, 1);
    assert_eq!(households[0].dissolved_tick, Some(10));
}

#[test]
fn household_with_member_remains_active() {
    let mut member = entity(1, 0, 0, 0.0);
    member.household_id = Some(1);
    let mut households = vec![empty_household()];
    assert!(dissolve_empty_households(&[member], &mut households, 10).is_empty());
    assert!(households[0].is_active());
}

#[test]
fn household_with_one_survivor_remains_active() {
    let mut survivor = entity(2, 0, 0, 0.0);
    survivor.household_id = Some(1);
    let mut households = vec![empty_household()];
    dissolve_empty_households(&[survivor], &mut households, 10);
    assert_eq!(households[0].dissolved_tick, None);
}

#[test]
fn last_member_death_dissolves_household() {
    let mut simulation = last_member_death_simulation();
    simulation.step(&mut plain_grid(1, 1));
    assert!(simulation.entities.is_empty());
    assert_eq!(simulation.households[0].dissolved_tick, Some(1));
}

#[test]
fn last_member_transfer_dissolves_old_household() {
    let mut simulation = reassignment_simulation();
    simulation.step(&mut plain_grid(1, 1));
    assert_eq!(simulation.entities[1].household_id, Some(2));
    assert_eq!(simulation.households[0].dissolved_tick, Some(1));
    assert_eq!(simulation.households[1].dissolved_tick, None);
}

#[test]
fn dissolution_sets_current_tick() {
    let mut households = vec![empty_household()];
    let result = dissolve_empty_households(&[], &mut households, 77);
    assert_eq!(result[0].dissolved_tick, 77);
    assert_eq!(households[0].dissolved_tick, Some(77));
}

#[test]
fn dissolution_is_idempotent() {
    let mut households = vec![empty_household()];
    assert_eq!(dissolve_empty_households(&[], &mut households, 10).len(), 1);
    assert!(dissolve_empty_households(&[], &mut households, 10).is_empty());
}

#[test]
fn dissolution_does_not_overwrite_original_tick() {
    let mut households = vec![empty_household()];
    dissolve_empty_households(&[], &mut households, 10);
    dissolve_empty_households(&[], &mut households, 20);
    assert_eq!(households[0].dissolved_tick, Some(10));
}

#[test]
fn dissolution_preserves_storage() {
    let mut households = vec![empty_household()];
    dissolve_empty_households(&[], &mut households, 10);
    assert_eq!(households[0].storage.amount(ItemKind::Food), 30);
}

#[test]
fn dissolution_preserves_residence() {
    let mut households = vec![empty_household()];
    dissolve_empty_households(&[], &mut households, 10);
    assert_eq!(
        (households[0].residence_x, households[0].residence_y),
        (12, 9)
    );
}

#[test]
fn dissolution_preserves_formed_tick() {
    let mut households = vec![empty_household()];
    dissolve_empty_households(&[], &mut households, 10);
    assert_eq!(households[0].formed_tick, 3);
}

#[test]
fn dissolution_does_not_remove_household_record() {
    let mut households = vec![empty_household()];
    dissolve_empty_households(&[], &mut households, 10);
    assert_eq!(households.len(), 1);
    assert_eq!(households[0].id, 1);
}

#[test]
fn dissolution_does_not_reuse_household_id() {
    let mut households = vec![empty_household()];
    dissolve_empty_households(&[], &mut households, 10);
    let mut entities = partnered_entities();
    let mut next_id = 2;
    assert_eq!(
        form_for_partnership(&mut entities, &mut households, &mut next_id, 1, 2, 11),
        Some(2)
    );
    assert_eq!(next_id, 3);
    assert_eq!(households[1].id, 2);
}

#[test]
fn cannot_join_dissolved_household() {
    let member = entity(1, 0, 0, 0.0);
    let mut entities = vec![member];
    let mut households = vec![empty_household()];
    households[0].dissolved_tick = Some(5);
    assert!(set_member_household(&mut entities, &households, 1, Some(1)).is_none());
    assert_eq!(entities[0].household_id, None);
}

#[test]
fn dissolved_household_is_not_resurrected() {
    let mut entities = partnered_entities();
    entities[0].household_id = Some(1);
    let mut households = vec![empty_household()];
    households[0].dissolved_tick = Some(5);
    let mut next_id = 2;
    assert_eq!(
        form_for_partnership(&mut entities, &mut households, &mut next_id, 1, 2, 10),
        None
    );
    assert_eq!(entities[1].household_id, None);
    assert_eq!(households[0].dissolved_tick, Some(5));
}

#[test]
fn newborn_cannot_join_dissolved_household() {
    let mut caregiver = entity(1, 0, 0, 0.0);
    caregiver.household_id = Some(1);
    let newborn = entity(2, 0, 0, 0.0);
    let mut entities = vec![caregiver, newborn];
    let mut households = vec![empty_household()];
    households[0].dissolved_tick = Some(5);
    assert_eq!(assign_newborn(&mut entities, &households, 2, 1), None);
    assert_eq!(entities[1].household_id, None);
}

#[test]
fn partnership_dissolution_does_not_dissolve_occupied_household() {
    let mut entities = partnered_entities();
    for (entity, other_id) in entities.iter_mut().zip([2, 1]) {
        entity.household_id = Some(1);
        entity.mind.memory.known_entities.push(KnownEntity {
            id: other_id,
            first_seen_tick: 0,
            last_seen_tick: 0,
            last_seen_x: 0,
            last_seen_y: 0,
            observed_ticks: 1,
            affinity: 0,
            last_interaction_tick: 0,
            interaction_count: 3,
            seek_retry_after_tick: None,
        });
    }
    assert!(super::super::partnerships::try_dissolve(&mut entities, 1, 2).is_some());
    let mut households = vec![empty_household()];
    dissolve_empty_households(&entities, &mut households, 10);
    assert!(entities.iter().all(|entity| entity.partner_id.is_none()));
    assert!(entities.iter().all(|entity| entity.household_id == Some(1)));
    assert_eq!(households[0].dissolved_tick, None);
}

#[test]
fn members_of_dissolved_household_is_empty() {
    let mut households = vec![empty_household()];
    dissolve_empty_households(&[], &mut households, 10);
    assert!(members_of(&[], 1).is_empty());
}

#[test]
fn end_to_end_death_preserves_household_history_and_resources() {
    let mut simulation = last_member_death_simulation();
    let before = simulation.households[0].clone();
    simulation.step(&mut plain_grid(1, 1));
    assert!(simulation.entities.is_empty());
    assert_eq!(simulation.households.len(), 1);
    assert_eq!(simulation.households[0].dissolved_tick, Some(1));
    assert_eq!(simulation.households[0].storage, before.storage);
    assert_eq!(simulation.households[0].formed_tick, before.formed_tick);
    assert_eq!(
        (
            simulation.households[0].residence_x,
            simulation.households[0].residence_y
        ),
        (before.residence_x, before.residence_y)
    );
}

#[test]
fn end_to_end_reassignment_dissolves_old_household_without_moving_storage() {
    let mut simulation = reassignment_simulation();
    let before = simulation.households.clone();
    simulation.step(&mut plain_grid(1, 1));
    let child = simulation
        .entities
        .iter()
        .find(|entity| entity.id == 3)
        .unwrap();
    assert_eq!(child.household_id, Some(2));
    assert_eq!(simulation.households[0].dissolved_tick, Some(1));
    assert_eq!(simulation.households[0].storage, before[0].storage);
    assert_eq!(simulation.households[1].storage, before[1].storage);
}

#[test]
fn identical_simulations_produce_identical_household_dissolution() {
    let mut first = last_member_death_simulation();
    let mut second = first.clone();
    first.step(&mut plain_grid(1, 1));
    second.step(&mut plain_grid(1, 1));
    assert_eq!(dissolution_state(&first), dissolution_state(&second));
    assert_eq!(first.households, second.households);
}

#[test]
fn normal_and_profiled_paths_match() {
    let mut normal = reassignment_simulation();
    let mut profiled = normal.clone();
    normal.step(&mut plain_grid(1, 1));
    profiled.profile_step(&mut plain_grid(1, 1));
    assert_eq!(dissolution_state(&normal), dissolution_state(&profiled));
    assert_eq!(normal.households, profiled.households);
}

#[test]
fn normal_and_profiled_autonomy_paths_match() {
    let mut normal = reassignment_simulation();
    let mut profiled = normal.clone();
    normal.step(&mut plain_grid(1, 1));
    profiled.profile_autonomy_step(&mut plain_grid(1, 1));
    assert_eq!(dissolution_state(&normal), dissolution_state(&profiled));
    assert_eq!(normal.households, profiled.households);
}
