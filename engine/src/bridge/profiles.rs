use serde::Serialize;

use super::to_json;
use crate::simulation::{AutonomyProfile, PhaseProfile, PopulationStats};

#[derive(Serialize)]
struct PhaseProfileDto {
    physiology_us: u64,
    population_index_us: u64,
    autonomy_us: u64,
    starvation_us: u64,
    resource_changes_us: u64,
    remove_dead_us: u64,
    pregnancies_us: u64,
    conceptions_us: u64,
    total_us: u64,
}

pub(crate) fn phase_profile_json(profile: &PhaseProfile) -> String {
    to_json(&PhaseProfileDto {
        physiology_us: profile.physiology_us,
        population_index_us: profile.population_index_us,
        autonomy_us: profile.autonomy_us,
        starvation_us: profile.starvation_us,
        resource_changes_us: profile.resource_changes_us,
        remove_dead_us: profile.remove_dead_us,
        pregnancies_us: profile.pregnancies_us,
        conceptions_us: profile.conceptions_us,
        total_us: profile.total_us,
    })
}

#[derive(Serialize)]
struct AutonomyProfileDto {
    resource_perception_us: u64,
    entity_perception_us: u64,
    plan_validation_us: u64,
    planning_us: u64,
    action_us: u64,
    sampled_entities: u32,
    planned_entities: u32,
    urgent_interrupts: u32,
    memory_reconciliation_us: u64,
    visible_scan_us: u64,
    known_resources_total: u32,
    known_resources_max: u32,
    visible_resources_seen: u32,
    social_us: u64,
}

pub(crate) fn autonomy_profile_json(profile: &AutonomyProfile) -> String {
    to_json(&AutonomyProfileDto {
        resource_perception_us: profile.resource_perception_us,
        entity_perception_us: profile.entity_perception_us,
        plan_validation_us: profile.plan_validation_us,
        planning_us: profile.planning_us,
        action_us: profile.action_us,
        sampled_entities: profile.sampled_entities,
        planned_entities: profile.planned_entities,
        urgent_interrupts: profile.urgent_interrupts,
        memory_reconciliation_us: profile.memory_reconciliation_us,
        visible_scan_us: profile.visible_scan_us,
        known_resources_total: profile.known_resources_total,
        known_resources_max: profile.known_resources_max,
        visible_resources_seen: profile.visible_resources_seen,
        social_us: profile.social_us,
    })
}

#[derive(Serialize)]
struct PopulationStatsDto {
    population: u32,
    births: u64,
    deaths: u64,
    females: u32,
    males: u32,
    pregnant: u32,
    hungry: u32,
    seeking_food: u32,
    average_hunger: f32,
    food_consumed: u64,
}

pub(crate) fn population_stats_json(stats: PopulationStats) -> String {
    to_json(&PopulationStatsDto {
        population: stats.population,
        births: stats.births,
        deaths: stats.deaths,
        females: stats.females,
        males: stats.males,
        pregnant: stats.pregnant,
        hungry: stats.hungry,
        seeking_food: stats.seeking_food,
        average_hunger: stats.average_hunger,
        food_consumed: stats.food_consumed,
    })
}
