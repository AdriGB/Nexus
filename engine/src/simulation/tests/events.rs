use super::super::autonomy::{AffinityChangeRecord, KnownEntity, SocialInteraction};
use super::super::entity::{Personality, Pregnancy};
use super::super::events::RecentEventHistory;
use super::super::time::TICKS_PER_YEAR;
use super::super::{
    EventLocation, Simulation, SimulationEventCause, SimulationEventDetails, SimulationEventKind,
};
use super::support::{entity, plain_grid};

fn interaction(
    actor_id: u32,
    target_id: u32,
    location: (u32, u32),
    actor_affinity_delta: i16,
    target_affinity_delta: i16,
) -> SocialInteraction {
    SocialInteraction {
        actor_id,
        target_id,
        location,
        actor_location: location,
        target_location: location,
        actor_affinity_delta,
        target_affinity_delta,
        actor_affinity_change: None,
        target_affinity_change: None,
    }
}

fn known_entity(id: u32, affinity: i16, x: u32, y: u32) -> KnownEntity {
    KnownEntity {
        id,
        first_seen_tick: 0,
        last_seen_tick: 0,
        last_seen_x: x,
        last_seen_y: y,
        observed_ticks: 1,
        affinity,
        last_interaction_tick: 0,
        interaction_count: 1,
        seek_retry_after_tick: None,
    }
}

#[test]
fn successful_social_interaction_records_one_complete_event() {
    let mut world = plain_grid(8, 8);
    let mut simulation = Simulation::default();
    simulation.entities = vec![entity(1, 2, 3, 0.0), entity(2, 3, 3, 0.0)];
    for entity in &mut simulation.entities {
        entity.age_ticks = 25 * TICKS_PER_YEAR;
    }
    simulation.next_entity_id = 3;

    simulation.step(&mut world);

    let events: Vec<_> = simulation
        .recent_events()
        .filter(|event| event.kind == SimulationEventKind::Interaction)
        .collect();
    assert_eq!(events.len(), 1);
    let event = events[0];
    assert_eq!(event.id, 2);
    assert_eq!(event.tick, 1);
    assert_eq!(event.location, EventLocation { x: 2, y: 3 });
    assert_eq!(event.actor_id, 1);
    assert_eq!(event.target_id, Some(2));
    assert_eq!(event.related_entity_ids, vec![1, 2]);
    assert_eq!(event.kind, SimulationEventKind::Interaction);
    assert_eq!(event.cause, SimulationEventCause::MutualSocialContact);
    let actor_delta = simulation.entities[0]
        .mind
        .memory
        .known_entities
        .iter()
        .find(|known| known.id == 2)
        .unwrap()
        .affinity;
    let target_delta = simulation.entities[1]
        .mind
        .memory
        .known_entities
        .iter()
        .find(|known| known.id == 1)
        .unwrap()
        .affinity;
    assert_eq!(
        event.details,
        SimulationEventDetails::Interaction {
            actor_affinity_delta: actor_delta,
            target_affinity_delta: target_delta,
        }
    );
}

#[test]
fn affinity_changes_follow_interaction_in_directed_order() {
    let mut simulation = Simulation::default();
    simulation.record_social_interactions(vec![SocialInteraction {
        actor_id: 1,
        target_id: 2,
        location: (4, 5),
        actor_location: (4, 5),
        target_location: (5, 5),
        actor_affinity_delta: 4,
        target_affinity_delta: -4,
        actor_affinity_change: Some(AffinityChangeRecord {
            target_id: 2,
            previous_affinity: 99,
            new_affinity: 103,
            delta: 4,
        }),
        target_affinity_change: Some(AffinityChangeRecord {
            target_id: 1,
            previous_affinity: -200,
            new_affinity: -204,
            delta: -4,
        }),
    }]);

    let events: Vec<_> = simulation.recent_events().collect();
    assert_eq!(events.len(), 3);
    assert_eq!(events[0].kind, SimulationEventKind::Interaction);
    assert_eq!((events[1].actor_id, events[1].target_id), (1, Some(2)));
    assert_eq!((events[2].actor_id, events[2].target_id), (2, Some(1)));
    assert_eq!(events[1].id, 2);
    assert_eq!(events[2].id, 3);
    assert_eq!(events[1].cause, SimulationEventCause::MutualSocialContact);
}

#[test]
fn interaction_affinity_changes_use_each_relationship_owners_location() {
    let mut actor = entity(1, 2, 2, 0.0);
    let mut target = entity(2, 2, 3, 0.0);
    for entity in [&mut actor, &mut target] {
        entity.age_ticks = 25 * TICKS_PER_YEAR;
        entity.personality = Personality {
            curiosity: 0.5,
            sociability: 0.5,
            cooperativeness: 0.5,
            caution: 0.5,
            persistence: 0.5,
        };
    }
    actor
        .mind
        .memory
        .known_entities
        .push(known_entity(2, 99, 2, 3));
    target
        .mind
        .memory
        .known_entities
        .push(known_entity(1, 99, 2, 2));
    let mut simulation = Simulation {
        entities: vec![actor, target],
        next_entity_id: 3,
        ..Simulation::default()
    };
    let mut world = plain_grid(8, 8);

    simulation.step(&mut world);

    let interaction_event = simulation
        .recent_events()
        .find(|event| event.kind == SimulationEventKind::Interaction)
        .expect("social interaction should be recorded");
    assert_eq!(interaction_event.location, EventLocation { x: 2, y: 2 });

    let affinity_events: Vec<_> = simulation
        .recent_events()
        .filter(|event| event.kind == SimulationEventKind::AffinityChange)
        .collect();
    assert_eq!(affinity_events.len(), 2);
    assert_eq!(
        (
            affinity_events[0].actor_id,
            affinity_events[0].target_id,
            affinity_events[0].location,
        ),
        (1, Some(2), EventLocation { x: 2, y: 2 })
    );
    assert_eq!(
        (
            affinity_events[1].actor_id,
            affinity_events[1].target_id,
            affinity_events[1].location,
        ),
        (2, Some(1), EventLocation { x: 2, y: 3 })
    );
}

#[test]
fn daily_decay_emits_a_real_affinity_change_event() {
    let mut actor = entity(1, 6, 7, 0.0);
    actor
        .mind
        .memory
        .known_entities
        .push(known_entity(2, 100, 8, 7));
    let mut simulation = Simulation {
        tick: super::super::autonomy::RELATIONSHIP_DECAY_START_TICKS,
        entities: vec![actor],
        next_entity_id: 3,
        ..Simulation::default()
    };

    simulation.run_daily_relationship_decay();

    assert_eq!(simulation.entities[0].mind.memory.affinity_to(2), Some(99));
    let events: Vec<_> = simulation.recent_events().collect();
    assert_eq!(events.len(), 1);
    assert_eq!(events[0].location, EventLocation { x: 6, y: 7 });
    assert_eq!(events[0].actor_id, 1);
    assert_eq!(events[0].target_id, Some(2));
    assert_eq!(events[0].cause, SimulationEventCause::RelationshipDecay);
    assert_eq!(
        events[0].details,
        SimulationEventDetails::AffinityChange {
            previous_affinity: 100,
            new_affinity: 99,
            delta: -1,
        }
    );
}

#[test]
fn affinity_change_events_are_deterministic_across_identical_steps() {
    let scenario = || {
        let mut a = entity(1, 2, 2, 0.0);
        let mut b = entity(2, 2, 3, 0.0);
        for entity in [&mut a, &mut b] {
            entity.age_ticks = 25 * TICKS_PER_YEAR;
            entity.personality = Personality {
                curiosity: 0.5,
                sociability: 0.5,
                cooperativeness: 0.5,
                caution: 0.5,
                persistence: 0.5,
            };
        }
        a.mind.memory.known_entities.push(known_entity(2, 99, 2, 3));
        b.mind.memory.known_entities.push(known_entity(1, 99, 2, 2));
        Simulation {
            entities: vec![a, b],
            next_entity_id: 3,
            ..Simulation::default()
        }
    };
    let mut first = scenario();
    let mut second = scenario();
    let mut first_world = plain_grid(8, 8);
    let mut second_world = plain_grid(8, 8);

    first.step(&mut first_world);
    second.step(&mut second_world);

    let first_events: Vec<_> = first.recent_events().cloned().collect();
    let second_events: Vec<_> = second.recent_events().cloned().collect();
    assert!(first_events
        .iter()
        .any(|event| event.kind == SimulationEventKind::AffinityChange));
    assert_eq!(first_events, second_events);
}

#[test]
fn mutual_first_sight_records_one_canonical_encounter() {
    let mut world = plain_grid(8, 8);
    let mut simulation = Simulation::default();
    simulation.entities = vec![entity(1, 2, 3, 0.0), entity(2, 3, 3, 0.0)];
    for entity in &mut simulation.entities {
        entity.age_ticks = 25 * TICKS_PER_YEAR;
    }
    simulation.next_entity_id = 3;

    simulation.step(&mut world);

    let encounters: Vec<_> = simulation
        .recent_events()
        .filter(|event| event.kind == SimulationEventKind::Encounter)
        .collect();
    assert_eq!(encounters.len(), 1);
    let encounter = encounters[0];
    assert_eq!(encounter.id, 1);
    assert_eq!(encounter.tick, 1);
    assert_eq!(encounter.location, EventLocation { x: 2, y: 3 });
    assert_eq!(encounter.actor_id, 1);
    assert_eq!(encounter.target_id, Some(2));
    assert_eq!(encounter.related_entity_ids, vec![1, 2]);
    assert_eq!(encounter.cause, SimulationEventCause::FirstEncounter);
    assert_eq!(encounter.details, SimulationEventDetails::Encounter);
}

#[test]
fn delayed_reciprocal_awareness_does_not_duplicate_encounter() {
    let mut world = plain_grid(8, 8);
    let mut adult = entity(1, 2, 3, 0.0);
    adult.age_ticks = 25 * TICKS_PER_YEAR;
    let infant = entity(2, 3, 3, 0.0);
    let mut simulation = Simulation::default();
    simulation.entities = vec![adult, infant];
    simulation.next_entity_id = 3;

    simulation.step(&mut world);
    simulation.entities[1].age_ticks = 25 * TICKS_PER_YEAR;
    simulation.step(&mut world);

    assert_eq!(
        simulation
            .recent_events()
            .filter(|event| event.kind == SimulationEventKind::Encounter)
            .count(),
        1
    );
}

#[test]
fn rejected_social_interactions_record_no_event() {
    let mut world = plain_grid(16, 8);
    let mut simulation = Simulation::default();
    simulation.entities = vec![entity(1, 1, 1, 0.0), entity(2, 12, 1, 0.0)];
    for entity in &mut simulation.entities {
        entity.age_ticks = 25 * TICKS_PER_YEAR;
    }
    simulation.next_entity_id = 3;

    simulation.step(&mut world);

    assert_eq!(simulation.recent_events().count(), 0);
}

#[test]
fn event_ids_and_order_are_monotonic() {
    let mut simulation = Simulation::default();
    simulation.tick = 9;
    simulation.record_social_interactions(vec![
        interaction(1, 2, (1, 1), 4, 3),
        interaction(1, 3, (1, 1), -2, 1),
    ]);

    let events: Vec<_> = simulation.recent_events().collect();
    assert_eq!(
        events.iter().map(|event| event.id).collect::<Vec<_>>(),
        vec![1, 2]
    );
    assert_eq!(
        events
            .iter()
            .map(|event| event.target_id)
            .collect::<Vec<_>>(),
        vec![Some(2), Some(3)]
    );
}

#[test]
fn event_history_evicts_the_oldest_event_at_capacity() {
    let mut simulation = Simulation {
        recent_events: RecentEventHistory::with_capacity(2),
        ..Simulation::default()
    };
    simulation.record_social_interactions(vec![interaction(1, 2, (0, 0), 1, 1)]);
    simulation.record_social_interactions(vec![interaction(1, 3, (0, 0), 2, 2)]);
    simulation.record_social_interactions(vec![interaction(1, 4, (0, 0), 3, 3)]);

    let events: Vec<_> = simulation.recent_events().collect();
    assert_eq!(events.len(), 2);
    assert_eq!(events[0].id, 2);
    assert_eq!(events[1].id, 3);
}

#[test]
fn same_seed_and_steps_produce_the_same_event_sequence() {
    let mut world_a = plain_grid(8, 8);
    let mut world_b = plain_grid(8, 8);
    let mut simulation_a = Simulation::default();
    simulation_a.entities = vec![entity(1, 2, 3, 0.0), entity(2, 3, 3, 0.0)];
    for entity in &mut simulation_a.entities {
        entity.age_ticks = 25 * TICKS_PER_YEAR;
    }
    simulation_a.next_entity_id = 3;
    let mut simulation_b = Simulation::default();
    simulation_b.entities = vec![entity(1, 2, 3, 0.0), entity(2, 3, 3, 0.0)];
    for entity in &mut simulation_b.entities {
        entity.age_ticks = 25 * TICKS_PER_YEAR;
    }
    simulation_b.next_entity_id = 3;

    for _ in 0..100 {
        simulation_a.step(&mut world_a);
        simulation_b.step(&mut world_b);
    }

    assert_eq!(
        simulation_a.recent_events().collect::<Vec<_>>(),
        simulation_b.recent_events().collect::<Vec<_>>()
    );
}

#[test]
fn successful_birth_records_mother_and_newborn() {
    let world = plain_grid(8, 8);
    let mut simulation = Simulation::default();
    let mut mother = entity(1, 3, 3, 0.0);
    mother.pregnancy = Some(Pregnancy {
        father_id: 2,
        conceived_tick: 0,
        due_tick: 10,
    });
    simulation.entities = vec![mother];
    simulation.next_entity_id = 2;
    simulation.tick = 10;

    simulation.update_pregnancies(&world);

    let events: Vec<_> = simulation.recent_events().collect();
    assert_eq!(events.len(), 1);
    let event = events[0];
    assert_eq!(event.kind, SimulationEventKind::Birth);
    assert_eq!(event.cause, SimulationEventCause::Born);
    assert_eq!(event.actor_id, 1);
    assert_eq!(event.target_id, None);
    assert_eq!(event.related_entity_ids, vec![1, 2]);
    assert_eq!(event.details, SimulationEventDetails::Birth { child_id: 2 });
    assert_eq!(event.location.x, simulation.entities[1].x);
    assert_eq!(event.location.y, simulation.entities[1].y);
}

#[test]
fn deaths_record_their_immediate_cause_before_removal() {
    let mut simulation = Simulation::default();
    let mut starved = entity(1, 2, 3, 0.0);
    starved.health = 0.0;
    let mut aged = entity(2, 4, 5, 0.0);
    aged.age_ticks = aged.lifespan_ticks;
    aged.health = 0.0;
    simulation.entities = vec![starved, aged];
    simulation.tick = 20;

    simulation.remove_dead_entities();

    assert!(simulation.entities.is_empty());
    let events: Vec<_> = simulation.recent_events().collect();
    assert_eq!(events.len(), 2);
    assert_eq!(events[0].actor_id, 1);
    assert_eq!(events[0].cause, SimulationEventCause::Starvation);
    assert_eq!(events[0].location, EventLocation { x: 2, y: 3 });
    assert_eq!(events[1].actor_id, 2);
    assert_eq!(events[1].cause, SimulationEventCause::NaturalDeath);
    assert_eq!(events[1].location, EventLocation { x: 4, y: 5 });
    assert!(events.iter().all(|event| {
        event.kind == SimulationEventKind::Death
            && event.details == SimulationEventDetails::Death
            && event.target_id.is_none()
    }));
}

#[test]
fn consumption_events_skip_zero_and_preserve_consumer_order() {
    let mut simulation = Simulation::default();
    simulation.entities = vec![entity(1, 2, 3, 0.0), entity(2, 4, 5, 0.0)];
    simulation.record_food_consumptions(&[(1, 3), (2, 0), (2, 7)]);

    let events: Vec<_> = simulation.recent_events().collect();
    assert_eq!(events.len(), 2);
    assert_eq!(events[0].id, 1);
    assert_eq!(events[0].actor_id, 1);
    assert_eq!(
        events[0].details,
        SimulationEventDetails::Consumption { amount: 3 }
    );
    assert_eq!(events[1].id, 2);
    assert_eq!(events[1].actor_id, 2);
    assert_eq!(
        events[1].details,
        SimulationEventDetails::Consumption { amount: 7 }
    );
}
