//! Canonical deterministic state hash for simulation state integrity and replay verification.

use std::fmt;

use super::Simulation;
use crate::world::Grid;

/// A canonical 64-bit state hash representing the complete logical state of a simulation.
///
/// If two simulations have the same `SimulationStateHash`, they have identical logical world
/// state (entities, minds, resources, households, genealogy, event history, and time).
/// Explicitly excludes wall-clock telemetry, spatial grid caches, profiler statistics,
/// and transient scratch buffers.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct SimulationStateHash(pub u64);

impl fmt::Display for SimulationStateHash {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{:016x}", self.0)
    }
}

#[inline]
fn mix64(mut value: u64) -> u64 {
    value ^= value >> 30;
    value = value.wrapping_mul(0xbf58_476d_1ce4_e5b9);
    value ^= value >> 27;
    value = value.wrapping_mul(0x94d0_49bb_1331_11eb);
    value ^ (value >> 31)
}

/// Deterministic 64-bit state hasher based on SplitMix64 mixing.
pub(crate) struct StateHasher {
    state: u64,
}

impl StateHasher {
    pub(crate) fn new() -> Self {
        Self {
            state: 0xcbf2_9ce4_8422_2325,
        }
    }

    #[inline]
    pub(crate) fn write_u64(&mut self, val: u64) {
        self.state = mix64(self.state ^ val.wrapping_mul(0x9e37_79b9_7f4a_7c15));
    }

    #[inline]
    pub(crate) fn write_u32(&mut self, val: u32) {
        self.write_u64(u64::from(val));
    }

    #[inline]
    pub(crate) fn write_u16(&mut self, val: u16) {
        self.write_u64(u64::from(val));
    }

    #[inline]
    pub(crate) fn write_usize(&mut self, val: usize) {
        self.write_u64(val as u64);
    }

    #[inline]
    pub(crate) fn write_u8(&mut self, val: u8) {
        self.write_u64(u64::from(val));
    }

    #[inline]
    pub(crate) fn write_i16(&mut self, val: i16) {
        self.write_u64(val as u16 as u64);
    }

    #[inline]
    pub(crate) fn write_f32(&mut self, val: f32) {
        let bits = if val.is_nan() {
            0x7fc0_0000
        } else if val == 0.0 {
            0
        } else {
            val.to_bits()
        };
        self.write_u32(bits);
    }

    #[inline]
    pub(crate) fn write_bool(&mut self, val: bool) {
        self.write_u64(if val { 1 } else { 0 });
    }

    #[inline]
    pub(crate) fn write_opt_u32(&mut self, val: Option<u32>) {
        match val {
            Some(v) => {
                self.write_bool(true);
                self.write_u32(v);
            }
            None => self.write_bool(false),
        }
    }

    #[inline]
    pub(crate) fn write_opt_u64(&mut self, val: Option<u64>) {
        match val {
            Some(v) => {
                self.write_bool(true);
                self.write_u64(v);
            }
            None => self.write_bool(false),
        }
    }

    pub(crate) fn finish(self) -> SimulationStateHash {
        SimulationStateHash(self.state)
    }
}

pub(crate) const HASH_VERSION: u64 = 1;

const TAG_META: u8 = 1;
const TAG_GRID: u8 = 2;
const TAG_ENTITY: u8 = 3;
const TAG_HOUSEHOLD: u8 = 4;
const TAG_GENEALOGY: u8 = 5;
const TAG_EVENT: u8 = 6;

pub(super) fn compute_state_hash(simulation: &Simulation, world: &Grid) -> SimulationStateHash {
    let mut hasher = StateHasher::new();
    hasher.write_u64(HASH_VERSION);

    // 1. Simulation scalar metadata & monotonic IDs
    // Note on `paused`: `paused` is an interactive simulation driver state, not an
    // intrinsic evolutionary state component. Two simulations with the same logical state
    // will hash identically whether paused or active. When persisting snapshots (Phase 2.11.8),
    // `paused` is stored separately in the snapshot envelope.
    hasher.write_u8(TAG_META);
    hasher.write_u64(simulation.tick);
    hasher.write_u64(simulation.seed);
    hasher.write_u32(simulation.next_entity_id);
    hasher.write_u32(simulation.next_household_id);
    hasher.write_u64(simulation.world_revision);
    hasher.write_u64(simulation.births);
    hasher.write_u64(simulation.deaths);
    hasher.write_u64(simulation.food_consumed);

    // 2. World dimensions, tile terrain, renewable resources, and deposits
    hasher.write_u8(TAG_GRID);
    hasher.write_u32(world.width);
    hasher.write_u32(world.height);
    for tile in &world.tiles {
        hasher.write_u8(tile.terrain as u8);
    }
    hasher.write_usize(world.renewable_resources.len());
    for renewable in &world.renewable_resources {
        hasher.write_usize(renewable.index);
        hasher.write_u32(renewable.kind as u32);
        hasher.write_u16(renewable.capacity);
    }

    // Mutable world resource deposits
    for (index, deposit) in world.resources.iter().enumerate() {
        if let Some(deposit) = deposit {
            hasher.write_usize(index);
            hasher.write_u32(deposit.kind as u32);
            hasher.write_u32(u32::from(deposit.amount));
        }
    }

    // 3. Living entities (ordered by ID)
    hasher.write_u8(TAG_ENTITY);
    hasher.write_usize(simulation.entities.len());
    for entity in &simulation.entities {
        entity.hash_state(&mut hasher);
    }

    // 4. Households (ordered by ID)
    hasher.write_u8(TAG_HOUSEHOLD);
    hasher.write_usize(simulation.households.len());
    for household in &simulation.households {
        household.hash_state(&mut hasher);
    }

    // 5. Lineage genealogy
    hasher.write_u8(TAG_GENEALOGY);
    simulation.genealogy.hash_state(&mut hasher);

    // 6. Recent event history ring
    hasher.write_u8(TAG_EVENT);
    simulation.recent_events.hash_state(&mut hasher);

    hasher.finish()
}
