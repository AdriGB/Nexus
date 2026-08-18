//! Caregiver and dependent invariants.
//!
//! These operations deliberately work on entity state rather than the whole
//! simulation so the dependency domain does not acquire unrelated authority.

use std::collections::{HashMap, HashSet};

use super::config::{FOOD_CONSUMED_PER_MEAL, HUNGER_REDUCTION_PER_MEAL};
use super::{Entity, Goal, LifeStage};
use crate::world::Grid;

pub(super) fn snap_infants_to_caregivers(entities: &mut [Entity]) {
    let positions: HashMap<u32, (u32, u32)> = entities
        .iter()
        .filter(|entity| entity.health > 0.0)
        .map(|entity| (entity.id, (entity.x, entity.y)))
        .collect();

    for entity in entities {
        if entity.health <= 0.0 || LifeStage::from_age_ticks(entity.age_ticks) != LifeStage::Infant
        {
            continue;
        }
        if let Some(position) = entity
            .caregiver_id
            .and_then(|caregiver_id| positions.get(&caregiver_id).copied())
        {
            entity.x = position.0;
            entity.y = position.1;
        }
    }
}

pub(super) fn feed_infants_of(entities: &mut [Entity], consumer_id: u32, consumed: u16) {
    let meal_fraction = f32::from(consumed) / f32::from(FOOD_CONSUMED_PER_MEAL);
    for entity in entities {
        if entity.health > 0.0
            && LifeStage::from_age_ticks(entity.age_ticks) == LifeStage::Infant
            && entity.caregiver_id == Some(consumer_id)
        {
            entity.hunger = (entity.hunger - HUNGER_REDUCTION_PER_MEAL * meal_fraction).max(0.0);
        }
    }
}

pub(super) fn clear_graduated_caregivers(entities: &mut [Entity]) {
    for entity in entities {
        if entity.health <= 0.0 {
            continue;
        }

        if matches!(
            LifeStage::from_age_ticks(entity.age_ticks),
            LifeStage::Infant | LifeStage::Child
        ) {
            continue;
        }

        if entity.caregiver_id.take().is_some() && entity.mind.current_goal == Some(Goal::Follow) {
            entity.mind.clear_goal();
            entity.path.clear();
            entity.path_index = 0;
            entity.movement_credit = 0.0;
        }
    }
}

pub(super) fn reassign_orphaned_dependents(entities: &mut [Entity], world: &Grid) {
    let alive: HashSet<u32> = entities.iter().map(|entity| entity.id).collect();
    let needs_reassignment: Vec<usize> = entities
        .iter()
        .enumerate()
        .filter(|(_, entity)| {
            matches!(
                LifeStage::from_age_ticks(entity.age_ticks),
                LifeStage::Infant | LifeStage::Child
            ) && entity
                .caregiver_id
                .is_none_or(|caregiver_id| !alive.contains(&caregiver_id))
        })
        .map(|(index, _)| index)
        .collect();

    for index in needs_reassignment {
        let position = (entities[index].x, entities[index].y);
        let new_caregiver = find_nearest_caregiver(entities, position, world);

        if entities[index].caregiver_id != new_caregiver {
            let entity = &mut entities[index];
            entity.caregiver_id = new_caregiver;
            entity.mind.clear_goal();
            entity.path.clear();
            entity.path_index = 0;
            entity.movement_credit = 0.0;
        }
    }
}

fn find_nearest_caregiver(entities: &[Entity], position: (u32, u32), world: &Grid) -> Option<u32> {
    let dependent_region = world.region_id_at(position.0, position.1);

    entities
        .iter()
        .filter(|entity| {
            entity.health > 0.0
                && caregiver_priority(LifeStage::from_age_ticks(entity.age_ticks)).is_some()
        })
        .filter(|entity| {
            let caregiver_region = world.region_id_at(entity.x, entity.y);
            match (dependent_region, caregiver_region) {
                (Some(left), Some(right)) => left == right,
                (None, None) => true,
                _ => false,
            }
        })
        .min_by_key(|entity| {
            (
                caregiver_priority(LifeStage::from_age_ticks(entity.age_ticks)).unwrap(),
                entity.x.abs_diff(position.0) + entity.y.abs_diff(position.1),
                entity.id,
            )
        })
        .map(|entity| entity.id)
}

fn caregiver_priority(stage: LifeStage) -> Option<u8> {
    match stage {
        LifeStage::Adult => Some(0),
        LifeStage::Elder => Some(1),
        _ => None,
    }
}
