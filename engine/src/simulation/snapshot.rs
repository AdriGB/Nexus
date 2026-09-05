//! Simulation persistence layer (Snapshot V1).
//!
//! Provides deterministic serialization and deserialization of the complete
//! simulation world state. Decoupled from internal collection types through
//! versioned DTOs (`SnapshotV1_*`).
//!
//! Every snapshot embeds the canonical `SimulationStateHash` in its header.
//! When restoring, the simulation rebuilds all spatial and population indices,
//! computes its state hash, and strictly verifies equality with the snapshot header.

use std::collections::{BTreeMap, BTreeSet, VecDeque};
use std::fmt;

use serde::{Deserialize, Serialize};

use super::autonomy::{
    Action, ConflictRecord, FailedExploration, Goal, GriefState, KnownEntity, KnownResource,
    Memory, Mind,
};
use super::entity::{Entity, EntityActivity, Personality, Pregnancy, Sex};
use super::events::{
    EventId, EventLocation, RecentEventHistory, SimulationEvent, SimulationEventCause,
    SimulationEventDetails, SimulationEventKind,
};
use super::genealogy::{Genealogy, LineageRecord};
use super::households::{Household, HouseholdInheritance, HouseholdMigration};
use super::inventory::{Inventory, ItemKind};
use super::spatial::SpatialGrid;
use super::Simulation;
use crate::pathfinding::PathfindingWorkspace;
use crate::world::{Grid, RenewableResource, ResourceDeposit, ResourceKind, Terrain, Tile};

pub const SNAPSHOT_FORMAT: &str = "nexus-snapshot/v1";
pub const ENGINE_VERSION: &str = env!("CARGO_PKG_VERSION");

#[derive(Debug, PartialEq, Eq)]
pub enum SnapshotError {
    InvalidFormat(String),
    UnsupportedVersion(String),
    CorruptedData(String),
    StateHashMismatch { expected: String, actual: String },
    JsonError(String),
}

impl fmt::Display for SnapshotError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::InvalidFormat(msg) => write!(f, "Invalid snapshot format: {}", msg),
            Self::UnsupportedVersion(msg) => write!(f, "Unsupported snapshot version: {}", msg),
            Self::CorruptedData(msg) => write!(f, "Corrupted snapshot data: {}", msg),
            Self::StateHashMismatch { expected, actual } => {
                write!(
                    f,
                    "State hash mismatch: snapshot header recorded {}, but reconstructed simulation hashed to {}",
                    expected, actual
                )
            }
            Self::JsonError(msg) => write!(f, "JSON error: {}", msg),
        }
    }
}

impl std::error::Error for SnapshotError {}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct SnapshotHeaderV1 {
    pub format: String,
    pub engine_version: String,
    pub hash_version: u32,
    pub created_at_tick: u64,
    pub paused: bool,
    pub simulation_seed: u64,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub world_seed: Option<u32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub sea_level: Option<f64>,
    pub world_width: u32,
    pub world_height: u32,
    pub state_hash: String,
}

/// Snapshot representation of an individual world tile.
///
/// ### State Hashing Contract
///
/// The continuous environmental fields (`altitude`, `moisture`, and
/// `temperature`) are stored in the snapshot because they represent continuous
/// physical state of the generated world environment and are needed by
/// renderer and weather systems.
///
/// **They do NOT enter into `SimulationStateHash`**, which strictly hashes only
/// discrete gameplay elements (`terrain as u8` and resource deposits).
///
/// Do NOT add these continuous floating-point fields to `SimulationStateHash` in
/// future revisions: cross-platform floating-point evaluation differences across
/// architectures (x86, ARM, WASM) would violate hash determinism and break
/// backward compatibility of snapshots.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct TileSnapshotV1 {
    pub terrain: u8,
    pub altitude: f64,
    pub moisture: f64,
    pub temperature: f64,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct ResourceDepositSnapshotV1 {
    pub kind: u8,
    pub amount: u16,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct RenewableResourceSnapshotV1 {
    pub index: usize,
    pub kind: u8,
    pub capacity: u16,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct WorldSnapshotV1 {
    pub width: u32,
    pub height: u32,
    pub tiles: Vec<TileSnapshotV1>,
    pub resources: Vec<Option<ResourceDepositSnapshotV1>>,
    pub renewable_resources: Vec<RenewableResourceSnapshotV1>,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct SimulationScalarsSnapshotV1 {
    pub tick: u64,
    pub seed: u64,
    pub next_entity_id: u32,
    pub next_household_id: u32,
    pub world_revision: u64,
    pub births: u64,
    pub deaths: u64,
    pub food_consumed: u64,
}

#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub struct PersonalitySnapshotV1 {
    pub curiosity: f32,
    pub sociability: f32,
    pub cooperativeness: f32,
    pub caution: f32,
    pub persistence: f32,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct PregnancySnapshotV1 {
    pub father_id: u32,
    pub conceived_tick: u64,
    pub due_tick: u64,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(tag = "type", content = "data")]
pub enum ActionSnapshotV1 {
    MoveTo { x: u32, y: u32 },
    Gather { kind: u8 },
    Consume { kind: u8 },
    ExploreArea { x: u32, y: u32 },
    Wait,
    ApproachEntity { id: u32 },
    Interact { id: u32 },
    ShareFood { id: u32 },
    DepositHouseholdFood { amount: u16 },
    WithdrawHouseholdFood { amount: u16 },
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct GriefSnapshotV1 {
    pub deceased_id: u32,
    pub started_tick: u64,
    pub ends_tick: u64,
    pub intensity: u8,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct KnownResourceSnapshotV1 {
    pub x: u32,
    pub y: u32,
    pub kind: u8,
    pub last_seen_tick: u64,
    pub estimated_amount: u16,
    pub failed_attempts: u16,
    pub avoid_until_tick: u64,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct FailedExplorationSnapshotV1 {
    pub chunk_index: u32,
    pub retry_after_tick: u64,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct KnownEntitySnapshotV1 {
    pub id: u32,
    pub first_seen_tick: u64,
    pub last_seen_tick: u64,
    pub last_seen_x: u32,
    pub last_seen_y: u32,
    pub observed_ticks: u32,
    pub affinity: i16,
    pub last_interaction_tick: u64,
    pub interaction_count: u32,
    pub seek_retry_after_tick: Option<u64>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ConflictRecordSnapshotV1 {
    pub entity_id: u32,
    pub last_conflict_tick: u64,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct MemorySnapshotV1 {
    pub known_resources: Vec<KnownResourceSnapshotV1>,
    pub known_chunks: Vec<u32>,
    pub failed_exploration: Vec<FailedExplorationSnapshotV1>,
    pub known_entities: Vec<KnownEntitySnapshotV1>,
    pub known_dead_entities: Vec<u32>,
    pub conflict_history: Vec<ConflictRecordSnapshotV1>,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct MindSnapshotV1 {
    pub perception_radius: u32,
    pub current_goal: Option<u32>,
    pub goal_since_tick: u64,
    pub current_plan: Vec<ActionSnapshotV1>,
    pub plan_index: usize,
    pub visible_entities: Vec<u32>,
    pub grief: Vec<GriefSnapshotV1>,
    pub memory: MemorySnapshotV1,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct EntitySnapshotV1 {
    pub id: u32,
    pub x: u32,
    pub y: u32,
    pub sex: u8,
    pub lifespan_ticks: u64,
    pub hunger: f32,
    pub health: f32,
    pub age_ticks: u64,
    pub path: Vec<(u32, u32)>,
    pub path_index: usize,
    pub activity: u32,
    pub pregnancy: Option<PregnancySnapshotV1>,
    pub postpartum_until_tick: u64,
    pub movement_credit: f32,
    pub mother_id: Option<u32>,
    pub father_id: Option<u32>,
    pub caregiver_id: Option<u32>,
    pub partner_id: Option<u32>,
    pub household_id: Option<u32>,
    pub personality: PersonalitySnapshotV1,
    pub inventory_capacity: u16,
    pub inventory_amounts: [u16; 4],
    pub action_tick: u32,
    pub mind: MindSnapshotV1,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct HouseholdMigrationSnapshotV1 {
    pub started_tick: u64,
    pub proposer_id: u32,
    pub target_x: u32,
    pub target_y: u32,
    pub completed_tick: Option<u64>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct HouseholdInheritanceSnapshotV1 {
    pub resolved_tick: u64,
    pub decedent_id: u32,
    pub heir_id: Option<u32>,
    pub destination_household_id: Option<u32>,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct HouseholdSnapshotV1 {
    pub id: u32,
    pub formed_tick: u64,
    pub dissolved_tick: Option<u64>,
    pub inheritance: Option<HouseholdInheritanceSnapshotV1>,
    pub migration: Option<HouseholdMigrationSnapshotV1>,
    pub residence_x: u32,
    pub residence_y: u32,
    pub storage_capacity: u16,
    pub storage_amounts: [u16; 4],
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct LineageRecordSnapshotV1 {
    pub entity_id: u32,
    pub mother_id: Option<u32>,
    pub father_id: Option<u32>,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct GenealogySnapshotV1 {
    pub records: Vec<LineageRecordSnapshotV1>,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(tag = "type", content = "data")]
pub enum EventDetailsSnapshotV1 {
    Interaction {
        actor_affinity_delta: i16,
        target_affinity_delta: i16,
    },
    Birth {
        child_id: u32,
    },
    Death,
    Consumption {
        amount: u16,
    },
    ResourceDiscovery {
        kind: u8,
        amount: u16,
    },
    Encounter,
    AffinityChange {
        previous_affinity: i16,
        new_affinity: i16,
        delta: i16,
    },
    FoodShared {
        amount: u16,
    },
    FoodShareRefused,
    HouseholdConflict {
        household_id: u32,
        actor_affinity_delta: i16,
        target_affinity_delta: i16,
    },
    PartnershipFormed {
        actor_affinity: i16,
        target_affinity: i16,
        compatibility_per_mille: u16,
    },
    PartnershipDissolved {
        actor_affinity: i16,
        target_affinity: i16,
    },
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct SimulationEventSnapshotV1 {
    pub id: u64,
    pub caused_by_event_id: Option<u64>,
    pub tick: u64,
    pub location_x: u32,
    pub location_y: u32,
    pub actor_id: u32,
    pub target_id: Option<u32>,
    pub related_entity_ids: Vec<u32>,
    pub kind: u32,
    pub cause: u32,
    pub details: EventDetailsSnapshotV1,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct EventsSnapshotV1 {
    pub capacity: usize,
    pub next_id: u64,
    pub total_created: u64,
    pub events: Vec<SimulationEventSnapshotV1>,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct SimulationSnapshotV1 {
    pub header: SnapshotHeaderV1,
    pub scalars: SimulationScalarsSnapshotV1,
    pub world: WorldSnapshotV1,
    pub entities: Vec<EntitySnapshotV1>,
    pub households: Vec<HouseholdSnapshotV1>,
    pub genealogy: GenealogySnapshotV1,
    pub events: EventsSnapshotV1,
}

pub(crate) fn terrain_to_u8(t: Terrain) -> u8 {
    t as u8
}

pub(crate) fn u8_to_terrain(b: u8) -> Result<Terrain, SnapshotError> {
    match b {
        0 => Ok(Terrain::DeepWater),
        1 => Ok(Terrain::ShallowWater),
        2 => Ok(Terrain::Beach),
        3 => Ok(Terrain::Plains),
        4 => Ok(Terrain::Grassland),
        5 => Ok(Terrain::Forest),
        6 => Ok(Terrain::DenseForest),
        7 => Ok(Terrain::Hills),
        8 => Ok(Terrain::Mountain),
        9 => Ok(Terrain::SnowPeak),
        10 => Ok(Terrain::Desert),
        11 => Ok(Terrain::Swamp),
        12 => Ok(Terrain::Tundra),
        other => Err(SnapshotError::CorruptedData(format!(
            "invalid terrain discriminant: {}",
            other
        ))),
    }
}

pub(crate) fn resource_kind_to_u8(k: ResourceKind) -> u8 {
    k as u8
}

pub(crate) fn u8_to_resource_kind(b: u8) -> Result<ResourceKind, SnapshotError> {
    match b {
        1 => Ok(ResourceKind::Food),
        2 => Ok(ResourceKind::Timber),
        3 => Ok(ResourceKind::Stone),
        4 => Ok(ResourceKind::Iron),
        other => Err(SnapshotError::CorruptedData(format!(
            "invalid resource kind discriminant: {}",
            other
        ))),
    }
}

#[allow(dead_code)]
pub(crate) fn item_kind_to_u8(k: ItemKind) -> u8 {
    match k {
        ItemKind::Food => 1,
        ItemKind::Timber => 2,
        ItemKind::Stone => 3,
        ItemKind::Iron => 4,
    }
}

#[allow(dead_code)]
pub(crate) fn u8_to_item_kind(b: u8) -> Result<ItemKind, SnapshotError> {
    match b {
        1 => Ok(ItemKind::Food),
        2 => Ok(ItemKind::Timber),
        3 => Ok(ItemKind::Stone),
        4 => Ok(ItemKind::Iron),
        other => Err(SnapshotError::CorruptedData(format!(
            "invalid item kind discriminant: {}",
            other
        ))),
    }
}

pub(crate) fn goal_to_u32(g: Goal) -> u32 {
    match g {
        Goal::Eat => 1,
        Goal::AcquireResource => 2,
        Goal::ConfrontHouseholdMember => 3,
        Goal::Explore => 4,
        Goal::Follow => 5,
        Goal::Grieve => 6,
        Goal::MigrateHousehold => 7,
        Goal::ProtectDependent => 8,
        Goal::Rest => 9,
        Goal::Socialize => 10,
        Goal::ShareFood => 11,
    }
}

pub(crate) fn u32_to_goal(val: u32) -> Result<Goal, SnapshotError> {
    match val {
        1 => Ok(Goal::Eat),
        2 => Ok(Goal::AcquireResource),
        3 => Ok(Goal::ConfrontHouseholdMember),
        4 => Ok(Goal::Explore),
        5 => Ok(Goal::Follow),
        6 => Ok(Goal::Grieve),
        7 => Ok(Goal::MigrateHousehold),
        8 => Ok(Goal::ProtectDependent),
        9 => Ok(Goal::Rest),
        10 => Ok(Goal::Socialize),
        11 => Ok(Goal::ShareFood),
        other => Err(SnapshotError::CorruptedData(format!(
            "invalid goal discriminant: {}",
            other
        ))),
    }
}

pub(crate) fn action_to_dto(action: &Action) -> ActionSnapshotV1 {
    match *action {
        Action::MoveTo(x, y) => ActionSnapshotV1::MoveTo { x, y },
        Action::Gather(k) => ActionSnapshotV1::Gather {
            kind: resource_kind_to_u8(k),
        },
        Action::Consume(k) => ActionSnapshotV1::Consume {
            kind: resource_kind_to_u8(k),
        },
        Action::ExploreArea(x, y) => ActionSnapshotV1::ExploreArea { x, y },
        Action::Wait => ActionSnapshotV1::Wait,
        Action::ApproachEntity(id) => ActionSnapshotV1::ApproachEntity { id },
        Action::Interact(id) => ActionSnapshotV1::Interact { id },
        Action::ShareFood(id) => ActionSnapshotV1::ShareFood { id },
        Action::DepositHouseholdFood(amount) => ActionSnapshotV1::DepositHouseholdFood { amount },
        Action::WithdrawHouseholdFood(amount) => ActionSnapshotV1::WithdrawHouseholdFood { amount },
    }
}

pub(crate) fn dto_to_action(dto: &ActionSnapshotV1) -> Result<Action, SnapshotError> {
    match *dto {
        ActionSnapshotV1::MoveTo { x, y } => Ok(Action::MoveTo(x, y)),
        ActionSnapshotV1::Gather { kind } => Ok(Action::Gather(u8_to_resource_kind(kind)?)),
        ActionSnapshotV1::Consume { kind } => Ok(Action::Consume(u8_to_resource_kind(kind)?)),
        ActionSnapshotV1::ExploreArea { x, y } => Ok(Action::ExploreArea(x, y)),
        ActionSnapshotV1::Wait => Ok(Action::Wait),
        ActionSnapshotV1::ApproachEntity { id } => Ok(Action::ApproachEntity(id)),
        ActionSnapshotV1::Interact { id } => Ok(Action::Interact(id)),
        ActionSnapshotV1::ShareFood { id } => Ok(Action::ShareFood(id)),
        ActionSnapshotV1::DepositHouseholdFood { amount } => {
            Ok(Action::DepositHouseholdFood(amount))
        }
        ActionSnapshotV1::WithdrawHouseholdFood { amount } => {
            Ok(Action::WithdrawHouseholdFood(amount))
        }
    }
}

pub(crate) fn activity_to_u32(act: EntityActivity) -> u32 {
    act as u32
}

pub(crate) fn u32_to_activity(val: u32) -> Result<EntityActivity, SnapshotError> {
    match val {
        0 => Ok(EntityActivity::Idle),
        1 => Ok(EntityActivity::SeekingFood),
        2 => Ok(EntityActivity::Moving),
        3 => Ok(EntityActivity::Starving),
        4 => Ok(EntityActivity::Exploring),
        5 => Ok(EntityActivity::Resting),
        6 => Ok(EntityActivity::Socializing),
        other => Err(SnapshotError::CorruptedData(format!(
            "invalid activity discriminant: {}",
            other
        ))),
    }
}

pub(crate) fn event_kind_to_u32(k: SimulationEventKind) -> u32 {
    match k {
        SimulationEventKind::Interaction => 1,
        SimulationEventKind::Birth => 2,
        SimulationEventKind::Death => 3,
        SimulationEventKind::Discovery => 4,
        SimulationEventKind::Consumption => 5,
        SimulationEventKind::Encounter => 6,
        SimulationEventKind::AffinityChange => 7,
        SimulationEventKind::PartnershipFormed => 8,
        SimulationEventKind::PartnershipDissolved => 9,
        SimulationEventKind::FoodShared => 10,
        SimulationEventKind::FoodShareRefused => 11,
        SimulationEventKind::HouseholdConflict => 12,
    }
}

pub(crate) fn u32_to_event_kind(val: u32) -> Result<SimulationEventKind, SnapshotError> {
    match val {
        1 => Ok(SimulationEventKind::Interaction),
        2 => Ok(SimulationEventKind::Birth),
        3 => Ok(SimulationEventKind::Death),
        4 => Ok(SimulationEventKind::Discovery),
        5 => Ok(SimulationEventKind::Consumption),
        6 => Ok(SimulationEventKind::Encounter),
        7 => Ok(SimulationEventKind::AffinityChange),
        8 => Ok(SimulationEventKind::PartnershipFormed),
        9 => Ok(SimulationEventKind::PartnershipDissolved),
        10 => Ok(SimulationEventKind::FoodShared),
        11 => Ok(SimulationEventKind::FoodShareRefused),
        12 => Ok(SimulationEventKind::HouseholdConflict),
        other => Err(SnapshotError::CorruptedData(format!(
            "invalid event kind discriminant: {}",
            other
        ))),
    }
}

pub(crate) fn event_cause_to_u32(c: SimulationEventCause) -> u32 {
    match c {
        SimulationEventCause::MutualSocialContact => 1,
        SimulationEventCause::Born => 2,
        SimulationEventCause::Starvation => 3,
        SimulationEventCause::NaturalDeath => 4,
        SimulationEventCause::AteFood => 5,
        SimulationEventCause::ResourceFound => 6,
        SimulationEventCause::FirstEncounter => 7,
        SimulationEventCause::RelationshipDecay => 8,
        SimulationEventCause::FoodShared => 9,
        SimulationEventCause::FoodShareRefused => 10,
        SimulationEventCause::MutualCommitment => 11,
        SimulationEventCause::HouseholdConflict => 12,
    }
}

pub(crate) fn u32_to_event_cause(val: u32) -> Result<SimulationEventCause, SnapshotError> {
    match val {
        1 => Ok(SimulationEventCause::MutualSocialContact),
        2 => Ok(SimulationEventCause::Born),
        3 => Ok(SimulationEventCause::Starvation),
        4 => Ok(SimulationEventCause::NaturalDeath),
        5 => Ok(SimulationEventCause::AteFood),
        6 => Ok(SimulationEventCause::ResourceFound),
        7 => Ok(SimulationEventCause::FirstEncounter),
        8 => Ok(SimulationEventCause::RelationshipDecay),
        9 => Ok(SimulationEventCause::FoodShared),
        10 => Ok(SimulationEventCause::FoodShareRefused),
        11 => Ok(SimulationEventCause::MutualCommitment),
        12 => Ok(SimulationEventCause::HouseholdConflict),
        other => Err(SnapshotError::CorruptedData(format!(
            "invalid event cause discriminant: {}",
            other
        ))),
    }
}

impl From<Action> for ActionSnapshotV1 {
    fn from(action: Action) -> Self {
        action_to_dto(&action)
    }
}

impl TryFrom<ActionSnapshotV1> for Action {
    type Error = SnapshotError;

    fn try_from(dto: ActionSnapshotV1) -> Result<Self, Self::Error> {
        dto_to_action(&dto)
    }
}

impl TryFrom<u8> for Terrain {
    type Error = SnapshotError;

    fn try_from(value: u8) -> Result<Self, Self::Error> {
        u8_to_terrain(value)
    }
}

impl TryFrom<u8> for ResourceKind {
    type Error = SnapshotError;

    fn try_from(value: u8) -> Result<Self, Self::Error> {
        u8_to_resource_kind(value)
    }
}

impl TryFrom<u8> for ItemKind {
    type Error = SnapshotError;

    fn try_from(value: u8) -> Result<Self, Self::Error> {
        u8_to_item_kind(value)
    }
}

impl TryFrom<u32> for Goal {
    type Error = SnapshotError;

    fn try_from(value: u32) -> Result<Self, Self::Error> {
        u32_to_goal(value)
    }
}

impl TryFrom<u32> for SimulationEventKind {
    type Error = SnapshotError;

    fn try_from(value: u32) -> Result<Self, Self::Error> {
        u32_to_event_kind(value)
    }
}

impl TryFrom<u32> for SimulationEventCause {
    type Error = SnapshotError;

    fn try_from(value: u32) -> Result<Self, Self::Error> {
        u32_to_event_cause(value)
    }
}

impl TryFrom<u32> for EntityActivity {
    type Error = SnapshotError;

    fn try_from(value: u32) -> Result<Self, Self::Error> {
        u32_to_activity(value)
    }
}

fn event_details_to_dto(d: &SimulationEventDetails) -> EventDetailsSnapshotV1 {
    match *d {
        SimulationEventDetails::Interaction {
            actor_affinity_delta,
            target_affinity_delta,
        } => EventDetailsSnapshotV1::Interaction {
            actor_affinity_delta,
            target_affinity_delta,
        },
        SimulationEventDetails::Birth { child_id } => EventDetailsSnapshotV1::Birth { child_id },
        SimulationEventDetails::Death => EventDetailsSnapshotV1::Death,
        SimulationEventDetails::Consumption { amount } => {
            EventDetailsSnapshotV1::Consumption { amount }
        }
        SimulationEventDetails::ResourceDiscovery { kind, amount } => {
            EventDetailsSnapshotV1::ResourceDiscovery {
                kind: resource_kind_to_u8(kind),
                amount,
            }
        }
        SimulationEventDetails::Encounter => EventDetailsSnapshotV1::Encounter,
        SimulationEventDetails::AffinityChange {
            previous_affinity,
            new_affinity,
            delta,
        } => EventDetailsSnapshotV1::AffinityChange {
            previous_affinity,
            new_affinity,
            delta,
        },
        SimulationEventDetails::FoodShared { amount } => {
            EventDetailsSnapshotV1::FoodShared { amount }
        }
        SimulationEventDetails::FoodShareRefused => EventDetailsSnapshotV1::FoodShareRefused,
        SimulationEventDetails::HouseholdConflict {
            household_id,
            actor_affinity_delta,
            target_affinity_delta,
        } => EventDetailsSnapshotV1::HouseholdConflict {
            household_id,
            actor_affinity_delta,
            target_affinity_delta,
        },
        SimulationEventDetails::PartnershipFormed {
            actor_affinity,
            target_affinity,
            compatibility_per_mille,
        } => EventDetailsSnapshotV1::PartnershipFormed {
            actor_affinity,
            target_affinity,
            compatibility_per_mille,
        },
        SimulationEventDetails::PartnershipDissolved {
            actor_affinity,
            target_affinity,
        } => EventDetailsSnapshotV1::PartnershipDissolved {
            actor_affinity,
            target_affinity,
        },
    }
}

fn dto_to_event_details(
    dto: &EventDetailsSnapshotV1,
) -> Result<SimulationEventDetails, SnapshotError> {
    match *dto {
        EventDetailsSnapshotV1::Interaction {
            actor_affinity_delta,
            target_affinity_delta,
        } => Ok(SimulationEventDetails::Interaction {
            actor_affinity_delta,
            target_affinity_delta,
        }),
        EventDetailsSnapshotV1::Birth { child_id } => {
            Ok(SimulationEventDetails::Birth { child_id })
        }
        EventDetailsSnapshotV1::Death => Ok(SimulationEventDetails::Death),
        EventDetailsSnapshotV1::Consumption { amount } => {
            Ok(SimulationEventDetails::Consumption { amount })
        }
        EventDetailsSnapshotV1::ResourceDiscovery { kind, amount } => {
            Ok(SimulationEventDetails::ResourceDiscovery {
                kind: u8_to_resource_kind(kind)?,
                amount,
            })
        }
        EventDetailsSnapshotV1::Encounter => Ok(SimulationEventDetails::Encounter),
        EventDetailsSnapshotV1::AffinityChange {
            previous_affinity,
            new_affinity,
            delta,
        } => Ok(SimulationEventDetails::AffinityChange {
            previous_affinity,
            new_affinity,
            delta,
        }),
        EventDetailsSnapshotV1::FoodShared { amount } => {
            Ok(SimulationEventDetails::FoodShared { amount })
        }
        EventDetailsSnapshotV1::FoodShareRefused => Ok(SimulationEventDetails::FoodShareRefused),
        EventDetailsSnapshotV1::HouseholdConflict {
            household_id,
            actor_affinity_delta,
            target_affinity_delta,
        } => Ok(SimulationEventDetails::HouseholdConflict {
            household_id,
            actor_affinity_delta,
            target_affinity_delta,
        }),
        EventDetailsSnapshotV1::PartnershipFormed {
            actor_affinity,
            target_affinity,
            compatibility_per_mille,
        } => Ok(SimulationEventDetails::PartnershipFormed {
            actor_affinity,
            target_affinity,
            compatibility_per_mille,
        }),
        EventDetailsSnapshotV1::PartnershipDissolved {
            actor_affinity,
            target_affinity,
        } => Ok(SimulationEventDetails::PartnershipDissolved {
            actor_affinity,
            target_affinity,
        }),
    }
}

impl SimulationSnapshotV1 {
    /// Captures the full state of `simulation` and `world` into a snapshot DTO.
    pub fn capture(
        simulation: &Simulation,
        world: &Grid,
        world_seed: Option<u32>,
        sea_level: Option<f64>,
    ) -> Self {
        let state_hash = simulation.state_hash(world).to_string();

        let header = SnapshotHeaderV1 {
            format: SNAPSHOT_FORMAT.to_string(),
            engine_version: ENGINE_VERSION.to_string(),
            hash_version: super::state_hash::HASH_VERSION,
            created_at_tick: simulation.tick,
            paused: simulation.paused,
            simulation_seed: simulation.seed,
            world_seed,
            sea_level,
            world_width: world.width,
            world_height: world.height,
            state_hash,
        };

        let scalars = SimulationScalarsSnapshotV1 {
            tick: simulation.tick,
            seed: simulation.seed,
            next_entity_id: simulation.next_entity_id,
            next_household_id: simulation.next_household_id,
            world_revision: simulation.world_revision,
            births: simulation.births,
            deaths: simulation.deaths,
            food_consumed: simulation.food_consumed,
        };

        let world_dto = WorldSnapshotV1 {
            width: world.width,
            height: world.height,
            tiles: world
                .tiles
                .iter()
                .map(|t| TileSnapshotV1 {
                    terrain: terrain_to_u8(t.terrain),
                    altitude: t.altitude,
                    moisture: t.moisture,
                    temperature: t.temperature,
                })
                .collect(),
            resources: world
                .resources
                .iter()
                .map(|dep| {
                    dep.as_ref().map(|d| ResourceDepositSnapshotV1 {
                        kind: resource_kind_to_u8(d.kind),
                        amount: d.amount,
                    })
                })
                .collect(),
            renewable_resources: world
                .renewable_resources
                .iter()
                .map(|r| RenewableResourceSnapshotV1 {
                    index: r.index,
                    kind: resource_kind_to_u8(r.kind),
                    capacity: r.capacity,
                })
                .collect(),
        };

        let entities = simulation
            .entities
            .iter()
            .map(|e| EntitySnapshotV1 {
                id: e.id,
                x: e.x,
                y: e.y,
                sex: match e.sex {
                    Sex::Female => 0,
                    Sex::Male => 1,
                },
                lifespan_ticks: e.lifespan_ticks,
                hunger: e.hunger,
                health: e.health,
                age_ticks: e.age_ticks,
                path: e.path.clone(),
                path_index: e.path_index,
                activity: activity_to_u32(e.activity),
                pregnancy: e.pregnancy.map(|p| PregnancySnapshotV1 {
                    father_id: p.father_id,
                    conceived_tick: p.conceived_tick,
                    due_tick: p.due_tick,
                }),
                postpartum_until_tick: e.postpartum_until_tick,
                movement_credit: e.movement_credit,
                mother_id: e.mother_id,
                father_id: e.father_id,
                caregiver_id: e.caregiver_id,
                partner_id: e.partner_id,
                household_id: e.household_id,
                personality: PersonalitySnapshotV1 {
                    curiosity: e.personality.curiosity,
                    sociability: e.personality.sociability,
                    cooperativeness: e.personality.cooperativeness,
                    caution: e.personality.caution,
                    persistence: e.personality.persistence,
                },
                inventory_capacity: e.inventory.capacity(),
                inventory_amounts: *e.inventory.amounts(),
                action_tick: e.action_tick,
                mind: MindSnapshotV1 {
                    perception_radius: e.mind.perception_radius,
                    current_goal: e.mind.current_goal.map(goal_to_u32),
                    goal_since_tick: e.mind.goal_since_tick,
                    current_plan: e.mind.current_plan.iter().map(action_to_dto).collect(),
                    plan_index: e.mind.plan_index,
                    visible_entities: e.mind.visible_entities.clone(),
                    grief: e
                        .mind
                        .grief
                        .iter()
                        .map(|g| GriefSnapshotV1 {
                            deceased_id: g.deceased_id,
                            started_tick: g.started_tick,
                            ends_tick: g.ends_tick,
                            intensity: g.intensity,
                        })
                        .collect(),
                    memory: MemorySnapshotV1 {
                        known_resources: e
                            .mind
                            .memory
                            .known_resources
                            .iter()
                            .map(|k| KnownResourceSnapshotV1 {
                                x: k.x,
                                y: k.y,
                                kind: resource_kind_to_u8(k.kind),
                                last_seen_tick: k.last_seen_tick,
                                estimated_amount: k.estimated_amount,
                                failed_attempts: k.failed_attempts,
                                avoid_until_tick: k.avoid_until_tick,
                            })
                            .collect(),
                        known_chunks: e.mind.memory.known_chunks.iter().copied().collect(),
                        failed_exploration: e
                            .mind
                            .memory
                            .failed_exploration
                            .iter()
                            .map(|f| FailedExplorationSnapshotV1 {
                                chunk_index: f.chunk_index,
                                retry_after_tick: f.retry_after_tick,
                            })
                            .collect(),
                        known_entities: e
                            .mind
                            .memory
                            .known_entities
                            .iter()
                            .map(|k| KnownEntitySnapshotV1 {
                                id: k.id,
                                first_seen_tick: k.first_seen_tick,
                                last_seen_tick: k.last_seen_tick,
                                last_seen_x: k.last_seen_x,
                                last_seen_y: k.last_seen_y,
                                observed_ticks: k.observed_ticks,
                                affinity: k.affinity,
                                last_interaction_tick: k.last_interaction_tick,
                                interaction_count: k.interaction_count,
                                seek_retry_after_tick: k.seek_retry_after_tick,
                            })
                            .collect(),
                        known_dead_entities: e.mind.memory.known_dead_entities.clone(),
                        conflict_history: e
                            .mind
                            .memory
                            .conflict_history
                            .iter()
                            .map(|c| ConflictRecordSnapshotV1 {
                                entity_id: c.entity_id,
                                last_conflict_tick: c.last_conflict_tick,
                            })
                            .collect(),
                    },
                },
            })
            .collect();

        let households = simulation
            .households
            .iter()
            .map(|h| HouseholdSnapshotV1 {
                id: h.id,
                formed_tick: h.formed_tick,
                dissolved_tick: h.dissolved_tick,
                inheritance: h.inheritance.map(|i| HouseholdInheritanceSnapshotV1 {
                    resolved_tick: i.resolved_tick,
                    decedent_id: i.decedent_id,
                    heir_id: i.heir_id,
                    destination_household_id: i.destination_household_id,
                }),
                migration: h.migration.map(|m| HouseholdMigrationSnapshotV1 {
                    started_tick: m.started_tick,
                    proposer_id: m.proposer_id,
                    target_x: m.target_x,
                    target_y: m.target_y,
                    completed_tick: m.completed_tick,
                }),
                residence_x: h.residence_x,
                residence_y: h.residence_y,
                storage_capacity: h.storage.capacity(),
                storage_amounts: *h.storage.amounts(),
            })
            .collect();

        let genealogy = GenealogySnapshotV1 {
            records: simulation
                .genealogy
                .records()
                .iter()
                .map(|r| LineageRecordSnapshotV1 {
                    entity_id: r.entity_id,
                    mother_id: r.mother_id,
                    father_id: r.father_id,
                })
                .collect(),
        };

        let events = EventsSnapshotV1 {
            capacity: simulation.recent_events.capacity(),
            next_id: simulation.recent_events.next_id().as_u64(),
            total_created: simulation.recent_events.total_created(),
            events: simulation
                .recent_events
                .iter()
                .map(|ev| SimulationEventSnapshotV1 {
                    id: ev.id.as_u64(),
                    caused_by_event_id: ev.caused_by_event_id.map(|id| id.as_u64()),
                    tick: ev.tick,
                    location_x: ev.location.x,
                    location_y: ev.location.y,
                    actor_id: ev.actor_id,
                    target_id: ev.target_id,
                    related_entity_ids: ev.related_entity_ids.clone(),
                    kind: event_kind_to_u32(ev.kind),
                    cause: event_cause_to_u32(ev.cause),
                    details: event_details_to_dto(&ev.details),
                })
                .collect(),
        };

        Self {
            header,
            scalars,
            world: world_dto,
            entities,
            households,
            genealogy,
            events,
        }
    }

    /// Reconstructs the complete `Simulation` and `Grid` state from this snapshot.
    /// Rebuilds spatial and caregiver caches, verifies structural invariants,
    /// and ensures the reconstructed simulation state hash matches the snapshot header.
    pub fn restore(self) -> Result<(Simulation, Grid), SnapshotError> {
        if self.header.format != SNAPSHOT_FORMAT {
            return Err(SnapshotError::InvalidFormat(format!(
                "expected format '{}', found '{}'",
                SNAPSHOT_FORMAT, self.header.format
            )));
        }

        if self.header.hash_version != super::state_hash::HASH_VERSION {
            return Err(SnapshotError::UnsupportedVersion(format!(
                "expected hash version {}, found {}",
                super::state_hash::HASH_VERSION,
                self.header.hash_version
            )));
        }

        if self.header.engine_version != ENGINE_VERSION {
            return Err(SnapshotError::UnsupportedVersion(format!(
                "expected engine version '{}', found '{}'",
                ENGINE_VERSION, self.header.engine_version
            )));
        }

        // 1. Reconstruct Grid
        let tile_count = (self.world.width as usize)
            .checked_mul(self.world.height as usize)
            .ok_or_else(|| {
                SnapshotError::CorruptedData("world dimensions arithmetic overflow".to_string())
            })?;

        if self.world.tiles.len() != tile_count {
            return Err(SnapshotError::CorruptedData(format!(
                "world tile count mismatch: expected {}, found {}",
                tile_count,
                self.world.tiles.len()
            )));
        }

        if self.world.resources.len() != tile_count {
            return Err(SnapshotError::CorruptedData(format!(
                "world resource count mismatch: expected {}, found {}",
                tile_count,
                self.world.resources.len()
            )));
        }

        let mut tiles = Vec::with_capacity(tile_count);
        for t in &self.world.tiles {
            tiles.push(Tile {
                terrain: u8_to_terrain(t.terrain)?,
                altitude: t.altitude,
                moisture: t.moisture,
                temperature: t.temperature,
            });
        }

        let mut resources = Vec::with_capacity(tile_count);
        for r in &self.world.resources {
            match r {
                Some(dep) => resources.push(Some(ResourceDeposit {
                    kind: u8_to_resource_kind(dep.kind)?,
                    amount: dep.amount,
                })),
                None => resources.push(None),
            }
        }

        let mut renewable_resources = Vec::with_capacity(self.world.renewable_resources.len());
        for r in &self.world.renewable_resources {
            if r.index >= tile_count {
                return Err(SnapshotError::CorruptedData(format!(
                    "renewable resource index {} out of bounds ({})",
                    r.index, tile_count
                )));
            }
            renewable_resources.push(RenewableResource {
                index: r.index,
                kind: u8_to_resource_kind(r.kind)?,
                capacity: r.capacity,
            });
        }

        let mut grid = Grid {
            width: self.world.width,
            height: self.world.height,
            tiles,
            region_ids: Vec::new(),
            regions: Vec::new(),
            resources,
            renewable_resources,
        };
        crate::regions::detect_regions(&mut grid);

        // 2. Reconstruct Entities
        let mut entities = Vec::with_capacity(self.entities.len());
        let mut previous_id = None;

        for e in self.entities {
            if let Some(prev) = previous_id {
                if e.id <= prev {
                    return Err(SnapshotError::CorruptedData(format!(
                        "entity ids must be strictly ascending: found {} after {}",
                        e.id, prev
                    )));
                }
            }
            if e.id >= self.scalars.next_entity_id {
                return Err(SnapshotError::CorruptedData(format!(
                    "entity id {} exceeds next_entity_id {}",
                    e.id, self.scalars.next_entity_id
                )));
            }
            if e.x >= grid.width || e.y >= grid.height {
                return Err(SnapshotError::CorruptedData(format!(
                    "entity {} location ({}, {}) is outside world dimensions ({}x{})",
                    e.id, e.x, e.y, grid.width, grid.height
                )));
            }
            previous_id = Some(e.id);

            let sex = match e.sex {
                0 => Sex::Female,
                1 => Sex::Male,
                other => {
                    return Err(SnapshotError::CorruptedData(format!(
                        "invalid sex discriminant: {}",
                        other
                    )))
                }
            };

            let activity = u32_to_activity(e.activity)?;
            let current_goal = match e.mind.current_goal {
                Some(g) => Some(u32_to_goal(g)?),
                None => None,
            };

            let mut current_plan = Vec::with_capacity(e.mind.current_plan.len());
            for act in &e.mind.current_plan {
                current_plan.push(dto_to_action(act)?);
            }

            let mut grief = Vec::with_capacity(e.mind.grief.len());
            for g in e.mind.grief {
                grief.push(GriefState {
                    deceased_id: g.deceased_id,
                    started_tick: g.started_tick,
                    ends_tick: g.ends_tick,
                    intensity: g.intensity,
                });
            }

            let mut known_resources = Vec::with_capacity(e.mind.memory.known_resources.len());
            for kr in e.mind.memory.known_resources {
                known_resources.push(KnownResource {
                    x: kr.x,
                    y: kr.y,
                    kind: u8_to_resource_kind(kr.kind)?,
                    last_seen_tick: kr.last_seen_tick,
                    estimated_amount: kr.estimated_amount,
                    failed_attempts: kr.failed_attempts,
                    avoid_until_tick: kr.avoid_until_tick,
                });
            }

            let known_chunks: BTreeSet<u32> = e.mind.memory.known_chunks.into_iter().collect();

            let mut failed_exploration = Vec::with_capacity(e.mind.memory.failed_exploration.len());
            for fe in e.mind.memory.failed_exploration {
                failed_exploration.push(FailedExploration {
                    chunk_index: fe.chunk_index,
                    retry_after_tick: fe.retry_after_tick,
                });
            }

            let mut known_entities = Vec::with_capacity(e.mind.memory.known_entities.len());
            for ke in e.mind.memory.known_entities {
                known_entities.push(KnownEntity {
                    id: ke.id,
                    first_seen_tick: ke.first_seen_tick,
                    last_seen_tick: ke.last_seen_tick,
                    last_seen_x: ke.last_seen_x,
                    last_seen_y: ke.last_seen_y,
                    observed_ticks: ke.observed_ticks,
                    affinity: ke.affinity,
                    last_interaction_tick: ke.last_interaction_tick,
                    interaction_count: ke.interaction_count,
                    seek_retry_after_tick: ke.seek_retry_after_tick,
                });
            }

            let mut conflict_history = Vec::with_capacity(e.mind.memory.conflict_history.len());
            for ch in e.mind.memory.conflict_history {
                conflict_history.push(ConflictRecord {
                    entity_id: ch.entity_id,
                    last_conflict_tick: ch.last_conflict_tick,
                });
            }

            let memory = Memory {
                known_resources,
                known_chunks,
                failed_exploration,
                known_entities,
                known_dead_entities: e.mind.memory.known_dead_entities,
                conflict_history,
            };

            let mind = Mind {
                perception_radius: e.mind.perception_radius,
                memory,
                current_goal,
                current_plan,
                plan_index: e.mind.plan_index,
                goal_since_tick: e.mind.goal_since_tick,
                utility_scores: Default::default(),
                decision_explanation: None,
                visible_entities: e.mind.visible_entities,
                grief,
            };

            let personality = Personality {
                curiosity: e.personality.curiosity,
                sociability: e.personality.sociability,
                cooperativeness: e.personality.cooperativeness,
                caution: e.personality.caution,
                persistence: e.personality.persistence,
            };

            let pregnancy = e.pregnancy.map(|p| Pregnancy {
                father_id: p.father_id,
                conceived_tick: p.conceived_tick,
                due_tick: p.due_tick,
            });

            entities.push(Entity {
                id: e.id,
                x: e.x,
                y: e.y,
                sex,
                lifespan_ticks: e.lifespan_ticks,
                hunger: e.hunger,
                health: e.health,
                age_ticks: e.age_ticks,
                path: e.path,
                path_index: e.path_index,
                activity,
                mind,
                pregnancy,
                postpartum_until_tick: e.postpartum_until_tick,
                movement_credit: e.movement_credit,
                mother_id: e.mother_id,
                father_id: e.father_id,
                caregiver_id: e.caregiver_id,
                partner_id: e.partner_id,
                household_id: e.household_id,
                personality,
                inventory: Inventory::from_raw(e.inventory_capacity, e.inventory_amounts),
                action_tick: e.action_tick,
            });
        }

        // 3. Reconstruct Households
        let mut households = Vec::with_capacity(self.households.len());
        let mut prev_household_id = None;

        for h in self.households {
            if let Some(prev) = prev_household_id {
                if h.id <= prev {
                    return Err(SnapshotError::CorruptedData(format!(
                        "household ids must be strictly ascending: found {} after {}",
                        h.id, prev
                    )));
                }
            }
            if h.id >= self.scalars.next_household_id {
                return Err(SnapshotError::CorruptedData(format!(
                    "household id {} exceeds next_household_id {}",
                    h.id, self.scalars.next_household_id
                )));
            }
            prev_household_id = Some(h.id);

            households.push(Household {
                id: h.id,
                formed_tick: h.formed_tick,
                dissolved_tick: h.dissolved_tick,
                inheritance: h.inheritance.map(|i| HouseholdInheritance {
                    resolved_tick: i.resolved_tick,
                    decedent_id: i.decedent_id,
                    heir_id: i.heir_id,
                    destination_household_id: i.destination_household_id,
                }),
                migration: h.migration.map(|m| HouseholdMigration {
                    started_tick: m.started_tick,
                    proposer_id: m.proposer_id,
                    target_x: m.target_x,
                    target_y: m.target_y,
                    completed_tick: m.completed_tick,
                }),
                residence_x: h.residence_x,
                residence_y: h.residence_y,
                storage: Inventory::from_raw(h.storage_capacity, h.storage_amounts),
            });
        }

        // 4. Reconstruct Genealogy
        let records: Vec<LineageRecord> = self
            .genealogy
            .records
            .into_iter()
            .map(|r| LineageRecord {
                entity_id: r.entity_id,
                mother_id: r.mother_id,
                father_id: r.father_id,
            })
            .collect();
        let genealogy = Genealogy::from_records(records);

        // 5. Reconstruct Events
        if self.events.next_id == 0 {
            return Err(SnapshotError::CorruptedData(
                "event next_id must be strictly positive".to_string(),
            ));
        }
        let mut event_deque = VecDeque::with_capacity(self.events.events.len());
        for ev in self.events.events {
            if ev.id == 0 {
                return Err(SnapshotError::CorruptedData(
                    "event id must be strictly positive".to_string(),
                ));
            }
            if let Some(caused_by) = ev.caused_by_event_id {
                if caused_by == 0 {
                    return Err(SnapshotError::CorruptedData(
                        "caused_by_event_id must be strictly positive".to_string(),
                    ));
                }
            }
            event_deque.push_back(SimulationEvent {
                id: EventId::new(ev.id),
                caused_by_event_id: ev.caused_by_event_id.map(EventId::new),
                tick: ev.tick,
                location: EventLocation {
                    x: ev.location_x,
                    y: ev.location_y,
                },
                actor_id: ev.actor_id,
                target_id: ev.target_id,
                related_entity_ids: ev.related_entity_ids,
                kind: u32_to_event_kind(ev.kind)?,
                cause: u32_to_event_cause(ev.cause)?,
                details: dto_to_event_details(&ev.details)?,
            });
        }
        let recent_events = RecentEventHistory::from_parts(
            event_deque,
            self.events.capacity,
            EventId::new(self.events.next_id),
            self.events.total_created,
        );

        // 6. Build Simulation Instance
        let mut simulation = Simulation {
            tick: self.scalars.tick,
            paused: self.header.paused,
            entities,
            population_cache: Vec::new(),
            caregiver_index: BTreeMap::new(),
            spatial_grid: SpatialGrid::default(),
            pathfinding_workspace: PathfindingWorkspace::new(),
            next_entity_id: self.scalars.next_entity_id,
            world_revision: self.scalars.world_revision,
            births: self.scalars.births,
            deaths: self.scalars.deaths,
            food_consumed: self.scalars.food_consumed,
            seed: self.scalars.seed,
            recent_events,
            genealogy,
            households,
            next_household_id: self.scalars.next_household_id,
        };

        // 7. Rebuild runtime indices
        simulation.rebuild_population_index(&grid);

        // 8. Verify State Hash
        let computed_hash = simulation.state_hash(&grid).to_string();
        if computed_hash != self.header.state_hash {
            return Err(SnapshotError::StateHashMismatch {
                expected: self.header.state_hash,
                actual: computed_hash,
            });
        }

        Ok((simulation, grid))
    }

    /// Serializes the snapshot to a compact JSON string.
    pub fn to_json(&self) -> Result<String, SnapshotError> {
        serde_json::to_string(self).map_err(|e| SnapshotError::JsonError(e.to_string()))
    }

    /// Serializes the snapshot to a formatted pretty JSON string.
    pub fn to_json_pretty(&self) -> Result<String, SnapshotError> {
        serde_json::to_string_pretty(self).map_err(|e| SnapshotError::JsonError(e.to_string()))
    }

    /// Deserializes a snapshot from a JSON string.
    pub fn from_json(json: &str) -> Result<Self, SnapshotError> {
        serde_json::from_str(json).map_err(|e| SnapshotError::JsonError(e.to_string()))
    }
}
