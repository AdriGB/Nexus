use super::super::config::{FOOD_CONSUMED_PER_MEAL, FOOD_SEARCH_THRESHOLD, MAX_HUNGER};
use super::super::entity::{Entity, LifeStage, Personality};
use super::super::inventory::ItemKind;
use super::super::spatial::EntitySnapshot;
use super::mind::{Action, DecisionExplanation, DecisionReason, Goal, Mind};
use super::relationships::{close_relationship_role, CloseRelationshipRole, RelationshipIdentity};
use super::URGENT_HUNGER_THRESHOLD;
use crate::pathfinding::{self, PathfindingWorkspace};
use crate::world::{Grid, ResourceKind};

const MIN_SWITCH_MARGIN: f32 = 0.02;
const MAX_SWITCH_MARGIN: f32 = 0.15;
pub(in crate::simulation) const DEPENDENT_PROTECTION_TRIGGER_DISTANCE: u32 = 4;
pub(in crate::simulation) const DEPENDENT_REUNION_RADIUS: u32 = 2;

/// Context passed to [`evaluate_goals`] to avoid nested tuple parameters.
pub(in crate::simulation) struct DecisionContext {
    pub tick: u64,
    pub origin: (u32, u32),
    pub food_in_inventory: u16,
    pub best_visible_food_share_score: Option<f32>,
    pub best_remembered_social_score: Option<i32>,
}

pub(super) struct HouseholdDecisionContext {
    pub decision: DecisionContext,
    pub household_food_available: bool,
    pub dependent_food_need: DependentFoodNeed,
    pub dependent_protection_target: Option<u32>,
    pub migration_target: Option<(u32, u32)>,
    pub best_household_conflict_score: Option<f32>,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub(super) enum DependentFoodNeed {
    #[default]
    None,
    Infant,
    Child,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub(super) struct FoodShareCandidate {
    pub target_id: u32,
    pub position: (u32, u32),
    pub score: f32,
    pub role: CloseRelationshipRole,
    pub hunger: f32,
}

fn food_share_relationship_bonus(role: CloseRelationshipRole) -> f32 {
    match role {
        CloseRelationshipRole::CurrentPartner => 0.25,
        CloseRelationshipRole::ParentChild => 0.20,
        CloseRelationshipRole::Sibling => 0.12,
        CloseRelationshipRole::Other => 0.0,
    }
}

pub(super) fn best_optional_food_share_candidate(
    actor: &Entity,
    population: &[EntitySnapshot],
) -> Option<FoodShareCandidate> {
    let identity = RelationshipIdentity::from_entity(actor);
    let mut best = None;

    for &target_id in &actor.mind.visible_entities {
        let Ok(index) = population.binary_search_by_key(&target_id, |snapshot| snapshot.id) else {
            continue;
        };
        let target = &population[index];
        if target.id == actor.id
            || target.hunger < FOOD_SEARCH_THRESHOLD
            || target.caregiver_id == Some(actor.id)
        {
            continue;
        }
        let Ok(known_index) = actor
            .mind
            .memory
            .known_entities
            .binary_search_by_key(&target.id, |known| known.id)
        else {
            continue;
        };
        let known = &actor.mind.memory.known_entities[known_index];
        if known.affinity < -200 {
            continue;
        }

        let role = close_relationship_role(identity, target);
        let need_factor = (target.hunger / MAX_HUNGER).clamp(0.0, 1.0);
        let affinity_factor = ((f32::from(known.affinity) + 1_000.0) / 2_000.0).clamp(0.0, 1.0);
        let candidate = FoodShareCandidate {
            target_id,
            position: (target.x, target.y),
            score: need_factor * (0.65 + 0.35 * affinity_factor)
                + food_share_relationship_bonus(role),
            role,
            hunger: target.hunger,
        };
        if best.is_none_or(|current| food_share_candidate_is_better(candidate, current)) {
            best = Some(candidate);
        }
    }
    best
}

fn food_share_candidate_is_better(
    candidate: FoodShareCandidate,
    current: FoodShareCandidate,
) -> bool {
    candidate.score.total_cmp(&current.score).is_gt()
        || candidate.score.total_cmp(&current.score).is_eq()
            && (candidate.role < current.role
                || candidate.role == current.role
                    && (candidate.hunger.total_cmp(&current.hunger).is_gt()
                        || candidate.hunger.total_cmp(&current.hunger).is_eq()
                            && candidate.target_id < current.target_id))
}

impl DependentFoodNeed {
    fn required_food(self) -> u16 {
        match self {
            Self::None => 0,
            Self::Infant => FOOD_CONSUMED_PER_MEAL,
            Self::Child => super::action::SHARE_FOOD_AMOUNT,
        }
    }
}

pub(super) fn dependent_food_need(
    caregiver_id: u32,
    mind: &Mind,
    population: &[EntitySnapshot],
) -> DependentFoodNeed {
    let mut hungry_child = false;
    for snapshot in population.iter().filter(|snapshot| {
        snapshot.caregiver_id == Some(caregiver_id) && snapshot.hunger >= FOOD_SEARCH_THRESHOLD
    }) {
        if snapshot.is_infant {
            return DependentFoodNeed::Infant;
        }
        if snapshot.is_child && mind.visible_entities.binary_search(&snapshot.id).is_ok() {
            hungry_child = true;
        }
    }
    if hungry_child {
        DependentFoodNeed::Child
    } else {
        DependentFoodNeed::None
    }
}

pub(super) fn dependent_protection_target(
    caregiver_id: u32,
    caregiver_position: (u32, u32),
    mind: &Mind,
    population: &[EntitySnapshot],
) -> Option<u32> {
    population
        .iter()
        .filter(|snapshot| snapshot.caregiver_id == Some(caregiver_id) && snapshot.is_child)
        .filter(|snapshot| mind.visible_entities.binary_search(&snapshot.id).is_ok())
        .filter_map(|snapshot| {
            let distance = super::mind::manhattan(caregiver_position, (snapshot.x, snapshot.y));
            (distance > DEPENDENT_PROTECTION_TRIGGER_DISTANCE).then_some((distance, snapshot.id))
        })
        .max_by(|a, b| a.0.cmp(&b.0).then_with(|| b.1.cmp(&a.1)))
        .map(|(_, id)| id)
}

pub(super) fn dependent_provisioning_goal(
    need: DependentFoodNeed,
    personal_food: u16,
) -> Option<Goal> {
    match need {
        DependentFoodNeed::None => None,
        DependentFoodNeed::Infant if personal_food >= FOOD_CONSUMED_PER_MEAL => Some(Goal::Eat),
        DependentFoodNeed::Child if personal_food >= super::action::SHARE_FOOD_AMOUNT => {
            Some(Goal::ShareFood)
        }
        DependentFoodNeed::Infant | DependentFoodNeed::Child => Some(Goal::AcquireResource),
    }
}

/// Maps persistence [0.0, 1.0] to the score margin required to abandon
/// the current goal for an alternative.
///
/// persistence 0.0 → 0.02 (barely any inertia)
/// persistence 0.5 → 0.0525
/// persistence 1.0 → 0.15 (requires a substantially better alternative)
pub(super) fn switch_margin(persistence: f32) -> f32 {
    MIN_SWITCH_MARGIN + (MAX_SWITCH_MARGIN - MIN_SWITCH_MARGIN) * persistence * persistence
}

#[cfg(test)]
pub fn evaluate_goals(
    mind: &mut Mind,
    hunger: f32,
    health: f32,
    age_ticks: u64,
    personality: &Personality,
    current_goal: Option<Goal>,
    context: DecisionContext,
) -> Goal {
    evaluate_goals_with_household(
        mind,
        hunger,
        health,
        age_ticks,
        personality,
        current_goal,
        HouseholdDecisionContext {
            decision: context,
            household_food_available: false,
            dependent_food_need: DependentFoodNeed::None,
            dependent_protection_target: None,
            migration_target: None,
            best_household_conflict_score: None,
        },
    )
}

pub(super) fn evaluate_goals_with_household(
    mind: &mut Mind,
    hunger: f32,
    health: f32,
    age_ticks: u64,
    personality: &Personality,
    current_goal: Option<Goal>,
    context: HouseholdDecisionContext,
) -> Goal {
    let HouseholdDecisionContext {
        decision: context,
        household_food_available,
        dependent_food_need,
        dependent_protection_target,
        migration_target,
        best_household_conflict_score,
    } = context;
    let DecisionContext {
        tick,
        origin,
        food_in_inventory,
        best_visible_food_share_score,
        best_remembered_social_score,
    } = context;
    let stage = LifeStage::from_age_ticks(age_ticks);

    if stage == LifeStage::Child {
        if hunger >= FOOD_SEARCH_THRESHOLD
            && mind
                .memory
                .known_resources
                .iter()
                .any(|known| known.kind == ResourceKind::Food && known.estimated_amount > 0)
        {
            mind.utility_scores = super::mind::UtilityScores {
                eat: if food_in_inventory > 0 { 1.0 } else { 0.0 },
                acquire_resource: if food_in_inventory > 0 { 0.0 } else { 1.0 },
                explore: 0.0,
                rest: 0.0,
                socialize: 0.0,
                share_food: 0.0,
            };
            mind.decision_explanation = Some(DecisionExplanation {
                chosen_goal: if food_in_inventory > 0 {
                    Goal::Eat
                } else {
                    Goal::AcquireResource
                },
                highest_utility_goal: if food_in_inventory > 0 {
                    Goal::Eat
                } else {
                    Goal::AcquireResource
                },
                chosen_score: 1.0,
                highest_score: 1.0,
                switch_margin: 0.0,
                reason: DecisionReason::DependentNeedsFood,
            });
            return if food_in_inventory > 0 {
                Goal::Eat
            } else {
                Goal::AcquireResource
            };
        }
        mind.utility_scores = super::mind::UtilityScores {
            eat: 0.0,
            acquire_resource: 0.0,
            explore: 0.0,
            rest: 0.5,
            socialize: 0.0,
            share_food: 0.0,
        };
        mind.decision_explanation = Some(DecisionExplanation {
            chosen_goal: Goal::Follow,
            highest_utility_goal: Goal::Rest,
            chosen_score: 0.0,
            highest_score: 0.5,
            switch_margin: 0.0,
            reason: DecisionReason::DependentFollowsCaregiver,
        });
        return Goal::Follow;
    }

    if matches!(stage, LifeStage::Adult | LifeStage::Elder) && hunger < URGENT_HUNGER_THRESHOLD {
        if let Some(goal) = dependent_provisioning_goal(dependent_food_need, food_in_inventory) {
            mind.utility_scores = super::mind::UtilityScores {
                eat: (goal == Goal::Eat) as u8 as f32,
                acquire_resource: (goal == Goal::AcquireResource) as u8 as f32,
                explore: 0.0,
                rest: 0.0,
                socialize: 0.0,
                share_food: (goal == Goal::ShareFood) as u8 as f32,
            };
            mind.decision_explanation = Some(DecisionExplanation {
                chosen_goal: goal,
                highest_utility_goal: goal,
                chosen_score: 1.0,
                highest_score: 1.0,
                switch_margin: 0.0,
                reason: DecisionReason::DependentProvisioning,
            });
            return goal;
        }
        if dependent_protection_target.is_some() {
            mind.decision_explanation = Some(DecisionExplanation {
                chosen_goal: Goal::ProtectDependent,
                highest_utility_goal: Goal::ProtectDependent,
                chosen_score: 1.0,
                highest_score: 1.0,
                switch_margin: 0.0,
                reason: DecisionReason::DependentProtection,
            });
            return Goal::ProtectDependent;
        }
    }

    if matches!(
        stage,
        LifeStage::Adolescent | LifeStage::Adult | LifeStage::Elder
    ) && hunger < URGENT_HUNGER_THRESHOLD
        && migration_target.is_some()
    {
        mind.decision_explanation = Some(DecisionExplanation {
            chosen_goal: Goal::MigrateHousehold,
            highest_utility_goal: Goal::MigrateHousehold,
            chosen_score: 1.0,
            highest_score: 1.0,
            switch_margin: 0.0,
            reason: DecisionReason::HouseholdMigration,
        });
        return Goal::MigrateHousehold;
    }

    let food_confidence = if mind
        .memory
        .known_resources
        .iter()
        .any(|known| known.kind == ResourceKind::Food && known.estimated_amount > 0)
    {
        1.0
    } else {
        0.25
    };
    let hunger_ratio = (hunger / 100.0).clamp(0.0, 1.0);
    let health_deficit = (1.0 - health / 100.0).clamp(0.0, 1.0);

    let curiosity_factor = 0.75 + personality.curiosity * 0.50;
    let caution_explore_factor = 1.15 - personality.caution * 0.30;
    let caution_rest_factor = 0.85 + personality.caution * 0.30;

    let has_food_in_inventory = food_in_inventory > 0;

    let acquire_resource_confidence = if household_food_available
        || mind
            .memory
            .known_resources
            .iter()
            .any(|known| known.kind == ResourceKind::Food && known.estimated_amount > 0)
    {
        1.0
    } else {
        0.25
    };

    let socialize = {
        let has_visible = !mind.visible_entities.is_empty();

        let best_remembered_score = best_remembered_social_score.or_else(|| {
            super::social::best_generic_remembered_social_score(mind, tick, origin, personality)
        });
        let sociability_factor = 0.3 + personality.sociability * 0.7;
        let sated_factor = (1.0 - hunger_ratio) * 0.6 + 0.4;

        let relationship_strength =
            best_remembered_score.map(|score| (score as f32 / 1_000.0).clamp(0.1, 1.0));

        if has_visible {
            sated_factor * 0.6 * sociability_factor
        } else if let Some(relationship_strength) = relationship_strength {
            sated_factor * (0.15 + relationship_strength * 0.45) * sociability_factor * 0.9
        } else {
            0.0
        }
    };

    let share_food = {
        let has_surplus_food = food_in_inventory >= 20; // At least 2 meals worth

        if !has_surplus_food {
            0.0
        } else {
            let sated_factor = (1.0 - hunger_ratio) * 0.7 + 0.3;
            best_visible_food_share_score.map_or(0.0, |score| sated_factor * score)
        }
    };

    let acquire_resource = if has_food_in_inventory {
        0.0
    } else {
        hunger_ratio * (0.7 + 0.3 * acquire_resource_confidence)
    };
    let grief = if hunger >= URGENT_HUNGER_THRESHOLD {
        0.0
    } else {
        mind.grief_pressure(tick)
    };
    let household_conflict = if hunger >= URGENT_HUNGER_THRESHOLD {
        0.0
    } else {
        best_household_conflict_score.unwrap_or(0.0)
    };

    mind.utility_scores = super::mind::UtilityScores {
        eat: if has_food_in_inventory {
            hunger_ratio * (0.65 + 0.35 * food_confidence)
        } else {
            0.0
        },
        acquire_resource,
        explore: ((1.0 - hunger_ratio) * 0.55 + (1.0 - food_confidence) * 0.2)
            * curiosity_factor
            * caution_explore_factor,
        rest: (health_deficit * 0.8 + 0.05) * caution_rest_factor,
        socialize,
        share_food,
    };

    let scores = [
        (household_conflict, Goal::ConfrontHouseholdMember),
        (grief, Goal::Grieve),
        (mind.utility_scores.eat, Goal::Eat),
        (mind.utility_scores.acquire_resource, Goal::AcquireResource),
        (mind.utility_scores.explore, Goal::Explore),
        (mind.utility_scores.rest, Goal::Rest),
        (mind.utility_scores.socialize, Goal::Socialize),
        (mind.utility_scores.share_food, Goal::ShareFood),
    ];
    let (best_score, best_goal) = scores
        .into_iter()
        .max_by(|left, right| left.0.total_cmp(&right.0))
        .unwrap_or((0.0, Goal::Explore));

    let margin = switch_margin(personality.persistence);
    if let Some(current) = current_goal {
        if current != best_goal {
            if let Some((current_score, _)) = scores.iter().find(|(_, goal)| *goal == current) {
                if best_score <= *current_score + margin {
                    mind.decision_explanation = Some(DecisionExplanation {
                        chosen_goal: current,
                        highest_utility_goal: best_goal,
                        chosen_score: *current_score,
                        highest_score: best_score,
                        switch_margin: margin,
                        reason: DecisionReason::GoalPersistence,
                    });
                    return current;
                }
            }
        }
    }

    mind.decision_explanation = Some(DecisionExplanation {
        chosen_goal: best_goal,
        highest_utility_goal: best_goal,
        chosen_score: best_score,
        highest_score: best_score,
        switch_margin: margin,
        reason: DecisionReason::HighestUtility,
    });
    best_goal
}

pub(super) fn invalidate_obsolete_food_plan(entity: &mut Entity) {
    if entity.mind.current_goal == Some(Goal::Eat) {
        if entity
            .inventory
            .amount(super::super::inventory::ItemKind::Food)
            == 0
        {
            entity.mind.clear_goal();
            entity.action_tick = 0;
        }
        return;
    }
    if entity.mind.current_goal != Some(Goal::AcquireResource) {
        return;
    }
    if entity
        .mind
        .current_plan
        .iter()
        .any(|action| matches!(action, Action::WithdrawHouseholdFood(_)))
    {
        return;
    }
    let Some(target) = entity
        .mind
        .current_plan
        .iter()
        .find_map(|action| action.destination())
    else {
        return;
    };
    let still_remembered = entity.mind.memory.known_resources.iter().any(|known| {
        known.kind == ResourceKind::Food
            && known.estimated_amount > 0
            && (known.x, known.y) == target
    });
    if !still_remembered {
        entity.mind.clear_goal();
        entity.path.clear();
        entity.path_index = 0;
        entity.action_tick = 0;
    }
}

fn plan_follow(
    entity: &mut Entity,
    world: &Grid,
    tick: u64,
    pathfinding_workspace: &mut PathfindingWorkspace,
    population: &[EntitySnapshot],
) {
    let origin = (entity.x, entity.y);

    let Some(caregiver_id) = entity.caregiver_id else {
        entity.mind.set_plan(Goal::Rest, vec![Action::Wait], tick);
        entity.activity = super::super::entity::EntityActivity::Resting;
        return;
    };
    let Some(target) = population
        .iter()
        .find(|snapshot| snapshot.id == caregiver_id)
        .map(|snapshot| (snapshot.x, snapshot.y))
    else {
        entity.mind.set_plan(Goal::Rest, vec![Action::Wait], tick);
        entity.activity = super::super::entity::EntityActivity::Resting;
        return;
    };

    if target == origin {
        entity.mind.set_plan(Goal::Follow, vec![], tick);
        entity.path.clear();
        entity.path_index = 0;
        entity.activity = super::super::entity::EntityActivity::Idle;
        return;
    }

    if let Some(path) =
        pathfinding::find_path_with_workspace(pathfinding_workspace, world, origin, target)
    {
        entity.path = path.into_iter().skip(1).collect();
        entity.path_index = 0;
        if entity.path.is_empty() {
            entity.mind.set_plan(Goal::Follow, vec![], tick);
            entity.activity = super::super::entity::EntityActivity::Idle;
        } else {
            entity
                .mind
                .set_plan(Goal::Follow, vec![Action::MoveTo(target.0, target.1)], tick);
            entity.activity = super::super::entity::EntityActivity::Moving;
        }
    } else {
        entity.mind.set_plan(Goal::Rest, vec![Action::Wait], tick);
        entity.activity = super::super::entity::EntityActivity::Resting;
    }
}

fn plan_protect_dependent(entity: &mut Entity, tick: u64, population: &[EntitySnapshot]) {
    let Some(target_id) =
        dependent_protection_target(entity.id, (entity.x, entity.y), &entity.mind, population)
    else {
        entity.mind.clear_goal();
        entity.path.clear();
        entity.path_index = 0;
        entity.activity = super::super::entity::EntityActivity::Idle;
        return;
    };
    entity.mind.set_plan(
        Goal::ProtectDependent,
        vec![Action::ApproachEntity(target_id)],
        tick,
    );
    entity.activity = super::super::entity::EntityActivity::Moving;
}

fn plan_share_food(
    entity: &mut Entity,
    world: &Grid,
    tick: u64,
    pathfinding_workspace: &mut PathfindingWorkspace,
    population: &[EntitySnapshot],
) {
    let hungry_dependent = population
        .iter()
        .filter(|snapshot| snapshot.caregiver_id == Some(entity.id))
        .filter(|snapshot| snapshot.is_child)
        .filter(|snapshot| snapshot.hunger >= FOOD_SEARCH_THRESHOLD)
        .filter(|snapshot| {
            entity
                .mind
                .visible_entities
                .binary_search(&snapshot.id)
                .is_ok()
        })
        .reduce(|best, candidate| {
            if candidate.hunger > best.hunger
                || (candidate.hunger == best.hunger && candidate.id < best.id)
            {
                candidate
            } else {
                best
            }
        })
        .map(|snapshot| (snapshot.id, (snapshot.x, snapshot.y)));

    if let Some((target_id, target_pos)) = hungry_dependent {
        plan_food_delivery(
            entity,
            world,
            tick,
            pathfinding_workspace,
            target_id,
            target_pos,
        );
        return;
    }

    let Some(candidate) = best_optional_food_share_candidate(entity, population) else {
        entity.mind.set_plan(Goal::Rest, vec![Action::Wait], tick);
        entity.activity = super::super::entity::EntityActivity::Resting;
        return;
    };

    plan_food_delivery(
        entity,
        world,
        tick,
        pathfinding_workspace,
        candidate.target_id,
        candidate.position,
    );
}

fn plan_food_delivery(
    entity: &mut Entity,
    world: &Grid,
    tick: u64,
    pathfinding_workspace: &mut PathfindingWorkspace,
    target_id: u32,
    target_pos: (u32, u32),
) {
    let origin = (entity.x, entity.y);
    if target_pos == origin {
        entity
            .mind
            .set_plan(Goal::ShareFood, vec![Action::ShareFood(target_id)], tick);
        entity.activity = super::super::entity::EntityActivity::Socializing;
        return;
    }

    if let Some(path) =
        pathfinding::find_path_with_workspace(pathfinding_workspace, world, origin, target_pos)
    {
        entity.path = path.into_iter().skip(1).collect();
        entity.path_index = 0;
        let mut actions = Vec::new();
        if target_pos != origin {
            actions.push(Action::ApproachEntity(target_id));
        }
        actions.push(Action::ShareFood(target_id));
        entity.mind.set_plan(Goal::ShareFood, actions, tick);
        entity.activity = super::super::entity::EntityActivity::Moving;
    } else {
        entity.mind.set_plan(Goal::Rest, vec![Action::Wait], tick);
        entity.activity = super::super::entity::EntityActivity::Resting;
    }
}

pub(super) fn plan_goal(
    entity: &mut Entity,
    world: &Grid,
    tick: u64,
    goal: Goal,
    pathfinding_workspace: &mut PathfindingWorkspace,
    population: &[EntitySnapshot],
    household_context: Option<super::HouseholdAutonomyContext>,
) {
    let origin = (entity.x, entity.y);
    match goal {
        Goal::Eat => {
            entity.path.clear();
            entity.path_index = 0;
            entity
                .mind
                .set_plan(Goal::Eat, vec![Action::Consume(ResourceKind::Food)], tick);
            entity.activity = super::super::entity::EntityActivity::SeekingFood;
        }
        Goal::AcquireResource => {
            let kind = ResourceKind::Food;
            let stage = LifeStage::from_age_ticks(entity.age_ticks);
            let personal_food = entity.inventory.amount(ItemKind::Food);
            let provisioning_need = (entity.hunger < URGENT_HUNGER_THRESHOLD
                && matches!(stage, LifeStage::Adult | LifeStage::Elder))
            .then(|| dependent_food_need(entity.id, &entity.mind, population))
            .filter(|need| *need != DependentFoodNeed::None);
            let requested = provisioning_need
                .map(|need| need.required_food().saturating_sub(personal_food))
                .unwrap_or_else(|| {
                    if stage == LifeStage::Adult && personal_food == 0 {
                        FOOD_CONSUMED_PER_MEAL
                    } else {
                        0
                    }
                });
            if requested > 0 {
                if let Some(context) = household_context.filter(|context| {
                    context.storage_food_amount > 0 && entity.inventory.remaining_capacity() > 0
                }) {
                    let amount = requested
                        .min(context.storage_food_amount)
                        .min(entity.inventory.remaining_capacity());
                    let home = context.residence;
                    if home == origin {
                        entity.mind.set_plan(
                            Goal::AcquireResource,
                            vec![Action::WithdrawHouseholdFood(amount)],
                            tick,
                        );
                        entity.activity = super::super::entity::EntityActivity::SeekingFood;
                        return;
                    }
                    if let Some(path) = pathfinding::find_path_with_workspace(
                        pathfinding_workspace,
                        world,
                        origin,
                        home,
                    ) {
                        entity.path = path.into_iter().skip(1).collect();
                        entity.path_index = 0;
                        entity.mind.set_plan(
                            Goal::AcquireResource,
                            vec![
                                Action::MoveTo(home.0, home.1),
                                Action::WithdrawHouseholdFood(amount),
                            ],
                            tick,
                        );
                        entity.activity = super::super::entity::EntityActivity::SeekingFood;
                        return;
                    }
                }
            }
            let mut targets = entity.mind.remembered_resource_targets(origin, tick, kind);
            while let Some(std::cmp::Reverse((_, _, target))) = targets.pop() {
                if let Some(path) = pathfinding::find_path_with_workspace(
                    pathfinding_workspace,
                    world,
                    origin,
                    target,
                ) {
                    entity.path = path.into_iter().skip(1).collect();
                    entity.path_index = 0;
                    let mut actions = Vec::new();
                    if target != origin {
                        actions.push(Action::MoveTo(target.0, target.1));
                    }
                    actions.push(Action::Gather(kind));
                    entity.mind.set_plan(Goal::AcquireResource, actions, tick);
                    entity.activity = super::super::entity::EntityActivity::SeekingFood;
                    return;
                }
                entity.mind.memory.mark_unreachable(target, tick);
            }
            if LifeStage::from_age_ticks(entity.age_ticks) == LifeStage::Child {
                plan_follow(entity, world, tick, pathfinding_workspace, population);
            } else {
                super::exploration::plan_exploration(entity, world, tick, pathfinding_workspace);
            }
        }
        Goal::ConfrontHouseholdMember => super::conflict::plan_household_conflict(
            entity,
            world,
            tick,
            population,
            pathfinding_workspace,
        ),
        Goal::Explore => {
            super::exploration::plan_exploration(entity, world, tick, pathfinding_workspace);
        }
        Goal::Follow => plan_follow(entity, world, tick, pathfinding_workspace, population),
        Goal::Grieve => {
            entity.path.clear();
            entity.path_index = 0;
            entity.mind.set_plan(Goal::Grieve, vec![Action::Wait], tick);
            entity.activity = super::super::entity::EntityActivity::Resting;
        }
        Goal::MigrateHousehold => {
            let target = household_context
                .and_then(|context| context.migration_target)
                .expect("migration goal requires a household migration target");
            entity.path.clear();
            entity.path_index = 0;
            if (entity.x, entity.y) == target {
                entity
                    .mind
                    .set_plan(Goal::MigrateHousehold, vec![Action::Wait], tick);
                entity.activity = super::super::entity::EntityActivity::Resting;
            } else if let Some(path) =
                pathfinding::find_path_with_workspace(pathfinding_workspace, world, origin, target)
            {
                entity.path = path.into_iter().skip(1).collect();
                entity.path_index = 0;
                entity.mind.set_plan(
                    Goal::MigrateHousehold,
                    vec![Action::MoveTo(target.0, target.1)],
                    tick,
                );
                entity.activity = super::super::entity::EntityActivity::Moving;
            } else {
                entity
                    .mind
                    .set_plan(Goal::MigrateHousehold, vec![Action::Wait], tick);
                entity.activity = super::super::entity::EntityActivity::Resting;
            }
        }
        Goal::ProtectDependent => plan_protect_dependent(entity, tick, population),
        Goal::Rest => {
            entity.path.clear();
            entity.path_index = 0;
            let active_migration = household_context.and_then(|context| context.migration_target);
            let deposit_amount = household_context
                .filter(|_| active_migration.is_none())
                .filter(|_| LifeStage::from_age_ticks(entity.age_ticks) == LifeStage::Adult)
                .map(|context| {
                    entity
                        .inventory
                        .amount(ItemKind::Food)
                        .saturating_sub(super::HOUSEHOLD_PERSONAL_FOOD_RESERVE)
                        .min(context.storage_remaining_capacity)
                })
                .unwrap_or(0);
            let deposit_action =
                (deposit_amount > 0).then_some(Action::DepositHouseholdFood(deposit_amount));
            if let Some(home) = household_context
                .map(|context| context.migration_target.unwrap_or(context.residence))
                .filter(|home| *home != origin)
            {
                if let Some(path) = pathfinding::find_path_with_workspace(
                    pathfinding_workspace,
                    world,
                    origin,
                    home,
                ) {
                    entity.path = path.into_iter().skip(1).collect();
                    let mut plan = vec![Action::MoveTo(home.0, home.1)];
                    plan.extend(deposit_action);
                    plan.push(Action::Wait);
                    entity.mind.set_plan(Goal::Rest, plan, tick);
                    entity.activity = super::super::entity::EntityActivity::Moving;
                    return;
                }
            }
            let mut plan = Vec::with_capacity(2);
            plan.extend(deposit_action);
            plan.push(Action::Wait);
            entity.mind.set_plan(Goal::Rest, plan, tick);
            entity.activity = super::super::entity::EntityActivity::Resting;
        }
        Goal::Socialize => {
            super::social::plan_socialize(entity, world, tick, pathfinding_workspace, population);
        }
        Goal::ShareFood => {
            plan_share_food(entity, world, tick, pathfinding_workspace, population);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::super::super::time::TICKS_PER_YEAR;
    use super::super::mind::{KnownEntity, KnownResource, Mind};
    use super::*;

    #[test]
    fn switch_margin_scales_with_persistence() {
        assert!((switch_margin(0.0) - 0.02).abs() < 0.001);
        assert!((switch_margin(0.5) - 0.0525).abs() < 0.001);
        assert!((switch_margin(1.0) - 0.15).abs() < 0.001);
        assert!(switch_margin(0.0) < switch_margin(0.5));
        assert!(switch_margin(0.5) < switch_margin(1.0));
    }

    #[test]
    fn high_persistence_retains_goal_when_alternative_is_slightly_better() {
        let mut mind = Mind::default();
        let personality = Personality {
            curiosity: 0.0,
            sociability: 0.5,
            cooperativeness: 0.5,
            caution: 0.5,
            persistence: 1.0,
        };
        let goal = evaluate_goals(
            &mut mind,
            35.0,
            100.0,
            25 * TICKS_PER_YEAR,
            &personality,
            Some(Goal::Eat),
            DecisionContext {
                tick: 0,
                origin: (0, 0),
                food_in_inventory: 10,
                best_visible_food_share_score: None,
                best_remembered_social_score: None,
            },
        );
        assert_eq!(goal, Goal::Eat);
        let explanation = mind.decision_explanation.expect("decision explanation");
        assert_eq!(explanation.reason, DecisionReason::GoalPersistence);
        assert_eq!(explanation.chosen_goal, Goal::Eat);
        assert_eq!(explanation.highest_utility_goal, Goal::Explore);
        assert!(explanation.highest_score > explanation.chosen_score);
    }

    #[test]
    fn low_persistence_switches_goal_when_alternative_is_slightly_better() {
        let mut mind = Mind::default();
        let personality = Personality {
            curiosity: 0.0,
            sociability: 0.5,
            cooperativeness: 0.5,
            caution: 0.5,
            persistence: 0.0,
        };
        let goal = evaluate_goals(
            &mut mind,
            35.0,
            100.0,
            25 * TICKS_PER_YEAR,
            &personality,
            Some(Goal::Eat),
            DecisionContext {
                tick: 0,
                origin: (0, 0),
                food_in_inventory: 0,
                best_visible_food_share_score: None,
                best_remembered_social_score: None,
            },
        );
        assert_eq!(goal, Goal::Explore);
        let explanation = mind.decision_explanation.expect("decision explanation");
        assert_eq!(explanation.reason, DecisionReason::HighestUtility);
        assert_eq!(explanation.chosen_goal, Goal::Explore);
        assert_eq!(explanation.highest_utility_goal, Goal::Explore);
    }

    #[test]
    fn strong_remembered_relationship_can_trigger_socialize() {
        let mut mind = Mind::default();
        mind.memory.known_entities.push(KnownEntity {
            id: 2,
            first_seen_tick: 0,
            last_seen_tick: 0,
            last_seen_x: 10,
            last_seen_y: 10,
            observed_ticks: 10,
            affinity: 500,
            last_interaction_tick: 0,
            interaction_count: 5,
            seek_retry_after_tick: None,
        });
        let personality = Personality {
            curiosity: 0.0,
            sociability: 1.0,
            cooperativeness: 0.5,
            caution: 0.5,
            persistence: 0.5,
        };

        let goal = evaluate_goals(
            &mut mind,
            0.0,
            100.0,
            25 * TICKS_PER_YEAR,
            &personality,
            None,
            DecisionContext {
                tick: 0,
                origin: (0, 0),
                food_in_inventory: 0,
                best_visible_food_share_score: None,
                best_remembered_social_score: None,
            },
        );

        assert_eq!(goal, Goal::Socialize);
    }

    #[test]
    fn hungry_child_explains_food_need() {
        let mut mind = Mind::default();
        mind.memory.known_resources.push(KnownResource {
            x: 1,
            y: 1,
            kind: ResourceKind::Food,
            last_seen_tick: 0,
            estimated_amount: 10,
            failed_attempts: 0,
            avoid_until_tick: 0,
        });

        let goal = evaluate_goals(
            &mut mind,
            FOOD_SEARCH_THRESHOLD,
            100.0,
            6 * TICKS_PER_YEAR,
            &Personality {
                curiosity: 0.5,
                sociability: 0.5,
                cooperativeness: 0.5,
                caution: 0.5,
                persistence: 0.5,
            },
            None,
            DecisionContext {
                tick: 0,
                origin: (0, 0),
                food_in_inventory: 0,
                best_visible_food_share_score: None,
                best_remembered_social_score: None,
            },
        );

        assert_eq!(goal, Goal::AcquireResource);
        assert_eq!(
            mind.decision_explanation
                .expect("decision explanation")
                .reason,
            DecisionReason::DependentNeedsFood
        );
    }

    #[test]
    fn sated_child_explains_caregiver_dependency() {
        let mut mind = Mind::default();

        let goal = evaluate_goals(
            &mut mind,
            0.0,
            100.0,
            6 * TICKS_PER_YEAR,
            &Personality {
                curiosity: 0.5,
                sociability: 0.5,
                cooperativeness: 0.5,
                caution: 0.5,
                persistence: 0.5,
            },
            None,
            DecisionContext {
                tick: 0,
                origin: (0, 0),
                food_in_inventory: 0,
                best_visible_food_share_score: None,
                best_remembered_social_score: None,
            },
        );

        assert_eq!(goal, Goal::Follow);
        assert_eq!(
            mind.decision_explanation
                .expect("decision explanation")
                .reason,
            DecisionReason::DependentFollowsCaregiver
        );
    }
}
