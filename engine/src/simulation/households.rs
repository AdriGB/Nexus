//! Persistent household identity with membership derived from living entities.

use std::collections::{HashMap, HashSet};

use super::{Entity, Inventory, LifeStage};

pub const DEFAULT_HOUSEHOLD_STORAGE_CAPACITY: u16 = 200;

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct Household {
    pub id: u32,
    pub formed_tick: u64,
    pub dissolved_tick: Option<u64>,
    pub residence_x: u32,
    pub residence_y: u32,
    pub storage: Inventory,
}

impl Household {
    pub(crate) fn is_active(&self) -> bool {
        self.dissolved_tick.is_none()
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) struct HouseholdDissolution {
    pub household_id: u32,
    pub dissolved_tick: u64,
}

pub(super) fn dissolve_empty_households(
    entities: &[Entity],
    households: &mut [Household],
    tick: u64,
) -> Vec<HouseholdDissolution> {
    let referenced_households: HashSet<u32> = entities
        .iter()
        .filter_map(|entity| entity.household_id)
        .collect();

    households
        .iter_mut()
        .filter(|household| household.is_active() && !referenced_households.contains(&household.id))
        .map(|household| {
            household.dissolved_tick = Some(tick);
            HouseholdDissolution {
                household_id: household.id,
                dissolved_tick: tick,
            }
        })
        .collect()
}

pub(crate) fn members_of(entities: &[Entity], household_id: u32) -> Vec<u32> {
    entities
        .iter()
        .filter(|entity| entity.household_id == Some(household_id))
        .map(|entity| entity.id)
        .collect()
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) struct HouseholdMembershipChange {
    pub entity_id: u32,
    pub previous_household_id: Option<u32>,
    pub new_household_id: Option<u32>,
}

pub(super) fn set_member_household(
    entities: &mut [Entity],
    households: &[Household],
    entity_id: u32,
    target_household_id: Option<u32>,
) -> Option<HouseholdMembershipChange> {
    if target_household_id.is_some_and(|household_id| {
        households
            .binary_search_by_key(&household_id, |household| household.id)
            .ok()
            .is_none_or(|index| !households[index].is_active())
    }) {
        return None;
    }
    let entity_index = entities
        .binary_search_by_key(&entity_id, |entity| entity.id)
        .ok()?;
    let previous_household_id = entities[entity_index].household_id;
    if previous_household_id == target_household_id {
        return None;
    }
    entities[entity_index].household_id = target_household_id;
    Some(HouseholdMembershipChange {
        entity_id,
        previous_household_id,
        new_household_id: target_household_id,
    })
}

pub(super) fn synchronize_dependent_memberships(
    entities: &mut [Entity],
    households: &[Household],
) -> Vec<HouseholdMembershipChange> {
    let caregiver_households: HashMap<u32, Option<u32>> = entities
        .iter()
        .filter(|entity| entity.health > 0.0)
        .map(|entity| (entity.id, entity.household_id))
        .collect();
    let transitions: Vec<_> = entities
        .iter()
        .filter(|entity| {
            entity.health > 0.0
                && matches!(
                    LifeStage::from_age_ticks(entity.age_ticks),
                    LifeStage::Infant | LifeStage::Child
                )
        })
        .filter_map(|entity| {
            let caregiver_id = entity.caregiver_id?;
            let caregiver_household = caregiver_households.get(&caregiver_id)?;
            Some((entity.id, *caregiver_household))
        })
        .collect();

    transitions
        .into_iter()
        .filter_map(|(entity_id, household_id)| {
            set_member_household(entities, households, entity_id, household_id)
        })
        .collect()
}

pub(super) fn assign_newborn(
    entities: &mut [Entity],
    households: &[Household],
    child_id: u32,
    caregiver_id: u32,
) -> Option<u32> {
    let caregiver_index = entities
        .binary_search_by_key(&caregiver_id, |entity| entity.id)
        .ok()?;
    let household_id = entities[caregiver_index].household_id?;
    let household_index = households
        .binary_search_by_key(&household_id, |household| household.id)
        .ok()?;
    if !households[household_index].is_active() {
        return None;
    }
    set_member_household(entities, households, child_id, Some(household_id))?;
    Some(household_id)
}

pub(super) fn form_for_partnership(
    entities: &mut [Entity],
    households: &mut Vec<Household>,
    next_household_id: &mut u32,
    first_id: u32,
    second_id: u32,
    tick: u64,
) -> Option<u32> {
    let first_index = entities
        .binary_search_by_key(&first_id, |entity| entity.id)
        .ok()?;
    let second_index = entities
        .binary_search_by_key(&second_id, |entity| entity.id)
        .ok()?;
    if first_index == second_index
        || entities[first_index].partner_id != Some(second_id)
        || entities[second_index].partner_id != Some(first_id)
    {
        return None;
    }

    match (
        entities[first_index].household_id,
        entities[second_index].household_id,
    ) {
        (Some(first_household), Some(second_household)) => {
            if first_household != second_household {
                return None;
            }
            let household_index = households
                .binary_search_by_key(&first_household, |household| household.id)
                .ok()?;
            return households[household_index]
                .is_active()
                .then_some(first_household);
        }
        (Some(household_id), None) => {
            set_member_household(entities, households, second_id, Some(household_id))?;
            return Some(household_id);
        }
        (None, Some(household_id)) => {
            set_member_household(entities, households, first_id, Some(household_id))?;
            return Some(household_id);
        }
        (None, None) => {}
    }

    let id = *next_household_id;
    let residence_founder = if first_id < second_id {
        &entities[first_index]
    } else {
        &entities[second_index]
    };
    let (residence_x, residence_y) = (residence_founder.x, residence_founder.y);
    *next_household_id = next_household_id.checked_add(1)?;
    households.push(Household {
        id,
        formed_tick: tick,
        dissolved_tick: None,
        residence_x,
        residence_y,
        storage: Inventory::new(DEFAULT_HOUSEHOLD_STORAGE_CAPACITY),
    });
    set_member_household(entities, households, first_id, Some(id))?;
    set_member_household(entities, households, second_id, Some(id))?;
    Some(id)
}
