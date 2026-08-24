use serde::Serialize;

use super::to_json;
use crate::simulation::{self, Simulation};

#[derive(Serialize)]
struct KinshipGenerationDto {
    entity_id: u32,
    generation: u16,
}

impl From<simulation::KinshipGeneration> for KinshipGenerationDto {
    fn from(relative: simulation::KinshipGeneration) -> Self {
        Self {
            entity_id: relative.entity_id,
            generation: relative.generation,
        }
    }
}

#[derive(Serialize)]
struct EntityKinshipDto {
    mother_id: Option<u32>,
    father_id: Option<u32>,
    children_ids: Vec<u32>,
    sibling_ids: Vec<u32>,
    ancestors: Vec<KinshipGenerationDto>,
    descendants: Vec<KinshipGenerationDto>,
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
                    children_ids: simulation::children_of(simulation.genealogy(), entity_id),
                    sibling_ids: simulation::siblings_of(simulation.genealogy(), entity_id),
                    ancestors: simulation::ancestors_of(simulation.genealogy(), entity_id)
                        .into_iter()
                        .map(Into::into)
                        .collect(),
                    descendants: simulation::descendants_of(simulation.genealogy(), entity_id)
                        .into_iter()
                        .map(Into::into)
                        .collect(),
                })
            },
        )
}
