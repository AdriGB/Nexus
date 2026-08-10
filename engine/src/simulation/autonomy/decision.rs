use super::super::config::FOOD_SEARCH_THRESHOLD;
use super::super::entity::{Entity, LifeStage, Personality};
use super::super::spatial::EntitySnapshot;
use super::mind::{chunk_index, manhattan, Action, Goal, Mind, KNOWLEDGE_CHUNK_SIZE};
use crate::pathfinding::{self, PathfindingWorkspace};
use crate::world::{Grid, ResourceKind};

const MIN_SWITCH_MARGIN: f32 = 0.02;
const MAX_SWITCH_MARGIN: f32 = 0.15;

/// Maps persistence [0.0, 1.0] to the score margin required to abandon
/// the current goal for an alternative.
///
/// persistence 0.0 → 0.02 (barely any inertia)
/// persistence 0.5 → 0.0525
/// persistence 1.0 → 0.15 (requires a substantially better alternative)
pub(super) fn switch_margin(persistence: f32) -> f32 {
    MIN_SWITCH_MARGIN + (MAX_SWITCH_MARGIN - MIN_SWITCH_MARGIN) * persistence * persistence
}

pub fn evaluate_goals(
    mind: &mut Mind,
    hunger: f32,
    health: f32,
    age_ticks: u64,
    personality: &Personality,
    current_goal: Option<Goal>,
) -> Goal {
    let stage = LifeStage::from_age_ticks(age_ticks);

    if stage == LifeStage::Child {
        if hunger >= FOOD_SEARCH_THRESHOLD
            && mind
                .memory
                .known_resources
                .iter()
                .any(|known| known.kind == ResourceKind::Food && known.estimated_amount > 0)
        {
            mind.utility_scores = super::mind::UtilityScores {
                eat: 1.0,
                explore: 0.0,
                rest: 0.0,
                socialize: 0.0,
            };
            return Goal::Eat;
        }
        mind.utility_scores = super::mind::UtilityScores {
            eat: 0.0,
            explore: 0.0,
            rest: 0.5,
            socialize: 0.0,
        };
        return Goal::Follow;
    }

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

    let curiosity_factor = 0.75 + personality.curiosity * 0.50;
    let caution_explore_factor = 1.15 - personality.caution * 0.30;
    let caution_rest_factor = 0.85 + personality.caution * 0.30;

    // Socialize: meaningful when there are visible candidates OR remembered
    // entities with high positive affinity that could be sought out.
    let socialize = {
        let has_visible = !mind.visible_entities.is_empty();
        
        let positive_affinity_count = mind
            .memory
            .known_entities
            .iter()
            .filter(|known| known.affinity > 0)
            .count() as f32;
        let total_known = mind.memory.known_entities.len().max(1) as f32;
        let affinity_ratio = positive_affinity_count / total_known;
        let sociability_factor = 0.3 + personality.sociability * 0.7;
        let sated_factor = (1.0 - hunger_ratio) * 0.6 + 0.4;
        
        let base_social = sated_factor * (0.15 + affinity_ratio * 0.45) * sociability_factor;
        
        if has_visible {
            // Visible candidates present — full utility
            base_social
        } else if affinity_ratio > 0.3 && personality.sociability > 0.4 {
            // No visible candidates but good relationships in memory
            // Utility is reduced since target must be sought first
            base_social * 0.5
        } else {
            0.0
        }
    };

    mind.utility_scores = super::mind::UtilityScores {
        eat: hunger_ratio * (0.65 + 0.35 * food_confidence),
        explore: ((1.0 - hunger_ratio) * 0.55 + (1.0 - food_confidence) * 0.2)
            * curiosity_factor
            * caution_explore_factor,
        rest: (health_deficit * 0.8 + 0.05) * caution_rest_factor,
        socialize,
    };

    let scores = [
        (mind.utility_scores.eat, Goal::Eat),
        (mind.utility_scores.explore, Goal::Explore),
        (mind.utility_scores.rest, Goal::Rest),
        (mind.utility_scores.socialize, Goal::Socialize),
    ];
    let (best_score, best_goal) = scores
        .into_iter()
        .max_by(|left, right| left.0.total_cmp(&right.0))
        .unwrap_or((0.0, Goal::Explore));

    if let Some(current) = current_goal {
        if current != best_goal {
            if let Some((current_score, _)) = scores.iter().find(|(_, goal)| *goal == current) {
                if best_score <= *current_score + switch_margin(personality.persistence) {
                    return current;
                }
            }
        }
    }

    best_goal
}

pub(super) fn invalidate_obsolete_food_plan(entity: &mut Entity) {
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

fn plan_follow(
    entity: &mut Entity,
    world: &Grid,
    tick: u64,
    pathfinding_workspace: &mut PathfindingWorkspace,
    population: &[EntitySnapshot],
) {
    let origin = (entity.x, entity.y);

    let Some(caregiver_id) = entity.caregiver_id else {
        entity.mind.set_plan(Goal::Rest, vec![Action::Wait], tick);
        entity.activity = super::super::entity::EntityActivity::Resting;
        return;
    };
    let Some(target) = population
        .iter()
        .find(|snapshot| snapshot.id == caregiver_id)
        .map(|snapshot| (snapshot.x, snapshot.y))
    else {
        entity.mind.set_plan(Goal::Rest, vec![Action::Wait], tick);
        entity.activity = super::super::entity::EntityActivity::Resting;
        return;
    };

    if target == origin {
        entity.mind.set_plan(Goal::Follow, vec![], tick);
        entity.path.clear();
        entity.path_index = 0;
        entity.activity = super::super::entity::EntityActivity::Idle;
        return;
    }

    if let Some(path) =
        pathfinding::find_path_with_workspace(pathfinding_workspace, world, origin, target)
    {
        entity.path = path.into_iter().skip(1).collect();
        entity.path_index = 0;
        if entity.path.is_empty() {
            entity.mind.set_plan(Goal::Follow, vec![], tick);
            entity.activity = super::super::entity::EntityActivity::Idle;
        } else {
            entity
                .mind
                .set_plan(Goal::Follow, vec![Action::MoveTo(target.0, target.1)], tick);
            entity.activity = super::super::entity::EntityActivity::Moving;
        }
    } else {
        entity.mind.set_plan(Goal::Rest, vec![Action::Wait], tick);
        entity.activity = super::super::entity::EntityActivity::Resting;
    }
}

pub(super) fn plan_goal(
    entity: &mut Entity,
    world: &Grid,
    tick: u64,
    goal: Goal,
    pathfinding_workspace: &mut PathfindingWorkspace,
    population: &[EntitySnapshot],
) {
    let origin = (entity.x, entity.y);
    match goal {
        Goal::Eat => {
            let mut targets = entity.mind.remembered_food_targets(origin, tick);
            while let Some(std::cmp::Reverse((_, _, target))) = targets.pop() {
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
                    entity.activity = super::super::entity::EntityActivity::SeekingFood;
                    return;
                }
                entity.mind.memory.mark_unreachable(target, tick);
            }
            if LifeStage::from_age_ticks(entity.age_ticks) == LifeStage::Child {
                plan_follow(entity, world, tick, pathfinding_workspace, population);
            } else {
                plan_exploration(entity, world, tick, pathfinding_workspace);
            }
        }
        Goal::Explore => plan_exploration(entity, world, tick, pathfinding_workspace),
        Goal::Follow => plan_follow(entity, world, tick, pathfinding_workspace, population),
        Goal::Rest => {
            entity.mind.set_plan(Goal::Rest, vec![Action::Wait], tick);
            entity.activity = super::super::entity::EntityActivity::Resting;
        }
        Goal::Socialize => {
            plan_socialize(entity, world, tick, pathfinding_workspace, population);
        }
    }
}

/// Select the best entity to socialize with based on affinity, familiarity, and distance.
/// 
/// First checks visible entities. If none are suitable but there are known entities
/// with positive affinity that are not currently visible, may select one from memory
/// to seek out.
fn select_social_target(
    mind: &Mind,
    origin: (u32, u32),
    entity_id: u32,
    population: &[EntitySnapshot],
    personality: &Personality,
) -> Option<u32> {
    let mut best_visible: Option<(i32, u32)> = None;
    let mut best_memory: Option<(i32, u32)> = None;

    for &visible_id in &mind.visible_entities {
        if visible_id == entity_id {
            continue;
        }

        let Some(snapshot) = population.iter().find(|s| s.id == visible_id) else {
            continue;
        };

        let distance = manhattan(origin, (snapshot.x, snapshot.y)) as i32;

        // Look up relationship
        let known = mind
            .memory
            .known_entities
            .iter()
            .find(|k| k.id == visible_id);

        let affinity = known.map_or(0, |k| k.affinity as i32);
        let familiarity = known.map_or(0u32, |k| k.interaction_count) as i32;

        // Skip entities with strong negative affinity
        if affinity < -200 {
            continue;
        }

        // Score: higher affinity and familiarity are better, closer is better
        // Sociability of the observer makes distance less important
        let distance_weight = (2.0 - personality.sociability * 1.5).max(0.5);
        let score = affinity * 2 + familiarity * 5 - (distance as f32 * distance_weight) as i32;

        match best_visible {
            None => best_visible = Some((score, visible_id)),
            Some((best_score, _)) if score > best_score => best_visible = Some((score, visible_id)),
            _ => {}
        }
    }

    // If we have a good visible candidate, use it
    if let Some((visible_score, _)) = best_visible {
        // Only consider memory if visible score is low (no great options nearby)
        if visible_score >= 50 {
            return best_visible.map(|(_, id)| id);
        }
    }

    // No great visible option — check memory for high-affinity entities to seek
    for known in &mind.memory.known_entities {
        if known.id == entity_id {
            continue;
        }

        // Only seek entities with clearly positive affinity
        if known.affinity <= 100 {
            continue;
        }

        // Skip if currently visible (already handled above)
        if mind
            .visible_entities
            .binary_search(&known.id)
            .is_ok()
        {
            continue;
        }

        // Calculate score based on affinity and familiarity
        // Distance uses last_seen position as an estimate
        let distance = manhattan(origin, (known.last_seen_x, known.last_seen_y)) as i32;
        let familiarity = known.interaction_count as i32;
        let affinity = known.affinity as i32;

        // Higher threshold for seeking from memory — must be worth the effort
        let distance_weight = (2.0 - personality.sociability * 1.5).max(0.5);
        let score = affinity * 2 + familiarity * 5 - (distance as f32 * distance_weight) as i32;

        // Require a minimum score to justify seeking from memory
        if score < 100 {
            continue;
        }

        match best_memory {
            None => best_memory = Some((score, known.id)),
            Some((best_score, _)) if score > best_score => best_memory = Some((score, known.id)),
            _ => {}
        }
    }

    // Return best from memory if available, otherwise fall back to visible
    best_memory
        .or(best_visible)
        .map(|(_, id)| id)
}

fn plan_socialize(
    entity: &mut Entity,
    world: &Grid,
    tick: u64,
    pathfinding_workspace: &mut PathfindingWorkspace,
    population: &[EntitySnapshot],
) {
    let origin = (entity.x, entity.y);

    let Some(target_id) = select_social_target(
        &entity.mind,
        origin,
        entity.id,
        population,
        &entity.personality,
    ) else {
        // No suitable target visible or in memory — fall back to exploration
        plan_exploration(entity, world, tick, pathfinding_workspace);
        return;
    };

    // Target may be from memory (not visible) or from visible entities.
    // If visible, use current position. If not visible, use last_seen.
    let is_visible = entity
        .mind
        .visible_entities
        .binary_search(&target_id)
        .is_ok();

    let target_pos = if is_visible {
        // Visible target: use current known position
        let Some(target_snapshot) = population.iter().find(|s| s.id == target_id) else {
            plan_exploration(entity, world, tick, pathfinding_workspace);
            return;
        };
        (target_snapshot.x, target_snapshot.y)
    } else {
        // Not visible: use last seen position from memory
        let Some(known) = entity
            .mind
            .memory
            .known_entities
            .iter()
            .find(|k| k.id == target_id)
        else {
            plan_exploration(entity, world, tick, pathfinding_workspace);
            return;
        };
        (known.last_seen_x, known.last_seen_y)
    };

    // Already close enough? Just interact (only if visible).
    if manhattan(origin, target_pos) <= super::social::SOCIAL_RADIUS {
        if is_visible {
            entity
                .mind
                .set_plan(Goal::Socialize, vec![Action::Interact(target_id)], tick);
            entity.activity = super::super::entity::EntityActivity::Socializing;
        } else {
            // Arrived at last_seen but target not visible — continue searching or abandon
            entity.mind.clear_goal();
            entity.path.clear();
            entity.path_index = 0;
            entity.activity = super::super::entity::EntityActivity::Idle;
        }
        return;
    }

    // Need to approach
    if let Some(path) =
        pathfinding::find_path_with_workspace(pathfinding_workspace, world, origin, target_pos)
    {
        entity.path = path.into_iter().skip(1).collect();
        entity.path_index = 0;
        entity.mind.set_plan(
            Goal::Socialize,
            vec![
                Action::ApproachEntity(target_id),
                Action::Interact(target_id),
            ],
            tick,
        );
        entity.activity = super::super::entity::EntityActivity::Moving;
    } else {
        // Can't reach target, fall back
        entity.mind.set_plan(Goal::Rest, vec![Action::Wait], tick);
        entity.activity = super::super::entity::EntityActivity::Resting;
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
        entity.activity = super::super::entity::EntityActivity::Resting;
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
        entity.activity = super::super::entity::EntityActivity::Resting;
        return;
    };
    entity.path = path.into_iter().skip(1).collect();
    entity.path_index = 0;
    if entity.path.is_empty() {
        entity.mind.set_plan(Goal::Rest, vec![Action::Wait], tick);
        entity.activity = super::super::entity::EntityActivity::Resting;
    } else {
        entity.mind.set_plan(
            Goal::Explore,
            vec![Action::ExploreArea(target.0, target.1)],
            tick,
        );
        entity.activity = super::super::entity::EntityActivity::Exploring;
    }
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

#[cfg(test)]
mod tests {
    use super::super::super::time::TICKS_PER_YEAR;
    use super::super::mind::{chunk_index, Mind, FAILED_EXPLORATION_RETRY_TICKS};
    use super::*;
    use crate::world::{Terrain, Tile};

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
    fn switch_margin_scales_with_persistence() {
        assert!((switch_margin(0.0) - 0.02).abs() < 0.001);
        assert!((switch_margin(0.5) - 0.0525).abs() < 0.001);
        assert!((switch_margin(1.0) - 0.15).abs() < 0.001);
        assert!(switch_margin(0.0) < switch_margin(0.5));
        assert!(switch_margin(0.5) < switch_margin(1.0));
    }

    #[test]
    fn high_persistence_retains_goal_when_alternative_is_slightly_better() {
        let mut mind = Mind::default();
        let personality = Personality {
            curiosity: 0.0,
            sociability: 0.5,
            cooperativeness: 0.5,
            caution: 0.5,
            persistence: 1.0,
        };
        let goal = evaluate_goals(
            &mut mind,
            35.0,
            100.0,
            25 * TICKS_PER_YEAR,
            &personality,
            Some(Goal::Eat),
        );
        assert_eq!(goal, Goal::Eat);
    }

    #[test]
    fn low_persistence_switches_goal_when_alternative_is_slightly_better() {
        let mut mind = Mind::default();
        let personality = Personality {
            curiosity: 0.0,
            sociability: 0.5,
            cooperativeness: 0.5,
            caution: 0.5,
            persistence: 0.0,
        };
        let goal = evaluate_goals(
            &mut mind,
            35.0,
            100.0,
            25 * TICKS_PER_YEAR,
            &personality,
            Some(Goal::Eat),
        );
        assert_eq!(goal, Goal::Explore);
    }

    #[test]
    fn ring_perimeter_contains_exactly_eight_r_tiles_when_unclipped() {
        let ring = ring_perimeter(5, 5, 3, 20, 20);

        assert_eq!(ring.len(), 24);
        let unique: std::collections::HashSet<_> = ring.iter().copied().collect();
        assert_eq!(unique.len(), ring.len());
    }

    #[test]
    fn ring_perimeter_clips_to_world_without_duplicates() {
        let ring = ring_perimeter(0, 0, 3, 10, 10);

        assert!(ring.iter().all(|&(x, y)| x < 10 && y < 10));
        let unique: std::collections::HashSet<_> = ring.iter().copied().collect();
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
        assert_eq!(mind.memory.failed_exploration_count(), 1);

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
}
