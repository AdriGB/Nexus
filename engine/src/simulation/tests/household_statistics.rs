use super::super::households::{household_stats, Household, HouseholdInheritance};
use super::super::time::TICKS_PER_YEAR;
use super::super::{Inventory, ItemKind, Simulation};
use super::support::entity;

fn household(id: u32, formed_tick: u64, dissolved_tick: Option<u64>, capacity: u16) -> Household {
    Household {
        id,
        formed_tick,
        dissolved_tick,
        inheritance: None,
        migration: None,
        residence_x: 0,
        residence_y: 0,
        storage: Inventory::new(capacity),
    }
}

#[test]
fn empty_world_has_zero_household_stats() {
    assert_eq!(Simulation::default().household_stats(), Default::default());
}

#[test]
fn active_dissolved_housed_and_unhoused_counts_obey_invariants() {
    let mut housed = entity(1, 0, 0, 0.0);
    housed.household_id = Some(1);
    let mut missing = entity(2, 0, 0, 0.0);
    missing.household_id = Some(99);
    let mut dissolved = entity(3, 0, 0, 0.0);
    dissolved.household_id = Some(2);
    let stats = household_stats(
        &[housed, missing, dissolved, entity(4, 0, 0, 0.0)],
        &[household(1, 0, None, 200), household(2, 0, Some(5), 200)],
        10,
    );
    assert_eq!(
        (
            stats.total_households,
            stats.active_households,
            stats.dissolved_households
        ),
        (2, 1, 1)
    );
    assert_eq!(
        stats.total_households,
        stats.active_households + stats.dissolved_households
    );
    assert_eq!((stats.housed_entities, stats.unhoused_entities), (1, 3));
}

#[test]
fn active_household_size_and_dependents_are_derived_in_one_snapshot() {
    let mut adult = entity(1, 0, 0, 0.0);
    adult.age_ticks = 25 * TICKS_PER_YEAR;
    adult.household_id = Some(1);
    let mut child = entity(2, 0, 0, 0.0);
    child.age_ticks = 8 * TICKS_PER_YEAR;
    child.household_id = Some(1);
    let mut single = entity(3, 0, 0, 0.0);
    single.age_ticks = 25 * TICKS_PER_YEAR;
    single.household_id = Some(2);
    let stats = household_stats(
        &[adult, child, single],
        &[household(1, 0, None, 200), household(2, 0, None, 200)],
        0,
    );
    assert_eq!(stats.average_active_household_size, 1.5);
    assert_eq!(stats.largest_active_household_size, 2);
    assert_eq!(stats.single_member_households, 1);
    assert_eq!(stats.households_with_dependents, 1);
}

#[test]
fn zero_active_households_have_safe_zero_averages_and_utilization() {
    let stats = household_stats(&[], &[household(1, 0, Some(3), 0)], 10);
    assert_eq!(stats.average_active_household_size, 0.0);
    assert_eq!(stats.active_storage_utilization, 0.0);
    assert_eq!(stats.average_active_household_age_ticks, 0.0);
}

#[test]
fn active_storage_aggregates_all_kinds_and_excludes_dissolved_estates() {
    let mut active = household(1, 0, None, 100);
    let mut second = household(2, 0, None, 50);
    let mut dissolved = household(3, 0, Some(4), 200);
    for (kind, amount) in [
        (ItemKind::Food, 10),
        (ItemKind::Timber, 8),
        (ItemKind::Stone, 6),
        (ItemKind::Iron, 4),
    ] {
        active.storage.add(kind, amount);
    }
    second.storage.add(ItemKind::Food, 2);
    dissolved.storage.add(ItemKind::Food, 100);
    let stats = household_stats(&[], &[active, second, dissolved], 10);
    assert_eq!(
        (stats.active_storage_capacity, stats.active_storage_used),
        (150, 30)
    );
    assert_eq!(stats.active_storage_utilization, 0.2);
    assert_eq!(
        (
            stats.active_food_stored,
            stats.active_timber_stored,
            stats.active_stone_stored,
            stats.active_iron_stored
        ),
        (12, 8, 6, 4)
    );
}

#[test]
fn inheritance_history_counts_settled_no_heir_but_not_abandonment() {
    let mut inherited = household(1, 0, Some(4), 200);
    inherited.inheritance = Some(HouseholdInheritance {
        resolved_tick: 4,
        decedent_id: 1,
        heir_id: Some(2),
        destination_household_id: None,
    });
    let mut no_heir = household(2, 0, Some(5), 200);
    no_heir.inheritance = Some(HouseholdInheritance {
        resolved_tick: 5,
        decedent_id: 3,
        heir_id: None,
        destination_household_id: None,
    });
    let abandoned = household(3, 0, Some(6), 200);
    let stats = household_stats(&[], &[inherited, no_heir, abandoned], 10);
    assert_eq!(stats.settled_inheritances, 2);
    assert_eq!(stats.inheritances_without_heir, 1);
}

#[test]
fn household_ages_and_dissolved_lifetimes_use_their_authoritative_ticks() {
    let stats = household_stats(
        &[],
        &[
            household(1, 10, None, 0),
            household(2, 20, None, 0),
            household(3, 5, Some(25), 0),
            household(4, 30, Some(20), 0),
        ],
        50,
    );
    assert_eq!(stats.average_active_household_age_ticks, 35.0);
    assert_eq!(stats.average_dissolved_household_lifetime_ticks, 10.0);
}

#[test]
fn household_stats_are_read_only_and_deterministic() {
    let mut member = entity(1, 0, 0, 0.0);
    member.household_id = Some(1);
    let entities = vec![member];
    let households = vec![household(1, 2, None, 200)];
    let original_households = households.clone();
    let first = household_stats(&entities, &households, 9);
    let second = household_stats(&entities, &households, 9);
    assert_eq!(first, second);
    assert_eq!(households, original_households);
    assert_eq!(entities[0].household_id, Some(1));
}
