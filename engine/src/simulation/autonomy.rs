use super::{Entity, EntityActivity, FOOD_SEARCH_THRESHOLD, MAX_HEALTH};
use crate::pathfinding;
use crate::world::{Grid, ResourceKind};
use std::collections::HashSet;

pub const DEFAULT_PERCEPTION_RADIUS: u32 = 6;
const KNOWLEDGE_CHUNK_SIZE: u32 = 8;
const RESOURCE_MEMORY_TTL: u64 = 2_000;
const FAILED_TARGET_RETRY_TICKS: u64 = 120;
const FOOD_CONSUMED_PER_MEAL: u16 = 10;
const HUNGER_REDUCTION_PER_MEAL: f32 = 50.0;
pub(super) const URGENT_HUNGER_THRESHOLD: f32 = 85.0;
const REST_HEALTH_PER_TICK: f32 = 0.25;

pub(super) fn update_entity(
    entity: &mut Entity,
    world: &mut Grid,
    tick: u64,
    population: &[(u32, (u32, u32))],
) -> u16 {
    let position = (entity.x, entity.y);
    perceive(&mut entity.mind, world, position, tick);
    perceive_entities(&mut entity.mind, entity.id, position, population);
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
        plan_goal(entity, world, tick, goal);
    }

    execute_current_action(entity, world, tick)
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

fn plan_goal(entity: &mut Entity, world: &Grid, tick: u64, goal: Goal) {
    let origin = (entity.x, entity.y);
    match goal {
        Goal::Eat => {
            for target in entity.mind.remembered_food_targets(origin, tick) {
                if let Some(path) = pathfinding::find_path(world, origin, target) {
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
            plan_exploration(entity, world, tick);
        }
        Goal::Explore => plan_exploration(entity, world, tick),
        Goal::Rest => {
            entity.mind.set_plan(Goal::Rest, vec![Action::Wait], tick);
            entity.activity = EntityActivity::Resting;
        }
    }
}

fn plan_exploration(entity: &mut Entity, world: &Grid, tick: u64) {
    let origin = (entity.x, entity.y);
    let Some(target) = exploration_target(&entity.mind, world, origin, entity.id) else {
        entity.mind.set_plan(Goal::Rest, vec![Action::Wait], tick);
        entity.activity = EntityActivity::Resting;
        return;
    };
    let Some(path) = pathfinding::find_path(world, origin, target) else {
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
            if entity.path_index < entity.path.len() {
                let next = entity.path[entity.path_index];
                entity.x = next.0;
                entity.y = next.1;
                entity.path_index += 1;
                entity.activity = if entity.mind.current_goal == Some(Goal::Explore) {
                    EntityActivity::Exploring
                } else {
                    EntityActivity::Moving
                };
            }
            if entity.path_index >= entity.path.len() {
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

#[derive(Clone, Debug, Default)]
pub struct Memory {
    pub known_resources: Vec<KnownResource>,
    known_chunks: HashSet<u32>,
}

impl Memory {
    pub fn known_chunk_count(&self) -> usize {
        self.known_chunks.len()
    }

    pub fn remembers_chunk(&self, world: &Grid, x: u32, y: u32) -> bool {
        self.known_chunks.contains(&chunk_index(world, x, y))
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

    pub fn remembered_food_targets(&self, origin: (u32, u32), tick: u64) -> Vec<(u32, u32)> {
        let mut targets: Vec<_> = self
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
                (distance, age, (known.x, known.y))
            })
            .collect();
        targets.sort_unstable();
        targets
            .into_iter()
            .map(|(_, _, position)| position)
            .collect()
    }
}

pub fn perceive(mind: &mut Mind, world: &Grid, position: (u32, u32), tick: u64) {
    mind.memory
        .known_resources
        .retain(|known| tick.saturating_sub(known.last_seen_tick) <= RESOURCE_MEMORY_TTL);

    let radius = mind.perception_radius as i64;
    let min_x = (i64::from(position.0) - radius).max(0) as u32;
    let max_x = (i64::from(position.0) + radius).min(i64::from(world.width) - 1) as u32;
    let min_y = (i64::from(position.1) - radius).max(0) as u32;
    let max_y = (i64::from(position.1) + radius).min(i64::from(world.height) - 1) as u32;

    for y in min_y..=max_y {
        for x in min_x..=max_x {
            if manhattan(position, (x, y)) > mind.perception_radius {
                continue;
            }
            mind.memory.known_chunks.insert(chunk_index(world, x, y));
            let deposit = world.resources[(y * world.width + x) as usize];
            match deposit {
                Some(deposit) => remember_resource(mind, x, y, deposit.kind, deposit.amount, tick),
                None => mind
                    .memory
                    .known_resources
                    .retain(|known| (known.x, known.y) != (x, y)),
            }
        }
    }
}

pub fn perceive_entities(
    mind: &mut Mind,
    entity_id: u32,
    position: (u32, u32),
    population: &[(u32, (u32, u32))],
) {
    mind.visible_entities.clear();
    mind.visible_entities.extend(
        population
            .iter()
            .filter(|(other_id, other_position)| {
                *other_id != entity_id
                    && manhattan(position, *other_position) <= mind.perception_radius
            })
            .map(|(other_id, _)| *other_id),
    );
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
) -> Option<(u32, u32)> {
    let chunks_wide = world.width.div_ceil(KNOWLEDGE_CHUNK_SIZE);
    let chunks_high = world.height.div_ceil(KNOWLEDGE_CHUNK_SIZE);
    let origin_chunk = (
        origin.0 / KNOWLEDGE_CHUNK_SIZE,
        origin.1 / KNOWLEDGE_CHUNK_SIZE,
    );
    let max_ring = chunks_wide.max(chunks_high);

    for ring in 1..=max_ring {
        let mut chunks = Vec::new();
        for cy in 0..chunks_high {
            for cx in 0..chunks_wide {
                if cx.abs_diff(origin_chunk.0).max(cy.abs_diff(origin_chunk.1)) == ring {
                    chunks.push((cx, cy));
                }
            }
        }
        chunks.sort_unstable_by_key(|&(cx, cy)| {
            let index = cy * chunks_wide + cx;
            index.wrapping_add(entity_id.wrapping_mul(2_654_435_761))
        });
        for (cx, cy) in chunks {
            let x = (cx * KNOWLEDGE_CHUNK_SIZE + KNOWLEDGE_CHUNK_SIZE / 2).min(world.width - 1);
            let y = (cy * KNOWLEDGE_CHUNK_SIZE + KNOWLEDGE_CHUNK_SIZE / 2).min(world.height - 1);
            if !mind.memory.remembers_chunk(world, x, y) {
                if let Some(target) = walkable_in_chunk(world, cx, cy) {
                    return Some(target);
                }
            }
        }
    }
    deterministic_wander_target(world, origin, entity_id)
}

fn remember_resource(mind: &mut Mind, x: u32, y: u32, kind: ResourceKind, amount: u16, tick: u64) {
    if let Some(known) = mind
        .memory
        .known_resources
        .iter_mut()
        .find(|known| (known.x, known.y, known.kind) == (x, y, kind))
    {
        known.last_seen_tick = tick;
        known.estimated_amount = amount;
        known.failed_attempts = 0;
        known.avoid_until_tick = 0;
    } else {
        mind.memory.known_resources.push(KnownResource {
            x,
            y,
            kind,
            last_seen_tick: tick,
            estimated_amount: amount,
            failed_attempts: 0,
            avoid_until_tick: 0,
        });
    }
}

fn walkable_in_chunk(world: &Grid, chunk_x: u32, chunk_y: u32) -> Option<(u32, u32)> {
    let start_x = chunk_x * KNOWLEDGE_CHUNK_SIZE;
    let start_y = chunk_y * KNOWLEDGE_CHUNK_SIZE;
    let end_x = (start_x + KNOWLEDGE_CHUNK_SIZE).min(world.width);
    let end_y = (start_y + KNOWLEDGE_CHUNK_SIZE).min(world.height);
    (start_y..end_y)
        .flat_map(|y| (start_x..end_x).map(move |x| (x, y)))
        .find(|&(x, y)| {
            world
                .get(x, y)
                .is_some_and(|tile| tile.terrain.is_walkable())
        })
}

fn deterministic_wander_target(
    world: &Grid,
    origin: (u32, u32),
    entity_id: u32,
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
            world
                .get(x, y)
                .is_some_and(|tile| tile.terrain.is_walkable())
        })
}

fn chunk_index(world: &Grid, x: u32, y: u32) -> u32 {
    let chunks_wide = world.width.div_ceil(KNOWLEDGE_CHUNK_SIZE);
    (y / KNOWLEDGE_CHUNK_SIZE) * chunks_wide + x / KNOWLEDGE_CHUNK_SIZE
}

fn manhattan(left: (u32, u32), right: (u32, u32)) -> u32 {
    left.0.abs_diff(right.0) + left.1.abs_diff(right.1)
}
