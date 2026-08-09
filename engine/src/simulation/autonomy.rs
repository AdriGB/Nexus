use super::config::{BASE_MOVEMENT_SPEED, FOOD_SEARCH_THRESHOLD, MAX_HEALTH};
use super::spatial::{EntitySnapshot, SpatialGrid};
use super::{Entity, EntityActivity};
use crate::pathfinding::{self, PathfindingWorkspace};
use crate::world::{Grid, ResourceKind};
use std::cmp::Reverse;
use std::collections::{BinaryHeap, HashSet};
use web_time::Instant;

pub const DEFAULT_PERCEPTION_RADIUS: u32 = 6;
const KNOWLEDGE_CHUNK_SIZE: u32 = 8;
const RESOURCE_MEMORY_TTL: u64 = 2_000;
const FAILED_TARGET_RETRY_TICKS: u64 = 120;
const FAILED_EXPLORATION_RETRY_TICKS: u64 = 240;
const FOOD_CONSUMED_PER_MEAL: u16 = 10;
const HUNGER_REDUCTION_PER_MEAL: f32 = 50.0;
pub(super) const URGENT_HUNGER_THRESHOLD: f32 = 85.0;
const REST_HEALTH_PER_TICK: f32 = 0.25;
const PROFILE_SAMPLE_RATE: usize = 4;
type RememberedFoodTargets = BinaryHeap<Reverse<(u32, u64, (u32, u32))>>;

#[derive(Clone, Debug, Default)]
pub(crate) struct AutonomyProfile {
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
    pub known_resources_total: u32,
    pub known_resources_max: u32,
    pub visible_resources_seen: u32,
}

pub(super) fn update_entity(
    entity: &mut Entity,
    world: &mut Grid,
    tick: u64,
    population: &[EntitySnapshot],
    spatial_grid: &SpatialGrid,
    pathfinding_workspace: &mut PathfindingWorkspace,
) -> u16 {
    let position = (entity.x, entity.y);
    perceive(&mut entity.mind, world, position, tick);
    perceive_entities(
        &mut entity.mind,
        entity.id,
        position,
        population,
        spatial_grid,
    );
    invalidate_obsolete_food_plan(entity);

    let should_interrupt = entity.hunger >= URGENT_HUNGER_THRESHOLD
        && entity.mind.current_goal != Some(Goal::Eat)
        && !entity
            .mind
            .remembered_food_targets(position, tick)
            .is_empty();
    if should_interrupt {
        entity.mind.clear_goal();
        entity.path.clear();
        entity.path_index = 0;
    }

    if entity.mind.current_action().is_none() {
        entity.mind.clear_goal();
        let goal = evaluate_goals(&mut entity.mind, entity.hunger, entity.health);
        plan_goal(entity, world, tick, goal, pathfinding_workspace);
    }

    execute_current_action(entity, world, tick)
}

// Keep this behaviorally identical to update_entity().
// This duplicate exists only to keep profiling instrumentation
// out of the normal simulation hot path.
fn profiled_update_entity(
    entity: &mut Entity,
    world: &mut Grid,
    tick: u64,
    population: &[EntitySnapshot],
    spatial_grid: &SpatialGrid,
    pathfinding_workspace: &mut PathfindingWorkspace,
    profile: &mut AutonomyProfile,
) -> u16 {
    let position = (entity.x, entity.y);

    let start = Instant::now();
    reconcile_resource_memory(&mut entity.mind, world, position, tick);
    let reconciliation_us = start.elapsed().as_micros() as u64;
    profile.memory_reconciliation_us += reconciliation_us;

    let start = Instant::now();
    let visible_count = scan_visible_resources(&mut entity.mind, world, position, tick);
    let visible_scan_us = start.elapsed().as_micros() as u64;
    profile.visible_scan_us += visible_scan_us;
    profile.visible_resources_seen += visible_count;
    profile.resource_perception_us += reconciliation_us + visible_scan_us;

    let known_resources_count = entity.mind.memory.known_resources.len() as u32;
    profile.known_resources_total += known_resources_count;
    profile.known_resources_max = profile.known_resources_max.max(known_resources_count);

    let start = Instant::now();
    perceive_entities(
        &mut entity.mind,
        entity.id,
        position,
        population,
        spatial_grid,
    );
    profile.entity_perception_us += start.elapsed().as_micros() as u64;

    let start = Instant::now();
    invalidate_obsolete_food_plan(entity);

    let should_interrupt = entity.hunger >= URGENT_HUNGER_THRESHOLD
        && entity.mind.current_goal != Some(Goal::Eat)
        && !entity
            .mind
            .remembered_food_targets(position, tick)
            .is_empty();
    if should_interrupt {
        profile.urgent_interrupts += 1;
        entity.mind.clear_goal();
        entity.path.clear();
        entity.path_index = 0;
    }
    profile.plan_validation_us += start.elapsed().as_micros() as u64;

    if entity.mind.current_action().is_none() {
        profile.planned_entities += 1;

        let start = Instant::now();
        entity.mind.clear_goal();
        let goal = evaluate_goals(&mut entity.mind, entity.hunger, entity.health);
        plan_goal(entity, world, tick, goal, pathfinding_workspace);
        profile.planning_us += start.elapsed().as_micros() as u64;
    }

    let start = Instant::now();
    let consumed = execute_current_action(entity, world, tick);
    profile.action_us += start.elapsed().as_micros() as u64;
    profile.sampled_entities += 1;

    consumed
}

pub(crate) fn profile_autonomy(
    entities: &mut [Entity],
    world: &mut Grid,
    tick: u64,
    population: &[EntitySnapshot],
    spatial_grid: &SpatialGrid,
    pathfinding_workspace: &mut PathfindingWorkspace,
) -> (u64, AutonomyProfile) {
    let mut profile = AutonomyProfile::default();
    let mut consumed = 0u64;

    for (index, entity) in entities
        .iter_mut()
        .filter(|entity| entity.health > 0.0)
        .enumerate()
    {
        if index % PROFILE_SAMPLE_RATE == 0 {
            consumed += u64::from(profiled_update_entity(
                entity,
                world,
                tick,
                population,
                spatial_grid,
                pathfinding_workspace,
                &mut profile,
            ));
        } else {
            consumed += u64::from(update_entity(
                entity,
                world,
                tick,
                population,
                spatial_grid,
                pathfinding_workspace,
            ));
        }
    }

    (consumed, profile)
}

fn invalidate_obsolete_food_plan(entity: &mut Entity) {
    if entity.mind.current_goal != Some(Goal::Eat) {
        return;
    }
    let Some(target) = entity
        .mind
        .current_plan
        .iter()
        .find_map(|action| action.destination())
    else {
        return;
    };
    let still_remembered = entity.mind.memory.known_resources.iter().any(|known| {
        known.kind == ResourceKind::Food
            && known.estimated_amount > 0
            && (known.x, known.y) == target
    });
    if !still_remembered {
        entity.mind.clear_goal();
        entity.path.clear();
        entity.path_index = 0;
    }
}

fn plan_goal(
    entity: &mut Entity,
    world: &Grid,
    tick: u64,
    goal: Goal,
    pathfinding_workspace: &mut PathfindingWorkspace,
) {
    let origin = (entity.x, entity.y);
    match goal {
        Goal::Eat => {
            let mut targets = entity.mind.remembered_food_targets(origin, tick);
            while let Some(Reverse((_, _, target))) = targets.pop() {
                if let Some(path) = pathfinding::find_path_with_workspace(
                    pathfinding_workspace,
                    world,
                    origin,
                    target,
                ) {
                    entity.path = path.into_iter().skip(1).collect();
                    entity.path_index = 0;
                    let mut actions = Vec::new();
                    if target != origin {
                        actions.push(Action::MoveTo(target.0, target.1));
                    }
                    actions.push(Action::Consume(ResourceKind::Food));
                    entity.mind.set_plan(Goal::Eat, actions, tick);
                    entity.activity = EntityActivity::SeekingFood;
                    return;
                }
                entity.mind.memory.mark_unreachable(target, tick);
            }
            plan_exploration(entity, world, tick, pathfinding_workspace);
        }
        Goal::Explore => plan_exploration(entity, world, tick, pathfinding_workspace),
        Goal::Rest => {
            entity.mind.set_plan(Goal::Rest, vec![Action::Wait], tick);
            entity.activity = EntityActivity::Resting;
        }
    }
}

fn plan_exploration(
    entity: &mut Entity,
    world: &Grid,
    tick: u64,
    pathfinding_workspace: &mut PathfindingWorkspace,
) {
    let origin = (entity.x, entity.y);
    entity.mind.memory.prune_exploration_failures(tick);

    let Some(target) = exploration_target(&entity.mind, world, origin, entity.id, tick) else {
        entity.mind.set_plan(Goal::Rest, vec![Action::Wait], tick);
        entity.activity = EntityActivity::Resting;
        return;
    };
    let Some(path) =
        pathfinding::find_path_with_workspace(pathfinding_workspace, world, origin, target)
    else {
        let failed_chunk = chunk_index(world, target.0, target.1);
        entity
            .mind
            .memory
            .mark_exploration_failed(failed_chunk, tick);
        entity.mind.set_plan(Goal::Rest, vec![Action::Wait], tick);
        entity.activity = EntityActivity::Resting;
        return;
    };
    entity.path = path.into_iter().skip(1).collect();
    entity.path_index = 0;
    if entity.path.is_empty() {
        entity.mind.set_plan(Goal::Rest, vec![Action::Wait], tick);
        entity.activity = EntityActivity::Resting;
    } else {
        entity.mind.set_plan(
            Goal::Explore,
            vec![Action::ExploreArea(target.0, target.1)],
            tick,
        );
        entity.activity = EntityActivity::Exploring;
    }
}

fn execute_current_action(entity: &mut Entity, world: &mut Grid, tick: u64) -> u16 {
    let Some(action) = entity.mind.current_action() else {
        entity.activity = EntityActivity::Idle;
        return 0;
    };
    match action {
        Action::MoveTo(_, _) | Action::ExploreArea(_, _) => {
            entity.movement_credit += BASE_MOVEMENT_SPEED;

            if entity.path_index < entity.path.len() {
                let next = entity.path[entity.path_index];

                let Some(step_cost) = pathfinding::step_cost(world, (entity.x, entity.y), next)
                else {
                    entity.movement_credit = 0.0;
                    entity.mind.clear_goal();
                    entity.path.clear();
                    entity.path_index = 0;
                    return 0;
                };

                if entity.movement_credit >= step_cost {
                    entity.movement_credit -= step_cost;
                    entity.x = next.0;
                    entity.y = next.1;
                    entity.path_index += 1;
                    entity.activity = if entity.mind.current_goal == Some(Goal::Explore) {
                        EntityActivity::Exploring
                    } else {
                        EntityActivity::Moving
                    };
                }
            }
            if entity.path_index >= entity.path.len() {
                entity.movement_credit = 0.0;
                entity.path.clear();
                entity.path_index = 0;
                entity.mind.advance_action();
                if action.destination().is_some() && entity.mind.current_goal == Some(Goal::Explore)
                {
                    entity.mind.clear_goal();
                    entity.activity = EntityActivity::Idle;
                }
            }
            0
        }
        Action::Consume(kind) => {
            entity.movement_credit = 0.0;
            let consumed = consume_food(entity, world);
            let position = (entity.x, entity.y);
            let amount = world.resources[(entity.y * world.width + entity.x) as usize]
                .filter(|deposit| deposit.kind == kind)
                .map_or(0, |deposit| deposit.amount);
            entity
                .mind
                .memory
                .update_known_amount(position, kind, amount, tick);
            entity.mind.advance_action();
            entity.mind.clear_goal();
            entity.activity = EntityActivity::Idle;
            consumed
        }
        Action::Wait => {
            entity.movement_credit = 0.0;
            if entity.hunger < FOOD_SEARCH_THRESHOLD {
                entity.health = (entity.health + REST_HEALTH_PER_TICK).min(MAX_HEALTH);
            }
            entity.mind.advance_action();
            entity.mind.clear_goal();
            entity.activity = EntityActivity::Resting;
            0
        }
    }
}

fn consume_food(entity: &mut Entity, world: &mut Grid) -> u16 {
    let index = (entity.y * world.width + entity.x) as usize;
    let Some(slot) = world.resources.get_mut(index) else {
        return 0;
    };
    let Some(deposit) = slot.as_mut() else {
        return 0;
    };
    if deposit.kind != ResourceKind::Food {
        return 0;
    }

    let consumed = deposit.amount.min(FOOD_CONSUMED_PER_MEAL);
    deposit.amount -= consumed;
    let meal_fraction = f32::from(consumed) / f32::from(FOOD_CONSUMED_PER_MEAL);
    entity.hunger = (entity.hunger - HUNGER_REDUCTION_PER_MEAL * meal_fraction).max(0.0);
    if deposit.amount == 0 {
        *slot = None;
    }
    consumed
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Goal {
    Eat,
    Explore,
    Rest,
}

impl Goal {
    pub fn label(self) -> &'static str {
        match self {
            Self::Eat => "Eat",
            Self::Explore => "Explore",
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
struct FailedExploration {
    chunk_index: u32,
    retry_after_tick: u64,
}

#[derive(Clone, Debug, Default)]
pub struct Memory {
    pub known_resources: Vec<KnownResource>,
    known_chunks: HashSet<u32>,
    failed_exploration: Vec<FailedExploration>,
}

impl Memory {
    pub fn known_chunk_count(&self) -> usize {
        self.known_chunks.len()
    }

    pub fn remembers_chunk(&self, world: &Grid, x: u32, y: u32) -> bool {
        self.known_chunks.contains(&chunk_index(world, x, y))
    }

    fn exploration_on_cooldown(&self, chunk_index: u32, tick: u64) -> bool {
        self.failed_exploration
            .iter()
            .any(|failure| failure.chunk_index == chunk_index && tick < failure.retry_after_tick)
    }

    fn mark_exploration_failed(&mut self, chunk_index: u32, tick: u64) {
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

    fn prune_exploration_failures(&mut self, tick: u64) {
        self.failed_exploration
            .retain(|failure| tick < failure.retry_after_tick);
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
        self.current_goal = Some(goal);
        self.current_plan = actions;
        self.plan_index = 0;
        self.goal_since_tick = tick;
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

fn remember_visible_chunks(mind: &mut Mind, world: &Grid, position: (u32, u32)) {
    let radius = mind.perception_radius;

    let min_x = position.0.saturating_sub(radius);
    let max_x = position.0.saturating_add(radius).min(world.width - 1);
    let min_y = position.1.saturating_sub(radius);
    let max_y = position.1.saturating_add(radius).min(world.height - 1);

    let min_chunk_x = min_x / KNOWLEDGE_CHUNK_SIZE;
    let max_chunk_x = max_x / KNOWLEDGE_CHUNK_SIZE;
    let min_chunk_y = min_y / KNOWLEDGE_CHUNK_SIZE;
    let max_chunk_y = max_y / KNOWLEDGE_CHUNK_SIZE;

    let chunks_wide = world.width.div_ceil(KNOWLEDGE_CHUNK_SIZE);

    for chunk_y in min_chunk_y..=max_chunk_y {
        for chunk_x in min_chunk_x..=max_chunk_x {
            let chunk_min_x = chunk_x * KNOWLEDGE_CHUNK_SIZE;
            let chunk_min_y = chunk_y * KNOWLEDGE_CHUNK_SIZE;
            let chunk_max_x = (chunk_min_x + KNOWLEDGE_CHUNK_SIZE - 1).min(world.width - 1);
            let chunk_max_y = (chunk_min_y + KNOWLEDGE_CHUNK_SIZE - 1).min(world.height - 1);

            let nearest_x = position.0.clamp(chunk_min_x, chunk_max_x);
            let nearest_y = position.1.clamp(chunk_min_y, chunk_max_y);

            if manhattan(position, (nearest_x, nearest_y)) <= radius {
                let index = chunk_y * chunks_wide + chunk_x;
                mind.memory.known_chunks.insert(index);
            }
        }
    }
}

pub fn perceive(mind: &mut Mind, world: &Grid, position: (u32, u32), tick: u64) {
    reconcile_resource_memory(mind, world, position, tick);
    scan_visible_resources(mind, world, position, tick);
}

fn reconcile_resource_memory(mind: &mut Mind, world: &Grid, position: (u32, u32), tick: u64) {
    // 1. TTL expiration — cheap O(K), no world access.
    mind.memory
        .known_resources
        .retain(|known| tick.saturating_sub(known.last_seen_tick) <= RESOURCE_MEMORY_TTL);

    // 2. Depletion reconciliation — only within the visible y-band.
    //    known_resources is sorted by (y, x, kind), so partition_point
    //    gives us the local subset without scanning all K entries.
    let radius = mind.perception_radius;
    let min_y = position.1.saturating_sub(radius);
    let max_y = position
        .1
        .saturating_add(radius)
        .min(world.height.saturating_sub(1));

    let start = mind
        .memory
        .known_resources
        .partition_point(|known| known.y < min_y);
    let end = mind
        .memory
        .known_resources
        .partition_point(|known| known.y <= max_y);

    let mut depleted = Vec::new();

    for index in start..end {
        let known = mind.memory.known_resources[index];

        if manhattan(position, (known.x, known.y)) > radius {
            continue;
        }

        let world_index = (known.y * world.width + known.x) as usize;

        if world.resources.get(world_index).is_none_or(Option::is_none) {
            depleted.push(index);
        }
    }

    for index in depleted.into_iter().rev() {
        mind.memory.known_resources.remove(index);
    }
}

fn scan_visible_resources(mind: &mut Mind, world: &Grid, position: (u32, u32), tick: u64) -> u32 {
    let radius = mind.perception_radius as i64;
    let min_x = (i64::from(position.0) - radius).max(0) as u32;
    let max_x = (i64::from(position.0) + radius).min(i64::from(world.width) - 1) as u32;
    let min_y = (i64::from(position.1) - radius).max(0) as u32;
    let max_y = (i64::from(position.1) + radius).min(i64::from(world.height) - 1) as u32;

    remember_visible_chunks(mind, world, position);

    let mut visible_count = 0u32;
    for y in min_y..=max_y {
        for x in min_x..=max_x {
            if manhattan(position, (x, y)) > mind.perception_radius {
                continue;
            }
            let deposit = world.resources[(y * world.width + x) as usize];
            if let Some(deposit) = deposit {
                remember_resource(mind, x, y, deposit.kind, deposit.amount, tick);
                visible_count += 1;
            }
        }
    }
    visible_count
}

pub fn perceive_entities(
    mind: &mut Mind,
    entity_id: u32,
    position: (u32, u32),
    population: &[EntitySnapshot],
    spatial_grid: &SpatialGrid,
) {
    mind.visible_entities.clear();

    spatial_grid.visit_candidates(
        position.0,
        position.1,
        mind.perception_radius,
        |snapshot_index| {
            let other = population[snapshot_index];

            if other.id == entity_id {
                return;
            }

            if manhattan(position, (other.x, other.y)) <= mind.perception_radius {
                mind.visible_entities.push(other.id);
            }
        },
    );

    mind.visible_entities.sort_unstable();
}

pub fn evaluate_goals(mind: &mut Mind, hunger: f32, health: f32) -> Goal {
    let food_confidence = if mind
        .memory
        .known_resources
        .iter()
        .any(|known| known.kind == ResourceKind::Food && known.estimated_amount > 0)
    {
        1.0
    } else {
        0.25
    };
    let hunger_ratio = (hunger / 100.0).clamp(0.0, 1.0);
    let health_deficit = (1.0 - health / 100.0).clamp(0.0, 1.0);
    mind.utility_scores = UtilityScores {
        eat: hunger_ratio * (0.65 + 0.35 * food_confidence),
        explore: (1.0 - hunger_ratio) * 0.55 + (1.0 - food_confidence) * 0.2,
        rest: health_deficit * 0.8 + 0.05,
    };

    let scores = [
        (mind.utility_scores.eat, Goal::Eat),
        (mind.utility_scores.explore, Goal::Explore),
        (mind.utility_scores.rest, Goal::Rest),
    ];
    scores
        .into_iter()
        .max_by(|left, right| left.0.total_cmp(&right.0))
        .map_or(Goal::Explore, |(_, goal)| goal)
}

pub fn exploration_target(
    mind: &Mind,
    world: &Grid,
    origin: (u32, u32),
    entity_id: u32,
    tick: u64,
) -> Option<(u32, u32)> {
    let chunks_wide = world.width.div_ceil(KNOWLEDGE_CHUNK_SIZE);
    let chunks_high = world.height.div_ceil(KNOWLEDGE_CHUNK_SIZE);
    let origin_chunk = (
        origin.0 / KNOWLEDGE_CHUNK_SIZE,
        origin.1 / KNOWLEDGE_CHUNK_SIZE,
    );
    let origin_region = world.region_id_at(origin.0, origin.1);
    let max_ring = origin_chunk
        .0
        .max(chunks_wide - 1 - origin_chunk.0)
        .max(origin_chunk.1)
        .max(chunks_high - 1 - origin_chunk.1);

    for ring in 1..=max_ring {
        let mut chunks = ring_perimeter(
            origin_chunk.0 as i32,
            origin_chunk.1 as i32,
            ring as i32,
            chunks_wide as i32,
            chunks_high as i32,
        );
        chunks.sort_unstable_by_key(|&(cx, cy)| {
            let index = cy * chunks_wide + cx;
            index.wrapping_add(entity_id.wrapping_mul(2_654_435_761))
        });
        for (cx, cy) in chunks {
            let candidate_chunk = cy * chunks_wide + cx;
            if mind.memory.exploration_on_cooldown(candidate_chunk, tick) {
                continue;
            }

            let x = (cx * KNOWLEDGE_CHUNK_SIZE + KNOWLEDGE_CHUNK_SIZE / 2).min(world.width - 1);
            let y = (cy * KNOWLEDGE_CHUNK_SIZE + KNOWLEDGE_CHUNK_SIZE / 2).min(world.height - 1);
            if mind.memory.remembers_chunk(world, x, y) {
                continue;
            }

            if let Some(target) = walkable_in_chunk(world, cx, cy, origin_region) {
                return Some(target);
            }
        }
    }

    deterministic_wander_target(mind, world, origin, entity_id, tick, origin_region)
}

fn ring_perimeter(
    center_x: i32,
    center_y: i32,
    ring: i32,
    chunks_wide: i32,
    chunks_high: i32,
) -> Vec<(u32, u32)> {
    debug_assert!(ring > 0);

    let mut chunks = Vec::with_capacity((ring as usize).saturating_mul(8));

    for dx in -ring..=ring {
        for y in [center_y - ring, center_y + ring] {
            let x = center_x + dx;

            if x >= 0 && y >= 0 && x < chunks_wide && y < chunks_high {
                chunks.push((x as u32, y as u32));
            }
        }
    }

    for dy in (-ring + 1)..ring {
        for x in [center_x - ring, center_x + ring] {
            let y = center_y + dy;

            if x >= 0 && y >= 0 && x < chunks_wide && y < chunks_high {
                chunks.push((x as u32, y as u32));
            }
        }
    }

    chunks
}

fn resource_key(x: u32, y: u32, kind: ResourceKind) -> (u32, u32, u8) {
    (y, x, kind as u8)
}

fn remember_resource(mind: &mut Mind, x: u32, y: u32, kind: ResourceKind, amount: u16, tick: u64) {
    let key = resource_key(x, y, kind);

    match mind
        .memory
        .known_resources
        .binary_search_by_key(&key, |known| resource_key(known.x, known.y, known.kind))
    {
        Ok(index) => {
            let known = &mut mind.memory.known_resources[index];
            known.last_seen_tick = tick;
            known.estimated_amount = amount;
            known.failed_attempts = 0;
            known.avoid_until_tick = 0;
        }
        Err(index) => {
            mind.memory.known_resources.insert(
                index,
                KnownResource {
                    x,
                    y,
                    kind,
                    last_seen_tick: tick,
                    estimated_amount: amount,
                    failed_attempts: 0,
                    avoid_until_tick: 0,
                },
            );
        }
    }
}

fn walkable_in_chunk(
    world: &Grid,
    chunk_x: u32,
    chunk_y: u32,
    required_region: Option<u32>,
) -> Option<(u32, u32)> {
    let start_x = chunk_x * KNOWLEDGE_CHUNK_SIZE;
    let start_y = chunk_y * KNOWLEDGE_CHUNK_SIZE;
    let end_x = (start_x + KNOWLEDGE_CHUNK_SIZE).min(world.width);
    let end_y = (start_y + KNOWLEDGE_CHUNK_SIZE).min(world.height);
    (start_y..end_y)
        .flat_map(|y| (start_x..end_x).map(move |x| (x, y)))
        .find(|&(x, y)| {
            let walkable = world
                .get(x, y)
                .is_some_and(|tile| tile.terrain.is_walkable());

            if !walkable {
                return false;
            }

            match required_region {
                Some(region_id) => world.region_id_at(x, y) == Some(region_id),
                None => true,
            }
        })
}

fn deterministic_wander_target(
    mind: &Mind,
    world: &Grid,
    origin: (u32, u32),
    entity_id: u32,
    tick: u64,
    required_region: Option<u32>,
) -> Option<(u32, u32)> {
    let offsets = [
        (8i32, 0i32),
        (0, 8),
        (-8, 0),
        (0, -8),
        (6, 6),
        (-6, 6),
        (6, -6),
        (-6, -6),
    ];
    let start = entity_id as usize % offsets.len();
    offsets
        .iter()
        .cycle()
        .skip(start)
        .take(offsets.len())
        .filter_map(|&(dx, dy)| {
            let x = i64::from(origin.0) + i64::from(dx);
            let y = i64::from(origin.1) + i64::from(dy);
            (x >= 0 && y >= 0 && x < i64::from(world.width) && y < i64::from(world.height))
                .then_some((x as u32, y as u32))
        })
        .find(|&(x, y)| {
            let walkable = world
                .get(x, y)
                .is_some_and(|tile| tile.terrain.is_walkable());
            let in_required_region = match required_region {
                Some(region_id) => world.region_id_at(x, y) == Some(region_id),
                None => true,
            };
            let target_chunk = chunk_index(world, x, y);

            walkable
                && in_required_region
                && !mind.memory.exploration_on_cooldown(target_chunk, tick)
        })
}

fn chunk_index(world: &Grid, x: u32, y: u32) -> u32 {
    let chunks_wide = world.width.div_ceil(KNOWLEDGE_CHUNK_SIZE);
    (y / KNOWLEDGE_CHUNK_SIZE) * chunks_wide + x / KNOWLEDGE_CHUNK_SIZE
}

fn manhattan(left: (u32, u32), right: (u32, u32)) -> u32 {
    left.0.abs_diff(right.0) + left.1.abs_diff(right.1)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::world::{ResourceDeposit, Terrain, Tile};
    use std::collections::HashSet;

    fn plain_grid(width: u32, height: u32) -> Grid {
        Grid {
            width,
            height,
            tiles: (0..width * height)
                .map(|_| Tile {
                    terrain: Terrain::Plains,
                    altitude: 0.0,
                    moisture: 0.5,
                    temperature: 0.5,
                })
                .collect(),
            region_ids: Vec::new(),
            regions: Vec::new(),
            resources: vec![None; (width * height) as usize],
        }
    }

    #[test]
    fn ring_perimeter_contains_exactly_eight_r_tiles_when_unclipped() {
        let ring = ring_perimeter(5, 5, 3, 20, 20);

        assert_eq!(ring.len(), 24);
        let unique: HashSet<_> = ring.iter().copied().collect();
        assert_eq!(unique.len(), ring.len());
    }

    #[test]
    fn batched_visible_chunks_match_tile_by_tile_reference() {
        let world = plain_grid(32, 32);

        for position in [(1, 1), (7, 7), (8, 8), (15, 15), (30, 30)] {
            let mut mind = Mind::default();
            let radius = mind.perception_radius;
            let mut expected = HashSet::new();

            for y in 0..world.height {
                for x in 0..world.width {
                    if manhattan(position, (x, y)) <= radius {
                        expected.insert(chunk_index(&world, x, y));
                    }
                }
            }

            remember_visible_chunks(&mut mind, &world, position);

            assert_eq!(
                mind.memory.known_chunks, expected,
                "mismatch at position {:?}",
                position,
            );
        }
    }

    #[test]
    fn ring_perimeter_clips_to_world_without_duplicates() {
        let ring = ring_perimeter(0, 0, 3, 10, 10);

        assert!(ring.iter().all(|&(x, y)| x < 10 && y < 10));
        let unique: HashSet<_> = ring.iter().copied().collect();
        assert_eq!(unique.len(), ring.len());
    }

    #[test]
    fn failed_exploration_chunk_is_skipped_until_retry_tick() {
        let world = plain_grid(32, 32);
        let mut mind = Mind::default();
        let origin = (12, 12);
        let entity_id = 1;
        let first = exploration_target(&mind, &world, origin, entity_id, 0).unwrap();
        let failed_chunk = chunk_index(&world, first.0, first.1);

        mind.memory.mark_exploration_failed(failed_chunk, 0);
        mind.memory.mark_exploration_failed(failed_chunk, 0);
        assert_eq!(mind.memory.failed_exploration.len(), 1);

        let during_cooldown = exploration_target(&mind, &world, origin, entity_id, 1).unwrap();
        let after_cooldown = exploration_target(
            &mind,
            &world,
            origin,
            entity_id,
            FAILED_EXPLORATION_RETRY_TICKS,
        )
        .unwrap();

        assert_ne!(
            chunk_index(&world, during_cooldown.0, during_cooldown.1),
            failed_chunk
        );
        assert_eq!(
            chunk_index(&world, after_cooldown.0, after_cooldown.1),
            failed_chunk
        );
    }

    #[test]
    fn perception_forgets_visible_depleted_resource() {
        let mut world = plain_grid(16, 16);
        let position = (5, 5);
        let index = (position.1 * world.width + position.0) as usize;

        world.resources[index] = Some(ResourceDeposit {
            kind: ResourceKind::Food,
            amount: 100,
        });

        let mut mind = Mind::default();
        perceive(&mut mind, &world, position, 10);

        assert!(mind
            .memory
            .known_resources
            .iter()
            .any(|known| (known.x, known.y) == position));

        world.resources[index] = None;
        perceive(&mut mind, &world, position, 11);

        assert!(!mind
            .memory
            .known_resources
            .iter()
            .any(|known| (known.x, known.y) == position));
    }

    #[test]
    fn perception_keeps_resource_outside_current_view() {
        let mut world = plain_grid(32, 32);
        let resource_position = (20, 20);
        let index = (resource_position.1 * world.width + resource_position.0) as usize;

        world.resources[index] = Some(ResourceDeposit {
            kind: ResourceKind::Food,
            amount: 100,
        });

        let mut mind = Mind::default();
        perceive(&mut mind, &world, resource_position, 10);

        world.resources[index] = None;
        perceive(&mut mind, &world, (5, 5), 11);

        assert!(mind
            .memory
            .known_resources
            .iter()
            .any(|known| (known.x, known.y) == resource_position));
    }

    #[test]
    fn perception_refreshes_visible_resource() {
        let mut world = plain_grid(16, 16);
        let position = (5, 5);
        let index = (position.1 * world.width + position.0) as usize;

        world.resources[index] = Some(ResourceDeposit {
            kind: ResourceKind::Food,
            amount: 100,
        });

        let mut mind = Mind::default();
        perceive(&mut mind, &world, position, 10);

        world.resources[index] = Some(ResourceDeposit {
            kind: ResourceKind::Food,
            amount: 40,
        });
        perceive(&mut mind, &world, position, 25);

        let known = mind
            .memory
            .known_resources
            .iter()
            .find(|known| {
                (known.x, known.y, known.kind) == (position.0, position.1, ResourceKind::Food)
            })
            .unwrap();

        assert_eq!(known.estimated_amount, 40);
        assert_eq!(known.last_seen_tick, 25);
    }

    #[test]
    fn remember_resource_keeps_memory_sorted_and_updates_existing() {
        let mut mind = Mind::default();

        remember_resource(&mut mind, 20, 10, ResourceKind::Food, 50, 1);
        remember_resource(&mut mind, 2, 3, ResourceKind::Stone, 30, 1);
        remember_resource(&mut mind, 8, 3, ResourceKind::Timber, 40, 1);

        let keys: Vec<_> = mind
            .memory
            .known_resources
            .iter()
            .map(|known| resource_key(known.x, known.y, known.kind))
            .collect();

        assert!(keys.windows(2).all(|pair| pair[0] < pair[1]));

        remember_resource(&mut mind, 2, 3, ResourceKind::Stone, 99, 42);

        assert_eq!(mind.memory.known_resources.len(), 3);

        let known = mind
            .memory
            .known_resources
            .iter()
            .find(|known| known.x == 2 && known.y == 3 && known.kind == ResourceKind::Stone)
            .unwrap();

        assert_eq!(known.estimated_amount, 99);
        assert_eq!(known.last_seen_tick, 42);
    }

    #[test]
    fn remembered_food_targets_are_ordered_by_distance_then_age() {
        let mut mind = Mind::default();

        remember_resource(&mut mind, 10, 0, ResourceKind::Food, 50, 100);
        remember_resource(&mut mind, 2, 0, ResourceKind::Food, 50, 100);
        remember_resource(&mut mind, 5, 0, ResourceKind::Food, 50, 200);

        let mut heap = mind.remembered_food_targets((0, 0), 300);
        let mut targets = Vec::new();
        while let Some(Reverse((distance, age, position))) = heap.pop() {
            targets.push((distance, age, position));
        }

        assert_eq!(targets.len(), 3);
        assert_eq!(targets[0].0, 2);
        assert_eq!(targets[1].0, 5);
        assert_eq!(targets[2].0, 10);

        remember_resource(&mut mind, 8, 0, ResourceKind::Food, 50, 50);
        remember_resource(&mut mind, 0, 8, ResourceKind::Food, 50, 250);

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
