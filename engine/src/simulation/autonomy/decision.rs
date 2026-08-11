use super::super::config::FOOD_SEARCH_THRESHOLD;
use super::super::entity::{Entity, LifeStage, Personality};
use super::super::spatial::EntitySnapshot;
use super::mind::{Action, Goal, Mind};
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
            .filter_map(|known| remembered_social_score(known, tick, origin, personality, 1.0))
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
            super::social::plan_socialize(entity, world, tick, pathfinding_workspace, population);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::super::super::time::TICKS_PER_YEAR;
    use super::super::mind::{KnownEntity, Mind};
    use super::*;

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
