use super::super::config::FOOD_SEARCH_THRESHOLD;
use super::super::entity::{Entity, LifeStage, Personality};
use super::super::spatial::EntitySnapshot;
use super::mind::{manhattan, Action, Goal, Mind};
use super::social::remembered_social_score;
use crate::pathfinding::{self, PathfindingWorkspace};
use crate::world::{Grid, ResourceKind};

const MIN_SWITCH_MARGIN: f32 = 0.02;
const MAX_SWITCH_MARGIN: f32 = 0.15;

/// Context passed to [`evaluate_goals`] to avoid nested tuple parameters.
pub(in crate::simulation) struct DecisionContext {
    pub tick: u64,
    pub origin: (u32, u32),
}

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
    context: DecisionContext,
) -> Goal {
    let DecisionContext { tick, origin } = context;
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

        let best_remembered_score = mind
            .memory
            .known_entities
            .iter()
            .filter(|known| mind.visible_entities.binary_search(&known.id).is_err())
            .filter_map(|known| remembered_social_score(known, tick, origin, personality))
            .max();
        let sociability_factor = 0.3 + personality.sociability * 0.7;
        let sated_factor = (1.0 - hunger_ratio) * 0.6 + 0.4;

        let relationship_strength =
            best_remembered_score.map(|score| (score as f32 / 1_000.0).clamp(0.1, 1.0));

        if has_visible {
            // Visible candidates present — full utility
            sated_factor * 0.6 * sociability_factor
        } else if let Some(relationship_strength) = relationship_strength {
            // No visible candidates but good relationships in memory
            // Utility is reduced since target must be sought first
            sated_factor * (0.15 + relationship_strength * 0.45) * sociability_factor * 0.9
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
                super::exploration::plan_exploration(entity, world, tick, pathfinding_workspace);
            }
        }
        Goal::Explore => {
            super::exploration::plan_exploration(entity, world, tick, pathfinding_workspace);
        }
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
    tick: u64,
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
        if known.seek_on_cooldown(tick) {
            continue;
        }

        // Skip if currently visible (already handled above)
        if mind.visible_entities.binary_search(&known.id).is_ok() {
            continue;
        }

        // Calculate score based on affinity and familiarity
        // Distance uses last_seen position as an estimate
        let Some(score) = remembered_social_score(known, tick, origin, personality) else {
            continue;
        };

        // Higher threshold for seeking from memory — must be worth the effort
        // Require a minimum score to justify seeking from memory
        match best_memory {
            None => best_memory = Some((score, known.id)),
            Some((best_score, _)) if score > best_score => best_memory = Some((score, known.id)),
            _ => {}
        }
    }

    // Return best from memory if available, otherwise fall back to visible
    best_memory.or(best_visible).map(|(_, id)| id)
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
        tick,
    ) else {
        // No suitable target visible or in memory — fall back to exploration
        super::exploration::plan_exploration(entity, world, tick, pathfinding_workspace);
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
            super::exploration::plan_exploration(entity, world, tick, pathfinding_workspace);
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
            super::exploration::plan_exploration(entity, world, tick, pathfinding_workspace);
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
            entity.mind.memory.mark_failed_social_seek(target_id, tick);
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
        if !is_visible {
            entity.mind.memory.mark_failed_social_seek(target_id, tick);
        }
        entity.mind.set_plan(Goal::Rest, vec![Action::Wait], tick);
        entity.activity = super::super::entity::EntityActivity::Resting;
    }
}

#[cfg(test)]
mod tests {
    use super::super::super::time::TICKS_PER_YEAR;
    use super::super::mind::{KnownEntity, Mind, FAILED_SOCIAL_SEEK_RETRY_TICKS};
    use super::*;

    fn social_personality() -> Personality {
        Personality {
            curiosity: 0.0,
            sociability: 1.0,
            cooperativeness: 0.5,
            caution: 0.5,
            persistence: 0.5,
        }
    }

    fn remembered_entity(id: u32, affinity: i16) -> KnownEntity {
        KnownEntity {
            id,
            first_seen_tick: 0,
            last_seen_tick: 0,
            last_seen_x: 10,
            last_seen_y: 0,
            observed_ticks: 1,
            affinity,
            last_interaction_tick: 0,
            interaction_count: 0,
            seek_retry_after_tick: None,
        }
    }

    #[test]
    fn failed_social_seek_does_not_immediately_retry() {
        let personality = social_personality();
        let mut mind = Mind::default();
        mind.memory.known_entities.push(remembered_entity(2, 800));

        assert_eq!(
            select_social_target(&mind, (0, 0), 1, &[], &personality, 10),
            Some(2)
        );

        assert!(mind.memory.mark_failed_social_seek(2, 10));
        assert_eq!(
            select_social_target(&mind, (0, 0), 1, &[], &personality, 11),
            None
        );
        assert_eq!(
            select_social_target(
                &mind,
                (0, 0),
                1,
                &[],
                &personality,
                10 + FAILED_SOCIAL_SEEK_RETRY_TICKS,
            ),
            Some(2)
        );
    }

    #[test]
    fn selects_highest_affinity_from_memory() {
        let personality = social_personality();
        let mut mind = Mind::default();
        mind.memory.known_entities = vec![
            remembered_entity(2, 0),
            remembered_entity(3, 800),
            remembered_entity(4, 0),
        ];

        assert_eq!(
            select_social_target(&mind, (0, 0), 1, &[], &personality, 0),
            Some(3)
        );
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
            DecisionContext {
                tick: 0,
                origin: (0, 0),
            },
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
            DecisionContext {
                tick: 0,
                origin: (0, 0),
            },
        );
        assert_eq!(goal, Goal::Explore);
    }

    #[test]
    fn strong_remembered_relationship_can_trigger_socialize() {
        let mut mind = Mind::default();
        mind.memory.known_entities.push(KnownEntity {
            id: 2,
            first_seen_tick: 0,
            last_seen_tick: 0,
            last_seen_x: 10,
            last_seen_y: 10,
            observed_ticks: 10,
            affinity: 500,
            last_interaction_tick: 0,
            interaction_count: 5,
            seek_retry_after_tick: None,
        });
        let personality = Personality {
            curiosity: 0.0,
            sociability: 1.0,
            cooperativeness: 0.5,
            caution: 0.5,
            persistence: 0.5,
        };

        let goal = evaluate_goals(
            &mut mind,
            0.0,
            100.0,
            25 * TICKS_PER_YEAR,
            &personality,
            None,
            DecisionContext {
                tick: 0,
                origin: (0, 0),
            },
        );

        assert_eq!(goal, Goal::Socialize);
    }
}
