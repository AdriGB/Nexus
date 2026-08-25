use super::super::config::{
    BASE_MOVEMENT_SPEED, FOOD_CONSUMED_PER_MEAL, FOOD_SEARCH_THRESHOLD, HUNGER_REDUCTION_PER_MEAL,
    MAX_HEALTH, PREGNANCY_PHASE_2_START_WEEK, PREGNANCY_PHASE_3_START_WEEK,
    PREGNANCY_PHASE_4_START_WEEK, PREGNANCY_SPEED_PHASE_1, PREGNANCY_SPEED_PHASE_2,
    PREGNANCY_SPEED_PHASE_3, PREGNANCY_SPEED_PHASE_4,
};
use super::super::entity::{Entity, EntityActivity, LifeStage};
use super::super::inventory::ItemKind;
use super::super::spatial::EntitySnapshot;
use super::super::time::TICKS_PER_WEEK;
use super::mind::{Action, Goal, GATHER_AMOUNT, GATHER_DURATION_TICKS};
use super::social::SOCIAL_RADIUS;
use crate::pathfinding::{self, PathfindingWorkspace};
use crate::world::{Grid, ResourceKind};

const REST_HEALTH_PER_TICK: f32 = 0.25;
pub(super) const SHARE_FOOD_AMOUNT: u16 = 10;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(in crate::simulation) struct FoodShareAttempt {
    pub actor_id: u32,
    pub target_id: u32,
    pub actor_location: (u32, u32),
    pub amount: u16,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(in crate::simulation) struct HouseholdDepositAttempt {
    pub actor_id: u32,
    pub amount: u16,
    pub actor_location: (u32, u32),
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(in crate::simulation) struct HouseholdWithdrawAttempt {
    pub actor_id: u32,
    pub amount: u16,
    pub actor_location: (u32, u32),
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub(in crate::simulation) struct ActionOutcome {
    pub food_consumed: u16,
    pub world_changed: bool,
    pub food_share_attempt: Option<FoodShareAttempt>,
    pub household_deposit_attempt: Option<HouseholdDepositAttempt>,
    pub household_withdraw_attempt: Option<HouseholdWithdrawAttempt>,
}

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

pub(super) fn execute_current_action(
    entity: &mut Entity,
    world: &mut Grid,
    tick: u64,
    population: &[EntitySnapshot],
    pathfinding_workspace: &mut PathfindingWorkspace,
) -> ActionOutcome {
    let Some(action) = entity.mind.current_action() else {
        entity.activity = EntityActivity::Idle;
        return ActionOutcome::default();
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
                    entity.action_tick = 0;
                    return ActionOutcome::default();
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
                entity.action_tick = 0;
                entity.mind.advance_action();
                if action.destination().is_some() && entity.mind.current_goal == Some(Goal::Explore)
                {
                    entity.activity = EntityActivity::Idle;
                }
            }
            ActionOutcome::default()
        }
        Action::Gather(kind) => {
            entity.movement_credit = 0.0;
            entity.activity = EntityActivity::SeekingFood;

            if entity.action_tick == 0 {
                // Starting a new gather action
                entity.action_tick = 1;
            } else {
                entity.action_tick = entity.action_tick.saturating_add(1);
            }

            if entity.action_tick >= GATHER_DURATION_TICKS {
                let gathered = gather_resource(entity, world, kind, tick);
                entity.action_tick = 0;
                entity.mind.advance_action();
                entity.activity = EntityActivity::Idle;
                ActionOutcome {
                    food_consumed: 0,
                    world_changed: gathered > 0,
                    food_share_attempt: None,
                    household_deposit_attempt: None,
                    household_withdraw_attempt: None,
                }
            } else {
                ActionOutcome::default()
            }
        }
        Action::Consume(kind) => {
            entity.movement_credit = 0.0;
            let consumed = consume_food_from_inventory(entity, kind);
            entity.mind.advance_action();
            entity.activity = EntityActivity::Idle;
            ActionOutcome {
                food_consumed: consumed,
                world_changed: false,
                food_share_attempt: None,
                household_deposit_attempt: None,
                household_withdraw_attempt: None,
            }
        }
        Action::Wait => {
            entity.movement_credit = 0.0;
            if entity.hunger < FOOD_SEARCH_THRESHOLD {
                entity.health = (entity.health + REST_HEALTH_PER_TICK).min(MAX_HEALTH);
            }
            entity.mind.advance_action();
            entity.activity = EntityActivity::Resting;
            ActionOutcome::default()
        }
        Action::ApproachEntity(target_id) => {
            let origin = (entity.x, entity.y);
            let protecting_dependent = entity.mind.current_goal == Some(Goal::ProtectDependent);

            if protecting_dependent
                && !population.iter().any(|snapshot| {
                    snapshot.id == target_id
                        && snapshot.is_child
                        && snapshot.caregiver_id == Some(entity.id)
                })
            {
                entity.movement_credit = 0.0;
                entity.mind.clear_goal();
                entity.path.clear();
                entity.path_index = 0;
                entity.activity = EntityActivity::Idle;
                return ActionOutcome::default();
            }

            // Use only information the entity actually knows:
            // - If target is currently visible, use perceived position.
            // - Otherwise, use last remembered position from memory.
            // - If no memory exists, the target is unknown — abandon.
            let currently_visible = entity
                .mind
                .visible_entities
                .binary_search(&target_id)
                .is_ok();

            let target_pos = if currently_visible {
                population
                    .iter()
                    .find(|s| s.id == target_id)
                    .map(|s| (s.x, s.y))
            } else {
                entity
                    .mind
                    .memory
                    .known_entities
                    .iter()
                    .find(|k| k.id == target_id)
                    .map(|k| (k.last_seen_x, k.last_seen_y))
            };

            let Some(target_pos) = target_pos else {
                // No information about target — clear goal
                entity.movement_credit = 0.0;
                entity.mind.clear_goal();
                entity.path.clear();
                entity.path_index = 0;
                return ActionOutcome::default();
            };

            // Already close enough?
            let completion_radius = if protecting_dependent {
                super::decision::DEPENDENT_REUNION_RADIUS
            } else {
                SOCIAL_RADIUS
            };
            if super::mind::manhattan(origin, target_pos) <= completion_radius {
                entity.movement_credit = 0.0;
                entity.path.clear();
                entity.path_index = 0;
                if currently_visible {
                    if protecting_dependent {
                        entity.mind.clear_goal();
                        entity.activity = EntityActivity::Idle;
                    } else {
                        entity.mind.advance_action();
                        entity.activity = EntityActivity::Socializing;
                    }
                } else {
                    // Arrived at last known position but target is not visible
                    // — abandon Socialize
                    if !protecting_dependent {
                        entity.mind.memory.mark_failed_social_seek(target_id, tick);
                    }
                    entity.mind.clear_goal();
                    entity.activity = EntityActivity::Idle;
                }
                return ActionOutcome::default();
            }

            // Replan path to target position
            if target_pos != entity.path.last().copied().unwrap_or(origin) {
                if let Some(path) = pathfinding::find_path_with_workspace(
                    pathfinding_workspace,
                    world,
                    origin,
                    target_pos,
                ) {
                    entity.path = path.into_iter().skip(1).collect();
                    entity.path_index = 0;
                }
            }

            // Move along path
            entity.movement_credit += effective_movement_speed(entity, tick);

            if entity.path_index < entity.path.len() {
                let next = entity.path[entity.path_index];

                let Some(step_cost) = pathfinding::step_cost(world, (entity.x, entity.y), next)
                else {
                    entity.movement_credit = 0.0;
                    entity.mind.clear_goal();
                    entity.path.clear();
                    entity.path_index = 0;
                    return ActionOutcome::default();
                };

                if entity.movement_credit >= step_cost {
                    entity.movement_credit -= step_cost;
                    entity.x = next.0;
                    entity.y = next.1;
                    entity.path_index += 1;
                    entity.activity = EntityActivity::Moving;
                }
            }

            // Check if we arrived after moving
            if entity.path_index >= entity.path.len() {
                entity.movement_credit = 0.0;
                entity.path.clear();
                entity.path_index = 0;
                if super::mind::manhattan((entity.x, entity.y), target_pos) <= completion_radius {
                    if currently_visible {
                        if protecting_dependent {
                            entity.mind.clear_goal();
                            entity.activity = EntityActivity::Idle;
                        } else {
                            entity.mind.advance_action();
                            entity.activity = EntityActivity::Socializing;
                        }
                    } else {
                        // Arrived at last known position but target is not visible
                        // — abandon Socialize
                        if !protecting_dependent {
                            entity.mind.memory.mark_failed_social_seek(target_id, tick);
                        }
                        entity.mind.clear_goal();
                        entity.activity = EntityActivity::Idle;
                    }
                } else if !currently_visible {
                    // Arrived at last known position but target is not here
                    // and not visible — abandon Socialize
                    if !protecting_dependent {
                        entity.mind.memory.mark_failed_social_seek(target_id, tick);
                    }
                    entity.mind.clear_goal();
                    entity.activity = EntityActivity::Idle;
                }
            }
            ActionOutcome::default()
        }
        Action::Interact(target_id) => {
            // Require the target to be currently visible — no omniscience
            if entity
                .mind
                .visible_entities
                .binary_search(&target_id)
                .is_err()
            {
                entity.movement_credit = 0.0;
                entity.mind.clear_goal();
                entity.path.clear();
                entity.path_index = 0;
                entity.activity = EntityActivity::Idle;
                return ActionOutcome::default();
            }

            let Some(snapshot) = population.iter().find(|s| s.id == target_id) else {
                entity.movement_credit = 0.0;
                entity.mind.clear_goal();
                entity.path.clear();
                entity.path_index = 0;
                entity.activity = EntityActivity::Idle;
                return ActionOutcome::default();
            };

            let origin = (entity.x, entity.y);
            let target_pos = (snapshot.x, snapshot.y);

            if super::mind::manhattan(origin, target_pos) > SOCIAL_RADIUS {
                // Target moved out of range — replan
                entity.movement_credit = 0.0;
                entity.path.clear();
                entity.path_index = 0;
                entity.mind.set_plan(
                    Goal::Socialize,
                    vec![
                        Action::ApproachEntity(target_id),
                        Action::Interact(target_id),
                    ],
                    tick,
                );
                entity.activity = EntityActivity::Moving;
                return ActionOutcome::default();
            }

            entity.movement_credit = 0.0;
            entity.mind.advance_action();
            entity.activity = EntityActivity::Socializing;
            ActionOutcome::default()
        }
        Action::ShareFood(target_id) => {
            // Require the target to be currently visible — no omniscience
            if entity
                .mind
                .visible_entities
                .binary_search(&target_id)
                .is_err()
            {
                entity.movement_credit = 0.0;
                entity.mind.clear_goal();
                entity.path.clear();
                entity.path_index = 0;
                entity.activity = EntityActivity::Idle;
                return ActionOutcome::default();
            }

            let Some(snapshot) = population.iter().find(|s| s.id == target_id) else {
                entity.movement_credit = 0.0;
                entity.mind.clear_goal();
                entity.path.clear();
                entity.path_index = 0;
                entity.activity = EntityActivity::Idle;
                return ActionOutcome::default();
            };

            let origin = (entity.x, entity.y);
            let target_pos = (snapshot.x, snapshot.y);

            if super::mind::manhattan(origin, target_pos) > SOCIAL_RADIUS {
                // Target moved out of range — replan
                entity.movement_credit = 0.0;
                entity.path.clear();
                entity.path_index = 0;
                entity.mind.set_plan(
                    Goal::ShareFood,
                    vec![
                        Action::ApproachEntity(target_id),
                        Action::ShareFood(target_id),
                    ],
                    tick,
                );
                entity.activity = EntityActivity::Moving;
                return ActionOutcome::default();
            }

            // Check if actor has food to share
            if entity.inventory.amount(ItemKind::Food) < SHARE_FOOD_AMOUNT {
                entity.movement_credit = 0.0;
                entity.mind.advance_action();
                entity.activity = EntityActivity::Idle;
                return ActionOutcome::default();
            }

            entity.movement_credit = 0.0;
            entity.mind.advance_action();
            entity.activity = EntityActivity::Socializing;
            ActionOutcome {
                food_consumed: 0,
                world_changed: false,
                food_share_attempt: Some(FoodShareAttempt {
                    actor_id: entity.id,
                    target_id,
                    actor_location: origin,
                    amount: SHARE_FOOD_AMOUNT,
                }),
                household_deposit_attempt: None,
                household_withdraw_attempt: None,
            }
        }
        Action::DepositHouseholdFood(amount) => {
            entity.movement_credit = 0.0;
            entity.mind.advance_action();
            entity.activity = EntityActivity::Idle;
            ActionOutcome {
                food_consumed: 0,
                world_changed: false,
                food_share_attempt: None,
                household_deposit_attempt: Some(HouseholdDepositAttempt {
                    actor_id: entity.id,
                    amount,
                    actor_location: (entity.x, entity.y),
                }),
                household_withdraw_attempt: None,
            }
        }
        Action::WithdrawHouseholdFood(amount) => {
            entity.movement_credit = 0.0;
            entity.mind.advance_action();
            entity.activity = EntityActivity::Idle;
            ActionOutcome {
                food_consumed: 0,
                world_changed: false,
                food_share_attempt: None,
                household_deposit_attempt: None,
                household_withdraw_attempt: Some(HouseholdWithdrawAttempt {
                    actor_id: entity.id,
                    amount,
                    actor_location: (entity.x, entity.y),
                }),
            }
        }
    }
}

fn gather_resource(entity: &mut Entity, world: &mut Grid, kind: ResourceKind, tick: u64) -> u16 {
    let index = (entity.y * world.width + entity.x) as usize;
    let Some(slot) = world.resources.get_mut(index) else {
        return 0;
    };
    let Some(deposit) = slot.as_mut() else {
        return 0;
    };
    if deposit.kind != kind {
        return 0;
    }

    let available_space = entity.inventory.remaining_capacity();
    if available_space == 0 {
        return 0;
    }

    let gathered = deposit.amount.min(GATHER_AMOUNT).min(available_space);
    deposit.amount -= gathered;
    entity.inventory.add(item_kind(kind), gathered);

    let position = (entity.x, entity.y);
    let amount = deposit.amount;
    entity
        .mind
        .memory
        .update_known_amount(position, kind, amount, tick);

    if deposit.amount == 0 {
        *slot = None;
    }
    gathered
}

fn consume_food_from_inventory(entity: &mut Entity, kind: ResourceKind) -> u16 {
    let consumed = entity
        .inventory
        .remove(item_kind(kind), FOOD_CONSUMED_PER_MEAL);
    let meal_fraction = f32::from(consumed) / f32::from(FOOD_CONSUMED_PER_MEAL);
    entity.hunger = (entity.hunger - HUNGER_REDUCTION_PER_MEAL * meal_fraction).max(0.0);
    consumed
}

const fn item_kind(kind: ResourceKind) -> ItemKind {
    match kind {
        ResourceKind::Food => ItemKind::Food,
        ResourceKind::Timber => ItemKind::Timber,
        ResourceKind::Stone => ItemKind::Stone,
        ResourceKind::Iron => ItemKind::Iron,
    }
}
