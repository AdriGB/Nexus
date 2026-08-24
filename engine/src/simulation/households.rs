//! Persistent household identity with membership derived from living entities.

use super::{Entity, Inventory};

pub const DEFAULT_HOUSEHOLD_STORAGE_CAPACITY: u16 = 200;

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct Household {
    pub id: u32,
    pub formed_tick: u64,
    pub residence_x: u32,
    pub residence_y: u32,
    pub storage: Inventory,
}

pub(crate) fn members_of(entities: &[Entity], household_id: u32) -> Vec<u32> {
    entities
        .iter()
        .filter(|entity| entity.household_id == Some(household_id))
        .map(|entity| entity.id)
        .collect()
}

pub(super) fn assign_newborn(
    entities: &mut [Entity],
    child_id: u32,
    caregiver_id: u32,
) -> Option<u32> {
    let caregiver_index = entities
        .binary_search_by_key(&caregiver_id, |entity| entity.id)
        .ok()?;
    let household_id = entities[caregiver_index].household_id?;
    let child_index = entities
        .binary_search_by_key(&child_id, |entity| entity.id)
        .ok()?;
    entities[child_index].household_id = Some(household_id);
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
        || entities[first_index].household_id.is_some()
        || entities[second_index].household_id.is_some()
    {
        return None;
    }

    let id = *next_household_id;
    let residence_founder = if first_id < second_id {
        &entities[first_index]
    } else {
        &entities[second_index]
    };
    let (residence_x, residence_y) = (residence_founder.x, residence_founder.y);
    *next_household_id = next_household_id.checked_add(1)?;
    entities[first_index].household_id = Some(id);
    entities[second_index].household_id = Some(id);
    households.push(Household {
        id,
        formed_tick: tick,
        residence_x,
        residence_y,
        storage: Inventory::new(DEFAULT_HOUSEHOLD_STORAGE_CAPACITY),
    });
    Some(id)
}
