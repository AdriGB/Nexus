use serde::Serialize;

use super::to_json;
use crate::simulation::{self, Entity, LifeStage, Personality};

#[derive(Serialize)]
struct UtilityScoresDto {
    eat: f32,
    explore: f32,
    rest: f32,
}

#[derive(Serialize)]
struct PersonalityDto {
    curiosity: f32,
    sociability: f32,
    cooperativeness: f32,
    caution: f32,
    persistence: f32,
}

impl From<Personality> for PersonalityDto {
    fn from(personality: Personality) -> Self {
        Self {
            curiosity: personality.curiosity,
            sociability: personality.sociability,
            cooperativeness: personality.cooperativeness,
            caution: personality.caution,
            persistence: personality.persistence,
        }
    }
}

#[derive(Serialize)]
struct EntityInfoDto {
    id: u32,
    x: u32,
    y: u32,
    sex: &'static str,
    hunger: f32,
    health: f32,
    age_ticks: u64,
    age_years: f64,
    lifespan_ticks: u64,
    pregnant: bool,
    pregnancy_due_tick: Option<u64>,
    activity: &'static str,
    remaining_path: usize,
    goal: &'static str,
    action: &'static str,
    goal_age_ticks: u64,
    known_resources: usize,
    known_entities: usize,
    known_chunks: usize,
    visible_entities: usize,
    utilities: UtilityScoresDto,
    movement_credit: f32,
    life_stage: &'static str,
    stage_movement_factor: f32,
    caregiver_id: Option<u32>,
    personality: PersonalityDto,
}

pub(crate) fn entity_info_json(entity: &Entity, tick: u64) -> String {
    let goal = entity
        .mind
        .current_goal
        .map_or("None", simulation::Goal::label);
    let action = entity
        .mind
        .current_action()
        .map_or("None", simulation::Action::label);
    let goal_age_ticks = entity
        .mind
        .current_goal
        .map_or(0, |_| tick.saturating_sub(entity.mind.goal_since_tick));
    let life_stage = LifeStage::from_age_ticks(entity.age_ticks);

    to_json(&EntityInfoDto {
        id: entity.id,
        x: entity.x,
        y: entity.y,
        sex: entity.sex.label(),
        hunger: entity.hunger,
        health: entity.health,
        age_ticks: entity.age_ticks,
        age_years: simulation::years_from_ticks(entity.age_ticks),
        lifespan_ticks: entity.lifespan_ticks,
        pregnant: entity.pregnancy.is_some(),
        pregnancy_due_tick: entity.pregnancy.map(|pregnancy| pregnancy.due_tick),
        activity: entity.activity.label(),
        remaining_path: entity.remaining_path_len(),
        goal,
        action,
        goal_age_ticks,
        known_resources: entity.mind.memory.known_resources.len(),
        known_entities: entity.mind.memory.known_entities.len(),
        known_chunks: entity.mind.memory.known_chunk_count(),
        visible_entities: entity.mind.visible_entities.len(),
        utilities: UtilityScoresDto {
            eat: entity.mind.utility_scores.eat,
            explore: entity.mind.utility_scores.explore,
            rest: entity.mind.utility_scores.rest,
        },
        movement_credit: entity.movement_credit,
        life_stage: life_stage.label(),
        stage_movement_factor: life_stage.movement_factor(),
        caregiver_id: entity.caregiver_id,
        personality: entity.personality.into(),
    })
}

#[derive(Serialize)]
struct KnownRelationshipInfoDto {
    id: u32,
    affinity: i16,
    interaction_count: u32,
    first_seen_tick: u64,
    last_seen_tick: u64,
    last_interaction_tick: u64,
    last_seen_x: u32,
    last_seen_y: u32,
    observed_ticks: u32,
    seek_retry_after_tick: Option<u64>,
}

/// Serializes known relationships ordered by emotional intensity:
/// absolute affinity descending, interaction count descending, then id.
/// Raw ticks let the frontend derive human-readable relative times.
pub(crate) fn entity_relationships_json(entity: &Entity) -> String {
    let mut relationships: Vec<KnownRelationshipInfoDto> = entity
        .mind
        .memory
        .known_entities
        .iter()
        .map(|known| KnownRelationshipInfoDto {
            id: known.id,
            affinity: known.affinity,
            interaction_count: known.interaction_count,
            first_seen_tick: known.first_seen_tick,
            last_seen_tick: known.last_seen_tick,
            last_interaction_tick: known.last_interaction_tick,
            last_seen_x: known.last_seen_x,
            last_seen_y: known.last_seen_y,
            observed_ticks: known.observed_ticks,
            seek_retry_after_tick: known.seek_retry_after_tick,
        })
        .collect();

    relationships.sort_unstable_by_key(|relationship| {
        use std::cmp::Reverse;

        (
            Reverse(relationship.affinity.unsigned_abs()),
            Reverse(relationship.interaction_count),
            relationship.id,
        )
    });

    to_json(&relationships)
}
