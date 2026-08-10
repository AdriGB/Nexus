use super::super::entity::{Entity, LifeStage, Personality};
use super::super::spatial::EntitySnapshot;
use super::mind::manhattan;
use std::collections::HashMap;

const SOCIAL_RADIUS: u32 = 2;
const MIN_INTERACTION_INTERVAL: u64 = 12;
const MAX_INTERACTION_INTERVAL: u64 = 72;

fn interaction_interval(a: &Personality, b: &Personality) -> u64 {
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
}
