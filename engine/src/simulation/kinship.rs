//! Biological kinship derived from persistent parentage.

use super::Entity;

pub(crate) fn children_of(entities: &[Entity], parent_id: u32) -> Vec<u32> {
    let mut children: Vec<_> = entities
        .iter()
        .filter(|entity| entity.mother_id == Some(parent_id) || entity.father_id == Some(parent_id))
        .map(|entity| entity.id)
        .collect();
    children.sort_unstable();
    children
}

pub(crate) fn siblings_of(entities: &[Entity], entity_id: u32) -> Vec<u32> {
    let Some(entity) = entities.iter().find(|entity| entity.id == entity_id) else {
        return Vec::new();
    };
    let mother_id = entity.mother_id;
    let father_id = entity.father_id;
    let mut siblings: Vec<_> = entities
        .iter()
        .filter(|candidate| candidate.id != entity_id)
        .filter(|candidate| {
            (mother_id.is_some() && candidate.mother_id == mother_id)
                || (father_id.is_some() && candidate.father_id == father_id)
        })
        .map(|candidate| candidate.id)
        .collect();
    siblings.sort_unstable();
    siblings.dedup();
    siblings
}
