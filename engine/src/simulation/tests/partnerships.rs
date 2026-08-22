use super::super::autonomy::{
    AffinityChangeRecord, FoodShareAttempt, KnownEntity, SocialInteraction,
};
use super::super::time::TICKS_PER_YEAR;
use super::super::{Simulation, SimulationEventCause, SimulationEventDetails, SimulationEventKind};
use super::support::{entity, grid_from_rows};

fn relationship(id: u32, affinity: i16, interaction_count: u32) -> KnownEntity {
    KnownEntity {
        id,
        first_seen_tick: 1,
        last_seen_tick: 10,
        last_seen_x: 0,
        last_seen_y: 0,
        observed_ticks: 10,
        affinity,
        last_interaction_tick: 9,
        interaction_count,
        seek_retry_after_tick: None,
    }
}

fn interaction() -> SocialInteraction {
    SocialInteraction {
        actor_id: 1,
        target_id: 2,
        location: (0, 0),
        actor_location: (0, 0),
        target_location: (0, 0),
        actor_affinity_delta: 0,
        target_affinity_delta: 0,
        actor_affinity_change: None::<AffinityChangeRecord>,
        target_affinity_change: None::<AffinityChangeRecord>,
    }
}

fn eligible_simulation(affinity: i16, interaction_count: u32) -> Simulation {
    let mut first = entity(1, 0, 0, 0.0);
    let mut second = entity(2, 0, 0, 0.0);
    for entity in [&mut first, &mut second] {
        entity.age_ticks = 25 * TICKS_PER_YEAR;
    }
    second.personality = first.personality;
    first
        .mind
        .memory
        .known_entities
        .push(relationship(2, affinity, interaction_count));
    second
        .mind
        .memory
        .known_entities
        .push(relationship(1, affinity, interaction_count));
    Simulation {
        tick: 10,
        entities: vec![first, second],
        next_entity_id: 3,
        ..Simulation::default()
    }
}

fn partnered_simulation(actor_affinity: i16, target_affinity: i16) -> Simulation {
    let mut simulation = eligible_simulation(actor_affinity, 3);
    simulation.entities[1].mind.memory.known_entities[0].affinity = target_affinity;
    simulation.entities[0].partner_id = Some(2);
    simulation.entities[1].partner_id = Some(1);
    simulation
}

#[test]
fn unilateral_non_positive_affinity_dissolves_and_reports_bilateral_evidence() {
    let mut simulation = partnered_simulation(0, 250);

    let dissolution = super::super::partnerships::try_dissolve(&mut simulation.entities, 1, 2)
        .expect("partnership should dissolve");

    assert_eq!(dissolution.actor_id, 1);
    assert_eq!(dissolution.target_id, 2);
    assert_eq!(dissolution.actor_affinity, 0);
    assert_eq!(dissolution.target_affinity, 250);
    assert!(simulation
        .entities
        .iter()
        .all(|entity| entity.partner_id.is_none()));
}

#[test]
fn bilateral_positive_affinity_keeps_the_partnership() {
    let mut simulation = partnered_simulation(1, 250);

    assert!(super::super::partnerships::try_dissolve(&mut simulation.entities, 1, 2).is_none());
    assert_eq!(simulation.entities[0].partner_id, Some(2));
    assert_eq!(simulation.entities[1].partner_id, Some(1));
}

#[test]
fn daily_decay_to_zero_dissolves_without_an_affinity_change_event() {
    let mut simulation = partnered_simulation(1, 250);
    simulation.tick = super::super::autonomy::RELATIONSHIP_DECAY_START_TICKS;
    simulation.entities[0].mind.memory.known_entities[0].last_interaction_tick = 0;
    simulation.entities[1].mind.memory.known_entities[0].last_interaction_tick = 0;

    simulation.run_daily_relationship_decay();

    assert!(simulation
        .entities
        .iter()
        .all(|entity| entity.partner_id.is_none()));
    assert!(simulation
        .recent_events()
        .all(|event| event.kind != SimulationEventKind::AffinityChange));
    let dissolution = simulation
        .recent_events()
        .find(|event| event.kind == SimulationEventKind::PartnershipDissolved)
        .expect("decay dissolution event");
    assert_eq!(dissolution.cause, SimulationEventCause::RelationshipDecay);
    assert_eq!(dissolution.caused_by_event_id, None);
    assert_eq!(
        dissolution.details,
        SimulationEventDetails::PartnershipDissolved {
            actor_affinity: 0,
            target_affinity: 249,
        }
    );
}

#[test]
fn unrelated_affinity_change_does_not_dissolve_an_existing_pair() {
    let mut simulation = partnered_simulation(250, 250);
    let mut third = entity(3, 0, 0, 0.0);
    third.age_ticks = 25 * TICKS_PER_YEAR;
    simulation.entities.push(third);

    assert!(super::super::partnerships::try_dissolve(&mut simulation.entities, 1, 3).is_none());
    assert_eq!(simulation.entities[0].partner_id, Some(2));
    assert_eq!(simulation.entities[1].partner_id, Some(1));
}

#[test]
fn social_interaction_dissolves_the_pair_before_attempting_formation() {
    let mut simulation = partnered_simulation(0, 250);

    simulation.record_social_interactions(vec![interaction()]);

    assert!(simulation
        .entities
        .iter()
        .all(|entity| entity.partner_id.is_none()));
    assert!(simulation
        .recent_events()
        .all(|event| event.kind != SimulationEventKind::PartnershipFormed));
    let events: Vec<_> = simulation.recent_events().collect();
    let interaction_event = events
        .iter()
        .find(|event| event.kind == SimulationEventKind::Interaction)
        .expect("interaction event");
    let dissolution = events
        .iter()
        .find(|event| event.kind == SimulationEventKind::PartnershipDissolved)
        .expect("dissolution event");
    assert_eq!(dissolution.cause, SimulationEventCause::MutualSocialContact);
    assert_eq!(dissolution.caused_by_event_id, Some(interaction_event.id));
    assert_eq!(
        dissolution.details,
        SimulationEventDetails::PartnershipDissolved {
            actor_affinity: 0,
            target_affinity: 250,
        }
    );
}

#[test]
fn refusal_resentment_dissolves_from_the_receivers_perspective() {
    let mut simulation = partnered_simulation(250, 10);
    simulation.entities[0].personality.cooperativeness = 0.0;

    simulation.process_food_share_attempts(vec![FoodShareAttempt {
        actor_id: 1,
        target_id: 2,
        actor_location: (0, 0),
        amount: 10,
    }]);

    assert_eq!(simulation.entities[1].mind.memory.affinity_to(1), Some(-5));
    assert!(simulation
        .entities
        .iter()
        .all(|entity| entity.partner_id.is_none()));
    let events: Vec<_> = simulation.recent_events().collect();
    let refusal = events
        .iter()
        .find(|event| event.kind == SimulationEventKind::FoodShareRefused)
        .expect("food refusal event");
    let dissolution = events
        .iter()
        .find(|event| event.kind == SimulationEventKind::PartnershipDissolved)
        .expect("dissolution event");
    assert_eq!(dissolution.cause, SimulationEventCause::FoodShareRefused);
    assert_eq!(dissolution.caused_by_event_id, Some(refusal.id));
    assert_eq!(
        dissolution.details,
        SimulationEventDetails::PartnershipDissolved {
            actor_affinity: -5,
            target_affinity: 250,
        }
    );
}

#[test]
fn third_positive_interaction_forms_a_symmetric_causal_partnership() {
    // SocialInteraction values are emitted after the social processor has
    // already persisted this third encounter in both memories.
    let mut simulation = eligible_simulation(210, 3);

    simulation.record_social_interactions(vec![interaction()]);

    assert_eq!(simulation.entities[0].partner_id, Some(2));
    assert_eq!(simulation.entities[1].partner_id, Some(1));
    let events: Vec<_> = simulation.recent_events().collect();
    let interaction_event = events
        .iter()
        .find(|event| event.kind == SimulationEventKind::Interaction)
        .expect("interaction event");
    let partnership_event = events
        .iter()
        .find(|event| event.kind == SimulationEventKind::PartnershipFormed)
        .expect("partnership event");
    assert_eq!(
        partnership_event.caused_by_event_id,
        Some(interaction_event.id)
    );
    assert_eq!(
        partnership_event.cause,
        SimulationEventCause::MutualCommitment
    );
    assert_eq!(
        partnership_event.details,
        SimulationEventDetails::PartnershipFormed {
            actor_affinity: 210,
            target_affinity: 210,
            compatibility_per_mille: 1_000,
        }
    );
}

#[test]
fn partnership_requires_bilateral_affinity_and_familiarity() {
    let mut low_affinity = eligible_simulation(199, 3);
    low_affinity.record_social_interactions(vec![interaction()]);
    assert!(low_affinity
        .entities
        .iter()
        .all(|entity| entity.partner_id.is_none()));

    let mut unfamiliar = eligible_simulation(300, 2);
    unfamiliar.record_social_interactions(vec![interaction()]);
    assert!(unfamiliar
        .entities
        .iter()
        .all(|entity| entity.partner_id.is_none()));
}

#[test]
fn death_clears_the_surviving_partners_reference_without_separation_event() {
    let mut world = grid_from_rows(&["P"]);
    let mut simulation = eligible_simulation(210, 2);
    simulation.entities[0].partner_id = Some(2);
    simulation.entities[1].partner_id = Some(1);
    simulation.entities[1].health = 0.0;

    simulation.step(&mut world);

    assert_eq!(simulation.entities.len(), 1);
    assert_eq!(simulation.entities[0].partner_id, None);
    assert!(simulation
        .recent_events()
        .all(|event| event.kind != SimulationEventKind::PartnershipDissolved));
}

#[test]
fn identical_breakups_produce_the_same_event_sequence() {
    let mut first = partnered_simulation(0, 250);
    let mut second = partnered_simulation(0, 250);

    first.record_social_interactions(vec![interaction()]);
    second.record_social_interactions(vec![interaction()]);

    assert_eq!(
        first.recent_events().cloned().collect::<Vec<_>>(),
        second.recent_events().cloned().collect::<Vec<_>>()
    );
}
