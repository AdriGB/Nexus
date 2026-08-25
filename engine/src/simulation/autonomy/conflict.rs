//! Deliberate, visible-only confrontation between hostile household members.

use super::mind::{manhattan, Action, Goal};
use super::social::SOCIAL_RADIUS;
use super::{Entity, EntitySnapshot};
use crate::pathfinding::{self, PathfindingWorkspace};
use crate::world::Grid;

pub(in crate::simulation) const HOUSEHOLD_EXIT_AFFINITY_THRESHOLD: i16 = -400;

#[derive(Clone, Copy, Debug, PartialEq)]
pub(in crate::simulation) struct HouseholdConflictCandidate {
    pub target_id: u32,
    pub score: f32,
    pub affinity: i16,
}

pub(in crate::simulation) fn best_household_conflict_candidate(
    entity: &Entity,
    population: &[EntitySnapshot],
    tick: u64,
) -> Option<HouseholdConflictCandidate> {
    let household_id = entity.household_id?;
    if matches!(
        super::super::LifeStage::from_age_ticks(entity.age_ticks),
        super::super::LifeStage::Infant | super::super::LifeStage::Child
    ) {
        return None;
    }
    entity
        .mind
        .visible_entities
        .iter()
        .filter_map(|target_id| {
            let snapshot = population
                .binary_search_by_key(target_id, |item| item.id)
                .ok()
                .map(|index| &population[index])?;
            if snapshot.household_id != Some(household_id)
                || snapshot.is_child
                || snapshot.is_infant
            {
                return None;
            }
            let known = entity
                .mind
                .memory
                .known_entities
                .binary_search_by_key(target_id, |known| known.id)
                .ok()
                .map(|index| &entity.mind.memory.known_entities[index])?;
            if known.affinity > -100
                || entity.mind.memory.knows_entity_dead(*target_id)
                || entity.mind.memory.conflict_on_cooldown(*target_id, tick)
            {
                return None;
            }
            let hostility = (-f32::from(known.affinity) / 1_000.0).clamp(0.0, 1.0);
            let hunger_stress = (entity.hunger / 100.0).clamp(0.0, 1.0);
            let score = hostility
                * (0.55 + 0.25 * hunger_stress + 0.20 * (1.0 - entity.personality.cooperativeness));
            Some(HouseholdConflictCandidate {
                target_id: *target_id,
                score,
                affinity: known.affinity,
            })
        })
        .max_by(|left, right| {
            left.score
                .total_cmp(&right.score)
                .then_with(|| right.affinity.cmp(&left.affinity))
                .then_with(|| right.target_id.cmp(&left.target_id))
        })
}

pub(super) fn plan_household_conflict(
    entity: &mut Entity,
    world: &Grid,
    tick: u64,
    population: &[EntitySnapshot],
    workspace: &mut PathfindingWorkspace,
) {
    let Some(candidate) = best_household_conflict_candidate(entity, population, tick) else {
        entity.mind.clear_goal();
        return;
    };
    let snapshot = population
        .binary_search_by_key(&candidate.target_id, |item| item.id)
        .ok()
        .map(|index| &population[index])
        .unwrap();
    let target = (snapshot.x, snapshot.y);
    let mut actions = Vec::new();
    if manhattan((entity.x, entity.y), target) > SOCIAL_RADIUS {
        let Some(path) =
            pathfinding::find_path_with_workspace(workspace, world, (entity.x, entity.y), target)
        else {
            entity.mind.clear_goal();
            return;
        };
        entity.path = path.into_iter().skip(1).collect();
        entity.path_index = 0;
        actions.push(Action::ApproachEntity(candidate.target_id));
    }
    actions.push(Action::Interact(candidate.target_id));
    entity
        .mind
        .set_plan(Goal::ConfrontHouseholdMember, actions, tick);
}
