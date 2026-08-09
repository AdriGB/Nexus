use super::super::config::{
    BASE_MOVEMENT_SPEED, FOOD_CONSUMED_PER_MEAL, FOOD_SEARCH_THRESHOLD, HUNGER_REDUCTION_PER_MEAL,
    MAX_HEALTH, PREGNANCY_PHASE_2_START_WEEK, PREGNANCY_PHASE_3_START_WEEK,
    PREGNANCY_PHASE_4_START_WEEK, PREGNANCY_SPEED_PHASE_1, PREGNANCY_SPEED_PHASE_2,
    PREGNANCY_SPEED_PHASE_3, PREGNANCY_SPEED_PHASE_4,
};
use super::super::entity::{Entity, EntityActivity, LifeStage};
use super::super::time::TICKS_PER_WEEK;
use super::mind::{Action, Goal};
use crate::pathfinding;
use crate::world::{Grid, ResourceKind};

const REST_HEALTH_PER_TICK: f32 = 0.25;

pub(in crate::simulation) fn effective_movement_speed(entity: &Entity, tick: u64) -> f32 {
    let stage_factor = LifeStage::from_age_ticks(entity.age_ticks).movement_factor();

    let Some(pregnancy) = entity.pregnancy else {
        return BASE_MOVEMENT_SPEED * stage_factor;
    };

    let elapsed = tick.saturating_sub(pregnancy.conceived_tick);
    let weeks = elapsed / TICKS_PER_WEEK;

    let pregnancy_factor = if weeks < PREGNANCY_PHASE_2_START_WEEK {
        PREGNANCY_SPEED_PHASE_1
    } else if weeks < PREGNANCY_PHASE_3_START_WEEK {
        PREGNANCY_SPEED_PHASE_2
    } else if weeks < PREGNANCY_PHASE_4_START_WEEK {
        PREGNANCY_SPEED_PHASE_3
    } else {
        PREGNANCY_SPEED_PHASE_4
    };

    BASE_MOVEMENT_SPEED * stage_factor * pregnancy_factor
}

pub(super) fn execute_current_action(entity: &mut Entity, world: &mut Grid, tick: u64) -> u16 {
    let Some(action) = entity.mind.current_action() else {
        entity.activity = EntityActivity::Idle;
        return 0;
    };
    match action {
        Action::MoveTo(_, _) | Action::ExploreArea(_, _) => {
            entity.movement_credit += effective_movement_speed(entity, tick);

            if entity.path_index < entity.path.len() {
                let next = entity.path[entity.path_index];

                let Some(step_cost) = pathfinding::step_cost(world, (entity.x, entity.y), next)
                else {
                    entity.movement_credit = 0.0;
                    entity.mind.clear_goal();
                    entity.path.clear();
                    entity.path_index = 0;
                    return 0;
                };

                if entity.movement_credit >= step_cost {
                    entity.movement_credit -= step_cost;
                    entity.x = next.0;
                    entity.y = next.1;
                    entity.path_index += 1;
                    entity.activity = if entity.mind.current_goal == Some(Goal::Explore) {
                        EntityActivity::Exploring
                    } else {
                        EntityActivity::Moving
                    };
                }
            }
            if entity.path_index >= entity.path.len() {
                entity.movement_credit = 0.0;
                entity.path.clear();
                entity.path_index = 0;
                entity.mind.advance_action();
                if action.destination().is_some() && entity.mind.current_goal == Some(Goal::Explore)
                {
                    entity.activity = EntityActivity::Idle;
                }
            }
            0
        }
        Action::Consume(kind) => {
            entity.movement_credit = 0.0;
            let consumed = consume_food(entity, world);
            let position = (entity.x, entity.y);
            let amount = world.resources[(entity.y * world.width + entity.x) as usize]
                .filter(|deposit| deposit.kind == kind)
                .map_or(0, |deposit| deposit.amount);
            entity
                .mind
                .memory
                .update_known_amount(position, kind, amount, tick);
            entity.mind.advance_action();
            entity.activity = EntityActivity::Idle;
            consumed
        }
        Action::Wait => {
            entity.movement_credit = 0.0;
            if entity.hunger < FOOD_SEARCH_THRESHOLD {
                entity.health = (entity.health + REST_HEALTH_PER_TICK).min(MAX_HEALTH);
            }
            entity.mind.advance_action();
            entity.activity = EntityActivity::Resting;
            0
        }
    }
}

fn consume_food(entity: &mut Entity, world: &mut Grid) -> u16 {
    let index = (entity.y * world.width + entity.x) as usize;
    let Some(slot) = world.resources.get_mut(index) else {
        return 0;
    };
    let Some(deposit) = slot.as_mut() else {
        return 0;
    };
    if deposit.kind != ResourceKind::Food {
        return 0;
    }

    let consumed = deposit.amount.min(FOOD_CONSUMED_PER_MEAL);
    deposit.amount -= consumed;
    let meal_fraction = f32::from(consumed) / f32::from(FOOD_CONSUMED_PER_MEAL);
    entity.hunger = (entity.hunger - HUNGER_REDUCTION_PER_MEAL * meal_fraction).max(0.0);
    if deposit.amount == 0 {
        *slot = None;
    }
    consumed
}
