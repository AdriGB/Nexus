//! Persistent household identity with membership derived from living entities.

use std::collections::{HashMap, HashSet};

use super::{
    descendants_of, siblings_of, DeathContext, Entity, Genealogy, HouseholdStats, Inventory,
    ItemKind, LifeStage,
};

pub const DEFAULT_HOUSEHOLD_STORAGE_CAPACITY: u16 = 200;

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct Household {
    pub id: u32,
    pub formed_tick: u64,
    pub dissolved_tick: Option<u64>,
    pub inheritance: Option<HouseholdInheritance>,
    pub residence_x: u32,
    pub residence_y: u32,
    pub storage: Inventory,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct HouseholdInheritance {
    pub resolved_tick: u64,
    pub decedent_id: u32,
    pub heir_id: Option<u32>,
    pub destination_household_id: Option<u32>,
}

impl Household {
    pub(crate) fn is_active(&self) -> bool {
        self.dissolved_tick.is_none()
    }
}

pub(super) fn household_stats(
    entities: &[Entity],
    households: &[Household],
    current_tick: u64,
) -> HouseholdStats {
    let active_ids: HashSet<u32> = households
        .iter()
        .filter(|household| household.is_active())
        .map(|household| household.id)
        .collect();
    let mut member_counts: HashMap<u32, u32> = HashMap::new();
    let mut dependent_households = HashSet::new();
    let mut housed_entities = 0u32;
    for entity in entities {
        let Some(household_id) = entity.household_id.filter(|id| active_ids.contains(id)) else {
            continue;
        };
        housed_entities = housed_entities.saturating_add(1);
        *member_counts.entry(household_id).or_default() += 1;
        if matches!(
            LifeStage::from_age_ticks(entity.age_ticks),
            LifeStage::Infant | LifeStage::Child
        ) {
            dependent_households.insert(household_id);
        }
    }

    let mut stats = HouseholdStats {
        total_households: households.len() as u32,
        housed_entities,
        unhoused_entities: (entities.len() as u32).saturating_sub(housed_entities),
        households_with_dependents: dependent_households.len() as u32,
        ..HouseholdStats::default()
    };
    let mut active_member_total = 0u64;
    let mut active_age_total = 0u64;
    let mut dissolved_lifetime_total = 0u64;
    for household in households {
        if household.is_active() {
            stats.active_households += 1;
            let size = member_counts.get(&household.id).copied().unwrap_or(0);
            active_member_total += u64::from(size);
            stats.largest_active_household_size = stats.largest_active_household_size.max(size);
            stats.single_member_households += u32::from(size == 1);
            stats.active_storage_capacity += u64::from(household.storage.capacity());
            stats.active_storage_used += u64::from(household.storage.used_capacity());
            stats.active_food_stored += u64::from(household.storage.amount(ItemKind::Food));
            stats.active_timber_stored += u64::from(household.storage.amount(ItemKind::Timber));
            stats.active_stone_stored += u64::from(household.storage.amount(ItemKind::Stone));
            stats.active_iron_stored += u64::from(household.storage.amount(ItemKind::Iron));
            active_age_total += current_tick.saturating_sub(household.formed_tick);
        } else {
            stats.dissolved_households += 1;
            let dissolved_tick = household
                .dissolved_tick
                .expect("inactive household is dissolved");
            dissolved_lifetime_total += dissolved_tick.saturating_sub(household.formed_tick);
            if let Some(inheritance) = household.inheritance {
                stats.settled_inheritances += 1;
                stats.inheritances_without_heir += u32::from(inheritance.heir_id.is_none());
            }
        }
    }
    if stats.active_households > 0 {
        stats.average_active_household_size =
            active_member_total as f32 / stats.active_households as f32;
        stats.average_active_household_age_ticks =
            active_age_total as f64 / f64::from(stats.active_households);
    }
    if stats.dissolved_households > 0 {
        stats.average_dissolved_household_lifetime_ticks =
            dissolved_lifetime_total as f64 / f64::from(stats.dissolved_households);
    }
    if stats.active_storage_capacity > 0 {
        stats.active_storage_utilization =
            stats.active_storage_used as f32 / stats.active_storage_capacity as f32;
    }
    stats
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) struct HouseholdDissolution {
    pub household_id: u32,
    pub dissolved_tick: u64,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) struct HouseholdInheritanceSettlement {
    pub household_id: u32,
    pub decedent_id: u32,
    pub heir_id: Option<u32>,
    pub destination_household_id: Option<u32>,
    pub transferred: [u16; ItemKind::ALL.len()],
}

pub(super) fn settle_basic_inheritances(
    entities: &mut [Entity],
    households: &mut [Household],
    genealogy: &Genealogy,
    deaths: &[DeathContext],
    dissolutions: &[HouseholdDissolution],
    tick: u64,
) -> Vec<HouseholdInheritanceSettlement> {
    let newly_dissolved: HashSet<u32> = dissolutions.iter().map(|item| item.household_id).collect();
    let living: HashSet<u32> = entities.iter().map(|entity| entity.id).collect();
    let mut settlements = Vec::new();

    for source_index in 0..households.len() {
        let household_id = households[source_index].id;
        if !newly_dissolved.contains(&household_id)
            || households[source_index].dissolved_tick != Some(tick)
            || households[source_index].inheritance.is_some()
        {
            continue;
        }
        let mut associated: Vec<_> = deaths
            .iter()
            .filter(|death| death.household_id == Some(household_id))
            .copied()
            .collect();
        if associated.is_empty() {
            continue;
        }
        associated.sort_unstable_by_key(|death| death.entity_id);

        let mut candidates = Vec::new();
        for death in &associated {
            if let Some(heir_id) = death.partner_id.filter(|id| living.contains(id)) {
                candidates.push((0u8, 0u16, death.entity_id, heir_id));
            }
            for relative in descendants_of(genealogy, death.entity_id) {
                if living.contains(&relative.entity_id) {
                    candidates.push((1, relative.generation, death.entity_id, relative.entity_id));
                }
            }
            if let Some(record) = genealogy.get(death.entity_id) {
                for heir_id in [record.mother_id, record.father_id].into_iter().flatten() {
                    if living.contains(&heir_id) {
                        candidates.push((2, 0, death.entity_id, heir_id));
                    }
                }
            }
            for heir_id in siblings_of(genealogy, death.entity_id) {
                if living.contains(&heir_id) {
                    candidates.push((3, 0, death.entity_id, heir_id));
                }
            }
        }
        candidates.sort_unstable();
        let selected = candidates.first().copied();
        let decedent_id = selected.map_or(associated[0].entity_id, |candidate| candidate.2);
        let heir_id = selected.map(|candidate| candidate.3);
        let destination_household_id = heir_id.and_then(|id| {
            entities
                .binary_search_by_key(&id, |entity| entity.id)
                .ok()
                .and_then(|index| entities[index].household_id)
                .filter(|target_id| {
                    households
                        .binary_search_by_key(target_id, |household| household.id)
                        .ok()
                        .is_some_and(|index| households[index].is_active())
                })
        });
        let mut transferred = [0; ItemKind::ALL.len()];
        if let Some(heir_id) = heir_id {
            if let Some(target_id) = destination_household_id {
                let target_index = households
                    .binary_search_by_key(&target_id, |household| household.id)
                    .expect("validated inheritance destination");
                let (source, target) = if source_index < target_index {
                    let (left, right) = households.split_at_mut(target_index);
                    (&mut left[source_index].storage, &mut right[0].storage)
                } else {
                    let (left, right) = households.split_at_mut(source_index);
                    (&mut right[0].storage, &mut left[target_index].storage)
                };
                transfer_inventory(source, target, &mut transferred);
            } else if let Ok(heir_index) =
                entities.binary_search_by_key(&heir_id, |entity| entity.id)
            {
                transfer_inventory(
                    &mut households[source_index].storage,
                    &mut entities[heir_index].inventory,
                    &mut transferred,
                );
            }
        }
        let inheritance = HouseholdInheritance {
            resolved_tick: tick,
            decedent_id,
            heir_id,
            destination_household_id,
        };
        households[source_index].inheritance = Some(inheritance);
        settlements.push(HouseholdInheritanceSettlement {
            household_id,
            decedent_id,
            heir_id,
            destination_household_id,
            transferred,
        });
    }
    settlements
}

fn transfer_inventory(
    source: &mut Inventory,
    destination: &mut Inventory,
    transferred: &mut [u16; ItemKind::ALL.len()],
) {
    for (index, kind) in ItemKind::ALL.into_iter().enumerate() {
        let accepted = destination.add(kind, source.amount(kind));
        source.remove(kind, accepted);
        transferred[index] = accepted;
    }
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
        inheritance: None,
        residence_x,
        residence_y,
        storage: Inventory::new(DEFAULT_HOUSEHOLD_STORAGE_CAPACITY),
    });
    set_member_household(entities, households, first_id, Some(id))?;
    set_member_household(entities, households, second_id, Some(id))?;
    Some(id)
}
