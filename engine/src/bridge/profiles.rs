use serde::Serialize;

use super::to_json;
use crate::simulation::{
    AutonomyProfile, EntityPassBreakdown, PerformanceSummary, PhaseProfile, PopulationStats,
    PostPassProfile, StateGauges, WorkCounters,
};

pub(crate) fn performance_summary_json(summary: &PerformanceSummary) -> String {
    to_json(summary)
}

#[derive(Serialize)]
struct StateGaugesDto {
    entities_alive: u64,
    known_entities_total: u64,
    known_entities_max_per_entity: u64,
    known_resources_total: u64,
    known_resources_max_per_entity: u64,
    known_dead_entities_total: u64,
    active_grief_states: u64,
    recent_events_len: u64,
    recent_events_capacity: u64,
    households_active: u64,
    genealogy_links: u64,
}

impl From<&StateGauges> for StateGaugesDto {
    fn from(state: &StateGauges) -> Self {
        Self {
            entities_alive: state.entities_alive,
            known_entities_total: state.known_entities_total,
            known_entities_max_per_entity: state.known_entities_max_per_entity,
            known_resources_total: state.known_resources_total,
            known_resources_max_per_entity: state.known_resources_max_per_entity,
            known_dead_entities_total: state.known_dead_entities_total,
            active_grief_states: state.active_grief_states,
            recent_events_len: state.recent_events_len,
            recent_events_capacity: state.recent_events_capacity,
            households_active: state.households_active,
            genealogy_links: state.genealogy_links,
        }
    }
}

#[derive(Serialize)]
struct WorkCountersDto {
    entities_processed: u64,
    entities_perceived: u64,
    goal_evaluations: u64,
    goal_changes: u64,
    plans_created: u64,
    actions_executed: u64,
    social_interactions: u64,
    social_pairs_scanned: u64,
    social_pairs_in_radius: u64,
    social_pairs_mutual: u64,
    social_pairs_due: u64,
    encounters_recorded: u64,
    discoveries_recorded: u64,
    food_consumptions_recorded: u64,
    food_share_attempts: u64,
    household_deposit_attempts: u64,
    household_withdraw_attempts: u64,
    household_conflict_attempts: u64,
    spatial_queries: u64,
    pathfinding_searches: u64,
    pathfinding_nodes_expanded: u64,
    events_created: u64,
    orphan_reassignment_scans: u64,
    household_sync_scans: u64,
    household_migration_scans: u64,
    conception_scans: u64,
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
            social_pairs_scanned: work.social_pairs_scanned,
            social_pairs_in_radius: work.social_pairs_in_radius,
            social_pairs_mutual: work.social_pairs_mutual,
            social_pairs_due: work.social_pairs_due,
            encounters_recorded: work.encounters_recorded,
            discoveries_recorded: work.discoveries_recorded,
            food_consumptions_recorded: work.food_consumptions_recorded,
            food_share_attempts: work.food_share_attempts,
            household_deposit_attempts: work.household_deposit_attempts,
            household_withdraw_attempts: work.household_withdraw_attempts,
            household_conflict_attempts: work.household_conflict_attempts,
            spatial_queries: work.spatial_queries,
            pathfinding_searches: work.pathfinding_searches,
            pathfinding_nodes_expanded: work.pathfinding_nodes_expanded,
            events_created: work.events_created,
            orphan_reassignment_scans: work.orphan_reassignment_scans,
            household_sync_scans: work.household_sync_scans,
            household_migration_scans: work.household_migration_scans,
            conception_scans: work.conception_scans,
        }
    }
}

/// The per-entity pass, decomposed (#207). Full population, microseconds.
///
/// `attributed_us` is the sum of the four blocks; `residual_us` is what is left
/// of `entity_pass_us` once they are subtracted. Both live here rather than
/// being recomputed by the consumer so that there is one definition of the
/// residual, and it is the one the engine uses.
#[derive(Serialize)]
struct EntityPassBreakdownDto {
    perceive_entities_us: u64,
    plan_validation_us: u64,
    planning_us: u64,
    resource_memory_us: u64,
    attributed_us: u64,
    residual_us: u64,
}

impl EntityPassBreakdownDto {
    /// Not a `From`: the residual is measured against the wall clock of the
    /// whole loop, which lives on `AutonomyProfile` and not on the breakdown.
    /// Taking it as an argument keeps that dependency visible instead of
    /// leaving a `residual_us` that is silently always zero.
    fn new(breakdown: &EntityPassBreakdown, entity_pass_us: u64) -> Self {
        let us = |nanos: u64| nanos / 1_000;
        let perceive_entities_us = us(breakdown.perceive_entities_ns);
        let plan_validation_us = us(breakdown.plan_validation_ns);
        let planning_us = us(breakdown.planning_ns);
        let resource_memory_us = us(breakdown.resource_memory_ns);
        let attributed_us =
            perceive_entities_us + plan_validation_us + planning_us + resource_memory_us;
        Self {
            perceive_entities_us,
            plan_validation_us,
            planning_us,
            resource_memory_us,
            attributed_us,
            residual_us: entity_pass_us.saturating_sub(attributed_us),
        }
    }
}

#[derive(Serialize)]
struct PhaseProfileDto {
    #[serde(flatten)]
    work: WorkCountersDto,
    #[serde(flatten)]
    state: StateGaugesDto,
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
        state: (&profile.state).into(),
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

/// Two populations are timed here and the payload has to keep them apart.
///
/// `social_us` and `entity_pass_us` are one timer around one whole pass, so
/// they cover every entity. Everything prefixed `sampled_` is timed per entity
/// over a `PROFILE_SAMPLE_RATE` sample. Adding a `sampled_` value to a
/// full-population one produces a number that means nothing, which is exactly
/// what the debug panel did before #191: it summed six sampled sub-phases
/// against `social_us` and showed the result as percentages.
///
/// `resource_perception_us` is deliberately **not** projected. The engine
/// defines it as `memory_reconciliation_us + visible_scan_us`, so shipping it
/// next to its own components invites any consumer that sums the fields to
/// double count that work.
#[derive(Serialize)]
struct AutonomyProfileDto {
    #[serde(flatten)]
    work: WorkCountersDto,
    #[serde(flatten)]
    state: StateGaugesDto,
    /// Full population. One timer around the social pass.
    social_us: u64,
    /// Full population. One timer around the per-entity loop.
    entity_pass_us: u64,
    /// The same loop, decomposed. Full population, microseconds.
    ///
    /// Nested rather than flattened: these four are one population and the
    /// `sampled_*` fields below are another, and the point of #191 was that
    /// mixing them silently produces nonsense. Keeping them in separate
    /// namespaces makes the mistake hard to make by accident.
    entity_pass: EntityPassBreakdownDto,
    /// `social_us + entity_pass_us`. The only total here that mixes no sampled
    /// value; compare it against `step_total_us`.
    attributed_passes_us: u64,
    /// Wall clock of the profiled step, the denominator for
    /// `attributed_passes_us`.
    step_total_us: u64,
    sampled_entity_perception_us: u64,
    sampled_plan_validation_us: u64,
    sampled_planning_us: u64,
    sampled_action_us: u64,
    sampled_memory_reconciliation_us: u64,
    sampled_visible_scan_us: u64,
    sampled_entities: u32,
    planned_entities: u32,
    urgent_interrupts: u32,
    sampled_known_resources_total: u32,
    sampled_known_resources_max: u32,
    visible_resources_seen: u32,
    post_pass: PostPassProfileDto,
}

impl From<&AutonomyProfile> for AutonomyProfileDto {
    fn from(profile: &AutonomyProfile) -> Self {
        Self {
            work: (&profile.work).into(),
            state: (&profile.state).into(),
            social_us: profile.social_us,
            entity_pass_us: profile.entity_pass_us,
            entity_pass: EntityPassBreakdownDto::new(&profile.entity_pass, profile.entity_pass_us),
            attributed_passes_us: profile.social_us.saturating_add(profile.entity_pass_us),
            step_total_us: profile.step_total_us,
            sampled_entity_perception_us: profile.entity_perception_us,
            sampled_plan_validation_us: profile.plan_validation_us,
            sampled_planning_us: profile.planning_us,
            sampled_action_us: profile.action_us,
            sampled_memory_reconciliation_us: profile.memory_reconciliation_us,
            sampled_visible_scan_us: profile.visible_scan_us,
            sampled_entities: profile.sampled_entities,
            planned_entities: profile.planned_entities,
            urgent_interrupts: profile.urgent_interrupts,
            sampled_known_resources_total: profile.sampled_known_resources_total,
            sampled_known_resources_max: profile.sampled_known_resources_max,
            visible_resources_seen: profile.visible_resources_seen,
            post_pass: (&profile.post_pass).into(),
        }
    }
}

#[derive(Serialize)]
struct PostPassProfileDto {
    resource_discoveries_us: u64,
    entity_encounters_us: u64,
    food_consumptions_us: u64,
    social_interactions_us: u64,
    food_share_us: u64,
    household_deposit_us: u64,
    household_withdraw_us: u64,
    household_conflict_us: u64,
    total_us: u64,
}

impl From<&PostPassProfile> for PostPassProfileDto {
    fn from(post_pass: &PostPassProfile) -> Self {
        Self {
            resource_discoveries_us: post_pass.resource_discoveries_us,
            entity_encounters_us: post_pass.entity_encounters_us,
            food_consumptions_us: post_pass.food_consumptions_us,
            social_interactions_us: post_pass.social_interactions_us,
            food_share_us: post_pass.food_share_us,
            household_deposit_us: post_pass.household_deposit_us,
            household_withdraw_us: post_pass.household_withdraw_us,
            household_conflict_us: post_pass.household_conflict_us,
            total_us: post_pass.total_us(),
        }
    }
}

pub(crate) fn autonomy_profile_json(profile: &AutonomyProfile) -> String {
    to_json(&AutonomyProfileDto::from(profile))
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

#[cfg(test)]
mod tests {
    use super::*;

    /// The DTOs are hand-written projections, so nothing stops one from silently
    /// drifting out of sync when a field is added upstream. Both sides of each
    /// projection are `Serialize`, so comparing key sets catches the drift here
    /// instead of in the UI. `WorkCountersDto` has fallen behind twice already,
    /// after #178 and after #194.
    fn assert_same_keys(what: &str, source: &serde_json::Value, projected: &serde_json::Value) {
        let source_keys = source.as_object().expect("source must be an object");
        let projected_keys = projected.as_object().expect("projection must be an object");

        let missing: Vec<&String> = source_keys
            .keys()
            .filter(|key| !projected_keys.contains_key(*key))
            .collect();
        assert!(
            missing.is_empty(),
            "{what} is missing {missing:?}; map them in the DTO struct and in its From impl"
        );

        let stale: Vec<&String> = projected_keys
            .keys()
            .filter(|key| !source_keys.contains_key(*key))
            .collect();
        assert!(
            stale.is_empty(),
            "{what} projects {stale:?}, which no longer exist upstream"
        );
    }

    #[test]
    fn work_counters_dto_covers_every_work_counter() {
        let counters = WorkCounters::default();
        assert_same_keys(
            "WorkCountersDto",
            &serde_json::to_value(&counters).expect("WorkCounters serializes"),
            &serde_json::to_value(WorkCountersDto::from(&counters))
                .expect("WorkCountersDto serializes"),
        );
    }

    #[test]
    fn state_gauges_dto_covers_every_state_gauge() {
        let gauges = StateGauges::default();
        assert_same_keys(
            "StateGaugesDto",
            &serde_json::to_value(&gauges).expect("StateGauges serializes"),
            &serde_json::to_value(StateGaugesDto::from(&gauges))
                .expect("StateGaugesDto serializes"),
        );
    }

    /// Timings that cover every entity: one timer around one whole pass.
    /// Anything else ending in `_us` is timed per entity over a
    /// `PROFILE_SAMPLE_RATE` sample and must say so in its name.
    const FULL_POPULATION_TIMINGS: [&str; 4] = [
        "social_us",
        "entity_pass_us",
        "attributed_passes_us",
        "step_total_us",
    ];

    /// Keys `AutonomyProfileDto` declares itself, minus the flattened work
    /// counters and state gauges, which are covered by the tests above.
    fn autonomy_dto_own_keys() -> Vec<String> {
        let dto = serde_json::to_value(AutonomyProfileDto::from(&AutonomyProfile::default()))
            .expect("AutonomyProfileDto serializes");
        let work = serde_json::to_value(WorkCountersDto::from(&WorkCounters::default()))
            .expect("WorkCountersDto serializes");
        let state = serde_json::to_value(StateGaugesDto::from(&StateGauges::default()))
            .expect("StateGaugesDto serializes");
        let flattened: std::collections::BTreeSet<String> = work
            .as_object()
            .expect("object")
            .keys()
            .chain(state.as_object().expect("object").keys())
            .cloned()
            .collect();
        dto.as_object()
            .expect("object")
            .keys()
            .filter(|key| !flattened.contains(*key))
            .cloned()
            .collect()
    }

    /// The payload has to say which timings are sampled. Before #191 the debug
    /// panel summed six per-entity sub-phases against `social_us`, which covers
    /// every entity, and showed the result as percentages. No consumer could
    /// tell the two apart from the field names.
    #[test]
    fn autonomy_profile_dto_marks_sampled_timings() {
        for key in autonomy_dto_own_keys() {
            if !key.ends_with("_us") || FULL_POPULATION_TIMINGS.contains(&key.as_str()) {
                continue;
            }
            assert!(
                key.starts_with("sampled_"),
                "`{key}` is timed per entity over a sample, so it must be prefixed `sampled_`; \
                 adding it to a full-population timing produces a meaningless total (#191)"
            );
        }
    }

    /// `resource_perception_us` is `memory_reconciliation + visible_scan` in the
    /// engine. Both components are projected, so shipping the rollup makes any
    /// consumer that sums the fields double count that work.
    #[test]
    fn autonomy_profile_dto_omits_the_resource_perception_rollup() {
        assert!(
            !autonomy_dto_own_keys()
                .iter()
                .any(|key| key == "resource_perception_us"),
            "`resource_perception_us` is a rollup of two fields that are also projected; \
             reporting it double counts (#191)"
        );
    }

    /// The two tests above inspect `AutonomyProfileDto` on its own. This one
    /// goes through the function the bridge actually calls, so an edit that
    /// assembles the payload by hand instead of through the DTO cannot bring
    /// back an unprefixed sampled timing or the rollup without failing here.
    #[test]
    fn autonomy_profile_json_shares_the_dtos_discipline() {
        let json = super::autonomy_profile_json(&AutonomyProfile {
            social_us: 300,
            entity_pass_us: 700,
            memory_reconciliation_us: 40,
            visible_scan_us: 60,
            ..AutonomyProfile::default()
        });
        let keys: std::collections::BTreeSet<String> =
            serde_json::from_str::<serde_json::Value>(&json)
                .expect("payload is JSON")
                .as_object()
                .expect("payload is an object")
                .keys()
                .cloned()
                .collect();

        assert!(
            !keys.contains("resource_perception_us"),
            "the payload reintroduced the rollup that double counts (#191)"
        );
        for key in keys {
            if !key.ends_with("_us") || FULL_POPULATION_TIMINGS.contains(&key.as_str()) {
                continue;
            }
            assert!(
                key.starts_with("sampled_"),
                "`{key}` is timed over a sample but is not prefixed, so no consumer can \
                 tell it apart from a full-population timing (#191)"
            );
        }
    }

    #[test]
    fn attributed_passes_is_the_sum_of_the_two_full_population_passes() {
        let profile = AutonomyProfile {
            social_us: 300,
            entity_pass_us: 700,
            ..AutonomyProfile::default()
        };
        let dto = serde_json::to_value(AutonomyProfileDto::from(&profile))
            .expect("AutonomyProfileDto serializes");
        assert_eq!(dto["attributed_passes_us"], 1_000);
    }

    /// #207: the entity-pass breakdown is nested, so the tripwire above cannot
    /// see it — it only walks top-level keys. This is the same check one level
    /// down. Nothing inside `entity_pass` may be sampled, and every block that
    /// gets timed has to be emitted, or the residual stops meaning what it
    /// claims to mean.
    #[test]
    fn entity_pass_breakdown_is_full_population_and_complete() {
        let json = super::autonomy_profile_json(&AutonomyProfile {
            entity_pass_us: 1_000,
            entity_pass: EntityPassBreakdown {
                perceive_entities_ns: 400_000,
                plan_validation_ns: 300_000,
                planning_ns: 200_000,
                resource_memory_ns: 100_000,
            },
            ..AutonomyProfile::default()
        });
        let payload: serde_json::Value = serde_json::from_str(&json).expect("payload is JSON");
        let entity_pass = payload["entity_pass"]
            .as_object()
            .expect("entity_pass is a nested object");

        for key in entity_pass.keys() {
            assert!(
                !key.starts_with("sampled_"),
                "`{key}` sits inside the full-population breakdown but is named as sampled; \
                 a consumer reading it against `entity_pass_us` would mix populations (#191)"
            );
        }
        assert_eq!(entity_pass["perceive_entities_us"], 400);
        assert_eq!(entity_pass["plan_validation_us"], 300);
        assert_eq!(entity_pass["planning_us"], 200);
        assert_eq!(entity_pass["resource_memory_us"], 100);
        assert_eq!(entity_pass["attributed_us"], 1_000);
        assert_eq!(entity_pass["residual_us"], 0);
        assert_eq!(
            entity_pass.len(),
            6,
            "a block is being timed but not emitted, or the reverse"
        );
    }
}
