//! Append-only biological lineage for every entity ever created.

use std::collections::BTreeMap;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct LineageRecord {
    pub entity_id: u32,
    pub mother_id: Option<u32>,
    pub father_id: Option<u32>,
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub(crate) struct Genealogy {
    records: Vec<LineageRecord>,
    /// Parent id -> child ids, in registration order.
    ///
    /// Maintained exclusively from [`Genealogy::register`]: there is no
    /// deserialization or rollback path that could bypass it, so the index can
    /// never drift from `records`. `BTreeMap` (not `HashMap`) keeps iteration
    /// order deterministic across runs and platforms.
    children: BTreeMap<u32, Vec<u32>>,
}

impl Genealogy {
    pub(crate) fn register(
        &mut self,
        entity_id: u32,
        mother_id: Option<u32>,
        father_id: Option<u32>,
    ) {
        debug_assert!(
            self.records
                .last()
                .is_none_or(|record| record.entity_id < entity_id),
            "lineage records must be registered in entity-ID order"
        );
        // A record is a child of each distinct parent exactly once, mirroring
        // what a linear scan over `records` would have yielded.
        let mut parents = [mother_id, father_id];
        if parents[0] == parents[1] {
            parents[1] = None;
        }
        for parent_id in parents.into_iter().flatten() {
            self.children.entry(parent_id).or_default().push(entity_id);
        }
        self.records.push(LineageRecord {
            entity_id,
            mother_id,
            father_id,
        });
    }

    pub(crate) fn get(&self, entity_id: u32) -> Option<&LineageRecord> {
        self.records
            .binary_search_by_key(&entity_id, |record| record.entity_id)
            .ok()
            .map(|index| &self.records[index])
    }

    pub(crate) fn records(&self) -> &[LineageRecord] {
        &self.records
    }

    /// Children of `parent_id` in registration order, without scanning every
    /// record. Equivalent to filtering [`Genealogy::records`].
    pub(crate) fn children_of(&self, parent_id: u32) -> &[u32] {
        self.children
            .get(&parent_id)
            .map(Vec::as_slice)
            .unwrap_or(&[])
    }

    pub(super) fn hash_state(&self, hasher: &mut super::state_hash::StateHasher) {
        hasher.write_usize(self.records.len());
        for record in &self.records {
            hasher.write_u32(record.entity_id);
            hasher.write_opt_u32(record.mother_id);
            hasher.write_opt_u32(record.father_id);
        }
    }
}
