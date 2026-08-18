use serde::Serialize;

use super::to_json;
use crate::simulation::{
    EntityEventSummary, Simulation, SimulationEvent, SimulationEventCause, SimulationEventDetails,
    SimulationEventKind,
};
use crate::world::ResourceKind;

#[derive(Serialize)]
struct EventLocationDto {
    x: u32,
    y: u32,
}

#[derive(Serialize)]
struct SimulationEventDto {
    id: String,
    tick: String,
    relative_time: String,
    location: EventLocationDto,
    actor_id: u32,
    target_id: Option<u32>,
    related_entity_ids: Vec<u32>,
    kind: &'static str,
    cause: &'static str,
    actor_affinity_delta: Option<i16>,
    target_affinity_delta: Option<i16>,
    child_id: Option<u32>,
    amount: Option<u16>,
    resource_kind: Option<&'static str>,
    previous_affinity: Option<i16>,
    new_affinity: Option<i16>,
    delta: Option<i16>,
}

#[derive(Serialize)]
struct EntityEventSummaryDto {
    entity_id: u32,
    total_events: u32,
    first_event_tick: Option<String>,
    latest_event_tick: Option<String>,
    births: u32,
    deaths: u32,
    consumptions: u32,
    discoveries: u32,
    encounters: u32,
    interactions: u32,
    affinity_changes: u32,
}

impl From<EntityEventSummary> for EntityEventSummaryDto {
    fn from(summary: EntityEventSummary) -> Self {
        Self {
            entity_id: summary.entity_id,
            total_events: summary.total_events,
            first_event_tick: summary.first_event_tick.map(|tick| tick.to_string()),
            latest_event_tick: summary.latest_event_tick.map(|tick| tick.to_string()),
            births: summary.births,
            deaths: summary.deaths,
            consumptions: summary.consumptions,
            discoveries: summary.discoveries,
            encounters: summary.encounters,
            interactions: summary.interactions,
            affinity_changes: summary.affinity_changes,
        }
    }
}

fn relative_event_time(current_tick: u64, event_tick: u64) -> String {
    let elapsed = current_tick.saturating_sub(event_tick);
    if elapsed == 0 {
        "just now".to_string()
    } else if elapsed < 24 {
        format!("{elapsed}h ago")
    } else {
        let days = elapsed / 24;
        if days < 365 {
            format!("{days}d ago")
        } else {
            format!("{}y ago", days / 365)
        }
    }
}

pub(super) fn simulation_events_json<'a>(
    events: impl DoubleEndedIterator<Item = &'a SimulationEvent>,
    current_tick: u64,
    entity_id: Option<u32>,
) -> String {
    let events: Vec<SimulationEventDto> = events
        .rev()
        .filter(|event| {
            entity_id.is_none_or(|id| {
                event.actor_id == id
                    || event.target_id == Some(id)
                    || event.related_entity_ids.contains(&id)
            })
        })
        .map(|event| {
            let kind = match event.kind {
                SimulationEventKind::Interaction => "interaction",
                SimulationEventKind::Birth => "birth",
                SimulationEventKind::Death => "death",
                SimulationEventKind::Consumption => "consumption",
                SimulationEventKind::Discovery => "discovery",
                SimulationEventKind::Encounter => "encounter",
                SimulationEventKind::AffinityChange => "affinity_change",
            };
            let cause = match event.cause {
                SimulationEventCause::MutualSocialContact => "mutual_social_contact",
                SimulationEventCause::Born => "born",
                SimulationEventCause::Starvation => "starvation",
                SimulationEventCause::NaturalDeath => "natural_death",
                SimulationEventCause::AteFood => "ate_food",
                SimulationEventCause::ResourceFound => "resource_found",
                SimulationEventCause::FirstEncounter => "first_encounter",
                SimulationEventCause::RelationshipDecay => "relationship_decay",
            };
            let (
                actor_affinity_delta,
                target_affinity_delta,
                child_id,
                amount,
                resource_kind,
                previous_affinity,
                new_affinity,
                delta,
            ) = match event.details {
                SimulationEventDetails::Interaction {
                    actor_affinity_delta,
                    target_affinity_delta,
                } => (
                    Some(actor_affinity_delta),
                    Some(target_affinity_delta),
                    None,
                    None,
                    None,
                    None,
                    None,
                    None,
                ),
                SimulationEventDetails::Birth { child_id } => {
                    (None, None, Some(child_id), None, None, None, None, None)
                }
                SimulationEventDetails::Death => (None, None, None, None, None, None, None, None),
                SimulationEventDetails::Consumption { amount } => {
                    (None, None, None, Some(amount), None, None, None, None)
                }
                SimulationEventDetails::ResourceDiscovery { kind, amount } => (
                    None,
                    None,
                    None,
                    Some(amount),
                    Some(match kind {
                        ResourceKind::Food => "food",
                        ResourceKind::Timber => "timber",
                        ResourceKind::Stone => "stone",
                        ResourceKind::Iron => "iron",
                    }),
                    None,
                    None,
                    None,
                ),
                SimulationEventDetails::Encounter => {
                    (None, None, None, None, None, None, None, None)
                }
                SimulationEventDetails::AffinityChange {
                    previous_affinity,
                    new_affinity,
                    delta,
                } => (
                    None,
                    None,
                    None,
                    None,
                    None,
                    Some(previous_affinity),
                    Some(new_affinity),
                    Some(delta),
                ),
            };

            SimulationEventDto {
                id: event.id.to_string(),
                tick: event.tick.to_string(),
                relative_time: relative_event_time(current_tick, event.tick),
                location: EventLocationDto {
                    x: event.location.x,
                    y: event.location.y,
                },
                actor_id: event.actor_id,
                target_id: event.target_id,
                related_entity_ids: event.related_entity_ids.clone(),
                kind,
                cause,
                actor_affinity_delta,
                target_affinity_delta,
                child_id,
                amount,
                resource_kind,
                previous_affinity,
                new_affinity,
                delta,
            }
        })
        .collect();

    to_json(&events)
}

/// Preserves the original interaction-only API for existing consumers.
pub(crate) fn recent_interaction_events_json(
    simulation: &Simulation,
    entity_id: Option<u32>,
) -> String {
    simulation_events_json(
        simulation
            .recent_events()
            .filter(|event| event.kind == SimulationEventKind::Interaction),
        simulation.tick(),
        entity_id,
    )
}

/// Serializes the bounded event history without mutating simulation state.
/// Filtering happens only when this bridge query is requested.
pub(crate) fn recent_events_json(simulation: &Simulation, entity_id: Option<u32>) -> String {
    simulation_events_json(simulation.recent_events(), simulation.tick(), entity_id)
}

pub(crate) fn entity_event_summary_json(simulation: &Simulation, entity_id: u32) -> String {
    to_json(&EntityEventSummaryDto::from(
        simulation.entity_event_summary(entity_id),
    ))
}
