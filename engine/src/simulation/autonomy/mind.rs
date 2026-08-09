use crate::world::{Grid, ResourceKind};
use std::cmp::Reverse;
use std::collections::{BinaryHeap, HashSet};

const DEFAULT_PERCEPTION_RADIUS: u32 = 6;
const FAILED_TARGET_RETRY_TICKS: u64 = 120;

pub(super) const KNOWLEDGE_CHUNK_SIZE: u32 = 8;
pub(super) const FAILED_EXPLORATION_RETRY_TICKS: u64 = 240;
pub(in crate::simulation) const URGENT_HUNGER_THRESHOLD: f32 = 85.0;

type RememberedFoodTargets = BinaryHeap<Reverse<(u32, u64, (u32, u32))>>;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Goal {
    Eat,
    Explore,
    Follow,
    Rest,
}

impl Goal {
    pub fn label(self) -> &'static str {
        match self {
            Self::Eat => "Eat",
            Self::Explore => "Explore",
            Self::Follow => "Follow",
            Self::Rest => "Rest",
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Action {
    MoveTo(u32, u32),
    Consume(ResourceKind),
    ExploreArea(u32, u32),
    Wait,
}

impl Action {
    pub fn label(self) -> &'static str {
        match self {
            Self::MoveTo(_, _) => "Move to target",
            Self::Consume(_) => "Consume resource",
            Self::ExploreArea(_, _) => "Explore area",
            Self::Wait => "Wait",
        }
    }

    pub fn destination(self) -> Option<(u32, u32)> {
        match self {
            Self::MoveTo(x, y) | Self::ExploreArea(x, y) => Some((x, y)),
            Self::Consume(_) | Self::Wait => None,
        }
    }
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

impl Memory {
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
    pub explore: f32,
    pub rest: f32,
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
}
