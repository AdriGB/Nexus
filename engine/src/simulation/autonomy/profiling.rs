//! Optional measurements for the canonical autonomy execution path.

pub(super) const PROFILE_SAMPLE_RATE: usize = 4;

#[derive(Clone, Debug, Default)]
pub(crate) struct AutonomyProfile {
    pub work: crate::simulation::WorkCounters,
    pub state: crate::simulation::StateGauges,
    pub resource_perception_us: u64,
    pub entity_perception_us: u64,
    pub plan_validation_us: u64,
    pub planning_us: u64,
    pub action_us: u64,
    pub sampled_entities: u32,
    pub planned_entities: u32,
    pub urgent_interrupts: u32,
    pub memory_reconciliation_us: u64,
    pub visible_scan_us: u64,
    pub sampled_known_resources_total: u32,
    pub sampled_known_resources_max: u32,
    pub visible_resources_seen: u32,
    pub social_us: u64,
}

pub(crate) fn should_profile_entity(index: usize) -> bool {
    index.is_multiple_of(PROFILE_SAMPLE_RATE)
}
