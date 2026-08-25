use super::super::time::TICKS_PER_DAY;
use crate::world::{Grid, ResourceKind};
use std::cmp::Reverse;
use std::collections::{BinaryHeap, HashSet};

const DEFAULT_PERCEPTION_RADIUS: u32 = 6;
const FAILED_TARGET_RETRY_TICKS: u64 = 120;

pub(super) const KNOWLEDGE_CHUNK_SIZE: u32 = 8;
pub(super) const FAILED_EXPLORATION_RETRY_TICKS: u64 = 240;
pub(in crate::simulation) const URGENT_HUNGER_THRESHOLD: f32 = 85.0;

/// Ticks required to complete a gathering action.
pub(crate) const GATHER_DURATION_TICKS: u32 = 10;
/// Amount of resource gathered per successful gathering action.
pub(in crate::simulation) const GATHER_AMOUNT: u16 = 10;

type RememberedFoodTargets = BinaryHeap<Reverse<(u32, u64, (u32, u32))>>;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Goal {
    Eat,
    AcquireResource,
    Explore,
    Follow,
    MigrateHousehold,
    ProtectDependent,
    Rest,
    Socialize,
    ShareFood,
}

impl Goal {
    pub fn label(self) -> &'static str {
        match self {
            Self::Eat => "Eat",
            Self::AcquireResource => "Acquire Resource",
            Self::Explore => "Explore",
            Self::Follow => "Follow",
            Self::MigrateHousehold => "Migrate Household",
            Self::ProtectDependent => "Protect Dependent",
            Self::Rest => "Rest",
            Self::Socialize => "Socialize",
            Self::ShareFood => "Share Food",
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Action {
    MoveTo(u32, u32),
    Gather(ResourceKind),
    Consume(ResourceKind),
    ExploreArea(u32, u32),
    Wait,
    ApproachEntity(u32),
    Interact(u32),
    ShareFood(u32),
    DepositHouseholdFood(u16),
    WithdrawHouseholdFood(u16),
}

impl Action {
    pub fn label(self) -> &'static str {
        match self {
            Self::MoveTo(_, _) => "Move to target",
            Self::Gather(_) => "Gather resource",
            Self::Consume(_) => "Consume resource",
            Self::ExploreArea(_, _) => "Explore area",
            Self::Wait => "Wait",
            Self::ApproachEntity(_) => "Approach entity",
            Self::Interact(_) => "Interact",
            Self::ShareFood(_) => "Share food",
            Self::DepositHouseholdFood(_) => "Deposit household food",
            Self::WithdrawHouseholdFood(_) => "Withdraw household food",
        }
    }

    pub fn destination(self) -> Option<(u32, u32)> {
        match self {
            Self::MoveTo(x, y) | Self::ExploreArea(x, y) => Some((x, y)),
            Self::Gather(_)
            | Self::Consume(_)
            | Self::Wait
            | Self::ApproachEntity(_)
            | Self::Interact(_)
            | Self::ShareFood(_) => None,
            Self::DepositHouseholdFood(_) => None,
            Self::WithdrawHouseholdFood(_) => None,
        }
    }

    #[cfg(test)]
    pub fn target_entity_id(self) -> Option<u32> {
        match self {
            Self::ApproachEntity(id) | Self::Interact(id) | Self::ShareFood(id) => Some(id),
            _ => None,
        }
    }
}

const MIN_AFFINITY: i16 = -1_000;
pub(super) const NEUTRAL_AFFINITY: i16 = 0;
const MAX_AFFINITY: i16 = 1_000;
pub(super) const FAILED_SOCIAL_SEEK_RETRY_TICKS: u64 = 50;

/// A relationship begins cooling toward neutral after this many ticks
/// without an interaction (30 simulated days).
pub(in crate::simulation) const RELATIONSHIP_DECAY_START_TICKS: u64 = 30 * TICKS_PER_DAY;

/// Affinity moved toward zero per daily decay pass (1 point per day).
pub(in crate::simulation) const RELATIONSHIP_DECAY_PER_DAY: i16 = 1;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum AffinityBand {
    Hostile,
    Ordinary,
    Bonded,
}

pub(super) fn affinity_band(affinity: i16) -> AffinityBand {
    if affinity < -200 {
        AffinityBand::Hostile
    } else if affinity < 100 {
        AffinityBand::Ordinary
    } else {
        AffinityBand::Bonded
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(in crate::simulation) struct AffinityChangeRecord {
    pub target_id: u32,
    pub previous_affinity: i16,
    pub new_affinity: i16,
    pub delta: i16,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct KnownResource {
    pub x: u32,
    pub y: u32,
    pub kind: ResourceKind,
    pub last_seen_tick: u64,
    pub estimated_amount: u16,
    pub failed_attempts: u16,
    pub avoid_until_tick: u64,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct KnownEntity {
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

impl KnownEntity {
    pub(super) fn mark_failed_seek(&mut self, tick: u64) {
        self.seek_retry_after_tick = Some(tick.saturating_add(FAILED_SOCIAL_SEEK_RETRY_TICKS));
    }

    pub(super) fn clear_seek_cooldown(&mut self) {
        self.seek_retry_after_tick = None;
    }

    pub(super) fn seek_on_cooldown(self, tick: u64) -> bool {
        self.seek_retry_after_tick
            .is_some_and(|retry_after_tick| tick < retry_after_tick)
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct FailedExploration {
    chunk_index: u32,
    retry_after_tick: u64,
}

#[derive(Clone, Debug, Default)]
pub struct Memory {
    pub known_resources: Vec<KnownResource>,
    pub(super) known_chunks: HashSet<u32>,
    failed_exploration: Vec<FailedExploration>,
    pub known_entities: Vec<KnownEntity>,
}

/// Moves an affinity value toward zero by at most `amount`, never
/// crossing zero: +400 -> +399, -400 -> -399, 0 -> 0.
fn move_toward_zero(value: i16, amount: i16) -> i16 {
    if value > 0 {
        value.saturating_sub(amount).max(0)
    } else if value < 0 {
        value.saturating_add(amount).min(0)
    } else {
        0
    }
}

impl Memory {
    /// Slowly cools relationship affinity toward neutral for relationships
    /// with no recent interaction.
    ///
    /// Called at a low frequency (daily) by `Simulation`; it never runs
    /// per tick and touches only this memory's own known relationships.
    /// Each individual's affinity is mutated only from their own memory;
    /// neither individual reads the other's memory.
    pub(in crate::simulation) fn decay_relationships(
        &mut self,
        tick: u64,
    ) -> Vec<AffinityChangeRecord> {
        let mut changes = Vec::new();
        for known in &mut self.known_entities {
            if known.affinity == 0 {
                continue;
            }
            let time_since_last_interaction = tick.saturating_sub(known.last_interaction_tick);
            if time_since_last_interaction >= RELATIONSHIP_DECAY_START_TICKS {
                let previous_affinity = known.affinity;
                known.affinity = move_toward_zero(known.affinity, RELATIONSHIP_DECAY_PER_DAY);
                let new_affinity = known.affinity;
                if affinity_band(previous_affinity) != affinity_band(new_affinity) {
                    changes.push(AffinityChangeRecord {
                        target_id: known.id,
                        previous_affinity,
                        new_affinity,
                        delta: new_affinity.saturating_sub(previous_affinity),
                    });
                }
            }
        }
        changes
    }

    pub fn known_chunk_count(&self) -> usize {
        self.known_chunks.len()
    }

    pub fn remembers_chunk(&self, world: &Grid, x: u32, y: u32) -> bool {
        self.known_chunks.contains(&chunk_index(world, x, y))
    }

    pub(super) fn exploration_on_cooldown(&self, chunk_index: u32, tick: u64) -> bool {
        self.failed_exploration
            .iter()
            .any(|failure| failure.chunk_index == chunk_index && tick < failure.retry_after_tick)
    }

    pub(super) fn mark_exploration_failed(&mut self, chunk_index: u32, tick: u64) {
        let retry_after_tick = tick.saturating_add(FAILED_EXPLORATION_RETRY_TICKS);

        if let Some(failure) = self
            .failed_exploration
            .iter_mut()
            .find(|failure| failure.chunk_index == chunk_index)
        {
            failure.retry_after_tick = retry_after_tick;
        } else {
            self.failed_exploration.push(FailedExploration {
                chunk_index,
                retry_after_tick,
            });
        }
    }

    pub(super) fn prune_exploration_failures(&mut self, tick: u64) {
        self.failed_exploration
            .retain(|failure| tick < failure.retry_after_tick);
    }

    #[cfg(test)]
    pub(super) fn failed_exploration_count(&self) -> usize {
        self.failed_exploration.len()
    }

    pub fn affinity_to(&self, entity_id: u32) -> Option<i16> {
        self.known_entities
            .binary_search_by_key(&entity_id, |known| known.id)
            .ok()
            .map(|index| self.known_entities[index].affinity)
    }

    #[allow(dead_code, reason = "used by the upcoming social interaction system")]
    pub fn adjust_affinity(&mut self, entity_id: u32, delta: i16) -> bool {
        let Ok(index) = self
            .known_entities
            .binary_search_by_key(&entity_id, |known| known.id)
        else {
            return false;
        };

        let known = &mut self.known_entities[index];
        known.affinity = known
            .affinity
            .saturating_add(delta)
            .clamp(MIN_AFFINITY, MAX_AFFINITY);

        true
    }

    pub(super) fn record_interaction(
        &mut self,
        entity_id: u32,
        tick: u64,
        affinity_delta: i16,
    ) -> Option<Option<AffinityChangeRecord>> {
        let Ok(index) = self
            .known_entities
            .binary_search_by_key(&entity_id, |known| known.id)
        else {
            return None;
        };

        let known = &mut self.known_entities[index];
        let previous_affinity = known.affinity;

        known.affinity = known
            .affinity
            .saturating_add(affinity_delta)
            .clamp(MIN_AFFINITY, MAX_AFFINITY);
        let new_affinity = known.affinity;

        known.last_interaction_tick = tick;
        known.interaction_count = known.interaction_count.saturating_add(1);
        known.clear_seek_cooldown();

        Some(
            (affinity_band(previous_affinity) != affinity_band(new_affinity)).then_some(
                AffinityChangeRecord {
                    target_id: entity_id,
                    previous_affinity,
                    new_affinity,
                    delta: new_affinity.saturating_sub(previous_affinity),
                },
            ),
        )
    }

    pub(super) fn mark_failed_social_seek(&mut self, entity_id: u32, tick: u64) -> bool {
        let Ok(index) = self
            .known_entities
            .binary_search_by_key(&entity_id, |known| known.id)
        else {
            return false;
        };

        self.known_entities[index].mark_failed_seek(tick);
        true
    }

    pub fn forget_resource(&mut self, position: (u32, u32), kind: ResourceKind) {
        self.known_resources
            .retain(|known| (known.x, known.y) != position || known.kind != kind);
    }

    pub fn mark_unreachable(&mut self, position: (u32, u32), tick: u64) {
        if let Some(known) = self
            .known_resources
            .iter_mut()
            .find(|known| (known.x, known.y) == position)
        {
            known.failed_attempts = known.failed_attempts.saturating_add(1);
            known.avoid_until_tick = tick.saturating_add(FAILED_TARGET_RETRY_TICKS);
        }
    }

    pub fn update_known_amount(
        &mut self,
        position: (u32, u32),
        kind: ResourceKind,
        amount: u16,
        tick: u64,
    ) {
        if amount == 0 {
            self.forget_resource(position, kind);
            return;
        }
        if let Some(known) = self
            .known_resources
            .iter_mut()
            .find(|known| (known.x, known.y) == position && known.kind == kind)
        {
            known.estimated_amount = amount;
            known.last_seen_tick = tick;
            known.failed_attempts = 0;
            known.avoid_until_tick = 0;
        }
    }
}

#[derive(Clone, Copy, Debug, Default)]
pub struct UtilityScores {
    pub eat: f32,
    pub acquire_resource: f32,
    pub explore: f32,
    pub rest: f32,
    pub socialize: f32,
    pub share_food: f32,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum DecisionReason {
    HighestUtility,
    GoalPersistence,
    DependentNeedsFood,
    DependentFollowsCaregiver,
    DependentProvisioning,
    DependentProtection,
    HouseholdMigration,
}

impl DecisionReason {
    pub fn label(self) -> &'static str {
        match self {
            Self::HighestUtility => "highest_utility",
            Self::GoalPersistence => "goal_persistence",
            Self::DependentNeedsFood => "dependent_needs_food",
            Self::DependentFollowsCaregiver => "dependent_follows_caregiver",
            Self::DependentProvisioning => "dependent_provisioning",
            Self::DependentProtection => "dependent_protection",
            Self::HouseholdMigration => "household_migration",
        }
    }
}

#[derive(Clone, Copy, Debug)]
pub struct DecisionExplanation {
    pub chosen_goal: Goal,
    pub highest_utility_goal: Goal,
    pub chosen_score: f32,
    pub highest_score: f32,
    pub switch_margin: f32,
    pub reason: DecisionReason,
}

#[derive(Clone, Debug)]
pub struct Mind {
    pub perception_radius: u32,
    pub memory: Memory,
    pub current_goal: Option<Goal>,
    pub current_plan: Vec<Action>,
    pub plan_index: usize,
    pub goal_since_tick: u64,
    pub utility_scores: UtilityScores,
    pub decision_explanation: Option<DecisionExplanation>,
    pub visible_entities: Vec<u32>,
}

impl Default for Mind {
    fn default() -> Self {
        Self {
            perception_radius: DEFAULT_PERCEPTION_RADIUS,
            memory: Memory::default(),
            current_goal: None,
            current_plan: Vec::new(),
            plan_index: 0,
            goal_since_tick: 0,
            utility_scores: UtilityScores::default(),
            decision_explanation: None,
            visible_entities: Vec::new(),
        }
    }
}

impl Mind {
    pub fn current_action(&self) -> Option<Action> {
        self.current_plan.get(self.plan_index).copied()
    }

    pub fn advance_action(&mut self) {
        self.plan_index = self.plan_index.saturating_add(1);
    }

    pub fn clear_goal(&mut self) {
        self.current_goal = None;
        self.current_plan.clear();
        self.plan_index = 0;
    }

    pub fn set_plan(&mut self, goal: Goal, actions: Vec<Action>, tick: u64) {
        let goal_changed = self.current_goal != Some(goal);

        self.current_goal = Some(goal);
        self.current_plan = actions;
        self.plan_index = 0;

        if goal_changed {
            self.goal_since_tick = tick;
        }
    }

    pub fn remembered_food_targets(&self, origin: (u32, u32), tick: u64) -> RememberedFoodTargets {
        let targets: Vec<_> = self
            .memory
            .known_resources
            .iter()
            .filter(|known| {
                known.kind == ResourceKind::Food
                    && known.estimated_amount > 0
                    && tick >= known.avoid_until_tick
            })
            .map(|known| {
                let distance = manhattan(origin, (known.x, known.y));
                let age = tick.saturating_sub(known.last_seen_tick);
                Reverse((distance, age, (known.x, known.y)))
            })
            .collect();

        BinaryHeap::from(targets)
    }

    pub fn remembered_resource_targets(
        &self,
        origin: (u32, u32),
        tick: u64,
        kind: ResourceKind,
    ) -> RememberedFoodTargets {
        let targets: Vec<_> = self
            .memory
            .known_resources
            .iter()
            .filter(|known| {
                known.kind == kind && known.estimated_amount > 0 && tick >= known.avoid_until_tick
            })
            .map(|known| {
                let distance = manhattan(origin, (known.x, known.y));
                let age = tick.saturating_sub(known.last_seen_tick);
                Reverse((distance, age, (known.x, known.y)))
            })
            .collect();

        BinaryHeap::from(targets)
    }
}

pub(super) fn manhattan(left: (u32, u32), right: (u32, u32)) -> u32 {
    left.0.abs_diff(right.0) + left.1.abs_diff(right.1)
}

pub(super) fn chunk_index(world: &Grid, x: u32, y: u32) -> u32 {
    let chunks_wide = world.width.div_ceil(KNOWLEDGE_CHUNK_SIZE);
    (y / KNOWLEDGE_CHUNK_SIZE) * chunks_wide + x / KNOWLEDGE_CHUNK_SIZE
}

#[cfg(test)]
mod tests {
    use super::*;

    fn known_entity(id: u32) -> KnownEntity {
        KnownEntity {
            id,
            first_seen_tick: 0,
            last_seen_tick: 0,
            last_seen_x: 0,
            last_seen_y: 0,
            observed_ticks: 1,
            affinity: NEUTRAL_AFFINITY,
            last_interaction_tick: 0,
            interaction_count: 0,
            seek_retry_after_tick: None,
        }
    }

    #[test]
    fn affinity_adjustment_is_clamped() {
        let mut memory = Memory::default();
        memory.known_entities.push(known_entity(1));

        assert!(memory.adjust_affinity(1, 400));
        assert_eq!(memory.affinity_to(1), Some(400));

        assert!(memory.adjust_affinity(1, 900));
        assert_eq!(memory.affinity_to(1), Some(MAX_AFFINITY));

        assert!(memory.adjust_affinity(1, -3_000));
        assert_eq!(memory.affinity_to(1), Some(MIN_AFFINITY));
    }

    #[test]
    fn adjusting_unknown_affinity_returns_false() {
        let mut memory = Memory::default();

        assert!(!memory.adjust_affinity(99, 100));
        assert_eq!(memory.affinity_to(99), None);
    }

    #[test]
    fn affinity_adjustment_only_changes_target() {
        let mut memory = Memory::default();
        memory.known_entities.push(known_entity(1));
        memory.known_entities.push(known_entity(2));

        assert!(memory.adjust_affinity(1, 250));

        assert_eq!(memory.affinity_to(1), Some(250));
        assert_eq!(memory.affinity_to(2), Some(0));
    }

    #[test]
    fn replanning_same_goal_preserves_goal_since_tick() {
        let mut mind = Mind::default();

        mind.set_plan(Goal::Explore, vec![], 10);
        assert_eq!(mind.goal_since_tick, 10);

        mind.set_plan(Goal::Explore, vec![], 50);
        assert_eq!(mind.goal_since_tick, 10);

        mind.set_plan(Goal::Rest, vec![], 80);
        assert_eq!(mind.goal_since_tick, 80);
    }

    #[test]
    fn remembered_food_targets_are_ordered_by_distance_then_age() {
        let mut mind = Mind::default();

        mind.memory.known_resources.push(KnownResource {
            x: 10,
            y: 0,
            kind: ResourceKind::Food,
            last_seen_tick: 100,
            estimated_amount: 50,
            failed_attempts: 0,
            avoid_until_tick: 0,
        });
        mind.memory.known_resources.push(KnownResource {
            x: 2,
            y: 0,
            kind: ResourceKind::Food,
            last_seen_tick: 100,
            estimated_amount: 50,
            failed_attempts: 0,
            avoid_until_tick: 0,
        });
        mind.memory.known_resources.push(KnownResource {
            x: 5,
            y: 0,
            kind: ResourceKind::Food,
            last_seen_tick: 200,
            estimated_amount: 50,
            failed_attempts: 0,
            avoid_until_tick: 0,
        });

        let mut heap = mind.remembered_food_targets((0, 0), 300);
        let mut targets = Vec::new();
        while let Some(Reverse((distance, age, position))) = heap.pop() {
            targets.push((distance, age, position));
        }

        assert_eq!(targets.len(), 3);
        assert_eq!(targets[0].0, 2);
        assert_eq!(targets[1].0, 5);
        assert_eq!(targets[2].0, 10);

        mind.memory.known_resources.push(KnownResource {
            x: 8,
            y: 0,
            kind: ResourceKind::Food,
            last_seen_tick: 50,
            estimated_amount: 50,
            failed_attempts: 0,
            avoid_until_tick: 0,
        });
        mind.memory.known_resources.push(KnownResource {
            x: 0,
            y: 8,
            kind: ResourceKind::Food,
            last_seen_tick: 250,
            estimated_amount: 50,
            failed_attempts: 0,
            avoid_until_tick: 0,
        });

        let mut heap = mind.remembered_food_targets((0, 0), 300);
        let mut targets = Vec::new();
        while let Some(Reverse((distance, age, position))) = heap.pop() {
            targets.push((distance, age, position));
        }

        let distance_eight: Vec<_> = targets.iter().filter(|target| target.0 == 8).collect();
        assert_eq!(distance_eight.len(), 2);
        assert_eq!(distance_eight[0].1, 50);
        assert_eq!(distance_eight[1].1, 250);
    }

    // ── Relationship decay ────────────────────────────────────────────────

    #[test]
    fn recent_relationship_does_not_decay() {
        let tick = RELATIONSHIP_DECAY_START_TICKS + 10_000;
        let mut known = known_entity(1);
        known.affinity = 300;
        known.last_interaction_tick = tick - (RELATIONSHIP_DECAY_START_TICKS - 1);
        let mut mind = Mind::default();
        mind.memory.known_entities.push(known);

        let _ = mind.memory.decay_relationships(tick);

        assert_eq!(mind.memory.known_entities[0].affinity, 300);
    }

    #[test]
    fn old_positive_relationship_decays_toward_neutral() {
        let tick = RELATIONSHIP_DECAY_START_TICKS + 10_000;
        let mut known = known_entity(1);
        known.affinity = 400;
        known.last_interaction_tick = tick - RELATIONSHIP_DECAY_START_TICKS;
        let mut mind = Mind::default();
        mind.memory.known_entities.push(known);

        let _ = mind.memory.decay_relationships(tick);

        assert_eq!(
            mind.memory.known_entities[0].affinity,
            400 - RELATIONSHIP_DECAY_PER_DAY
        );
    }

    #[test]
    fn old_negative_relationship_decays_toward_neutral() {
        let tick = RELATIONSHIP_DECAY_START_TICKS + 10_000;
        let mut known = known_entity(1);
        known.affinity = -400;
        known.last_interaction_tick = tick - RELATIONSHIP_DECAY_START_TICKS;
        let mut mind = Mind::default();
        mind.memory.known_entities.push(known);

        let _ = mind.memory.decay_relationships(tick);

        assert_eq!(
            mind.memory.known_entities[0].affinity,
            -400 + RELATIONSHIP_DECAY_PER_DAY
        );
    }

    #[test]
    fn relationship_decay_never_crosses_zero() {
        let tick = RELATIONSHIP_DECAY_START_TICKS + 100;
        let cases = [
            (1i16, 0i16),
            (-1, 0),
            (0, 0),
            (RELATIONSHIP_DECAY_PER_DAY, 0),
            (-RELATIONSHIP_DECAY_PER_DAY, 0),
        ];

        for (affinity, expected) in cases {
            let mut known = known_entity(1);
            known.affinity = affinity;
            known.last_interaction_tick = 0;
            let mut mind = Mind::default();
            mind.memory.known_entities.push(known);

            let _ = mind.memory.decay_relationships(tick);

            assert_eq!(mind.memory.known_entities[0].affinity, expected);
        }
    }

    #[test]
    fn decay_accumulates_across_daily_passes_until_neutral() {
        let mut known = known_entity(1);
        known.affinity = 3;
        known.last_interaction_tick = 0;
        let mut mind = Mind::default();
        mind.memory.known_entities.push(known);

        let tick = RELATIONSHIP_DECAY_START_TICKS;
        let _ = mind.memory.decay_relationships(tick);
        assert_eq!(mind.memory.known_entities[0].affinity, 2);
        let _ = mind.memory.decay_relationships(tick + TICKS_PER_DAY);
        assert_eq!(mind.memory.known_entities[0].affinity, 1);
        let _ = mind.memory.decay_relationships(tick + 2 * TICKS_PER_DAY);
        assert_eq!(mind.memory.known_entities[0].affinity, 0);
        let _ = mind.memory.decay_relationships(tick + 3 * TICKS_PER_DAY);
        assert_eq!(mind.memory.known_entities[0].affinity, 0);
    }

    #[test]
    fn interaction_resets_decay_age() {
        let tick = RELATIONSHIP_DECAY_START_TICKS + 10_000;
        let mut known = known_entity(1);
        known.affinity = 300;
        known.last_interaction_tick = tick - 2 * RELATIONSHIP_DECAY_START_TICKS;
        let mut mind = Mind::default();
        mind.memory.known_entities.push(known);

        assert_eq!(mind.memory.record_interaction(1, tick, 0), Some(None));

        let _ = mind.memory.decay_relationships(tick);

        assert_eq!(mind.memory.known_entities[0].affinity, 300);
        assert_eq!(mind.memory.known_entities[0].last_interaction_tick, tick);
    }

    #[test]
    fn relationship_decay_is_deterministic() {
        let scenario = || {
            let decay_tick = 4 * RELATIONSHIP_DECAY_START_TICKS;
            let mut a = known_entity(1);
            a.affinity = 250;
            a.last_interaction_tick = decay_tick - RELATIONSHIP_DECAY_START_TICKS;
            let mut b = known_entity(2);
            b.affinity = -120;
            b.last_interaction_tick = 5;
            let mut mind = Mind::default();
            mind.memory.known_entities.push(a);
            mind.memory.known_entities.push(b);
            mind
        };

        let mut first = scenario();
        let mut second = scenario();
        let decay_tick = 5 * RELATIONSHIP_DECAY_START_TICKS;
        let first_changes = first.memory.decay_relationships(decay_tick);
        let second_changes = second.memory.decay_relationships(decay_tick);

        assert_eq!(first.memory.known_entities, second.memory.known_entities);
        assert_eq!(first_changes, second_changes);
    }

    #[test]
    fn affinity_band_uses_exact_boundaries() {
        assert_eq!(affinity_band(MIN_AFFINITY), AffinityBand::Hostile);
        assert_eq!(affinity_band(-201), AffinityBand::Hostile);
        assert_eq!(affinity_band(-200), AffinityBand::Ordinary);
        assert_eq!(affinity_band(99), AffinityBand::Ordinary);
        assert_eq!(affinity_band(100), AffinityBand::Bonded);
        assert_eq!(affinity_band(MAX_AFFINITY), AffinityBand::Bonded);
    }

    #[test]
    fn record_interaction_handles_unknown_relationship_and_zero_delta() {
        let mut memory = Memory::default();
        assert_eq!(memory.record_interaction(99, 10, 8), None);

        memory.known_entities.push(known_entity(1));
        assert_eq!(memory.record_interaction(1, 10, 0), Some(None));
        assert_eq!(memory.known_entities[0].affinity, 0);
        assert_eq!(memory.known_entities[0].last_interaction_tick, 10);
        assert_eq!(memory.known_entities[0].interaction_count, 1);
    }

    #[test]
    fn record_interaction_reports_clamped_actual_delta_and_multi_band_jump() {
        let mut positive = known_entity(1);
        positive.affinity = 99;
        let mut memory = Memory::default();
        memory.known_entities.push(positive);

        assert_eq!(
            memory.record_interaction(1, 10, i16::MAX),
            Some(Some(AffinityChangeRecord {
                target_id: 1,
                previous_affinity: 99,
                new_affinity: MAX_AFFINITY,
                delta: 901,
            }))
        );

        assert_eq!(
            memory.record_interaction(1, 11, i16::MIN),
            Some(Some(AffinityChangeRecord {
                target_id: 1,
                previous_affinity: MAX_AFFINITY,
                new_affinity: MIN_AFFINITY,
                delta: -2_000,
            }))
        );
    }

    #[test]
    fn record_interaction_reports_entering_and_leaving_bands() {
        let cases = [
            (-200, -1, -201, AffinityBand::Hostile),
            (-201, 1, -200, AffinityBand::Ordinary),
            (99, 1, 100, AffinityBand::Bonded),
            (100, -1, 99, AffinityBand::Ordinary),
        ];

        for (previous, requested_delta, expected, expected_band) in cases {
            let mut known = known_entity(1);
            known.affinity = previous;
            let mut memory = Memory::default();
            memory.known_entities.push(known);

            let change = memory
                .record_interaction(1, 10, requested_delta)
                .flatten()
                .expect("boundary crossing should be reported");
            assert_eq!(change.previous_affinity, previous);
            assert_eq!(change.new_affinity, expected);
            assert_eq!(change.delta, requested_delta);
            assert_eq!(affinity_band(change.new_affinity), expected_band);
        }
    }

    #[test]
    fn decay_relationships_reports_only_band_crossings() {
        let mut bonded = known_entity(1);
        bonded.affinity = 100;
        let mut hostile = known_entity(2);
        hostile.affinity = -201;
        let mut ordinary = known_entity(3);
        ordinary.affinity = 50;
        let mut memory = Memory::default();
        memory.known_entities = vec![bonded, hostile, ordinary];

        let changes = memory.decay_relationships(RELATIONSHIP_DECAY_START_TICKS);
        assert_eq!(
            changes,
            vec![
                AffinityChangeRecord {
                    target_id: 1,
                    previous_affinity: 100,
                    new_affinity: 99,
                    delta: -1,
                },
                AffinityChangeRecord {
                    target_id: 2,
                    previous_affinity: -201,
                    new_affinity: -200,
                    delta: 1,
                },
            ]
        );
        assert_eq!(memory.known_entities[2].affinity, 49);
    }
}
