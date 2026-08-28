//! Biological kinship derived from persistent parentage.

use super::{genealogy::Genealogy, Entity};
use std::collections::{BTreeMap, HashSet, VecDeque};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct KinshipGeneration {
    pub entity_id: u32,
    pub generation: u16,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum KinshipRelation {
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

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct FamilyTreeNode {
    pub entity_id: u32,
    pub generation: i16,
    pub alive: bool,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub(crate) struct FamilyTreeEdge {
    pub parent_id: u32,
    pub child_id: u32,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct FamilyTree {
    pub focal_id: u32,
    pub nodes: Vec<FamilyTreeNode>,
    pub edges: Vec<FamilyTreeEdge>,
}

pub(crate) fn children_of(genealogy: &Genealogy, parent_id: u32) -> Vec<u32> {
    genealogy.children_of(parent_id).to_vec()
}

pub(crate) fn siblings_of(genealogy: &Genealogy, entity_id: u32) -> Vec<u32> {
    let Some(entity) = genealogy.get(entity_id) else {
        return Vec::new();
    };
    let mother_id = entity.mother_id;
    let father_id = entity.father_id;
    genealogy
        .records()
        .iter()
        .filter(|candidate| candidate.entity_id != entity_id)
        .filter(|candidate| {
            (mother_id.is_some() && candidate.mother_id == mother_id)
                || (father_id.is_some() && candidate.father_id == father_id)
        })
        .map(|candidate| candidate.entity_id)
        .collect()
}

pub(crate) fn ancestors_of(genealogy: &Genealogy, entity_id: u32) -> Vec<KinshipGeneration> {
    let mut visited = HashSet::from([entity_id]);
    let mut queue = VecDeque::from([(entity_id, 0u16)]);
    let mut ancestors = Vec::new();

    while let Some((current_id, generation)) = queue.pop_front() {
        let Some(record) = genealogy.get(current_id) else {
            continue;
        };
        for parent_id in [record.mother_id, record.father_id].into_iter().flatten() {
            if visited.insert(parent_id) {
                let generation = generation.saturating_add(1);
                ancestors.push(KinshipGeneration {
                    entity_id: parent_id,
                    generation,
                });
                queue.push_back((parent_id, generation));
            }
        }
    }

    ancestors.sort_unstable_by_key(|relative| (relative.generation, relative.entity_id));
    ancestors
}

pub(crate) fn descendants_of(genealogy: &Genealogy, entity_id: u32) -> Vec<KinshipGeneration> {
    let mut visited = HashSet::from([entity_id]);
    let mut queue = VecDeque::from([(entity_id, 0u16)]);
    let mut descendants = Vec::new();

    while let Some((current_id, generation)) = queue.pop_front() {
        for child_id in children_of(genealogy, current_id) {
            if visited.insert(child_id) {
                let generation = generation.saturating_add(1);
                descendants.push(KinshipGeneration {
                    entity_id: child_id,
                    generation,
                });
                queue.push_back((child_id, generation));
            }
        }
    }

    descendants.sort_unstable_by_key(|relative| (relative.generation, relative.entity_id));
    descendants
}

pub(crate) fn relationship_between(
    genealogy: &Genealogy,
    first_id: u32,
    second_id: u32,
) -> KinshipRelation {
    if first_id == second_id {
        return KinshipRelation::SamePerson;
    }

    if let Some(generations) = generation_of(descendants_of(genealogy, first_id), second_id) {
        return if generations == 1 {
            KinshipRelation::Parent
        } else {
            KinshipRelation::Ancestor { generations }
        };
    }
    if let Some(generations) = generation_of(ancestors_of(genealogy, first_id), second_id) {
        return if generations == 1 {
            KinshipRelation::Child
        } else {
            KinshipRelation::Descendant { generations }
        };
    }

    if let (Some(first), Some(second)) = (genealogy.get(first_id), genealogy.get(second_id)) {
        let same_mother = first.mother_id.is_some() && first.mother_id == second.mother_id;
        let same_father = first.father_id.is_some() && first.father_id == second.father_id;
        if same_mother && same_father {
            return KinshipRelation::FullSibling;
        }
        if same_mother || same_father {
            return KinshipRelation::HalfSibling;
        }
    }

    let first_ancestors = ancestors_of(genealogy, first_id);
    let second_ancestors = ancestors_of(genealogy, second_id);
    let nearest_common = first_ancestors
        .iter()
        .filter_map(|first| {
            second_ancestors
                .iter()
                .find(|second| second.entity_id == first.entity_id)
                .map(|second| (first, second))
        })
        .min_by_key(|(first, second)| {
            (
                first.generation.saturating_add(second.generation),
                first.generation.max(second.generation),
                first.entity_id,
            )
        });

    let Some((first, second)) = nearest_common else {
        return KinshipRelation::Unrelated;
    };
    match (first.generation, second.generation) {
        (1, second_distance) => KinshipRelation::AuntUncle {
            generations_removed: second_distance.saturating_sub(2),
        },
        (first_distance, 1) => KinshipRelation::NieceNephew {
            generations_removed: first_distance.saturating_sub(2),
        },
        (first_distance, second_distance) => KinshipRelation::Cousin {
            degree: first_distance.min(second_distance).saturating_sub(1),
            removed: first_distance.abs_diff(second_distance),
        },
    }
}

fn generation_of(relatives: Vec<KinshipGeneration>, entity_id: u32) -> Option<u16> {
    relatives
        .into_iter()
        .find(|relative| relative.entity_id == entity_id)
        .map(|relative| relative.generation)
}

pub(crate) fn family_tree_of(
    genealogy: &Genealogy,
    living_entities: &[Entity],
    entity_id: u32,
    ancestor_depth: u16,
    descendant_depth: u16,
) -> FamilyTree {
    if genealogy.get(entity_id).is_none() {
        return FamilyTree {
            focal_id: entity_id,
            nodes: Vec::new(),
            edges: Vec::new(),
        };
    }

    let mut generations = BTreeMap::from([(entity_id, 0i16)]);
    for relative in ancestors_of(genealogy, entity_id)
        .into_iter()
        .filter(|relative| relative.generation <= ancestor_depth)
    {
        generations
            .entry(relative.entity_id)
            .or_insert_with(|| -i16::try_from(relative.generation).unwrap_or(i16::MAX));
    }
    for relative in descendants_of(genealogy, entity_id)
        .into_iter()
        .filter(|relative| relative.generation <= descendant_depth)
    {
        generations
            .entry(relative.entity_id)
            .or_insert_with(|| i16::try_from(relative.generation).unwrap_or(i16::MAX));
    }

    let mut nodes: Vec<_> = generations
        .iter()
        .map(|(&relative_id, &generation)| FamilyTreeNode {
            entity_id: relative_id,
            generation,
            alive: living_entities
                .binary_search_by_key(&relative_id, |entity| entity.id)
                .is_ok(),
        })
        .collect();
    nodes.sort_unstable_by_key(|node| (node.generation, node.entity_id));

    let included: HashSet<_> = generations.keys().copied().collect();
    let mut edges = Vec::new();
    for record in genealogy.records() {
        if !included.contains(&record.entity_id) {
            continue;
        }
        for parent_id in [record.mother_id, record.father_id].into_iter().flatten() {
            if included.contains(&parent_id) {
                edges.push(FamilyTreeEdge {
                    parent_id,
                    child_id: record.entity_id,
                });
            }
        }
    }
    edges.sort_unstable();
    edges.dedup();

    FamilyTree {
        focal_id: entity_id,
        nodes,
        edges,
    }
}
