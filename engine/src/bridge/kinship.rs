use serde::Serialize;

use super::to_json;
use crate::simulation::{self, Simulation};

#[derive(Serialize)]
struct EntityKinshipDto {
    mother_id: Option<u32>,
    father_id: Option<u32>,
    children_ids: Vec<u32>,
    sibling_ids: Vec<u32>,
}

pub(crate) fn entity_kinship_json(simulation: &Simulation, entity_id: u32) -> String {
    simulation
        .entities()
        .iter()
        .find(|entity| entity.id == entity_id)
        .map_or_else(
            || "{}".to_string(),
            |entity| {
                to_json(&EntityKinshipDto {
                    mother_id: entity.mother_id,
                    father_id: entity.father_id,
                    children_ids: simulation::children_of(simulation.entities(), entity_id),
                    sibling_ids: simulation::siblings_of(simulation.entities(), entity_id),
                })
            },
        )
}
