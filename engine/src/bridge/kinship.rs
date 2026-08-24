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

#[derive(Serialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
enum KinshipRelationDto {
    SamePerson,
    Parent,
    Child,
    FullSibling,
    HalfSibling,
    Ancestor { generations: u16 },
    Descendant { generations: u16 },
    AuntUncle { generations_removed: u16 },
    NieceNephew { generations_removed: u16 },
    Cousin { degree: u16, removed: u16 },
    Unrelated,
}

#[derive(Serialize)]
struct FamilyTreeNodeDto {
    entity_id: u32,
    generation: i16,
    alive: bool,
}

#[derive(Serialize)]
struct FamilyTreeEdgeDto {
    parent_id: u32,
    child_id: u32,
}

#[derive(Serialize)]
struct FamilyTreeDto {
    focal_id: u32,
    nodes: Vec<FamilyTreeNodeDto>,
    edges: Vec<FamilyTreeEdgeDto>,
}

impl From<simulation::FamilyTree> for FamilyTreeDto {
    fn from(tree: simulation::FamilyTree) -> Self {
        Self {
            focal_id: tree.focal_id,
            nodes: tree
                .nodes
                .into_iter()
                .map(|node| FamilyTreeNodeDto {
                    entity_id: node.entity_id,
                    generation: node.generation,
                    alive: node.alive,
                })
                .collect(),
            edges: tree
                .edges
                .into_iter()
                .map(|edge| FamilyTreeEdgeDto {
                    parent_id: edge.parent_id,
                    child_id: edge.child_id,
                })
                .collect(),
        }
    }
}

impl From<simulation::KinshipRelation> for KinshipRelationDto {
    fn from(relation: simulation::KinshipRelation) -> Self {
        match relation {
            simulation::KinshipRelation::SamePerson => Self::SamePerson,
            simulation::KinshipRelation::Parent => Self::Parent,
            simulation::KinshipRelation::Child => Self::Child,
            simulation::KinshipRelation::FullSibling => Self::FullSibling,
            simulation::KinshipRelation::HalfSibling => Self::HalfSibling,
            simulation::KinshipRelation::Ancestor { generations } => Self::Ancestor { generations },
            simulation::KinshipRelation::Descendant { generations } => {
                Self::Descendant { generations }
            }
            simulation::KinshipRelation::AuntUncle {
                generations_removed,
            } => Self::AuntUncle {
                generations_removed,
            },
            simulation::KinshipRelation::NieceNephew {
                generations_removed,
            } => Self::NieceNephew {
                generations_removed,
            },
            simulation::KinshipRelation::Cousin { degree, removed } => {
                Self::Cousin { degree, removed }
            }
            simulation::KinshipRelation::Unrelated => Self::Unrelated,
        }
    }
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

pub(crate) fn entity_relationship_json(
    simulation: &Simulation,
    first_id: u32,
    second_id: u32,
) -> String {
    to_json(&KinshipRelationDto::from(simulation::relationship_between(
        simulation.genealogy(),
        first_id,
        second_id,
    )))
}

pub(crate) fn entity_family_tree_json(
    simulation: &Simulation,
    entity_id: u32,
    ancestor_depth: u16,
    descendant_depth: u16,
) -> String {
    to_json(&FamilyTreeDto::from(simulation::family_tree_of(
        simulation.genealogy(),
        simulation.entities(),
        entity_id,
        ancestor_depth,
        descendant_depth,
    )))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn cousin_relationship_serializes_as_structured_data() {
        let payload = to_json(&KinshipRelationDto::from(
            simulation::KinshipRelation::Cousin {
                degree: 2,
                removed: 1,
            },
        ));
        let json: serde_json::Value = serde_json::from_str(&payload).unwrap();

        assert_eq!(json["kind"], "cousin");
        assert_eq!(json["degree"], 2);
        assert_eq!(json["removed"], 1);
    }

    #[test]
    fn generation_relationship_serializes_without_ui_labels() {
        let payload = to_json(&KinshipRelationDto::from(
            simulation::KinshipRelation::Ancestor { generations: 3 },
        ));
        let json: serde_json::Value = serde_json::from_str(&payload).unwrap();

        assert_eq!(
            json,
            serde_json::json!({ "kind": "ancestor", "generations": 3 })
        );
    }
}
