//! Append-only biological lineage for every entity ever created.

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct LineageRecord {
    pub entity_id: u32,
    pub mother_id: Option<u32>,
    pub father_id: Option<u32>,
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub(crate) struct Genealogy {
    records: Vec<LineageRecord>,
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
}
