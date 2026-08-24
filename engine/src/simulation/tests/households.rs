use super::super::households::{form_for_partnership, members_of, Household};
use super::super::partnerships;
use super::super::Simulation;
use super::support::entity;

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
            formed_tick: 42
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
            formed_tick: 0
        }]
    );
}
