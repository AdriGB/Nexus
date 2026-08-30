mod action;
mod conflict;
mod decision;
mod exploration;
mod mind;
mod perception;
mod profiling;
mod relationships;
mod social;

#[cfg(test)]
pub use self::decision::evaluate_goals;
#[cfg(test)]
pub use self::exploration::exploration_target;
pub(in crate::simulation) use self::mind::AffinityChangeRecord;
#[cfg(test)]
pub use self::mind::KnownEntity;
#[cfg(test)]
pub use self::mind::KnownResource;
pub use self::mind::{Action, Goal, GriefState, Mind};
#[cfg(test)]
pub(in crate::simulation) use self::mind::{
    RELATIONSHIP_DECAY_PER_DAY, RELATIONSHIP_DECAY_START_TICKS,
};
#[cfg(test)]
pub use self::perception::perceive;
pub(in crate::simulation) use self::perception::{EntityEncounter, ResourceDiscovery};
pub(in crate::simulation) use self::social::SOCIAL_RADIUS;

#[cfg(test)]
pub(super) use self::action::effective_movement_speed;
pub(super) use self::action::ActionOutcome;
pub(in crate::simulation) use self::action::FoodShareAttempt;
pub(in crate::simulation) use self::action::HouseholdConflictAttempt;
pub(in crate::simulation) use self::action::HouseholdDepositAttempt;
pub(in crate::simulation) use self::action::HouseholdWithdrawAttempt;
#[cfg(test)]
pub(in crate::simulation) use self::conflict::best_household_conflict_candidate;
pub(in crate::simulation) use self::conflict::HOUSEHOLD_EXIT_AFFINITY_THRESHOLD;
pub(in crate::simulation) use self::decision::DecisionContext;
use self::decision::HouseholdDecisionContext;
#[cfg(test)]
pub(in crate::simulation) use self::decision::{
    DEPENDENT_PROTECTION_TRIGGER_DISTANCE, DEPENDENT_REUNION_RADIUS,
};
pub(crate) use self::mind::GATHER_DURATION_TICKS;
pub(super) use self::mind::URGENT_HUNGER_THRESHOLD;
pub(in crate::simulation) use self::mind::{GRIEF_MAX_DURATION_TICKS, GRIEF_MIN_DURATION_TICKS};
pub(super) use self::profiling::should_profile_entity;
pub(crate) use self::profiling::{AutonomyProfile, EntityPassBreakdown, PostPassProfile};
pub(in crate::simulation) use self::relationships::{
    close_relationship_role_between, CloseRelationshipRole, RelationshipIdentity,
};
pub(in crate::simulation) use self::social::personality_compatibility;
pub(in crate::simulation) use self::social::SocialInteraction;

use super::entity::{Entity, LifeStage};
use super::inventory::ItemKind;
use super::spatial::{EntitySnapshot, SpatialGrid};
use crate::pathfinding::PathfindingWorkspace;
use crate::world::Grid;
use web_time::Instant;

const HOUSEHOLD_PERSONAL_FOOD_RESERVE: u16 = 20;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(in crate::simulation) struct HouseholdAutonomyContext {
    pub residence: (u32, u32),
    pub migration_target: Option<(u32, u32)>,
    pub storage_remaining_capacity: u16,
    pub storage_food_amount: u16,
}

pub(super) struct EntityUpdateContext<'a> {
    /// Posiciones en el snapshot de población de quienes tienen a esta entidad
    /// como cuidador, ya resueltas por el índice y en orden de población. Vacío
    /// si nadie depende de ella, que es el caso más común.
    pub dependents: &'a [u32],
    pub household: Option<HouseholdAutonomyContext>,
    pub profile: Option<&'a mut AutonomyProfile>,
    pub work: Option<&'a mut crate::simulation::WorkCounters>,
    /// Full-population accumulator for the per-entity pass. Unlike `profile`,
    /// this one is **not** filtered by `should_profile_entity`: every entity
    /// contributes, which is the whole point. `None` in a non-profiled tick, so
    /// the timers cost nothing on the normal path.
    pub entity_pass: Option<&'a mut EntityPassBreakdown>,
}

pub(super) fn process_social_interactions(
    entities: &mut [Entity],
    population: &[EntitySnapshot],
    tick: u64,
    work: Option<&mut crate::simulation::WorkCounters>,
) -> Vec<SocialInteraction> {
    social::process_social_interactions(entities, population, tick, work)
}

pub(in crate::simulation) fn record_directed_affinity(
    entity: &mut Entity,
    target_id: u32,
    tick: u64,
    delta: i16,
) -> Option<AffinityChangeRecord> {
    entity
        .mind
        .memory
        .record_interaction(target_id, tick, delta)
        .flatten()
}

pub(super) fn update_entity(
    entity: &mut Entity,
    world: &mut Grid,
    tick: u64,
    population: &[EntitySnapshot],
    spatial_grid: &SpatialGrid,
    pathfinding_workspace: &mut PathfindingWorkspace,
    context: EntityUpdateContext<'_>,
) -> (ActionOutcome, Vec<ResourceDiscovery>, Vec<EntityEncounter>) {
    let EntityUpdateContext {
        dependents,
        household: household_context,
        mut profile,
        mut work,
        mut entity_pass,
    } = context;
    if let Some(work) = work.as_deref_mut() {
        work.entities_perceived += 1;
    }
    entity.mind.prune_expired_grief(tick);
    let position = (entity.x, entity.y);

    let pass_start = entity_pass.as_ref().map(|_| Instant::now());
    let start = profile.as_ref().map(|_| Instant::now());
    perception::reconcile_resource_memory(&mut entity.mind, world, position, tick);
    if let Some(profile) = profile.as_deref_mut() {
        profile.memory_reconciliation_us += start
            .expect("profile timer must exist")
            .elapsed()
            .as_micros() as u64;
    }

    let start = profile.as_ref().map(|_| Instant::now());
    let (visible_count, discoveries) =
        perception::scan_visible_resources(&mut entity.mind, entity.id, world, position, tick);
    if let Some(profile) = profile.as_deref_mut() {
        profile.visible_scan_us += start
            .expect("profile timer must exist")
            .elapsed()
            .as_micros() as u64;
        profile.visible_resources_seen += visible_count;
        let known_resources = entity.mind.memory.known_resources.len() as u32;
        profile.sampled_known_resources_total += known_resources;
        profile.sampled_known_resources_max =
            profile.sampled_known_resources_max.max(known_resources);
        profile.resource_perception_us = profile
            .memory_reconciliation_us
            .saturating_add(profile.visible_scan_us);
    }
    if let Some(breakdown) = entity_pass.as_mut() {
        breakdown.resource_memory_ns += pass_start
            .expect("entity pass timer must exist")
            .elapsed()
            .as_nanos() as u64;
    }

    let pass_start = entity_pass.as_ref().map(|_| Instant::now());
    let start = profile.as_ref().map(|_| Instant::now());
    let encounters = perception::perceive_entities(
        &mut entity.mind,
        entity.id,
        position,
        tick,
        population,
        spatial_grid,
    );
    if let Some(profile) = profile.as_deref_mut() {
        profile.entity_perception_us += start
            .expect("profile timer must exist")
            .elapsed()
            .as_micros() as u64;
    }
    if let Some(breakdown) = entity_pass.as_mut() {
        breakdown.perceive_entities_ns += pass_start
            .expect("entity pass timer must exist")
            .elapsed()
            .as_nanos() as u64;
    }

    let pass_start = entity_pass.as_ref().map(|_| Instant::now());
    let start = profile.as_ref().map(|_| Instant::now());
    decision::invalidate_obsolete_food_plan(entity);
    if entity.mind.invalidate_known_dead_target_plan() {
        entity.path.clear();
        entity.path_index = 0;
        entity.action_tick = 0;
    }

    let dependent_food_need = decision::dependent_food_need(&entity.mind, population, dependents);
    let dependent_protection_target =
        decision::dependent_protection_target(position, &entity.mind, population, dependents);
    let provisioning_goal = (entity.hunger < URGENT_HUNGER_THRESHOLD)
        .then(|| {
            decision::dependent_provisioning_goal(
                dependent_food_need,
                entity.inventory.amount(ItemKind::Food),
            )
        })
        .flatten();
    if provisioning_goal.is_some_and(|goal| entity.mind.current_goal != Some(goal)) {
        entity.mind.clear_goal();
        entity.path.clear();
        entity.path_index = 0;
        entity.action_tick = 0;
    }
    if provisioning_goal.is_none()
        && dependent_protection_target.is_some()
        && entity.hunger < URGENT_HUNGER_THRESHOLD
        && entity.mind.current_goal != Some(Goal::ProtectDependent)
    {
        entity.mind.clear_goal();
        entity.path.clear();
        entity.path_index = 0;
        entity.action_tick = 0;
    }
    let migration_target = household_context.and_then(|context| context.migration_target);
    let stage = LifeStage::from_age_ticks(entity.age_ticks);
    let higher_priority_goal = provisioning_goal.is_some()
        || entity.mind.current_goal == Some(Goal::ProtectDependent)
        || entity.hunger >= URGENT_HUNGER_THRESHOLD
            && matches!(
                entity.mind.current_goal,
                Some(Goal::Eat | Goal::AcquireResource)
            );
    if migration_target.is_some()
        && matches!(
            stage,
            LifeStage::Adolescent | LifeStage::Adult | LifeStage::Elder
        )
        && !higher_priority_goal
        && entity.mind.current_goal != Some(Goal::MigrateHousehold)
    {
        entity.mind.clear_goal();
        entity.path.clear();
        entity.path_index = 0;
        entity.action_tick = 0;
    }

    let should_interrupt = entity.hunger >= URGENT_HUNGER_THRESHOLD
        && !matches!(
            entity.mind.current_goal,
            Some(Goal::Eat | Goal::AcquireResource)
        )
        && (entity.inventory.amount(ItemKind::Food) > 0
            || household_context.is_some_and(|context| context.storage_food_amount > 0)
            || !entity
                .mind
                .remembered_food_targets(position, tick)
                .is_empty());
    if should_interrupt {
        if let Some(profile) = profile.as_deref_mut() {
            profile.urgent_interrupts += 1;
        }
        entity.mind.clear_goal();
        entity.path.clear();
        entity.path_index = 0;
        entity.action_tick = 0;
    }
    if let Some(profile) = profile.as_deref_mut() {
        profile.plan_validation_us += start
            .expect("profile timer must exist")
            .elapsed()
            .as_micros() as u64;
    }
    if let Some(breakdown) = entity_pass.as_mut() {
        breakdown.plan_validation_ns += pass_start
            .expect("entity pass timer must exist")
            .elapsed()
            .as_nanos() as u64;
    }

    let pass_start = entity_pass.as_ref().map(|_| Instant::now());
    if entity.mind.current_action().is_none() {
        if let Some(work) = work.as_deref_mut() {
            work.goal_evaluations += 1;
        }
        let start = profile.as_ref().map(|_| Instant::now());
        if let Some(profile) = profile.as_deref_mut() {
            profile.planned_entities += 1;
        }
        let current_goal = entity.mind.current_goal;
        let best_visible_food_share_score =
            decision::best_optional_food_share_candidate(entity, population)
                .map(|candidate| candidate.score);
        let best_remembered_social_score =
            social::best_relationship_aware_remembered_score(entity, population, tick);
        let best_household_conflict_score =
            conflict::best_household_conflict_candidate(entity, population, tick)
                .map(|candidate| candidate.score);

        let household_food_available =
            household_context.is_some_and(|context| context.storage_food_amount > 0);
        let goal = decision::evaluate_goals_with_household(
            &mut entity.mind,
            entity.hunger,
            entity.health,
            entity.age_ticks,
            &entity.personality,
            current_goal,
            HouseholdDecisionContext {
                decision: DecisionContext {
                    tick,
                    origin: position,
                    food_in_inventory: entity.inventory.amount(ItemKind::Food),
                    best_visible_food_share_score,
                    best_remembered_social_score,
                },
                household_food_available,
                dependent_food_need,
                dependent_protection_target,
                migration_target,
                best_household_conflict_score,
            },
        );
        if Some(goal) != current_goal {
            if let Some(work) = work.as_deref_mut() {
                work.goal_changes += 1;
            }
        }
        decision::plan_goal(
            entity,
            world,
            tick,
            goal,
            pathfinding_workspace,
            population,
            dependents,
            household_context,
        );
        if let Some(work) = work.as_deref_mut() {
            work.plans_created += 1;
        }
        if let Some(profile) = profile.as_deref_mut() {
            profile.planning_us += start
                .expect("profile timer must exist")
                .elapsed()
                .as_micros() as u64;
        }
    }
    if let Some(breakdown) = entity_pass.as_mut() {
        breakdown.planning_ns += pass_start
            .expect("entity pass timer must exist")
            .elapsed()
            .as_nanos() as u64;
    }

    let start = profile.as_ref().map(|_| Instant::now());
    let outcome =
        action::execute_current_action(entity, world, tick, population, pathfinding_workspace);
    if let Some(work) = work {
        work.actions_executed += 1;
    }
    if let Some(profile) = profile {
        profile.action_us += start
            .expect("profile timer must exist")
            .elapsed()
            .as_micros() as u64;
        profile.sampled_entities += 1;
    }

    (outcome, discoveries, encounters)
}
