//! Tick orchestration for the simulation.
//!
//! This module owns execution order, while [`Simulation`] owns state and the
//! domain modules own their rules. Keeping orchestration here makes additions
//! to the tick lifecycle visible without turning `Simulation` into a scheduler.

use super::{
    autonomy, dependents, households, physiology, AutonomyProfile, PhaseProfile, Simulation,
};
use crate::world::Grid;
use web_time::Instant;

pub(super) fn run_step(simulation: &mut Simulation, world: &mut Grid) {
    simulation.tick = simulation.tick.saturating_add(1);
    simulation.regenerate_renewable_resources(world);
    physiology::advance(&mut simulation.entities);
    dependents::clear_graduated_caregivers(&mut simulation.entities);
    let (consumed_this_tick, world_changed) = simulation.update_autonomy(world);
    physiology::resolve_starvation(&mut simulation.entities);
    simulation.record_resource_changes(consumed_this_tick, world_changed);
    simulation.remove_dead_entities();
    dependents::reassign_orphaned_dependents(&mut simulation.entities, world);
    simulation.update_pregnancies(world);
    households::synchronize_dependent_memberships(&mut simulation.entities, &simulation.households);
    households::dissolve_empty_households(
        &simulation.entities,
        &mut simulation.households,
        simulation.tick,
    );
    dependents::snap_infants_to_caregivers(&mut simulation.entities);
    simulation.run_daily_relationship_decay();
    simulation.try_daily_conceptions();
}

pub(super) fn run_profiled_step(simulation: &mut Simulation, world: &mut Grid) -> PhaseProfile {
    let total_start = Instant::now();

    simulation.tick = simulation.tick.saturating_add(1);
    simulation.regenerate_renewable_resources(world);

    let start = Instant::now();
    physiology::advance(&mut simulation.entities);
    let physiology_us = start.elapsed().as_micros() as u64;

    dependents::clear_graduated_caregivers(&mut simulation.entities);
    dependents::snap_infants_to_caregivers(&mut simulation.entities);

    let start = Instant::now();
    simulation.rebuild_population_index(world);
    let population_index_us = start.elapsed().as_micros() as u64;

    let start = Instant::now();
    let (
        consumed_this_tick,
        world_changed,
        consumer_ids,
        discoveries,
        encounters,
        interactions,
        food_share_attempts,
        household_deposit_attempts,
        household_withdraw_attempts,
    ) = simulation.run_autonomy(world);
    simulation.record_resource_discoveries(discoveries);
    simulation.record_entity_encounters(encounters);
    simulation.record_food_consumptions(&consumer_ids);
    simulation.record_social_interactions(interactions);
    simulation.process_food_share_attempts(food_share_attempts);
    simulation.process_household_deposit_attempts(household_deposit_attempts);
    simulation.process_household_withdraw_attempts(household_withdraw_attempts);
    let autonomy_us = start.elapsed().as_micros() as u64;

    dependents::snap_infants_to_caregivers(&mut simulation.entities);
    for (id, amount) in consumer_ids {
        dependents::feed_infants_of(&mut simulation.entities, id, amount);
    }

    let start = Instant::now();
    physiology::resolve_starvation(&mut simulation.entities);
    let starvation_us = start.elapsed().as_micros() as u64;

    let start = Instant::now();
    simulation.record_resource_changes(consumed_this_tick, world_changed);
    let resource_changes_us = start.elapsed().as_micros() as u64;

    let start = Instant::now();
    simulation.remove_dead_entities();
    let remove_dead_us = start.elapsed().as_micros() as u64;

    let start = Instant::now();
    dependents::reassign_orphaned_dependents(&mut simulation.entities, world);
    simulation.update_pregnancies(world);
    households::synchronize_dependent_memberships(&mut simulation.entities, &simulation.households);
    households::dissolve_empty_households(
        &simulation.entities,
        &mut simulation.households,
        simulation.tick,
    );
    dependents::snap_infants_to_caregivers(&mut simulation.entities);
    let pregnancies_us = start.elapsed().as_micros() as u64;

    simulation.run_daily_relationship_decay();

    let start = Instant::now();
    simulation.try_daily_conceptions();
    let conceptions_us = start.elapsed().as_micros() as u64;

    PhaseProfile {
        physiology_us,
        population_index_us,
        autonomy_us,
        starvation_us,
        resource_changes_us,
        remove_dead_us,
        pregnancies_us,
        conceptions_us,
        total_us: total_start.elapsed().as_micros() as u64,
    }
}

pub(super) fn run_profiled_autonomy_step(
    simulation: &mut Simulation,
    world: &mut Grid,
) -> AutonomyProfile {
    simulation.tick = simulation.tick.saturating_add(1);
    simulation.regenerate_renewable_resources(world);
    physiology::advance(&mut simulation.entities);
    dependents::clear_graduated_caregivers(&mut simulation.entities);
    dependents::snap_infants_to_caregivers(&mut simulation.entities);
    simulation.rebuild_population_index(world);

    let tick = simulation.tick;
    let population_cache = &simulation.population_cache;
    let spatial_grid = &simulation.spatial_grid;
    let pathfinding_workspace = &mut simulation.pathfinding_workspace;
    let household_contexts: Vec<_> = simulation
        .entities
        .iter()
        .map(|entity| {
            entity.household_id.and_then(|household_id| {
                simulation
                    .households
                    .binary_search_by_key(&household_id, |household| household.id)
                    .ok()
                    .filter(|index| simulation.households[*index].is_active())
                    .map(|index| {
                        let household = &simulation.households[index];
                        autonomy::HouseholdAutonomyContext {
                            residence: (household.residence_x, household.residence_y),
                            storage_remaining_capacity: household.storage.remaining_capacity(),
                            storage_food_amount: household.storage.amount(super::ItemKind::Food),
                        }
                    })
            })
        })
        .collect();

    let (
        consumed_this_tick,
        world_changed,
        profile,
        consumer_ids,
        discoveries,
        encounters,
        interactions,
        food_share_attempts,
        household_deposit_attempts,
        household_withdraw_attempts,
    ) = autonomy::profile_autonomy(
        &mut simulation.entities,
        world,
        tick,
        population_cache,
        spatial_grid,
        pathfinding_workspace,
        &household_contexts,
    );
    simulation.record_resource_discoveries(discoveries);
    simulation.record_entity_encounters(encounters);
    simulation.record_food_consumptions(&consumer_ids);
    simulation.record_social_interactions(interactions);
    simulation.process_food_share_attempts(food_share_attempts);
    simulation.process_household_deposit_attempts(household_deposit_attempts);
    simulation.process_household_withdraw_attempts(household_withdraw_attempts);

    dependents::snap_infants_to_caregivers(&mut simulation.entities);
    for (id, amount) in consumer_ids {
        dependents::feed_infants_of(&mut simulation.entities, id, amount);
    }

    physiology::resolve_starvation(&mut simulation.entities);
    simulation.record_resource_changes(consumed_this_tick, world_changed);
    simulation.remove_dead_entities();
    dependents::reassign_orphaned_dependents(&mut simulation.entities, world);
    simulation.update_pregnancies(world);
    households::synchronize_dependent_memberships(&mut simulation.entities, &simulation.households);
    households::dissolve_empty_households(
        &simulation.entities,
        &mut simulation.households,
        simulation.tick,
    );
    // Covers both newly reassigned infants and newborns assigned to their mother.
    dependents::snap_infants_to_caregivers(&mut simulation.entities);
    simulation.run_daily_relationship_decay();
    simulation.try_daily_conceptions();

    profile
}
