use super::super::entity::Pregnancy;
use super::super::households::{form_for_partnership, members_of, Household};
use super::super::partnerships;
use super::super::{Sex, Simulation};
use super::support::{entity, fertile_entity, plain_grid};

fn partnered_entities() -> Vec<super::super::Entity> {
    let mut first = entity(1, 0, 0, 0.0);
    let mut second = entity(2, 0, 0, 0.0);
    first.partner_id = Some(2);
    second.partner_id = Some(1);
    vec![first, second]
}

#[test]
fn partnership_forms_symmetric_household_with_derived_members() {
    let mut entities = partnered_entities();
    let mut households = Vec::new();
    let mut next_id = 1;

    assert_eq!(
        form_for_partnership(&mut entities, &mut households, &mut next_id, 1, 2, 42),
        Some(1)
    );
    assert_eq!(entities[0].household_id, Some(1));
    assert_eq!(entities[1].household_id, Some(1));
    assert_eq!(members_of(&entities, 1), vec![1, 2]);
    assert_eq!(
        households,
        vec![Household {
            id: 1,
            formed_tick: 42,
            residence_x: 0,
            residence_y: 0,
        }]
    );
}

#[test]
fn unpartnered_or_assigned_entities_do_not_form_another_household() {
    let mut unpartnered = vec![entity(1, 0, 0, 0.0), entity(2, 0, 0, 0.0)];
    let mut households = Vec::new();
    let mut next_id = 1;
    assert_eq!(
        form_for_partnership(&mut unpartnered, &mut households, &mut next_id, 1, 2, 0),
        None
    );

    let mut assigned = partnered_entities();
    assigned[0].household_id = Some(9);
    assert_eq!(
        form_for_partnership(&mut assigned, &mut households, &mut next_id, 1, 2, 0),
        None
    );
    assert!(households.is_empty());
    assert_eq!(next_id, 1);
}

#[test]
fn household_ids_are_allocated_deterministically() {
    let build = || {
        let mut entities = partnered_entities();
        entities[0].x = 7;
        entities[0].y = 11;
        entities[1].x = 19;
        entities[1].y = 23;
        let mut households = Vec::new();
        let mut next_id = 1;
        form_for_partnership(&mut entities, &mut households, &mut next_id, 1, 2, 7);
        (entities, households, next_id)
    };

    let (first_entities, first_households, first_next_id) = build();
    let (second_entities, second_households, second_next_id) = build();
    assert_eq!(
        first_entities
            .iter()
            .map(|entity| entity.household_id)
            .collect::<Vec<_>>(),
        second_entities
            .iter()
            .map(|entity| entity.household_id)
            .collect::<Vec<_>>()
    );
    assert_eq!(first_households, second_households);
    assert_eq!(first_next_id, second_next_id);
}

#[test]
fn identical_simulations_produce_identical_residences() {
    let form = || {
        let mut entities = partnered_entities();
        entities[0].x = 7;
        entities[0].y = 11;
        entities[1].x = 19;
        entities[1].y = 23;
        let mut households = Vec::new();
        let mut next_id = 1;
        form_for_partnership(&mut entities, &mut households, &mut next_id, 1, 2, 7);
        households[0]
    };

    assert_eq!(form(), form());
}

#[test]
fn household_residence_uses_lower_id_founder_position() {
    let mut entities = partnered_entities();
    entities[0].x = 12;
    entities[0].y = 8;
    entities[1].x = 13;
    entities[1].y = 8;
    let mut households = Vec::new();
    let mut next_id = 1;

    form_for_partnership(&mut entities, &mut households, &mut next_id, 1, 2, 5);

    assert_eq!(
        (households[0].residence_x, households[0].residence_y),
        (12, 8)
    );
}

#[test]
fn residence_selection_is_independent_of_actor_target_order() {
    let form = |first_id, second_id| {
        let mut entities = partnered_entities();
        entities[0].x = 12;
        entities[0].y = 8;
        entities[1].x = 30;
        entities[1].y = 21;
        let mut households = Vec::new();
        let mut next_id = 1;
        form_for_partnership(
            &mut entities,
            &mut households,
            &mut next_id,
            first_id,
            second_id,
            5,
        );
        households[0]
    };

    assert_eq!(form(1, 2), form(2, 1));
}

#[test]
fn household_residence_persists_when_members_move() {
    let mut entities = partnered_entities();
    entities[0].x = 4;
    entities[0].y = 6;
    let mut households = Vec::new();
    let mut next_id = 1;
    form_for_partnership(&mut entities, &mut households, &mut next_id, 1, 2, 5);

    entities[0].x = 50;
    entities[0].y = 60;
    entities[1].x = 70;
    entities[1].y = 80;

    assert_eq!(
        (households[0].residence_x, households[0].residence_y),
        (4, 6)
    );
}

#[test]
fn partnership_dissolution_does_not_remove_household_membership() {
    let mut entities = partnered_entities();
    for (entity, other_id) in entities.iter_mut().zip([2, 1]) {
        entity.household_id = Some(1);
        entity
            .mind
            .memory
            .known_entities
            .push(super::super::autonomy::KnownEntity {
                id: other_id,
                first_seen_tick: 0,
                last_seen_tick: 0,
                last_seen_x: 0,
                last_seen_y: 0,
                observed_ticks: 1,
                affinity: 0,
                last_interaction_tick: 0,
                interaction_count: 1,
                seek_retry_after_tick: None,
            });
    }

    assert!(partnerships::try_dissolve(&mut entities, 1, 2).is_some());
    assert_eq!(members_of(&entities, 1), vec![1, 2]);
}

#[test]
fn partnership_dissolution_does_not_change_residence() {
    let mut entities = partnered_entities();
    entities[0].x = 3;
    entities[0].y = 7;
    let mut households = Vec::new();
    let mut next_id = 1;
    form_for_partnership(&mut entities, &mut households, &mut next_id, 1, 2, 0);
    for (entity, other_id) in entities.iter_mut().zip([2, 1]) {
        entity
            .mind
            .memory
            .known_entities
            .push(super::super::autonomy::KnownEntity {
                id: other_id,
                first_seen_tick: 0,
                last_seen_tick: 0,
                last_seen_x: 0,
                last_seen_y: 0,
                observed_ticks: 1,
                affinity: 0,
                last_interaction_tick: 0,
                interaction_count: 1,
                seek_retry_after_tick: None,
            });
    }
    let residence = households[0];

    partnerships::try_dissolve(&mut entities, 1, 2).unwrap();

    assert_eq!(households[0], residence);
}

#[test]
fn dead_entities_leave_active_membership_naturally() {
    let mut entities = partnered_entities();
    entities[0].household_id = Some(1);
    entities[1].household_id = Some(1);
    entities[1].health = 0.0;
    let mut simulation = Simulation {
        entities,
        households: vec![Household {
            id: 1,
            formed_tick: 0,
            residence_x: 0,
            residence_y: 0,
        }],
        next_household_id: 2,
        ..Simulation::default()
    };

    simulation.remove_dead_entities();

    assert_eq!(members_of(simulation.entities(), 1), vec![1]);
    assert_eq!(
        simulation.households(),
        &[Household {
            id: 1,
            formed_tick: 0,
            residence_x: 0,
            residence_y: 0,
        }]
    );
}

#[test]
fn member_death_does_not_change_residence() {
    let mut entities = partnered_entities();
    entities[0].x = 5;
    entities[0].y = 9;
    let mut households = Vec::new();
    let mut next_id = 1;
    form_for_partnership(&mut entities, &mut households, &mut next_id, 1, 2, 0);
    entities[0].health = 0.0;
    let residence = households[0];
    let mut simulation = Simulation {
        entities,
        households,
        next_household_id: next_id,
        ..Simulation::default()
    };

    simulation.remove_dead_entities();

    assert_eq!(simulation.households[0], residence);
}

fn due_birth(mother_household: Option<u32>, father_household: Option<u32>) -> Simulation {
    let mut mother = fertile_entity(1, Sex::Female, 1, 1);
    let mut father = fertile_entity(2, Sex::Male, 2, 1);
    mother.household_id = mother_household;
    father.household_id = father_household;
    mother.pregnancy = Some(Pregnancy {
        father_id: 2,
        conceived_tick: 0,
        due_tick: 1,
    });
    Simulation {
        tick: 1,
        entities: vec![mother, father],
        next_entity_id: 3,
        households: mother_household
            .into_iter()
            .chain(father_household)
            .collect::<std::collections::BTreeSet<_>>()
            .into_iter()
            .map(|id| Household {
                id,
                formed_tick: 0,
                residence_x: 1,
                residence_y: 1,
            })
            .collect(),
        next_household_id: 10,
        ..Simulation::default()
    }
}

#[test]
fn newborn_inherits_mothers_household() {
    let mut simulation = due_birth(Some(4), Some(4));

    simulation.update_pregnancies(&plain_grid(4, 4));

    assert_eq!(simulation.entities[2].caregiver_id, Some(1));
    assert_eq!(simulation.entities[2].household_id, Some(4));
    assert_eq!(simulation.entities[0].household_id, Some(4));
}

#[test]
fn newborn_without_maternal_household_remains_unassigned() {
    let mut simulation = due_birth(None, None);

    simulation.update_pregnancies(&plain_grid(4, 4));

    assert_eq!(simulation.entities[2].household_id, None);
}

#[test]
fn fathers_different_household_does_not_override_mothers() {
    let mut simulation = due_birth(Some(4), Some(9));

    simulation.update_pregnancies(&plain_grid(4, 4));

    assert_eq!(simulation.entities[2].father_id, Some(2));
    assert_eq!(simulation.entities[1].household_id, Some(9));
    assert_eq!(simulation.entities[2].household_id, Some(4));
}

#[test]
fn newborn_appears_in_derived_household_members() {
    let mut simulation = due_birth(Some(4), Some(9));

    simulation.update_pregnancies(&plain_grid(4, 4));

    assert_eq!(members_of(simulation.entities(), 4), vec![1, 3]);
    let payload: serde_json::Value =
        serde_json::from_str(&crate::bridge::entity_household_json(&simulation, 3)).unwrap();
    assert_eq!(payload["household_id"], 4);
    assert_eq!(payload["member_ids"], serde_json::json!([1, 3]));
    assert_eq!(payload["residence_x"], 1);
    assert_eq!(payload["residence_y"], 1);
}

#[test]
fn newborn_membership_does_not_change_residence() {
    let mut simulation = due_birth(Some(4), Some(9));
    let residence = simulation.households[0];

    simulation.update_pregnancies(&plain_grid(4, 4));

    assert_eq!(simulation.households[0], residence);
}

#[test]
fn household_bridge_exposes_residence() {
    let mut simulation = due_birth(Some(4), None);
    simulation.update_pregnancies(&plain_grid(4, 4));

    let payload: serde_json::Value =
        serde_json::from_str(&crate::bridge::entity_household_json(&simulation, 3)).unwrap();

    assert_eq!(payload["residence_x"], 1);
    assert_eq!(payload["residence_y"], 1);
}

#[test]
fn birth_does_not_create_new_household() {
    let mut simulation = due_birth(None, Some(9));
    let households_before = simulation.households.clone();
    let next_id_before = simulation.next_household_id;

    simulation.update_pregnancies(&plain_grid(4, 4));

    assert_eq!(simulation.entities[2].household_id, None);
    assert_eq!(simulation.households, households_before);
    assert_eq!(simulation.next_household_id, next_id_before);
}

#[test]
fn newborn_household_assignment_is_deterministic() {
    let mut first = due_birth(Some(4), Some(9));
    let mut second = due_birth(Some(4), Some(9));

    first.update_pregnancies(&plain_grid(4, 4));
    second.update_pregnancies(&plain_grid(4, 4));

    assert_eq!(
        first.entities[2].household_id,
        second.entities[2].household_id
    );
    assert_eq!(
        members_of(first.entities(), 4),
        members_of(second.entities(), 4)
    );
    assert_eq!(first.households, second.households);
}
