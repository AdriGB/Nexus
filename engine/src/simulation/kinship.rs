//! Biological kinship derived from persistent parentage.

use super::genealogy::Genealogy;

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
