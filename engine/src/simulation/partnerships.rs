//! Persistent partnership formation, dissolution, and invariants.

use super::autonomy::personality_compatibility;
use super::{relationship_between, Entity, Genealogy, KinshipRelation, LifeStage};

const MIN_INTERACTIONS: u32 = 3;
const BASE_AFFINITY_THRESHOLD: i16 = 300;
const MAX_COMPATIBILITY_DISCOUNT: i16 = 100;
pub(super) const DISSOLUTION_AFFINITY_THRESHOLD: i16 = 0;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) struct PartnershipFormation {
    pub actor_id: u32,
    pub target_id: u32,
    pub actor_affinity: i16,
    pub target_affinity: i16,
    pub compatibility_per_mille: u16,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) struct PartnershipDissolution {
    pub actor_id: u32,
    pub target_id: u32,
    pub actor_affinity: i16,
    pub target_affinity: i16,
}

pub(super) fn try_dissolve(
    entities: &mut [Entity],
    actor_id: u32,
    target_id: u32,
) -> Option<PartnershipDissolution> {
    let actor_index = entities
        .binary_search_by_key(&actor_id, |entity| entity.id)
        .ok()?;
    let target_index = entities
        .binary_search_by_key(&target_id, |entity| entity.id)
        .ok()?;
    if actor_index == target_index {
        return None;
    }

    let (actor, target) = if actor_index < target_index {
        let (left, right) = entities.split_at_mut(target_index);
        (&mut left[actor_index], &mut right[0])
    } else {
        let (left, right) = entities.split_at_mut(actor_index);
        (&mut right[0], &mut left[target_index])
    };

    if actor.partner_id != Some(target_id) || target.partner_id != Some(actor_id) {
        return None;
    }

    let actor_affinity = actor.mind.memory.affinity_to(target_id)?;
    let target_affinity = target.mind.memory.affinity_to(actor_id)?;
    if actor_affinity > DISSOLUTION_AFFINITY_THRESHOLD
        && target_affinity > DISSOLUTION_AFFINITY_THRESHOLD
    {
        return None;
    }

    actor.partner_id = None;
    target.partner_id = None;
    Some(PartnershipDissolution {
        actor_id,
        target_id,
        actor_affinity,
        target_affinity,
    })
}

pub(super) fn dissolve_unhealthy(entities: &mut [Entity]) -> Vec<PartnershipDissolution> {
    let pairs: Vec<_> = entities
        .iter()
        .filter_map(|entity| {
            entity
                .partner_id
                .filter(|partner_id| entity.id < *partner_id)
                .map(|partner_id| (entity.id, partner_id))
        })
        .collect();

    pairs
        .into_iter()
        .filter_map(|(actor_id, target_id)| try_dissolve(entities, actor_id, target_id))
        .collect()
}

pub(super) fn try_form(
    entities: &mut [Entity],
    genealogy: &Genealogy,
    actor_id: u32,
    target_id: u32,
) -> Option<PartnershipFormation> {
    let actor_index = entities
        .binary_search_by_key(&actor_id, |entity| entity.id)
        .ok()?;
    let target_index = entities
        .binary_search_by_key(&target_id, |entity| entity.id)
        .ok()?;
    if actor_index == target_index {
        return None;
    }

    let (actor, target) = if actor_index < target_index {
        let (left, right) = entities.split_at_mut(target_index);
        (&mut left[actor_index], &mut right[0])
    } else {
        let (left, right) = entities.split_at_mut(actor_index);
        (&mut right[0], &mut left[target_index])
    };

    if actor.health <= 0.0
        || target.health <= 0.0
        || actor.partner_id.is_some()
        || target.partner_id.is_some()
        || LifeStage::from_age_ticks(actor.age_ticks) != LifeStage::Adult
        || LifeStage::from_age_ticks(target.age_ticks) != LifeStage::Adult
    {
        return None;
    }

    let actor_relationship = actor
        .mind
        .memory
        .known_entities
        .iter()
        .find(|known| known.id == target.id)?;
    let target_relationship = target
        .mind
        .memory
        .known_entities
        .iter()
        .find(|known| known.id == actor.id)?;
    if actor_relationship.interaction_count < MIN_INTERACTIONS
        || target_relationship.interaction_count < MIN_INTERACTIONS
    {
        return None;
    }

    let compatibility = personality_compatibility(&actor.personality, &target.personality);
    let compatibility_discount =
        (compatibility * f32::from(MAX_COMPATIBILITY_DISCOUNT)).round() as i16;
    let affinity_threshold = BASE_AFFINITY_THRESHOLD - compatibility_discount;
    if actor_relationship.affinity < affinity_threshold
        || target_relationship.affinity < affinity_threshold
    {
        return None;
    }

    if matches!(
        relationship_between(genealogy, actor_id, target_id),
        KinshipRelation::SamePerson
            | KinshipRelation::Parent
            | KinshipRelation::Child
            | KinshipRelation::FullSibling
            | KinshipRelation::HalfSibling
            | KinshipRelation::Ancestor { .. }
            | KinshipRelation::Descendant { .. }
            | KinshipRelation::AuntUncle { .. }
            | KinshipRelation::NieceNephew { .. }
            | KinshipRelation::Cousin { degree: 1, .. }
    ) {
        return None;
    }

    let formation = PartnershipFormation {
        actor_id,
        target_id,
        actor_affinity: actor_relationship.affinity,
        target_affinity: target_relationship.affinity,
        compatibility_per_mille: (compatibility * 1_000.0).round() as u16,
    };
    actor.partner_id = Some(target_id);
    target.partner_id = Some(actor_id);
    Some(formation)
}

pub(super) fn clear_missing_partners(entities: &mut [Entity]) {
    let alive: std::collections::HashSet<u32> = entities.iter().map(|entity| entity.id).collect();
    for entity in entities {
        if entity.partner_id.is_some_and(|id| !alive.contains(&id)) {
            entity.partner_id = None;
        }
    }
}
