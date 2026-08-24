//! Biological kinship derived from persistent parentage.

use super::genealogy::Genealogy;
use std::collections::{HashSet, VecDeque};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct KinshipGeneration {
    pub entity_id: u32,
    pub generation: u16,
}

pub(crate) fn children_of(genealogy: &Genealogy, parent_id: u32) -> Vec<u32> {
    genealogy
        .records()
        .iter()
        .filter(|record| record.mother_id == Some(parent_id) || record.father_id == Some(parent_id))
        .map(|record| record.entity_id)
        .collect()
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
