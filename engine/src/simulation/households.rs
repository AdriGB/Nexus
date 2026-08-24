//! Persistent household identity with membership derived from living entities.

use super::Entity;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct Household {
    pub id: u32,
    pub formed_tick: u64,
}

pub(crate) fn members_of(entities: &[Entity], household_id: u32) -> Vec<u32> {
    entities
        .iter()
        .filter(|entity| entity.household_id == Some(household_id))
        .map(|entity| entity.id)
        .collect()
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
    *next_household_id = next_household_id.checked_add(1)?;
    entities[first_index].household_id = Some(id);
    entities[second_index].household_id = Some(id);
    households.push(Household {
        id,
        formed_tick: tick,
    });
    Some(id)
}
