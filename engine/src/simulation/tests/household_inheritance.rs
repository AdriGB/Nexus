use super::super::genealogy::Genealogy;
use super::super::households::{
    settle_basic_inheritances, Household, HouseholdDissolution, HouseholdInheritance,
};
use super::super::time::TICKS_PER_YEAR;
use super::super::{DeathContext, Inventory, ItemKind, Simulation};
use super::support::{entity, plain_grid};

fn household(id: u32, active: bool, capacity: u16) -> Household {
    Household {
        id,
        formed_tick: 0,
        dissolved_tick: (!active).then_some(7),
        inheritance: None,
        residence_x: id,
        residence_y: id,
        storage: Inventory::new(capacity),
    }
}

fn settle(
    entities: &mut [super::super::Entity],
    households: &mut [Household],
    genealogy: &Genealogy,
    deaths: &[DeathContext],
) {
    settle_basic_inheritances(
        entities,
        households,
        genealogy,
        deaths,
        &[HouseholdDissolution {
            household_id: 1,
            dissolved_tick: 7,
        }],
        7,
    );
}

#[test]
fn partner_is_first_basic_heir_and_active_household_is_preferred() {
    let mut partner = entity(2, 0, 0, 0.0);
    partner.household_id = Some(2);
    let mut child = entity(3, 0, 0, 0.0);
    child.household_id = Some(2);
    let mut entities = vec![partner, child];
    let mut genealogy = Genealogy::default();
    genealogy.register(1, None, None);
    genealogy.register(2, None, None);
    genealogy.register(3, Some(1), None);
    let mut households = vec![household(1, false, 200), household(2, true, 200)];
    households[0].storage.add(ItemKind::Food, 30);

    settle(
        &mut entities,
        &mut households,
        &genealogy,
        &[DeathContext {
            entity_id: 1,
            household_id: Some(1),
            partner_id: Some(2),
        }],
    );

    assert_eq!(households[0].inheritance.unwrap().heir_id, Some(2));
    assert_eq!(
        households[0].inheritance.unwrap().destination_household_id,
        Some(2)
    );
    assert_eq!(households[1].storage.amount(ItemKind::Food), 30);
    assert_eq!(entities[0].household_id, Some(2));
    assert_eq!(households[0].dissolved_tick, Some(7));
}

#[test]
fn nearest_descendant_generation_and_lower_id_break_ties() {
    let entities = vec![
        entity(3, 0, 0, 0.0),
        entity(4, 0, 0, 0.0),
        entity(5, 0, 0, 0.0),
    ];
    let mut genealogy = Genealogy::default();
    genealogy.register(1, None, None);
    genealogy.register(2, Some(1), None);
    genealogy.register(3, Some(2), None);
    genealogy.register(4, Some(1), None);
    genealogy.register(5, Some(1), None);
    let mut entities = entities;
    let mut households = vec![household(1, false, 200)];
    settle(
        &mut entities,
        &mut households,
        &genealogy,
        &[DeathContext {
            entity_id: 1,
            household_id: Some(1),
            partner_id: None,
        }],
    );
    assert_eq!(households[0].inheritance.unwrap().heir_id, Some(4));
}

#[test]
fn parent_then_sibling_are_basic_fallbacks() {
    let mut genealogy = Genealogy::default();
    genealogy.register(1, Some(2), None);
    genealogy.register(2, None, None);
    genealogy.register(3, Some(2), None);
    let death = [DeathContext {
        entity_id: 1,
        household_id: Some(1),
        partner_id: None,
    }];

    let mut entities = vec![entity(2, 0, 0, 0.0), entity(3, 0, 0, 0.0)];
    let mut households = vec![household(1, false, 200)];
    settle(&mut entities, &mut households, &genealogy, &death);
    assert_eq!(households[0].inheritance.unwrap().heir_id, Some(2));

    let mut entities = vec![entity(3, 0, 0, 0.0)];
    let mut households = vec![household(1, false, 200)];
    settle(&mut entities, &mut households, &genealogy, &death);
    assert_eq!(households[0].inheritance.unwrap().heir_id, Some(3));
}

#[test]
fn heir_without_household_receives_all_kinds_with_bounded_capacity() {
    let mut heir = entity(2, 0, 0, 0.0);
    heir.inventory = Inventory::new(25);
    let mut entities = vec![heir];
    let mut genealogy = Genealogy::default();
    genealogy.register(1, None, None);
    genealogy.register(2, Some(1), None);
    let mut households = vec![household(1, false, 200)];
    for (kind, amount) in [
        (ItemKind::Food, 10),
        (ItemKind::Timber, 8),
        (ItemKind::Stone, 12),
        (ItemKind::Iron, 5),
    ] {
        households[0].storage.add(kind, amount);
    }
    let before: Vec<_> = ItemKind::ALL
        .into_iter()
        .map(|kind| households[0].storage.amount(kind))
        .collect();
    settle(
        &mut entities,
        &mut households,
        &genealogy,
        &[DeathContext {
            entity_id: 1,
            household_id: Some(1),
            partner_id: None,
        }],
    );

    assert_eq!(entities[0].inventory.used_capacity(), 25);
    for (index, kind) in ItemKind::ALL.into_iter().enumerate() {
        assert_eq!(
            households[0].storage.amount(kind) + entities[0].inventory.amount(kind),
            before[index]
        );
    }
    assert_eq!(households[0].storage.used_capacity(), 10);
    assert_eq!(
        households[0].inheritance.unwrap().destination_household_id,
        None
    );
}

#[test]
fn no_eligible_relative_records_no_heir_and_preserves_estate() {
    let mut households = vec![household(1, false, 200)];
    households[0].storage.add(ItemKind::Stone, 20);
    settle(
        &mut [],
        &mut households,
        &Genealogy::default(),
        &[DeathContext {
            entity_id: 9,
            household_id: Some(1),
            partner_id: None,
        }],
    );
    assert_eq!(
        households[0].inheritance,
        Some(HouseholdInheritance {
            resolved_tick: 7,
            decedent_id: 9,
            heir_id: None,
            destination_household_id: None
        })
    );
    assert_eq!(households[0].storage.amount(ItemKind::Stone), 20);
}

#[test]
fn abandoned_household_does_not_trigger_inheritance() {
    let mut households = vec![household(1, false, 200)];
    households[0].storage.add(ItemKind::Food, 20);
    settle(&mut [], &mut households, &Genealogy::default(), &[]);
    assert_eq!(households[0].inheritance, None);
    assert_eq!(households[0].storage.amount(ItemKind::Food), 20);
}

#[test]
fn inheritance_is_processed_once_and_record_is_not_overwritten() {
    let mut entities = vec![entity(2, 0, 0, 0.0), entity(3, 0, 0, 0.0)];
    let mut genealogy = Genealogy::default();
    genealogy.register(1, None, None);
    genealogy.register(2, Some(1), None);
    genealogy.register(3, Some(1), None);
    let mut households = vec![household(1, false, 200)];
    households[0].storage.add(ItemKind::Food, 10);
    let first = [DeathContext {
        entity_id: 1,
        household_id: Some(1),
        partner_id: None,
    }];
    settle(&mut entities, &mut households, &genealogy, &first);
    let record = households[0].inheritance;
    entities.retain(|entity| entity.id == 3);
    settle(&mut entities, &mut households, &genealogy, &first);
    assert_eq!(households[0].inheritance, record);
}

#[test]
fn same_tick_deaths_select_deterministically_by_class_then_decedent() {
    let mut entities = vec![entity(3, 0, 0, 0.0), entity(4, 0, 0, 0.0)];
    let genealogy = Genealogy::default();
    let deaths = [
        DeathContext {
            entity_id: 2,
            household_id: Some(1),
            partner_id: Some(4),
        },
        DeathContext {
            entity_id: 1,
            household_id: Some(1),
            partner_id: Some(3),
        },
    ];
    let mut households = vec![household(1, false, 200)];
    settle(&mut entities, &mut households, &genealogy, &deaths);
    assert_eq!(households[0].inheritance.unwrap().decedent_id, 1);
    assert_eq!(households[0].inheritance.unwrap().heir_id, Some(3));
}

fn partner_lifecycle_simulation() -> Simulation {
    let mut decedent = entity(1, 0, 0, 0.0);
    decedent.age_ticks = 25 * TICKS_PER_YEAR;
    decedent.health = 0.0;
    decedent.household_id = Some(1);
    decedent.partner_id = Some(2);
    let mut partner = entity(2, 0, 0, 0.0);
    partner.age_ticks = 25 * TICKS_PER_YEAR;
    partner.household_id = Some(2);
    partner.partner_id = Some(1);
    let mut source = household(1, true, 200);
    source.storage.add(ItemKind::Food, 12);
    source.storage.add(ItemKind::Iron, 4);
    Simulation {
        entities: vec![decedent, partner],
        next_entity_id: 3,
        households: vec![source, household(2, true, 200)],
        next_household_id: 3,
        ..Simulation::default()
    }
}

#[test]
fn partner_inheritance_runs_through_normal_lifecycle() {
    let mut simulation = partner_lifecycle_simulation();
    simulation.step(&mut plain_grid(1, 1));
    assert_eq!(simulation.entities.len(), 1);
    assert_eq!(simulation.entities[0].household_id, Some(2));
    assert_eq!(simulation.households[0].dissolved_tick, Some(1));
    assert_eq!(
        simulation.households[0].inheritance.unwrap().heir_id,
        Some(2)
    );
    assert_eq!(simulation.households[1].storage.amount(ItemKind::Food), 12);
    assert_eq!(simulation.households[1].storage.amount(ItemKind::Iron), 4);
}

#[test]
fn orphan_inheritance_follows_reassignment_destination_end_to_end() {
    let mut parent = entity(1, 0, 0, 0.0);
    parent.age_ticks = 25 * TICKS_PER_YEAR;
    parent.health = 0.0;
    parent.household_id = Some(1);
    let mut replacement = entity(2, 0, 0, 0.0);
    replacement.age_ticks = 25 * TICKS_PER_YEAR;
    replacement.household_id = Some(2);
    let mut child = entity(3, 0, 0, 0.0);
    child.age_ticks = 8 * TICKS_PER_YEAR;
    child.caregiver_id = Some(1);
    child.household_id = Some(1);
    let mut source = household(1, true, 200);
    source.storage.add(ItemKind::Food, 10);
    source.storage.add(ItemKind::Stone, 6);
    let mut simulation = Simulation {
        entities: vec![parent, replacement, child],
        next_entity_id: 4,
        households: vec![source, household(2, true, 200)],
        next_household_id: 3,
        ..Simulation::default()
    };
    simulation.genealogy.register(1, None, None);
    simulation.genealogy.register(2, None, None);
    simulation.genealogy.register(3, Some(1), None);

    simulation.step(&mut plain_grid(1, 1));

    let child = simulation
        .entities
        .iter()
        .find(|entity| entity.id == 3)
        .unwrap();
    assert_eq!(child.caregiver_id, Some(2));
    assert_eq!(child.household_id, Some(2));
    assert_eq!(
        simulation.households[0].inheritance.unwrap().heir_id,
        Some(3)
    );
    assert_eq!(
        simulation.households[0]
            .inheritance
            .unwrap()
            .destination_household_id,
        Some(2)
    );
    assert_eq!(simulation.households[1].storage.amount(ItemKind::Food), 10);
    assert_eq!(simulation.households[1].storage.amount(ItemKind::Stone), 6);
}

#[test]
fn normal_and_profiled_inheritance_paths_match() {
    let mut normal = partner_lifecycle_simulation();
    let mut profiled = normal.clone();
    let mut autonomy_profiled = normal.clone();
    normal.step(&mut plain_grid(1, 1));
    profiled.profile_step(&mut plain_grid(1, 1));
    autonomy_profiled.profile_autonomy_step(&mut plain_grid(1, 1));
    let state = |simulation: &Simulation| {
        simulation
            .households
            .iter()
            .map(|household| (household.inheritance, household.storage.clone()))
            .collect::<Vec<_>>()
    };
    assert_eq!(state(&normal), state(&profiled));
    assert_eq!(state(&normal), state(&autonomy_profiled));
}
