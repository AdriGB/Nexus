use serde::Serialize;

use super::to_json;
use crate::simulation::{AutonomyProfile, PhaseProfile, PopulationStats, WorkCounters};

#[derive(Serialize)]
struct WorkCountersDto {
    entities_processed: u64,
    entities_perceived: u64,
    goal_evaluations: u64,
    goal_changes: u64,
    plans_created: u64,
    actions_executed: u64,
    social_interactions: u64,
    spatial_queries: u64,
    pathfinding_searches: u64,
    pathfinding_nodes_expanded: u64,
    events_created: u64,
}

impl From<&WorkCounters> for WorkCountersDto {
    fn from(work: &WorkCounters) -> Self {
        Self {
            entities_processed: work.entities_processed,
            entities_perceived: work.entities_perceived,
            goal_evaluations: work.goal_evaluations,
            goal_changes: work.goal_changes,
            plans_created: work.plans_created,
            actions_executed: work.actions_executed,
            social_interactions: work.social_interactions,
            spatial_queries: work.spatial_queries,
            pathfinding_searches: work.pathfinding_searches,
            pathfinding_nodes_expanded: work.pathfinding_nodes_expanded,
            events_created: work.events_created,
        }
    }
}

#[derive(Serialize)]
struct PhaseProfileDto {
    #[serde(flatten)]
    work: WorkCountersDto,
    world_maintenance_us: u64,
    physiology_us: u64,
    dependent_care_us: u64,
    households_us: u64,
    spatial_index_us: u64,
    autonomy_us: u64,
    survival_us: u64,
    mortality_us: u64,
    lifecycle_us: u64,
    relationships_us: u64,
    reproduction_us: u64,
    total_us: u64,
}

pub(crate) fn phase_profile_json(profile: &PhaseProfile) -> String {
    to_json(&PhaseProfileDto {
        work: (&profile.work).into(),
        world_maintenance_us: profile.world_maintenance_us,
        physiology_us: profile.physiology_us,
        dependent_care_us: profile.dependent_care_us,
        households_us: profile.households_us,
        spatial_index_us: profile.spatial_index_us,
        autonomy_us: profile.autonomy_us,
        survival_us: profile.survival_us,
        mortality_us: profile.mortality_us,
        lifecycle_us: profile.lifecycle_us,
        relationships_us: profile.relationships_us,
        reproduction_us: profile.reproduction_us,
        total_us: profile.total_us,
    })
}

#[derive(Serialize)]
struct AutonomyProfileDto {
    #[serde(flatten)]
    work: WorkCountersDto,
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
        work: (&profile.work).into(),
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
