//! Food-sharing domain rules — willingness and consequence constants.
//!
//! Extracted from `Simulation::process_food_share_attempts` (A03) to keep the
//! decision rule in one domain module while `Simulation` remains the composition
//! root that translates outcomes into `transfer_item`, affinity changes, events
//! and partnership dissolution.

use super::autonomy::{CloseRelationshipRole, RelationshipIdentity};
use super::entity::Entity;

pub(crate) const GRATITUDE_DELTA: i16 = 20;
pub(crate) const RESENTMENT_DELTA: i16 = -15;

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
