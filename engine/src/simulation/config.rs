pub(crate) const MAX_POPULATION: usize = 10_000;

pub(super) const MAX_HUNGER: f32 = 100.0;
pub(super) const MAX_HEALTH: f32 = 100.0;

pub(super) const HUNGER_PER_TICK: f32 = 1.0;
pub(super) const FOOD_SEARCH_THRESHOLD: f32 = 60.0;
pub(super) const STARVATION_DAMAGE_PER_TICK: f32 = 2.0;
pub(super) const BASE_MOVEMENT_SPEED: f32 = 1.0;
pub(super) const PREGNANCY_PHASE_2_START_WEEK: u64 = 14;
pub(super) const PREGNANCY_PHASE_3_START_WEEK: u64 = 28;
pub(super) const PREGNANCY_PHASE_4_START_WEEK: u64 = 36;

pub(super) const PREGNANCY_SPEED_PHASE_1: f32 = 1.0;
pub(super) const PREGNANCY_SPEED_PHASE_2: f32 = 0.9;
pub(super) const PREGNANCY_SPEED_PHASE_3: f32 = 0.75;
pub(super) const PREGNANCY_SPEED_PHASE_4: f32 = 0.6;

pub(super) const FOOD_CONSUMED_PER_MEAL: u16 = 10;
pub(super) const HUNGER_REDUCTION_PER_MEAL: f32 = 50.0;
