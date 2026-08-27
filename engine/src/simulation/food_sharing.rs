//! Food-sharing domain rules — willingness and consequence constants.
//!
//! Extracted from `Simulation::process_food_share_attempts` (A03) to keep the
//! decision rule in one domain module while `Simulation` remains the composition
//! root that translates outcomes into `transfer_item`, affinity changes, events
//! and partnership dissolution.
//!
//! Invariante: toda decisión usa solo información conocida por el actor
//! (cooperativeness, afinidad recordada, rol de parentesco) y no inspección
//! global.

use super::autonomy::{CloseRelationshipRole, FoodShareAttempt, RelationshipIdentity};
use super::entity::Entity;
use super::events::{EventLocation, PendingSimulationEvent};
use super::inventory::ItemKind;
use super::{Simulation, SimulationEventCause, SimulationEventDetails, SimulationEventKind};

pub(crate) const GRATITUDE_DELTA: i16 = 20;
pub(crate) const RESENTMENT_DELTA: i16 = -15;

/// Resultado explícito de un intento (contrato A03). `moved` es la cantidad
/// realmente transferida (0 si `willing` fue false o sin stock/capacidad).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct FoodShareOutcome {
    pub attempt: FoodShareAttempt,
    pub willing: bool,
    pub moved: u16,
}

/// Pure willingness rule: feeds-own-dependent is always willing; otherwise
/// cooperativeness (0.0..1.0), affinity (-1000..1000) and close-relationship
/// role determine the threshold.
pub(crate) fn is_willing(
    actor: &Entity,
    target: &Entity,
    target_is_dependent_of_actor: bool,
) -> bool {
    if target_is_dependent_of_actor {
        return true;
    }
    let affinity = actor.mind.memory.affinity_to(target.id).unwrap_or(0);
    let role = crate::simulation::autonomy::close_relationship_role_between(
        RelationshipIdentity::from_entity(actor),
        RelationshipIdentity::from_entity(target),
    );
    relationship_willingness(actor.personality.cooperativeness, affinity, role)
}

fn relationship_willingness(
    cooperativeness: f32,
    affinity: i16,
    role: CloseRelationshipRole,
) -> bool {
    let affinity_factor = ((f32::from(affinity) + 1_000.0) / 2_000.0).clamp(0.0, 1.0);
    let relationship_bonus = match role {
        CloseRelationshipRole::CurrentPartner => 0.20,
        CloseRelationshipRole::ParentChild => 0.15,
        CloseRelationshipRole::Sibling => 0.10,
        CloseRelationshipRole::Other => 0.0,
    };
    cooperativeness * 0.7 + affinity_factor * 0.3 + relationship_bonus >= 0.5
}

#[cfg(test)]
pub(crate) fn willingness_for_test(cooperativeness: f32, affinity: i16) -> bool {
    relationship_willingness(cooperativeness, affinity, CloseRelationshipRole::Other)
}

/// Ejecuta todos los intentos pendientes. Extraído de `Simulation` para
/// reducir `mod.rs` y aislar la regla de negocio; `Simulation` sigue siendo
/// el único propietario del estado y el que registra eventos.
pub(crate) fn process(simulation: &mut Simulation, attempts: Vec<FoodShareAttempt>) {
    for attempt in attempts {
        let Ok(actor_index) = simulation
            .entities
            .binary_search_by_key(&attempt.actor_id, |entity| entity.id)
        else {
            continue;
        };
        let Ok(target_index) = simulation
            .entities
            .binary_search_by_key(&attempt.target_id, |entity| entity.id)
        else {
            continue;
        };

        let target_is_dependent =
            simulation.entities[target_index].caregiver_id == Some(attempt.actor_id);
        let willing = is_willing(
            &simulation.entities[actor_index],
            &simulation.entities[target_index],
            target_is_dependent,
        );

        let moved = if willing {
            simulation.transfer_item(
                attempt.actor_id,
                attempt.target_id,
                ItemKind::Food,
                attempt.amount,
            )
        } else {
            0
        };

        // Registrar outcome explícito para futura observabilidad (no usado aún
        // en producción, pero establece el contrato A03).
        let _outcome = FoodShareOutcome {
            attempt,
            willing,
            moved,
        };

        if moved > 0 {
            let event_id = simulation.push_event(PendingSimulationEvent {
                caused_by_event_id: None,
                tick: simulation.tick,
                location: EventLocation {
                    x: attempt.actor_location.0,
                    y: attempt.actor_location.1,
                },
                actor_id: attempt.actor_id,
                target_id: Some(attempt.target_id),
                related_entity_ids: vec![attempt.actor_id, attempt.target_id],
                kind: SimulationEventKind::FoodShared,
                cause: SimulationEventCause::FoodShared,
                details: SimulationEventDetails::FoodShared { amount: moved },
            });
            let target_location = (
                simulation.entities[target_index].x,
                simulation.entities[target_index].y,
            );
            if let Some(change) = super::autonomy::record_directed_affinity(
                &mut simulation.entities[target_index],
                attempt.actor_id,
                simulation.tick,
                GRATITUDE_DELTA,
            ) {
                simulation.record_affinity_change(
                    attempt.target_id,
                    target_location,
                    change,
                    SimulationEventCause::FoodShared,
                    Some(event_id),
                );
            }
        } else {
            let event_id = simulation.push_event(PendingSimulationEvent {
                caused_by_event_id: None,
                tick: simulation.tick,
                location: EventLocation {
                    x: attempt.actor_location.0,
                    y: attempt.actor_location.1,
                },
                actor_id: attempt.actor_id,
                target_id: Some(attempt.target_id),
                related_entity_ids: vec![attempt.actor_id, attempt.target_id],
                kind: SimulationEventKind::FoodShareRefused,
                cause: SimulationEventCause::FoodShareRefused,
                details: SimulationEventDetails::FoodShareRefused,
            });
            let target_location = (
                simulation.entities[target_index].x,
                simulation.entities[target_index].y,
            );
            if let Some(change) = super::autonomy::record_directed_affinity(
                &mut simulation.entities[target_index],
                attempt.actor_id,
                simulation.tick,
                RESENTMENT_DELTA,
            ) {
                simulation.record_affinity_change(
                    attempt.target_id,
                    target_location,
                    change,
                    SimulationEventCause::FoodShareRefused,
                    Some(event_id),
                );
            }
            if let Some(dissolution) = super::partnerships::try_dissolve(
                &mut simulation.entities,
                attempt.target_id,
                attempt.actor_id,
            ) {
                simulation.record_partnership_dissolution(
                    dissolution,
                    target_location,
                    SimulationEventCause::FoodShareRefused,
                    Some(event_id),
                );
            }
        }
    }
}
