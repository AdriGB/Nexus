mod action;
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
pub use self::perception::perceive;
pub(in crate::simulation) use self::perception::{EntityEncounter, ResourceDiscovery};
#[cfg(test)]
pub(in crate::simulation) use self::social::SOCIAL_RADIUS;

#[cfg(test)]
pub(super) use self::action::effective_movement_speed;
pub(super) use self::action::ActionOutcome;
pub(in crate::simulation) use self::action::FoodShareAttempt;
pub(in crate::simulation) use self::action::HouseholdDepositAttempt;
pub(in crate::simulation) use self::action::HouseholdWithdrawAttempt;
pub(in crate::simulation) use self::decision::DecisionContext;
use self::decision::HouseholdDecisionContext;
#[cfg(test)]
pub(in crate::simulation) use self::decision::{
    DEPENDENT_PROTECTION_TRIGGER_DISTANCE, DEPENDENT_REUNION_RADIUS,
};
pub(crate) use self::mind::GATHER_DURATION_TICKS;
pub(super) use self::mind::URGENT_HUNGER_THRESHOLD;
pub(in crate::simulation) use self::mind::{GRIEF_MAX_DURATION_TICKS, GRIEF_MIN_DURATION_TICKS};
pub(crate) use self::profiling::{profile_autonomy, AutonomyProfile};
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

pub(crate) const HOUSEHOLD_PERSONAL_FOOD_RESERVE: u16 = 20;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct HouseholdAutonomyContext {
    pub residence: (u32, u32),
    pub migration_target: Option<(u32, u32)>,
    pub storage_remaining_capacity: u16,
    pub storage_food_amount: u16,
}

pub(super) fn process_social_interactions(
    entities: &mut [Entity],
    population: &[EntitySnapshot],
    tick: u64,
) -> Vec<SocialInteraction> {
    social::process_social_interactions(entities, population, tick)
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
    household_context: Option<HouseholdAutonomyContext>,
) -> (ActionOutcome, Vec<ResourceDiscovery>, Vec<EntityEncounter>) {
    entity.mind.prune_expired_grief(tick);
    let position = (entity.x, entity.y);
    let discoveries = perceive(&mut entity.mind, entity.id, world, position, tick);
    let encounters = perception::perceive_entities(
        &mut entity.mind,
        entity.id,
        position,
        tick,
        population,
        spatial_grid,
    );
    decision::invalidate_obsolete_food_plan(entity);
    if entity.mind.invalidate_known_dead_target_plan() {
        entity.path.clear();
        entity.path_index = 0;
        entity.action_tick = 0;
    }

    let dependent_food_need = decision::dependent_food_need(entity.id, &entity.mind, population);
    let dependent_protection_target =
        decision::dependent_protection_target(entity.id, position, &entity.mind, population);
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
        entity.mind.clear_goal();
        entity.path.clear();
        entity.path_index = 0;
        entity.action_tick = 0;
    }

    if entity.mind.current_action().is_none() {
        let current_goal = entity.mind.current_goal;
        let best_visible_food_share_score =
            decision::best_optional_food_share_candidate(entity, population)
                .map(|candidate| candidate.score);
        let best_remembered_social_score =
            social::best_relationship_aware_remembered_score(entity, population, tick);

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
            },
        );
        decision::plan_goal(
            entity,
            world,
            tick,
            goal,
            pathfinding_workspace,
            population,
            household_context,
        );
    }

    (
        action::execute_current_action(entity, world, tick, population, pathfinding_workspace),
        discoveries,
        encounters,
    )
}
