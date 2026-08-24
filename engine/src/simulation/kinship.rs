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
