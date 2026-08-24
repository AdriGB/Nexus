use serde::Serialize;

use super::to_json;
use crate::simulation::{self, Simulation};

#[derive(Serialize)]
struct EntityHouseholdDto {
    household_id: Option<u32>,
    member_ids: Vec<u32>,
    formed_tick: Option<u64>,
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
            .map(|index| simulation.households()[index])
    });

    to_json(&EntityHouseholdDto {
        household_id,
        member_ids: household_id.map_or_else(Vec::new, |id| {
            simulation::members_of(simulation.entities(), id)
        }),
        formed_tick: household.map(|household| household.formed_tick),
    })
}
