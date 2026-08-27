use super::super::households::{members_of, Household};
use super::super::{Inventory, ItemKind, Simulation};
use super::support::{entity, plain_grid};

// Invariante A05: membresía de hogares y acceso a storage deben coincidir
// con `entity.household_id` y estado del household.
// Esta suite establece el contrato que la futura `HouseholdAggregate` debe preservar.

fn household(id: u32, residence: (u32, u32)) -> Household {
    Household {
        id,
        formed_tick: 0,
        dissolved_tick: None,
        inheritance: None,
        migration: None,
        residence_x: residence.0,
        residence_y: residence.1,
        storage: Inventory::new(200),
    }
}

fn assert_invariants(simulation: &Simulation) {
    // IDs únicos y ordenados para binary_search
    for window in simulation.entities.windows(2) {
        assert!(
            window[0].id < window[1].id,
            "entity IDs must be sorted and unique: {:?}",
            simulation.entities.iter().map(|e| e.id).collect::<Vec<_>>()
        );
    }
    let household_ids: Vec<u32> = simulation.households.iter().map(|h| h.id).collect();
    let mut sorted = household_ids.clone();
    sorted.sort_unstable();
    sorted.dedup();
    assert_eq!(
        household_ids.len(),
        sorted.len(),
        "household IDs must be unique"
    );
    assert_eq!(
        household_ids,
        {
            let mut s = household_ids.clone();
            s.sort_unstable();
            s
        },
        "household IDs must be sorted for binary_search"
    );

    // Membresía: entity.household_id -> household existe y está activo, y viceversa
    for entity in &simulation.entities {
        if let Some(hid) = entity.household_id {
            let hh = simulation
                .households
                .iter()
                .find(|h| h.id == hid)
                .unwrap_or_else(|| {
                    panic!("entity {} points to missing household {}", entity.id, hid)
                });
            assert!(
                hh.is_active(),
                "entity {} points to dissolved household {}",
                entity.id,
                hid
            );
            // members_of debe incluir a la entidad
            let members = members_of(&simulation.entities, hid);
            assert!(
                members.contains(&entity.id),
                "members_of({}) debe contener a entity {}, got {:?}",
                hid,
                entity.id,
                members
            );
        }
    }
    // members_of no debe inventar miembros
    for hh in &simulation.households {
        let members = members_of(&simulation.entities, hh.id);
        for &member_id in &members {
            let entity = simulation
                .entities
                .iter()
                .find(|e| e.id == member_id)
                .unwrap();
            assert_eq!(
                entity.household_id,
                Some(hh.id),
                "members_of returned {} but entity.household_id is {:?}",
                member_id,
                entity.household_id
            );
        }
        // dissolved households must have no members
        if !hh.is_active() {
            assert!(
                members.is_empty(),
                "dissolved household {} still has members {:?}",
                hh.id,
                members
            );
        }
    }

    // Storage access: solo miembros activos en residencia pueden deposit/withdraw
    // (verificado indirectamente: process no debe paniquear y debe preservar capacidad)
    for hh in &simulation.households {
        assert!(hh.storage.used_capacity() <= hh.storage.capacity());
    }
}

#[test]
fn empty_world_preserves_invariants() {
    let simulation = Simulation::default();
    assert_invariants(&simulation);
}

#[test]
fn single_household_invariants_hold_after_creation() {
    let mut simulation = Simulation {
        entities: vec![{
            let mut e = entity(1, 0, 0, 0.0);
            e.household_id = Some(1);
            e
        }],
        next_entity_id: 2,
        households: vec![household(1, (0, 0))],
        next_household_id: 2,
        ..Simulation::default()
    };
    assert_invariants(&simulation);
    simulation.step(&mut plain_grid(1, 1));
    assert_invariants(&simulation);
}

#[test]
fn partnership_formation_preserves_invariants() {
    let mut simulation = Simulation {
        entities: vec![
            {
                let mut e = entity(1, 0, 0, 0.0);
                e.age_ticks = 25 * super::super::time::TICKS_PER_YEAR;
                e
            },
            {
                let mut e = entity(2, 0, 0, 0.0);
                e.age_ticks = 25 * super::super::time::TICKS_PER_YEAR;
                e
            },
        ],
        next_entity_id: 3,
        ..Simulation::default()
    };
    // Forzar partnership vía memoria social
    simulation.entities[0]
        .mind
        .memory
        .known_entities
        .push(super::super::autonomy::KnownEntity {
            id: 2,
            first_seen_tick: 0,
            last_seen_tick: 0,
            last_seen_x: 0,
            last_seen_y: 0,
            observed_ticks: 1,
            affinity: 300,
            last_interaction_tick: 0,
            interaction_count: 3,
            seek_retry_after_tick: None,
        });
    simulation.entities[1]
        .mind
        .memory
        .known_entities
        .push(super::super::autonomy::KnownEntity {
            id: 1,
            first_seen_tick: 0,
            last_seen_tick: 0,
            last_seen_x: 0,
            last_seen_y: 0,
            observed_ticks: 1,
            affinity: 300,
            last_interaction_tick: 0,
            interaction_count: 3,
            seek_retry_after_tick: None,
        });
    for _ in 0..10 {
        simulation.step(&mut plain_grid(4, 4));
        assert_invariants(&simulation);
    }
}

#[test]
fn random_ticks_preserve_invariants() {
    let mut simulation = Simulation::with_population(42, &plain_grid(8, 8), 10);
    for _ in 0..100 {
        simulation.step(&mut plain_grid(8, 8));
        assert_invariants(&simulation);
    }
}

#[test]
fn deposit_withdraw_preserve_invariants() {
    let mut simulation = Simulation {
        entities: vec![{
            let mut e = entity(1, 2, 2, 0.0);
            e.household_id = Some(1);
            e.x = 2;
            e.y = 2;
            e.inventory.add(ItemKind::Food, 30);
            e
        }],
        next_entity_id: 2,
        households: vec![{
            let mut h = household(1, (2, 2));
            h.storage.add(ItemKind::Food, 10);
            h
        }],
        next_household_id: 2,
        ..Simulation::default()
    };
    assert_invariants(&simulation);
    simulation.deposit_to_household(1, ItemKind::Food, 15);
    assert_invariants(&simulation);
    simulation.withdraw_from_household(1, ItemKind::Food, 5);
    assert_invariants(&simulation);
    // Intento con household incorrecto no debe romper
    simulation.entities[0].household_id = None;
    simulation.deposit_to_household(1, ItemKind::Food, 10);
    assert_invariants(&simulation);
}
