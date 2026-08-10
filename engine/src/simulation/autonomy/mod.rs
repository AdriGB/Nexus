mod action;
mod decision;
mod mind;
mod perception;
mod profiling;
mod social;

pub use self::decision::evaluate_goals;
#[cfg(test)]
pub use self::decision::exploration_target;
#[cfg(test)]
pub use self::mind::KnownEntity;
#[cfg(test)]
pub use self::mind::KnownResource;
pub use self::mind::{Action, Goal, Mind};
pub use self::perception::perceive;
#[cfg(test)]
pub(in crate::simulation) use self::social::SOCIAL_RADIUS;

#[cfg(test)]
pub(super) use self::action::effective_movement_speed;
pub(super) use self::mind::URGENT_HUNGER_THRESHOLD;
pub(crate) use self::profiling::{profile_autonomy, AutonomyProfile};

use super::entity::Entity;
use super::spatial::{EntitySnapshot, SpatialGrid};
use crate::pathfinding::PathfindingWorkspace;
use crate::world::Grid;

pub(super) fn process_social_interactions(
    entities: &mut [Entity],
    population: &[EntitySnapshot],
    tick: u64,
) {
    social::process_social_interactions(entities, population, tick);
}

pub(super) fn update_entity(
    entity: &mut Entity,
    world: &mut Grid,
    tick: u64,
    population: &[EntitySnapshot],
    spatial_grid: &SpatialGrid,
    pathfinding_workspace: &mut PathfindingWorkspace,
) -> u16 {
    let position = (entity.x, entity.y);
    perceive(&mut entity.mind, world, position, tick);
    perception::perceive_entities(
        &mut entity.mind,
        entity.id,
        position,
        tick,
        population,
        spatial_grid,
    );
    decision::invalidate_obsolete_food_plan(entity);

    let should_interrupt = entity.hunger >= URGENT_HUNGER_THRESHOLD
        && entity.mind.current_goal != Some(Goal::Eat)
        && !entity
            .mind
            .remembered_food_targets(position, tick)
            .is_empty();
    if should_interrupt {
        entity.mind.clear_goal();
        entity.path.clear();
        entity.path_index = 0;
    }

    if entity.mind.current_action().is_none() {
        let current_goal = entity.mind.current_goal;

        let goal = evaluate_goals(
            &mut entity.mind,
            entity.hunger,
            entity.health,
            entity.age_ticks,
            &entity.personality,
            current_goal,
            (tick, position),
        );
        decision::plan_goal(entity, world, tick, goal, pathfinding_workspace, population);
    }

    action::execute_current_action(entity, world, tick, population, pathfinding_workspace)
}
