use serde::Serialize;

use super::to_json;
use crate::simulation::{self, ItemKind, Simulation};

#[derive(Serialize)]
struct InventoryItemDto {
    kind: &'static str,
    amount: u16,
}

#[derive(Serialize)]
struct InventoryDto {
    capacity: u16,
    used_capacity: u16,
    remaining_capacity: u16,
    items: Vec<InventoryItemDto>,
}

#[derive(Serialize)]
struct EntityHouseholdDto {
    household_id: Option<u32>,
    member_ids: Vec<u32>,
    formed_tick: Option<u64>,
    residence_x: Option<u32>,
    residence_y: Option<u32>,
    storage: Option<InventoryDto>,
}

pub(crate) fn entity_household_json(simulation: &Simulation, entity_id: u32) -> String {
    let household_id = simulation
        .entities()
        .binary_search_by_key(&entity_id, |entity| entity.id)
        .ok()
        .and_then(|index| simulation.entities()[index].household_id);
    let household = household_id.and_then(|id| {
        simulation
            .households()
            .binary_search_by_key(&id, |household| household.id)
            .ok()
            .map(|index| &simulation.households()[index])
    });

    to_json(&EntityHouseholdDto {
        household_id,
        member_ids: household_id.map_or_else(Vec::new, |id| {
            simulation::members_of(simulation.entities(), id)
        }),
        formed_tick: household.map(|household| household.formed_tick),
        residence_x: household.map(|household| household.residence_x),
        residence_y: household.map(|household| household.residence_y),
        storage: household.map(|household| InventoryDto {
            capacity: household.storage.capacity(),
            used_capacity: household.storage.used_capacity(),
            remaining_capacity: household.storage.remaining_capacity(),
            items: ItemKind::ALL
                .into_iter()
                .filter_map(|kind| {
                    let amount = household.storage.amount(kind);
                    (amount > 0).then_some(InventoryItemDto {
                        kind: kind.label(),
                        amount,
                    })
                })
                .collect(),
        }),
    })
}
