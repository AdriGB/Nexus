use serde::Serialize;

use super::to_json;
use crate::simulation::{self, Entity, ItemKind, LifeStage, Personality};

#[derive(Serialize)]
struct InventoryItemDto {
    kind: &'static str,
    amount: u16,
}

#[derive(Serialize)]
struct InventoryDto {
    capacity: u16,
    used_capacity: u16,
    remaining_capacity: u16,
    items: Vec<InventoryItemDto>,
}

#[derive(Serialize)]
struct UtilityScoresDto {
    eat: f32,
    acquire_resource: f32,
    explore: f32,
    rest: f32,
    socialize: f32,
    share_food: f32,
}

#[derive(Serialize)]
struct DecisionExplanationDto {
    chosen_goal: &'static str,
    highest_utility_goal: &'static str,
    chosen_score: f32,
    highest_score: f32,
    switch_margin: f32,
    reason: &'static str,
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
    action_progress_ticks: u32,
    action_duration_ticks: Option<u32>,
    goal_age_ticks: u64,
    known_resources: usize,
    known_entities: usize,
    known_chunks: usize,
    visible_entities: usize,
    utilities: UtilityScoresDto,
    decision_explanation: Option<DecisionExplanationDto>,
    movement_credit: f32,
    life_stage: &'static str,
    stage_movement_factor: f32,
    caregiver_id: Option<u32>,
    partner_id: Option<u32>,
    mother_id: Option<u32>,
    father_id: Option<u32>,
    personality: PersonalityDto,
    inventory: InventoryDto,
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
        action_progress_ticks: entity.action_tick,
        action_duration_ticks: matches!(
            entity.mind.current_action(),
            Some(simulation::Action::Gather(_))
        )
        .then_some(simulation::GATHER_DURATION_TICKS),
        goal_age_ticks,
        known_resources: entity.mind.memory.known_resources.len(),
        known_entities: entity.mind.memory.known_entities.len(),
        known_chunks: entity.mind.memory.known_chunk_count(),
        visible_entities: entity.mind.visible_entities.len(),
        utilities: UtilityScoresDto {
            eat: entity.mind.utility_scores.eat,
            acquire_resource: entity.mind.utility_scores.acquire_resource,
            explore: entity.mind.utility_scores.explore,
            rest: entity.mind.utility_scores.rest,
            socialize: entity.mind.utility_scores.socialize,
            share_food: entity.mind.utility_scores.share_food,
        },
        decision_explanation: entity.mind.decision_explanation.map(|explanation| {
            DecisionExplanationDto {
                chosen_goal: explanation.chosen_goal.label(),
                highest_utility_goal: explanation.highest_utility_goal.label(),
                chosen_score: explanation.chosen_score,
                highest_score: explanation.highest_score,
                switch_margin: explanation.switch_margin,
                reason: explanation.reason.label(),
            }
        }),
        movement_credit: entity.movement_credit,
        life_stage: life_stage.label(),
        stage_movement_factor: life_stage.movement_factor(),
        caregiver_id: entity.caregiver_id,
        partner_id: entity.partner_id,
        mother_id: entity.mother_id,
        father_id: entity.father_id,
        personality: entity.personality.into(),
        inventory: InventoryDto {
            capacity: entity.inventory.capacity(),
            used_capacity: entity.inventory.used_capacity(),
            remaining_capacity: entity.inventory.remaining_capacity(),
            items: ItemKind::ALL
                .into_iter()
                .filter_map(|kind| {
                    let amount = entity.inventory.amount(kind);
                    (amount > 0).then_some(InventoryItemDto {
                        kind: kind.label(),
                        amount,
                    })
                })
                .collect(),
        },
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
