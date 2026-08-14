use super::super::autonomy::SocialInteraction;
use super::super::entity::Pregnancy;
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
        actor_affinity_delta,
        target_affinity_delta,
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

    let events: Vec<_> = simulation.recent_events().collect();
    assert_eq!(events.len(), 1);
    let event = events[0];
    assert_eq!(event.id, 1);
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
