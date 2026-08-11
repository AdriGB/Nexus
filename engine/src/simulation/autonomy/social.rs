use super::super::entity::{Entity, EntityActivity, LifeStage, Personality};
use super::super::spatial::EntitySnapshot;
use super::super::time::TICKS_PER_DAY;
use super::exploration::plan_exploration;
use super::mind::{manhattan, Action, Goal, KnownEntity, Mind};
use crate::pathfinding::{self, PathfindingWorkspace};
use crate::world::Grid;
use std::collections::HashMap;

pub(in crate::simulation) const SOCIAL_RADIUS: u32 = 2;
pub(super) const MIN_INTERACTION_INTERVAL: u64 = 12;
pub(super) const MAX_INTERACTION_INTERVAL: u64 = 72;
const MIN_REMEMBERED_SOCIAL_SCORE: i32 = 100;
const STALE_PENALTY_PER_DAY: i32 = 10;

pub(super) fn remembered_social_score(
    known: &KnownEntity,
    tick: u64,
    origin: (u32, u32),
    personality: &Personality,
) -> Option<i32> {
    if known.seek_on_cooldown(tick) {
        return None;
    }

    let distance = manhattan(origin, (known.last_seen_x, known.last_seen_y));
    let distance_weight = (2.0 - personality.sociability * 1.5).max(0.5);
    let distance_penalty = (distance as f32 * distance_weight) as i32;
    let age_days = tick.saturating_sub(known.last_seen_tick) / TICKS_PER_DAY;
    let stale_penalty = i32::try_from(age_days)
        .unwrap_or(i32::MAX)
        .saturating_mul(STALE_PENALTY_PER_DAY);
    let score = i32::from(known.affinity.max(0))
        .saturating_mul(2)
        .saturating_add(
            i32::try_from(known.interaction_count)
                .unwrap_or(i32::MAX)
                .saturating_mul(5),
        )
        .saturating_sub(distance_penalty)
        .saturating_sub(stale_penalty);

    (score >= MIN_REMEMBERED_SOCIAL_SCORE).then_some(score)
}

pub(super) fn interaction_interval(a: &Personality, b: &Personality) -> u64 {
    let sociability = (a.sociability + b.sociability) * 0.5;
    let span = (MAX_INTERACTION_INTERVAL - MIN_INTERACTION_INTERVAL) as f32;

    MAX_INTERACTION_INTERVAL - (span * sociability).round() as u64
}

fn personality_compatibility(a: &Personality, b: &Personality) -> f32 {
    let curiosity = (a.curiosity - b.curiosity).abs();
    let caution = (a.caution - b.caution).abs();
    let sociability = (a.sociability - b.sociability).abs();

    1.0 - (curiosity + caution + sociability) / 3.0
}

fn interaction_delta(compatibility: f32, other_cooperativeness: f32) -> i16 {
    let compatibility_effect = ((compatibility - 0.5) * 8.0).round() as i16;
    let cooperation_effect = ((other_cooperativeness - 0.5) * 8.0).round() as i16;

    compatibility_effect + cooperation_effect
}

/// Select the best entity to socialize with based on affinity, familiarity,
/// and distance.
///
/// First checks visible entities. If none are suitable but there are known
/// entities with positive affinity that are not currently visible, may select
/// one from memory to seek out.
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
        let known = mind
            .memory
            .known_entities
            .iter()
            .find(|k| k.id == visible_id);
        let affinity = known.map_or(0, |k| k.affinity as i32);
        let familiarity = known.map_or(0u32, |k| k.interaction_count) as i32;

        if affinity < -200 {
            continue;
        }

        let distance_weight = (2.0 - personality.sociability * 1.5).max(0.5);
        let score = affinity * 2 + familiarity * 5 - (distance as f32 * distance_weight) as i32;

        match best_visible {
            None => best_visible = Some((score, visible_id)),
            Some((best_score, _)) if score > best_score => {
                best_visible = Some((score, visible_id));
            }
            _ => {}
        }
    }

    if let Some((visible_score, _)) = best_visible {
        if visible_score >= 50 {
            return best_visible.map(|(_, id)| id);
        }
    }

    for known in &mind.memory.known_entities {
        if known.id == entity_id {
            continue;
        }

        if known.seek_on_cooldown(tick) {
            continue;
        }

        if mind.visible_entities.binary_search(&known.id).is_ok() {
            continue;
        }

        let Some(score) = remembered_social_score(known, tick, origin, personality) else {
            continue;
        };

        match best_memory {
            None => best_memory = Some((score, known.id)),
            Some((best_score, _)) if score > best_score => {
                best_memory = Some((score, known.id));
            }
            _ => {}
        }
    }

    best_memory.or(best_visible).map(|(_, id)| id)
}

/// Plan a Socialize goal for the entity.
///
/// If no suitable target exists, falls back to exploration.
pub(super) fn plan_socialize(
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
        plan_exploration(entity, world, tick, pathfinding_workspace);
        return;
    };

    let is_visible = entity
        .mind
        .visible_entities
        .binary_search(&target_id)
        .is_ok();

    let target_pos = if is_visible {
        let Some(target_snapshot) = population.iter().find(|s| s.id == target_id) else {
            plan_exploration(entity, world, tick, pathfinding_workspace);
            return;
        };
        (target_snapshot.x, target_snapshot.y)
    } else {
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

    if manhattan(origin, target_pos) <= SOCIAL_RADIUS {
        if is_visible {
            entity
                .mind
                .set_plan(Goal::Socialize, vec![Action::Interact(target_id)], tick);
            entity.activity = EntityActivity::Socializing;
        } else {
            entity.mind.memory.mark_failed_social_seek(target_id, tick);
            entity.mind.clear_goal();
            entity.path.clear();
            entity.path_index = 0;
            entity.activity = EntityActivity::Idle;
        }
        return;
    }

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
        entity.activity = EntityActivity::Moving;
    } else {
        if !is_visible {
            entity.mind.memory.mark_failed_social_seek(target_id, tick);
        }
        entity.mind.set_plan(Goal::Rest, vec![Action::Wait], tick);
        entity.activity = EntityActivity::Resting;
    }
}

pub(super) fn process_social_interactions(
    entities: &mut [Entity],
    population: &[EntitySnapshot],
    tick: u64,
) {
    let id_to_index: HashMap<u32, usize> = entities
        .iter()
        .enumerate()
        .map(|(index, entity)| (entity.id, index))
        .collect();

    let mut pairs: Vec<(usize, usize, i16, i16)> = Vec::new();

    for (entity_index, entity) in entities.iter().enumerate() {
        if entity.health <= 0.0 || LifeStage::from_age_ticks(entity.age_ticks) == LifeStage::Infant
        {
            continue;
        }

        let a_id = entity.id;
        let a_pos = match population.get(entity_index) {
            Some(snapshot) => (snapshot.x, snapshot.y),
            None => continue,
        };

        for &b_id in &entity.mind.visible_entities {
            if a_id >= b_id {
                continue;
            }

            let b_index = match id_to_index.get(&b_id) {
                Some(&index) => index,
                None => continue,
            };

            let b_pos = match population.get(b_index) {
                Some(snapshot) => (snapshot.x, snapshot.y),
                None => continue,
            };

            if manhattan(a_pos, b_pos) > SOCIAL_RADIUS {
                continue;
            }

            if entities[b_index].health <= 0.0
                || LifeStage::from_age_ticks(entities[b_index].age_ticks) == LifeStage::Infant
            {
                continue;
            }

            if entities[b_index]
                .mind
                .visible_entities
                .binary_search(&a_id)
                .is_err()
            {
                continue;
            }

            let last_a_to_b = entities[entity_index]
                .mind
                .memory
                .known_entities
                .binary_search_by_key(&b_id, |known| known.id)
                .ok()
                .map(|index| {
                    entities[entity_index].mind.memory.known_entities[index].last_interaction_tick
                })
                .unwrap_or(0);

            let last_b_to_a = entities[b_index]
                .mind
                .memory
                .known_entities
                .binary_search_by_key(&a_id, |known| known.id)
                .ok()
                .map(|index| {
                    entities[b_index].mind.memory.known_entities[index].last_interaction_tick
                })
                .unwrap_or(0);

            let last_interaction = last_a_to_b.max(last_b_to_a);
            let interval = interaction_interval(
                &entities[entity_index].personality,
                &entities[b_index].personality,
            );

            if last_interaction != 0 && tick.saturating_sub(last_interaction) < interval {
                continue;
            }

            let compatibility = personality_compatibility(
                &entities[entity_index].personality,
                &entities[b_index].personality,
            );
            let delta_a =
                interaction_delta(compatibility, entities[b_index].personality.cooperativeness);
            let delta_b = interaction_delta(
                compatibility,
                entities[entity_index].personality.cooperativeness,
            );

            pairs.push((entity_index, b_index, delta_a, delta_b));
        }
    }

    for (index_a, index_b, delta_a, delta_b) in pairs {
        let a_id = entities[index_a].id;
        let b_id = entities[index_b].id;
        let recorded_a = entities[index_a]
            .mind
            .memory
            .record_interaction(b_id, tick, delta_a);
        let recorded_b = entities[index_b]
            .mind
            .memory
            .record_interaction(a_id, tick, delta_b);
        debug_assert!(recorded_a && recorded_b);
    }
}

#[cfg(test)]
mod tests {
    use super::super::mind::{KnownEntity, Mind, FAILED_SOCIAL_SEEK_RETRY_TICKS};
    use super::*;

    fn personality(sociability: f32, cooperativeness: f32) -> Personality {
        Personality {
            curiosity: 0.5,
            sociability,
            cooperativeness,
            caution: 0.5,
            persistence: 0.5,
        }
    }

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
    fn interaction_interval_scales_with_sociability() {
        assert_eq!(
            interaction_interval(&personality(1.0, 0.5), &personality(1.0, 0.5)),
            12
        );
        assert_eq!(
            interaction_interval(&personality(0.0, 0.5), &personality(0.0, 0.5)),
            72
        );
        assert_eq!(
            interaction_interval(&personality(0.5, 0.5), &personality(0.5, 0.5)),
            42
        );
    }

    #[test]
    fn identical_personalities_are_fully_compatible() {
        assert!(
            (personality_compatibility(&personality(0.5, 0.5), &personality(0.5, 0.5)) - 1.0).abs()
                < 0.001
        );
    }

    #[test]
    fn opposite_personalities_are_incompatible() {
        let a = Personality {
            curiosity: 0.0,
            caution: 0.0,
            sociability: 0.0,
            cooperativeness: 0.5,
            persistence: 0.5,
        };
        let b = Personality {
            curiosity: 1.0,
            caution: 1.0,
            sociability: 1.0,
            cooperativeness: 0.5,
            persistence: 0.5,
        };
        assert!((personality_compatibility(&a, &b) - 0.0).abs() < 0.001);
    }

    #[test]
    fn cooperative_partner_gives_positive_delta() {
        assert_eq!(interaction_delta(1.0, 1.0), 8);
    }

    #[test]
    fn uncooperative_partner_gives_negative_delta() {
        assert_eq!(interaction_delta(0.0, 0.0), -8);
    }

    #[test]
    fn neutral_defaults_give_moderate_positive_delta() {
        assert_eq!(interaction_delta(1.0, 0.5), 4);
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
}
