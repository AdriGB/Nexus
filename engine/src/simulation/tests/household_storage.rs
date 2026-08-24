use super::super::autonomy::KnownEntity;
use super::super::households::{Household, DEFAULT_HOUSEHOLD_STORAGE_CAPACITY};
use super::super::{Inventory, ItemKind, Simulation};
use super::support::entity;

fn household() -> Household {
    Household {
        id: 1,
        formed_tick: 10,
        dissolved_tick: None,
        inheritance: None,
        residence_x: 2,
        residence_y: 3,
        storage: Inventory::new(DEFAULT_HOUSEHOLD_STORAGE_CAPACITY),
    }
}

fn simulation(member_position: (u32, u32)) -> Simulation {
    let mut member = entity(1, member_position.0, member_position.1, 0.0);
    member.household_id = Some(1);
    Simulation {
        entities: vec![member],
        next_entity_id: 2,
        households: vec![household()],
        next_household_id: 2,
        ..Simulation::default()
    }
}

#[test]
fn new_household_storage_starts_empty() {
    let household = household();
    assert_eq!(household.storage.used_capacity(), 0);
}

#[test]
fn household_storage_has_bounded_capacity() {
    let household = household();
    assert_eq!(
        household.storage.capacity(),
        DEFAULT_HOUSEHOLD_STORAGE_CAPACITY
    );
    assert_eq!(household.storage.remaining_capacity(), 200);
}

#[test]
fn member_can_deposit_at_residence() {
    let mut simulation = simulation((2, 3));
    simulation.entities[0].inventory.add(ItemKind::Food, 12);
    assert_eq!(simulation.deposit_to_household(1, ItemKind::Food, 7), 7);
    assert_eq!(simulation.entities[0].inventory.amount(ItemKind::Food), 5);
    assert_eq!(simulation.households[0].storage.amount(ItemKind::Food), 7);
}

#[test]
fn member_cannot_deposit_away_from_residence() {
    let mut simulation = simulation((1, 3));
    simulation.entities[0].inventory.add(ItemKind::Food, 12);
    let before = simulation.clone();
    assert_eq!(simulation.deposit_to_household(1, ItemKind::Food, 7), 0);
    assert_eq!(
        simulation.entities[0].inventory,
        before.entities[0].inventory
    );
    assert_eq!(simulation.households, before.households);
}

#[test]
fn non_member_cannot_access_storage() {
    let mut simulation = simulation((2, 3));
    simulation.entities[0].household_id = None;
    simulation.entities[0].inventory.add(ItemKind::Food, 10);
    assert_eq!(simulation.deposit_to_household(1, ItemKind::Food, 5), 0);
    assert_eq!(simulation.withdraw_from_household(1, ItemKind::Food, 5), 0);
}

#[test]
fn deposit_respects_storage_capacity() {
    let mut simulation = simulation((2, 3));
    simulation.entities[0].inventory.add(ItemKind::Food, 20);
    simulation.households[0].storage.add(ItemKind::Stone, 195);
    assert_eq!(simulation.deposit_to_household(1, ItemKind::Food, 50), 5);
    assert_eq!(simulation.entities[0].inventory.amount(ItemKind::Food), 15);
    assert_eq!(simulation.households[0].storage.used_capacity(), 200);
}

#[test]
fn deposit_respects_source_amount() {
    let mut simulation = simulation((2, 3));
    simulation.entities[0].inventory.add(ItemKind::Food, 4);
    assert_eq!(simulation.deposit_to_household(1, ItemKind::Food, 20), 4);
    assert_eq!(simulation.entities[0].inventory.amount(ItemKind::Food), 0);
    assert_eq!(simulation.households[0].storage.amount(ItemKind::Food), 4);
}

#[test]
fn member_can_withdraw_at_residence() {
    let mut simulation = simulation((2, 3));
    simulation.households[0].storage.add(ItemKind::Timber, 12);
    assert_eq!(
        simulation.withdraw_from_household(1, ItemKind::Timber, 7),
        7
    );
    assert_eq!(simulation.entities[0].inventory.amount(ItemKind::Timber), 7);
    assert_eq!(simulation.households[0].storage.amount(ItemKind::Timber), 5);
}

#[test]
fn withdraw_respects_personal_capacity() {
    let mut simulation = simulation((2, 3));
    simulation.entities[0].inventory.add(ItemKind::Stone, 48);
    simulation.households[0].storage.add(ItemKind::Food, 10);
    assert_eq!(simulation.withdraw_from_household(1, ItemKind::Food, 20), 2);
    assert_eq!(simulation.entities[0].inventory.used_capacity(), 50);
    assert_eq!(simulation.households[0].storage.amount(ItemKind::Food), 8);
}

#[test]
fn withdraw_respects_available_storage() {
    let mut simulation = simulation((2, 3));
    simulation.households[0].storage.add(ItemKind::Food, 3);
    assert_eq!(simulation.withdraw_from_household(1, ItemKind::Food, 20), 3);
    assert_eq!(simulation.entities[0].inventory.amount(ItemKind::Food), 3);
    assert_eq!(simulation.households[0].storage.amount(ItemKind::Food), 0);
}

#[test]
fn failed_transfer_does_not_mutate_state() {
    let mut simulation = simulation((2, 3));
    simulation.entities[0].inventory.add(ItemKind::Food, 8);
    let inventory = simulation.entities[0].inventory.clone();
    let position = (simulation.entities[0].x, simulation.entities[0].y);
    let households = simulation.households.clone();
    assert_eq!(simulation.deposit_to_household(99, ItemKind::Food, 8), 0);
    assert_eq!(simulation.deposit_to_household(1, ItemKind::Food, 0), 0);
    assert_eq!(simulation.entities[0].inventory, inventory);
    assert_eq!(
        (simulation.entities[0].x, simulation.entities[0].y),
        position
    );
    assert_eq!(simulation.households, households);
}

#[test]
fn household_storage_survives_member_movement() {
    let mut simulation = simulation((2, 3));
    simulation.households[0].storage.add(ItemKind::Food, 30);
    simulation.entities[0].x = 9;
    assert_eq!(simulation.households[0].storage.amount(ItemKind::Food), 30);
}

#[test]
fn household_storage_survives_member_death() {
    let mut simulation = simulation((2, 3));
    simulation.households[0].storage.add(ItemKind::Food, 30);
    simulation.entities[0].health = 0.0;
    simulation.remove_dead_entities();
    assert_eq!(simulation.households[0].storage.amount(ItemKind::Food), 30);
}

#[test]
fn household_storage_survives_partnership_dissolution() {
    let mut simulation = simulation((2, 3));
    let mut partner = entity(2, 2, 3, 0.0);
    partner.household_id = Some(1);
    partner.partner_id = Some(1);
    simulation.entities[0].partner_id = Some(2);
    simulation.entities.push(partner);
    simulation.entities.sort_by_key(|entity| entity.id);
    for (member, other_id) in simulation.entities.iter_mut().zip([2, 1]) {
        member.mind.memory.known_entities.push(KnownEntity {
            id: other_id,
            first_seen_tick: 0,
            last_seen_tick: 0,
            last_seen_x: 2,
            last_seen_y: 3,
            observed_ticks: 1,
            affinity: 0,
            last_interaction_tick: 0,
            interaction_count: 1,
            seek_retry_after_tick: None,
        });
    }
    simulation.households[0].storage.add(ItemKind::Food, 14);
    super::super::partnerships::try_dissolve(&mut simulation.entities, 1, 2).unwrap();
    assert_eq!(simulation.households[0].storage.amount(ItemKind::Food), 14);
}

#[test]
fn identical_transfer_sequences_are_deterministic() {
    let run = || {
        let mut simulation = simulation((2, 3));
        simulation.entities[0].inventory.add(ItemKind::Food, 30);
        simulation.deposit_to_household(1, ItemKind::Food, 20);
        simulation.withdraw_from_household(1, ItemKind::Food, 6);
        simulation
    };
    let first = run();
    let second = run();
    assert_eq!(first.entities[0].inventory, second.entities[0].inventory);
    assert_eq!(first.households, second.households);
}

#[test]
fn household_bridge_exposes_storage() {
    let mut simulation = simulation((2, 3));
    simulation.households[0].storage.add(ItemKind::Food, 30);
    simulation.households[0].storage.add(ItemKind::Timber, 5);
    let payload: serde_json::Value =
        serde_json::from_str(&crate::bridge::entity_household_json(&simulation, 1)).unwrap();
    assert_eq!(payload["storage"]["capacity"], 200);
    assert_eq!(payload["storage"]["used_capacity"], 35);
    assert_eq!(
        payload["storage"]["items"],
        serde_json::json!([
            {"kind": "Food", "amount": 30},
            {"kind": "Timber", "amount": 5}
        ])
    );
}
