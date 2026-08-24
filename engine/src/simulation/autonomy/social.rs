use super::super::config::{MAX_HEALTH, MAX_HUNGER};
use super::super::entity::{Entity, EntityActivity, LifeStage, Personality};
use super::super::spatial::EntitySnapshot;
use super::super::time::TICKS_PER_DAY;
use super::exploration::plan_exploration;
use super::mind::{manhattan, Action, AffinityChangeRecord, Goal, KnownEntity, Mind};
use crate::pathfinding::{self, PathfindingWorkspace};
use crate::world::Grid;
use std::collections::HashMap;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(in crate::simulation) struct SocialInteraction {
    pub actor_id: u32,
    pub target_id: u32,
    pub location: (u32, u32),
    pub actor_location: (u32, u32),
    pub target_location: (u32, u32),
    pub actor_affinity_delta: i16,
    pub target_affinity_delta: i16,
    pub actor_affinity_change: Option<AffinityChangeRecord>,
    pub target_affinity_change: Option<AffinityChangeRecord>,
}

pub(in crate::simulation) const SOCIAL_RADIUS: u32 = 2;
pub(super) const MIN_INTERACTION_INTERVAL: u64 = 12;
pub(super) const MAX_INTERACTION_INTERVAL: u64 = 72;
const MIN_REMEMBERED_SOCIAL_SCORE: i32 = 100;
/// Scales how strongly the actor's own current needs amplify the
/// distance penalty when choosing a social target: hunger or low health
/// in [0, 100] maps to a need pressure in [1.0, 2.0] with this constant.
const NEED_DISTANCE_PENALTY: f32 = 1.0;
const STALE_PENALTY_PER_DAY: i32 = 10;

/// Actor-only need pressure: how much current hunger or low health makes
/// the actor prefer nearby social targets. Uses `max`, not a sum, so
/// being both hungry and injured never exceeds 2.0. Target needs are
/// never observed.
fn social_need_pressure(hunger: f32, health: f32) -> f32 {
    let hunger_pressure = (hunger / MAX_HUNGER).clamp(0.0, 1.0);
    let health_pressure = (1.0 - health / MAX_HEALTH).clamp(0.0, 1.0);

    1.0 + hunger_pressure.max(health_pressure) * NEED_DISTANCE_PENALTY
}

pub(super) fn remembered_social_score(
    known: &KnownEntity,
    tick: u64,
    origin: (u32, u32),
    personality: &Personality,
    need_pressure: f32,
) -> Option<i32> {
    if known.seek_on_cooldown(tick) {
        return None;
    }

    let distance = manhattan(origin, (known.last_seen_x, known.last_seen_y));
    let base_weight = (2.0 - personality.sociability * 1.5).max(0.5);
    let distance_penalty = (distance as f32 * base_weight * need_pressure) as i32;
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

pub(in crate::simulation) fn personality_compatibility(a: &Personality, b: &Personality) -> f32 {
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
    need_pressure: f32,
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

        let distance_weight = (2.0 - personality.sociability * 1.5).max(0.5) * need_pressure;
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

        let Some(score) = remembered_social_score(known, tick, origin, personality, need_pressure)
        else {
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
    let pressure = social_need_pressure(entity.hunger, entity.health);

    let Some(target_id) = select_social_target(
        &entity.mind,
        origin,
        entity.id,
        population,
        &entity.personality,
        pressure,
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
) -> Vec<SocialInteraction> {
    let id_to_index: HashMap<u32, usize> = entities
        .iter()
        .enumerate()
        .map(|(index, entity)| (entity.id, index))
        .collect();

    let mut pairs: Vec<(usize, usize, SocialInteraction)> = Vec::new();

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

            pairs.push((
                entity_index,
                b_index,
                SocialInteraction {
                    actor_id: a_id,
                    target_id: b_id,
                    location: a_pos,
                    actor_location: a_pos,
                    target_location: b_pos,
                    actor_affinity_delta: delta_a,
                    target_affinity_delta: delta_b,
                    actor_affinity_change: None,
                    target_affinity_change: None,
                },
            ));
        }
    }

    let mut interactions = Vec::with_capacity(pairs.len());
    for (index_a, index_b, mut interaction) in pairs {
        let recorded_a = entities[index_a].mind.memory.record_interaction(
            interaction.target_id,
            tick,
            interaction.actor_affinity_delta,
        );
        let recorded_b = entities[index_b].mind.memory.record_interaction(
            interaction.actor_id,
            tick,
            interaction.target_affinity_delta,
        );
        debug_assert!(recorded_a.is_some() && recorded_b.is_some());
        if let (Some(actor_change), Some(target_change)) = (recorded_a, recorded_b) {
            interaction.actor_affinity_change = actor_change;
            interaction.target_affinity_change = target_change;
            interactions.push(interaction);
        }
    }

    interactions
}

#[cfg(test)]
mod tests {
    use super::super::super::spatial::EntitySnapshot;
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
            select_social_target(&mind, (0, 0), 1, &[], &personality, 1.0, 10),
            Some(2)
        );

        assert!(mind.memory.mark_failed_social_seek(2, 10));
        assert_eq!(
            select_social_target(&mind, (0, 0), 1, &[], &personality, 1.0, 11),
            None
        );
        assert_eq!(
            select_social_target(
                &mind,
                (0, 0),
                1,
                &[],
                &personality,
                1.0,
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
            select_social_target(&mind, (0, 0), 1, &[], &personality, 1.0, 0),
            Some(3)
        );
    }

    // ── Need-aware social target selection ────────────────────────────────

    fn visible_scenario() -> (Mind, Vec<EntitySnapshot>) {
        let mut mind = Mind::default();
        mind.visible_entities = vec![2, 3];
        mind.memory.known_entities.push(remembered_entity(2, 245));
        mind.memory.known_entities.push(remembered_entity(3, 230));
        let population = vec![
            EntitySnapshot {
                id: 2,
                x: 12,
                y: 0,
                hunger: 0.0,
                caregiver_id: None,
                is_child: false,
                is_infant: false,
            },
            EntitySnapshot {
                id: 3,
                x: 2,
                y: 0,
                hunger: 0.0,
                caregiver_id: None,
                is_child: false,
                is_infant: false,
            },
        ];
        (mind, population)
    }

    fn remembered_at(id: u32, affinity: i16, distance_x: u32) -> KnownEntity {
        KnownEntity {
            id,
            first_seen_tick: 0,
            last_seen_tick: 0,
            last_seen_x: distance_x,
            last_seen_y: 0,
            observed_ticks: 1,
            affinity,
            last_interaction_tick: 0,
            interaction_count: 0,
            seek_retry_after_tick: None,
        }
    }

    #[test]
    fn social_need_pressure_increases_with_hunger() {
        assert_eq!(social_need_pressure(0.0, 100.0), 1.0);
        assert_eq!(social_need_pressure(50.0, 100.0), 1.5);
        assert_eq!(social_need_pressure(100.0, 100.0), 2.0);
    }

    #[test]
    fn social_need_pressure_increases_with_low_health() {
        assert_eq!(social_need_pressure(0.0, 100.0), 1.0);
        assert_eq!(social_need_pressure(0.0, 50.0), 1.5);
        assert_eq!(social_need_pressure(0.0, 0.0), 2.0);
    }

    #[test]
    fn low_need_prefers_high_affinity_distant_target() {
        let (mind, population) = visible_scenario();
        let personality = personality(0.0, 0.5);

        let selected = select_social_target(&mind, (0, 0), 1, &population, &personality, 1.2, 0);

        assert_eq!(
            selected,
            Some(2),
            "low need allows crossing ground for a closer-valued friend"
        );
    }

    #[test]
    fn moderate_need_prefers_closer_social_target() {
        let (mind, population) = visible_scenario();
        let personality = personality(0.0, 0.5);

        let selected = select_social_target(&mind, (0, 0), 1, &population, &personality, 1.7, 0);

        assert_eq!(selected, Some(3), "moderate need prefers the nearby target");
    }

    #[test]
    fn low_need_prefers_high_affinity_distant_remembered_target() {
        let mut mind = Mind::default();
        mind.memory.known_entities.push(remembered_at(2, 245, 12));
        mind.memory.known_entities.push(remembered_at(3, 230, 2));
        let personality = personality(0.0, 0.5);

        let selected = select_social_target(&mind, (0, 0), 1, &[], &personality, 1.2, 0);

        assert_eq!(selected, Some(2));
    }

    #[test]
    fn moderate_need_prefers_closer_remembered_target() {
        let mut mind = Mind::default();
        mind.memory.known_entities.push(remembered_at(2, 245, 12));
        mind.memory.known_entities.push(remembered_at(3, 230, 2));
        let personality = personality(0.0, 0.5);

        let selected = select_social_target(&mind, (0, 0), 1, &[], &personality, 1.7, 0);

        assert_eq!(selected, Some(3));
    }

    #[test]
    fn need_aware_social_selection_is_deterministic() {
        let (mind_a, population_a) = visible_scenario();
        let (mind_b, population_b) = visible_scenario();
        let personality = personality(0.0, 0.5);

        let first = select_social_target(&mind_a, (0, 0), 1, &population_a, &personality, 1.7, 0);
        let second = select_social_target(&mind_b, (0, 0), 1, &population_b, &personality, 1.7, 0);

        assert_eq!(first, second);
    }
}
