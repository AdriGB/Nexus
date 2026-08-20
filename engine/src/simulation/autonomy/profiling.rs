use super::super::entity::{Entity, LifeStage};
use super::super::inventory::ItemKind;
use super::super::spatial::{EntitySnapshot, SpatialGrid};
use super::action::execute_current_action;
use super::decision::{evaluate_goals, invalidate_obsolete_food_plan, plan_goal, DecisionContext};
use super::mind::{Goal, URGENT_HUNGER_THRESHOLD};
use super::perception::{
    perceive_entities, reconcile_resource_memory, scan_visible_resources, EntityEncounter,
    ResourceDiscovery,
};
use crate::pathfinding::PathfindingWorkspace;
use crate::world::Grid;
use web_time::Instant;

const PROFILE_SAMPLE_RATE: usize = 4;

type ProfileAutonomyResult = (
    u64,
    bool,
    AutonomyProfile,
    Vec<(u32, u16)>,
    Vec<ResourceDiscovery>,
    Vec<EntityEncounter>,
    Vec<super::SocialInteraction>,
);

#[derive(Clone, Debug, Default)]
pub(crate) struct AutonomyProfile {
    pub resource_perception_us: u64,
    pub entity_perception_us: u64,
    pub plan_validation_us: u64,
    pub planning_us: u64,
    pub action_us: u64,
    pub sampled_entities: u32,
    pub planned_entities: u32,
    pub urgent_interrupts: u32,
    pub memory_reconciliation_us: u64,
    pub visible_scan_us: u64,
    pub known_resources_total: u32,
    pub known_resources_max: u32,
    pub visible_resources_seen: u32,
    pub social_us: u64,
}

fn profiled_update_entity(
    entity: &mut Entity,
    world: &mut Grid,
    tick: u64,
    population: &[EntitySnapshot],
    spatial_grid: &SpatialGrid,
    pathfinding_workspace: &mut PathfindingWorkspace,
    profile: &mut AutonomyProfile,
) -> (
    super::ActionOutcome,
    Vec<ResourceDiscovery>,
    Vec<EntityEncounter>,
) {
    let position = (entity.x, entity.y);

    let start = Instant::now();
    reconcile_resource_memory(&mut entity.mind, world, position, tick);
    let reconciliation_us = start.elapsed().as_micros() as u64;
    profile.memory_reconciliation_us += reconciliation_us;

    let start = Instant::now();
    let (visible_count, discoveries) =
        scan_visible_resources(&mut entity.mind, entity.id, world, position, tick);
    let visible_scan_us = start.elapsed().as_micros() as u64;
    profile.visible_scan_us += visible_scan_us;
    profile.visible_resources_seen += visible_count;
    profile.resource_perception_us += reconciliation_us + visible_scan_us;

    let known_resources_count = entity.mind.memory.known_resources.len() as u32;
    profile.known_resources_total += known_resources_count;
    profile.known_resources_max = profile.known_resources_max.max(known_resources_count);

    let start = Instant::now();
    let encounters = perceive_entities(
        &mut entity.mind,
        entity.id,
        position,
        tick,
        population,
        spatial_grid,
    );
    profile.entity_perception_us += start.elapsed().as_micros() as u64;

    let start = Instant::now();
    invalidate_obsolete_food_plan(entity);

    let should_interrupt = entity.hunger >= URGENT_HUNGER_THRESHOLD
        && !matches!(
            entity.mind.current_goal,
            Some(Goal::Eat | Goal::AcquireResource)
        )
        && !entity
            .mind
            .remembered_food_targets(position, tick)
            .is_empty();
    if should_interrupt {
        profile.urgent_interrupts += 1;
        entity.mind.clear_goal();
        entity.path.clear();
        entity.path_index = 0;
        entity.action_tick = 0;
    }
    profile.plan_validation_us += start.elapsed().as_micros() as u64;

    if entity.mind.current_action().is_none() {
        profile.planned_entities += 1;

        let start = Instant::now();
        let current_goal = entity.mind.current_goal;
        let goal = evaluate_goals(
            &mut entity.mind,
            entity.hunger,
            entity.health,
            entity.age_ticks,
            &entity.personality,
            current_goal,
            DecisionContext {
                tick,
                origin: position,
                food_in_inventory: entity.inventory.amount(ItemKind::Food),
            },
        );
        plan_goal(entity, world, tick, goal, pathfinding_workspace, population);
        profile.planning_us += start.elapsed().as_micros() as u64;
    }

    let start = Instant::now();
    let outcome = execute_current_action(entity, world, tick, population, pathfinding_workspace);
    profile.action_us += start.elapsed().as_micros() as u64;
    profile.sampled_entities += 1;

    (outcome, discoveries, encounters)
}

pub(crate) fn profile_autonomy(
    entities: &mut [Entity],
    world: &mut Grid,
    tick: u64,
    population: &[EntitySnapshot],
    spatial_grid: &SpatialGrid,
    pathfinding_workspace: &mut PathfindingWorkspace,
) -> ProfileAutonomyResult {
    let mut profile = AutonomyProfile::default();
    let mut consumed = 0u64;
    let mut world_changed = false;
    let mut consumer_ids = Vec::new();
    let mut discoveries = Vec::new();
    let mut encounters = Vec::new();

    for (index, entity) in entities
        .iter_mut()
        .filter(|entity| {
            entity.health > 0.0 && LifeStage::from_age_ticks(entity.age_ticks) != LifeStage::Infant
        })
        .enumerate()
    {
        let (result, entity_discoveries, entity_encounters) = if index % PROFILE_SAMPLE_RATE == 0 {
            profiled_update_entity(
                entity,
                world,
                tick,
                population,
                spatial_grid,
                pathfinding_workspace,
                &mut profile,
            )
        } else {
            super::update_entity(
                entity,
                world,
                tick,
                population,
                spatial_grid,
                pathfinding_workspace,
            )
        };
        discoveries.extend(entity_discoveries);
        encounters.extend(entity_encounters);

        if result.food_consumed > 0 {
            consumer_ids.push((entity.id, result.food_consumed));
        }
        consumed += u64::from(result.food_consumed);
        world_changed |= result.world_changed;
    }

    let social_start = Instant::now();
    let interactions = super::social::process_social_interactions(entities, population, tick);
    profile.social_us += social_start.elapsed().as_micros() as u64;

    (
        consumed,
        world_changed,
        profile,
        consumer_ids,
        discoveries,
        encounters,
        interactions,
    )
}
