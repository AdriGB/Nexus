mod autonomy;
mod config;
mod dependents;
mod entity;
mod events;
mod food_sharing;
mod genealogy;
mod grief;
mod households;
mod inventory;
mod kinship;
mod lifecycle;
mod partnerships;
mod performance;
mod physiology;
mod pipeline;
mod renewal;
mod spatial;
mod time;

use self::autonomy::Mind;
pub(crate) use self::autonomy::GATHER_DURATION_TICKS;
pub(crate) use self::autonomy::{Action, AutonomyProfile, Goal};
pub(crate) use self::config::MAX_POPULATION;
use self::config::{FOOD_SEARCH_THRESHOLD, MAX_HEALTH};
pub use self::entity::{Entity, EntityActivity, LifeStage, Personality, Sex};
pub(crate) use self::events::EntityEventSummary;
pub(crate) use self::events::EventId;
pub use self::events::{
    EventLocation, SimulationEvent, SimulationEventCause, SimulationEventDetails,
    SimulationEventKind,
};
use self::events::{PendingSimulationEvent, RecentEventHistory};
use self::genealogy::Genealogy;
pub(crate) use self::households::{members_of, Household};
pub use self::inventory::{Inventory, ItemKind};
pub(crate) use self::kinship::{
    ancestors_of, children_of, descendants_of, family_tree_of, relationship_between, siblings_of,
    FamilyTree, KinshipGeneration, KinshipRelation,
};
use self::lifecycle::{
    founder_age_for, lifespan_for, personality_for, sex_for, spawn_candidates, try_conceptions,
    DAILY_CONCEPTION_THRESHOLD,
};
pub(crate) use self::performance::{PerformanceRun, PerformanceSummary};
use self::spatial::{EntitySnapshot, SpatialGrid};
pub(crate) use self::time::years_from_ticks;
use self::time::TICKS_PER_DAY;
use crate::pathfinding::PathfindingWorkspace;
use crate::world::Grid;
use std::collections::HashSet;
use web_time::Instant;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) struct DeathContext {
    pub entity_id: u32,
    pub household_id: Option<u32>,
    pub partner_id: Option<u32>,
    pub caregiver_id: Option<u32>,
}

pub const INITIAL_POPULATION: u32 = 10;

/// Named outcome of one autonomy tick (A07). Replaces positional tuple.
#[derive(Debug, Default)]
struct AutonomyTickOutcome {
    consumed: u64,
    world_changed: bool,
    consumer_ids: Vec<(u32, u16)>,
    discoveries: Vec<autonomy::ResourceDiscovery>,
    encounters: Vec<autonomy::EntityEncounter>,
    interactions: Vec<autonomy::SocialInteraction>,
    food_share_attempts: Vec<autonomy::FoodShareAttempt>,
    household_deposit_attempts: Vec<autonomy::HouseholdDepositAttempt>,
    household_withdraw_attempts: Vec<autonomy::HouseholdWithdrawAttempt>,
    household_conflict_attempts: Vec<autonomy::HouseholdConflictAttempt>,
}

#[derive(Clone, Copy, Debug)]
pub struct PopulationStats {
    pub population: u32,
    pub births: u64,
    pub deaths: u64,
    pub hungry: u32,
    pub seeking_food: u32,
    pub average_hunger: f32,
    pub food_consumed: u64,
    pub females: u32,
    pub males: u32,
    pub pregnant: u32,
}

#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub(crate) struct HouseholdStats {
    pub total_households: u32,
    pub active_households: u32,
    pub dissolved_households: u32,
    pub housed_entities: u32,
    pub unhoused_entities: u32,
    pub average_active_household_size: f32,
    pub largest_active_household_size: u32,
    pub single_member_households: u32,
    pub households_with_dependents: u32,
    pub active_storage_capacity: u64,
    pub active_storage_used: u64,
    pub active_storage_utilization: f32,
    pub active_food_stored: u64,
    pub active_timber_stored: u64,
    pub active_stone_stored: u64,
    pub active_iron_stored: u64,
    pub settled_inheritances: u32,
    pub inheritances_without_heir: u32,
    pub average_active_household_age_ticks: f64,
    pub average_dissolved_household_lifetime_ticks: f64,
}

#[derive(Clone, Debug, Default, PartialEq, Eq, serde::Serialize)]
pub(crate) struct WorkCounters {
    pub entities_processed: u64,
    pub entities_perceived: u64,
    pub goal_evaluations: u64,
    pub goal_changes: u64,
    pub plans_created: u64,
    pub actions_executed: u64,
    pub social_interactions: u64,
    pub spatial_queries: u64,
    pub pathfinding_searches: u64,
    pub pathfinding_nodes_expanded: u64,
    pub events_created: u64,
    // A08 — maintenance scans (global, sin pathfinding) para medir antes de event-driven
    pub orphan_reassignment_scans: u64,
    pub household_sync_scans: u64,
    pub household_migration_scans: u64,
    pub conception_scans: u64,
}

impl WorkCounters {
    pub(crate) fn accumulate(&mut self, other: &Self) {
        self.entities_processed = self
            .entities_processed
            .saturating_add(other.entities_processed);
        self.entities_perceived = self
            .entities_perceived
            .saturating_add(other.entities_perceived);
        self.goal_evaluations = self.goal_evaluations.saturating_add(other.goal_evaluations);
        self.goal_changes = self.goal_changes.saturating_add(other.goal_changes);
        self.plans_created = self.plans_created.saturating_add(other.plans_created);
        self.actions_executed = self.actions_executed.saturating_add(other.actions_executed);
        self.social_interactions = self
            .social_interactions
            .saturating_add(other.social_interactions);
        self.spatial_queries = self.spatial_queries.saturating_add(other.spatial_queries);
        self.pathfinding_searches = self
            .pathfinding_searches
            .saturating_add(other.pathfinding_searches);
        self.pathfinding_nodes_expanded = self
            .pathfinding_nodes_expanded
            .saturating_add(other.pathfinding_nodes_expanded);
        self.events_created = self.events_created.saturating_add(other.events_created);
        self.orphan_reassignment_scans = self
            .orphan_reassignment_scans
            .saturating_add(other.orphan_reassignment_scans);
        self.household_sync_scans = self
            .household_sync_scans
            .saturating_add(other.household_sync_scans);
        self.household_migration_scans = self
            .household_migration_scans
            .saturating_add(other.household_migration_scans);
        self.conception_scans = self.conception_scans.saturating_add(other.conception_scans);
    }
}

#[derive(Clone, Debug, Default, PartialEq, Eq, serde::Serialize)]
pub(crate) struct StateGauges {
    pub entities_alive: u64,
    pub known_entities_total: u64,
    pub known_entities_max_per_entity: u64,
    pub known_resources_total: u64,
    pub known_resources_max_per_entity: u64,
    pub known_dead_entities_total: u64,
    pub active_grief_states: u64,
    pub recent_events_len: u64,
    pub recent_events_capacity: u64,
    pub households_active: u64,
    pub genealogy_links: u64,
}

impl StateGauges {
    fn retain_maximums(&mut self, other: &Self) {
        self.entities_alive = self.entities_alive.max(other.entities_alive);
        self.known_entities_total = self.known_entities_total.max(other.known_entities_total);
        self.known_entities_max_per_entity = self
            .known_entities_max_per_entity
            .max(other.known_entities_max_per_entity);
        self.known_resources_total = self.known_resources_total.max(other.known_resources_total);
        self.known_resources_max_per_entity = self
            .known_resources_max_per_entity
            .max(other.known_resources_max_per_entity);
        self.known_dead_entities_total = self
            .known_dead_entities_total
            .max(other.known_dead_entities_total);
        self.active_grief_states = self.active_grief_states.max(other.active_grief_states);
        self.recent_events_len = self.recent_events_len.max(other.recent_events_len);
        self.recent_events_capacity = self
            .recent_events_capacity
            .max(other.recent_events_capacity);
        self.households_active = self.households_active.max(other.households_active);
        self.genealogy_links = self.genealogy_links.max(other.genealogy_links);
    }

    #[cfg(test)]
    pub(crate) fn dominates(&self, other: &Self) -> bool {
        self.entities_alive >= other.entities_alive
            && self.known_entities_total >= other.known_entities_total
            && self.known_entities_max_per_entity >= other.known_entities_max_per_entity
            && self.known_resources_total >= other.known_resources_total
            && self.known_resources_max_per_entity >= other.known_resources_max_per_entity
            && self.known_dead_entities_total >= other.known_dead_entities_total
            && self.active_grief_states >= other.active_grief_states
            && self.recent_events_len >= other.recent_events_len
            && self.recent_events_capacity >= other.recent_events_capacity
            && self.households_active >= other.households_active
            && self.genealogy_links >= other.genealogy_links
    }
}

#[derive(Clone, Debug, Default)]
pub(crate) struct PhaseProfile {
    pub world_maintenance_us: u64,
    pub physiology_us: u64,
    pub dependent_care_us: u64,
    pub households_us: u64,
    pub spatial_index_us: u64,
    pub autonomy_us: u64,
    pub survival_us: u64,
    pub mortality_us: u64,
    pub lifecycle_us: u64,
    pub relationships_us: u64,
    pub reproduction_us: u64,
    pub total_us: u64,
    pub work: WorkCounters,
    pub state: StateGauges,
}

#[derive(Clone, Debug)]
pub struct Simulation {
    tick: u64,
    paused: bool,
    entities: Vec<Entity>,
    population_cache: Vec<EntitySnapshot>,
    spatial_grid: SpatialGrid,
    pathfinding_workspace: PathfindingWorkspace,
    next_entity_id: u32,
    world_revision: u64,
    births: u64,
    deaths: u64,
    food_consumed: u64,
    seed: u64,
    recent_events: RecentEventHistory,
    genealogy: Genealogy,
    households: Vec<Household>,
    next_household_id: u32,
}

impl Default for Simulation {
    fn default() -> Self {
        Self {
            tick: 0,
            paused: true,
            entities: Vec::new(),
            population_cache: Vec::new(),
            spatial_grid: SpatialGrid::default(),
            pathfinding_workspace: PathfindingWorkspace::new(),
            next_entity_id: 1,
            world_revision: 0,
            births: 0,
            deaths: 0,
            food_consumed: 0,
            seed: 0,
            recent_events: RecentEventHistory::default(),
            genealogy: Genealogy::default(),
            households: Vec::new(),
            next_household_id: 1,
        }
    }
}

impl Simulation {
    fn state_gauges(&self) -> StateGauges {
        let mut gauges = StateGauges {
            entities_alive: self.entities.len() as u64,
            recent_events_len: self.recent_events.len() as u64,
            recent_events_capacity: self.recent_events.capacity() as u64,
            households_active: self
                .households
                .iter()
                .filter(|household| household.is_active())
                .count() as u64,
            genealogy_links: self
                .genealogy
                .records()
                .iter()
                .map(|record| {
                    u64::from(record.mother_id.is_some()) + u64::from(record.father_id.is_some())
                })
                .sum(),
            ..StateGauges::default()
        };

        for entity in &self.entities {
            let known_entities = entity.mind.memory.known_entities.len() as u64;
            let known_resources = entity.mind.memory.known_resources.len() as u64;
            gauges.known_entities_total += known_entities;
            gauges.known_entities_max_per_entity =
                gauges.known_entities_max_per_entity.max(known_entities);
            gauges.known_resources_total += known_resources;
            gauges.known_resources_max_per_entity =
                gauges.known_resources_max_per_entity.max(known_resources);
            gauges.known_dead_entities_total += entity.mind.memory.known_dead_entities.len() as u64;
            gauges.active_grief_states += entity.mind.grief.len() as u64;
        }

        gauges
    }

    pub fn with_population(seed: u64, world: &Grid, count: u32) -> Self {
        let mut simulation = Self {
            seed,
            ..Self::default()
        };
        simulation.spawn_entities(world, count);
        simulation
    }

    pub fn tick(&self) -> u64 {
        self.tick
    }

    pub fn is_paused(&self) -> bool {
        self.paused
    }

    pub fn entities(&self) -> &[Entity] {
        &self.entities
    }

    pub(crate) fn genealogy(&self) -> &Genealogy {
        &self.genealogy
    }

    pub(crate) fn households(&self) -> &[Household] {
        &self.households
    }

    pub fn transfer_item(
        &mut self,
        source_id: u32,
        target_id: u32,
        kind: ItemKind,
        quantity: u16,
    ) -> u16 {
        if source_id == target_id || quantity == 0 {
            return 0;
        }
        let Ok(source_index) = self
            .entities
            .binary_search_by_key(&source_id, |entity| entity.id)
        else {
            return 0;
        };
        let Ok(target_index) = self
            .entities
            .binary_search_by_key(&target_id, |entity| entity.id)
        else {
            return 0;
        };

        let moved = quantity
            .min(self.entities[source_index].inventory.amount(kind))
            .min(self.entities[target_index].inventory.remaining_capacity());
        if moved == 0 {
            return 0;
        }

        let (source, target) = if source_index < target_index {
            let (before_target, from_target) = self.entities.split_at_mut(target_index);
            (&mut before_target[source_index], &mut from_target[0])
        } else {
            let (before_source, from_source) = self.entities.split_at_mut(source_index);
            (&mut from_source[0], &mut before_source[target_index])
        };
        let removed = source.inventory.remove(kind, moved);
        let accepted = target.inventory.add(kind, moved);
        debug_assert_eq!(removed, moved);
        debug_assert_eq!(accepted, moved);
        moved
    }

    pub(crate) fn deposit_to_household(
        &mut self,
        entity_id: u32,
        kind: ItemKind,
        quantity: u16,
    ) -> u16 {
        let Some((entity_index, household_index)) =
            self.household_storage_access(entity_id, quantity)
        else {
            return 0;
        };
        let moved = quantity
            .min(self.entities[entity_index].inventory.amount(kind))
            .min(
                self.households[household_index]
                    .storage
                    .remaining_capacity(),
            );
        if moved == 0 {
            return 0;
        }
        let removed = self.entities[entity_index].inventory.remove(kind, moved);
        let accepted = self.households[household_index].storage.add(kind, moved);
        debug_assert_eq!(removed, moved);
        debug_assert_eq!(accepted, moved);
        moved
    }

    pub(crate) fn withdraw_from_household(
        &mut self,
        entity_id: u32,
        kind: ItemKind,
        quantity: u16,
    ) -> u16 {
        let Some((entity_index, household_index)) =
            self.household_storage_access(entity_id, quantity)
        else {
            return 0;
        };
        let moved = quantity
            .min(self.households[household_index].storage.amount(kind))
            .min(self.entities[entity_index].inventory.remaining_capacity());
        if moved == 0 {
            return 0;
        }
        let removed = self.households[household_index].storage.remove(kind, moved);
        let accepted = self.entities[entity_index].inventory.add(kind, moved);
        debug_assert_eq!(removed, moved);
        debug_assert_eq!(accepted, moved);
        moved
    }

    fn household_storage_access(&self, entity_id: u32, quantity: u16) -> Option<(usize, usize)> {
        if quantity == 0 {
            return None;
        }
        let entity_index = self
            .entities
            .binary_search_by_key(&entity_id, |entity| entity.id)
            .ok()?;
        let household_id = self.entities[entity_index].household_id?;
        let household_index = self
            .households
            .binary_search_by_key(&household_id, |household| household.id)
            .ok()?;
        let household = &self.households[household_index];
        (household.is_active()
            && (self.entities[entity_index].x, self.entities[entity_index].y)
                == (household.residence_x, household.residence_y))
            .then_some((entity_index, household_index))
    }

    pub(crate) fn recent_events(&self) -> impl DoubleEndedIterator<Item = &SimulationEvent> {
        self.recent_events.iter()
    }

    pub(crate) fn entity_event_summary(&self, entity_id: u32) -> EntityEventSummary {
        self.recent_events.summary_for(entity_id)
    }

    pub fn world_revision(&self) -> u64 {
        self.world_revision
    }

    fn regenerate_renewable_resources(&mut self, world: &mut Grid) {
        if self.tick.is_multiple_of(time::TICKS_PER_DAY) && renewal::regenerate(world) {
            self.world_revision = self.world_revision.saturating_add(1);
        }
    }

    pub fn population_stats(&self) -> PopulationStats {
        let population = self.entities.len() as u32;
        let hunger_total: f32 = self.entities.iter().map(|entity| entity.hunger).sum();
        PopulationStats {
            population,
            births: self.births,
            deaths: self.deaths,
            hungry: self
                .entities
                .iter()
                .filter(|entity| entity.hunger >= FOOD_SEARCH_THRESHOLD)
                .count() as u32,
            seeking_food: self
                .entities
                .iter()
                .filter(|entity| {
                    matches!(
                        entity.activity,
                        EntityActivity::SeekingFood | EntityActivity::Moving
                    )
                })
                .count() as u32,
            average_hunger: if population == 0 {
                0.0
            } else {
                hunger_total / population as f32
            },
            food_consumed: self.food_consumed,
            females: self
                .entities
                .iter()
                .filter(|entity| entity.sex == Sex::Female)
                .count() as u32,
            males: self
                .entities
                .iter()
                .filter(|entity| entity.sex == Sex::Male)
                .count() as u32,
            pregnant: self
                .entities
                .iter()
                .filter(|entity| entity.pregnancy.is_some())
                .count() as u32,
        }
    }

    pub(crate) fn household_stats(&self) -> HouseholdStats {
        households::household_stats(&self.entities, &self.households, self.tick)
    }

    pub fn advance(&mut self, ticks: u32, world: &mut Grid) -> u64 {
        if !self.paused {
            for _ in 0..ticks {
                pipeline::run_step(self, world);
            }
        }
        self.tick
    }

    pub fn step(&mut self, world: &mut Grid) -> u64 {
        pipeline::run_step(self, world);
        self.tick
    }

    pub(crate) fn profile_step(&mut self, world: &mut Grid) -> PhaseProfile {
        pipeline::run_profiled_step(self, world)
    }

    pub(crate) fn profile_autonomy_step(&mut self, world: &mut Grid) -> AutonomyProfile {
        pipeline::run_profiled_autonomy_step(self, world)
    }

    pub(crate) fn profile_run(&mut self, world: &mut Grid, ticks: u32) -> PerformanceSummary {
        let mut run = PerformanceRun::default();
        for _ in 0..ticks {
            run.record(self.profile_step(world));
        }
        run.summarize()
    }

    pub fn pause(&mut self) {
        self.paused = true;
    }

    pub fn resume(&mut self) {
        self.paused = false;
    }

    pub fn spawn_entities(&mut self, world: &Grid, count: u32) -> u32 {
        let available = MAX_POPULATION.saturating_sub(self.entities.len());
        let requested = usize::try_from(count).unwrap_or(usize::MAX).min(available);
        if requested == 0 {
            return 0;
        }

        let occupied: HashSet<_> = self
            .entities
            .iter()
            .map(|entity| (entity.x, entity.y))
            .collect();
        let mut spawned = 0;
        for position in spawn_candidates(world)
            .into_iter()
            .filter(|position| !occupied.contains(position))
            .take(requested)
        {
            if self.push_founder(position).is_some() {
                spawned += 1;
            }
        }
        spawned
    }

    fn push_founder(&mut self, position: (u32, u32)) -> Option<u32> {
        let age_ticks = founder_age_for(self.seed, self.next_entity_id);
        self.push_entity(position, age_ticks)
    }

    fn push_entity(&mut self, position: (u32, u32), age_ticks: u64) -> Option<u32> {
        self.push_entity_with_parentage(position, age_ticks, None, None)
    }

    fn push_entity_with_parentage(
        &mut self,
        (x, y): (u32, u32),
        age_ticks: u64,
        mother_id: Option<u32>,
        father_id: Option<u32>,
    ) -> Option<u32> {
        let id = self.next_entity_id;
        self.next_entity_id = self.next_entity_id.checked_add(1)?;
        self.entities.push(Entity {
            id,
            x,
            y,
            sex: sex_for(self.seed, id),
            lifespan_ticks: lifespan_for(self.seed, id),
            hunger: 0.0,
            health: MAX_HEALTH,
            age_ticks,
            path: Vec::new(),
            path_index: 0,
            activity: EntityActivity::Idle,
            mind: Mind::default(),
            pregnancy: None,
            postpartum_until_tick: 0,
            movement_credit: 0.0,
            mother_id,
            father_id,
            caregiver_id: None,
            partner_id: None,
            household_id: None,
            personality: personality_for(self.seed, id),
            inventory: Inventory::default(),
            action_tick: 0,
        });
        self.genealogy.register(id, mother_id, father_id);
        Some(id)
    }

    fn execute_autonomy(
        &mut self,
        world: &mut Grid,
        profile: Option<&mut AutonomyProfile>,
        work: Option<&mut WorkCounters>,
    ) -> (u64, bool, Vec<(u32, u16)>) {
        let outcome = self.run_autonomy(world, profile, work);
        self.record_resource_discoveries(outcome.discoveries);
        self.record_entity_encounters(outcome.encounters);
        self.record_food_consumptions(&outcome.consumer_ids);
        self.record_social_interactions(outcome.interactions);
        self.process_food_share_attempts(outcome.food_share_attempts);
        self.process_household_deposit_attempts(outcome.household_deposit_attempts);
        self.process_household_withdraw_attempts(outcome.household_withdraw_attempts);
        self.process_household_conflict_attempts(outcome.household_conflict_attempts);
        (
            outcome.consumed,
            outcome.world_changed,
            outcome.consumer_ids,
        )
    }

    fn run_autonomy(
        &mut self,
        world: &mut Grid,
        mut profile: Option<&mut AutonomyProfile>,
        mut work: Option<&mut WorkCounters>,
    ) -> AutonomyTickOutcome {
        let tick = self.tick;
        let population_cache = &self.population_cache;
        let spatial_grid = &self.spatial_grid;
        let pathfinding_workspace = &mut self.pathfinding_workspace;
        let households = &self.households;

        let mut consumed = 0u64;
        let mut world_changed = false;
        let mut consumer_ids = Vec::new();
        let mut discoveries = Vec::new();
        let mut encounters = Vec::new();
        let mut food_share_attempts = Vec::new();
        let mut household_deposit_attempts = Vec::new();
        let mut household_withdraw_attempts = Vec::new();
        let mut household_conflict_attempts = Vec::new();

        for (index, entity) in self
            .entities
            .iter_mut()
            .filter(|entity| {
                entity.health > 0.0
                    && LifeStage::from_age_ticks(entity.age_ticks) != LifeStage::Infant
            })
            .enumerate()
        {
            if let Some(work) = work.as_deref_mut() {
                work.entities_processed += 1;
            }
            let household_context = entity.household_id.and_then(|household_id| {
                households
                    .binary_search_by_key(&household_id, |household| household.id)
                    .ok()
                    .filter(|index| households[*index].is_active())
                    .map(|index| {
                        let household = &households[index];
                        autonomy::HouseholdAutonomyContext {
                            residence: (household.residence_x, household.residence_y),
                            migration_target: household.active_migration_target(),
                            storage_remaining_capacity: household.storage.remaining_capacity(),
                            storage_food_amount: household.storage.amount(ItemKind::Food),
                        }
                    })
            });
            let (result, entity_discoveries, entity_encounters) = autonomy::update_entity(
                entity,
                world,
                tick,
                population_cache,
                spatial_grid,
                pathfinding_workspace,
                autonomy::EntityUpdateContext {
                    household: household_context,
                    work: work.as_deref_mut(),
                    profile: profile
                        .as_deref_mut()
                        .filter(|_| autonomy::should_profile_entity(index)),
                },
            );
            discoveries.extend(entity_discoveries);
            encounters.extend(entity_encounters);
            if result.food_consumed > 0 {
                consumer_ids.push((entity.id, result.food_consumed));
            }
            consumed += u64::from(result.food_consumed);
            world_changed |= result.world_changed;
            if let Some(attempt) = result.food_share_attempt {
                food_share_attempts.push(attempt);
            }
            if let Some(attempt) = result.household_deposit_attempt {
                household_deposit_attempts.push(attempt);
            }
            if let Some(attempt) = result.household_withdraw_attempt {
                household_withdraw_attempts.push(attempt);
            }
            if let Some(attempt) = result.household_conflict_attempt {
                household_conflict_attempts.push(attempt);
            }
        }

        let social_start = profile.as_ref().map(|_| Instant::now());
        let interactions = autonomy::process_social_interactions(
            &mut self.entities,
            &self.population_cache,
            self.tick,
        );
        if let Some(work) = work {
            work.social_interactions += interactions.len() as u64;
        }
        if let Some(profile) = profile {
            profile.social_us += social_start
                .expect("profile timer must exist")
                .elapsed()
                .as_micros() as u64;
        }

        AutonomyTickOutcome {
            consumed,
            world_changed,
            consumer_ids,
            discoveries,
            encounters,
            interactions,
            food_share_attempts,
            household_deposit_attempts,
            household_withdraw_attempts,
            household_conflict_attempts,
        }
    }

    fn process_household_deposit_attempts(
        &mut self,
        attempts: Vec<autonomy::HouseholdDepositAttempt>,
    ) {
        for attempt in attempts {
            debug_assert_eq!(
                self.entities
                    .binary_search_by_key(&attempt.actor_id, |entity| entity.id)
                    .ok()
                    .map(|index| (self.entities[index].x, self.entities[index].y)),
                Some(attempt.actor_location)
            );
            self.deposit_to_household(attempt.actor_id, ItemKind::Food, attempt.amount);
        }
    }

    fn process_household_withdraw_attempts(
        &mut self,
        attempts: Vec<autonomy::HouseholdWithdrawAttempt>,
    ) {
        for attempt in attempts {
            debug_assert_eq!(
                self.entities
                    .binary_search_by_key(&attempt.actor_id, |entity| entity.id)
                    .ok()
                    .map(|index| (self.entities[index].x, self.entities[index].y)),
                Some(attempt.actor_location)
            );
            self.withdraw_from_household(attempt.actor_id, ItemKind::Food, attempt.amount);
        }
    }

    fn process_household_conflict_attempts(
        &mut self,
        mut attempts: Vec<autonomy::HouseholdConflictAttempt>,
    ) {
        attempts.sort_by_key(|attempt| {
            let pair = (
                attempt.actor_id.min(attempt.target_id),
                attempt.actor_id.max(attempt.target_id),
            );
            let affinity = self
                .entities
                .binary_search_by_key(&attempt.actor_id, |entity| entity.id)
                .ok()
                .and_then(|index| {
                    self.entities[index]
                        .mind
                        .memory
                        .affinity_to(attempt.target_id)
                })
                .unwrap_or(0);
            (pair, affinity, attempt.actor_id)
        });
        attempts.dedup_by_key(|attempt| {
            (
                attempt.actor_id.min(attempt.target_id),
                attempt.actor_id.max(attempt.target_id),
            )
        });

        for attempt in attempts {
            let (Ok(actor_index), Ok(target_index)) = (
                self.entities
                    .binary_search_by_key(&attempt.actor_id, |entity| entity.id),
                self.entities
                    .binary_search_by_key(&attempt.target_id, |entity| entity.id),
            ) else {
                continue;
            };
            if actor_index == target_index {
                continue;
            }
            let household_id = self.entities[actor_index].household_id;
            if self.entities[actor_index].health <= 0.0
                || self.entities[target_index].health <= 0.0
                || household_id.is_none()
                || self.entities[target_index].household_id != household_id
                || !matches!(
                    LifeStage::from_age_ticks(self.entities[actor_index].age_ticks),
                    LifeStage::Adolescent | LifeStage::Adult | LifeStage::Elder
                )
                || !matches!(
                    LifeStage::from_age_ticks(self.entities[target_index].age_ticks),
                    LifeStage::Adolescent | LifeStage::Adult | LifeStage::Elder
                )
                || self.entities[actor_index]
                    .x
                    .abs_diff(self.entities[target_index].x)
                    + self.entities[actor_index]
                        .y
                        .abs_diff(self.entities[target_index].y)
                    > autonomy::SOCIAL_RADIUS
                || household_id.is_some_and(|id| {
                    self.households
                        .binary_search_by_key(&id, |household| household.id)
                        .ok()
                        .is_none_or(|index| !self.households[index].is_active())
                })
            {
                continue;
            }

            let incompatibility = 1.0
                - autonomy::personality_compatibility(
                    &self.entities[actor_index].personality,
                    &self.entities[target_index].personality,
                );
            let actor_delta = -(10
                + (incompatibility * 10.0).round() as i16
                + ((1.0 - self.entities[target_index].personality.cooperativeness) * 10.0).round()
                    as i16);
            let target_delta = -(10
                + (incompatibility * 10.0).round() as i16
                + ((1.0 - self.entities[actor_index].personality.cooperativeness) * 10.0).round()
                    as i16);
            let household_id = household_id.unwrap();
            let event_id = self.push_event(PendingSimulationEvent {
                caused_by_event_id: None,
                tick: self.tick,
                location: EventLocation {
                    x: attempt.actor_location.0,
                    y: attempt.actor_location.1,
                },
                actor_id: attempt.actor_id,
                target_id: Some(attempt.target_id),
                related_entity_ids: vec![attempt.actor_id, attempt.target_id],
                kind: SimulationEventKind::HouseholdConflict,
                cause: SimulationEventCause::HouseholdConflict,
                details: SimulationEventDetails::HouseholdConflict {
                    household_id,
                    actor_affinity_delta: actor_delta,
                    target_affinity_delta: target_delta,
                },
            });
            let actor_location = (self.entities[actor_index].x, self.entities[actor_index].y);
            let target_location = (self.entities[target_index].x, self.entities[target_index].y);
            let actor_change = autonomy::record_directed_affinity(
                &mut self.entities[actor_index],
                attempt.target_id,
                self.tick,
                actor_delta,
            );
            let target_change = autonomy::record_directed_affinity(
                &mut self.entities[target_index],
                attempt.actor_id,
                self.tick,
                target_delta,
            );
            self.entities[actor_index]
                .mind
                .memory
                .mark_conflict(attempt.target_id, self.tick);
            self.entities[target_index]
                .mind
                .memory
                .mark_conflict(attempt.actor_id, self.tick);
            if let Some(change) = actor_change {
                self.record_affinity_change(
                    attempt.actor_id,
                    actor_location,
                    change,
                    SimulationEventCause::HouseholdConflict,
                    Some(event_id),
                );
            }
            if let Some(change) = target_change {
                self.record_affinity_change(
                    attempt.target_id,
                    target_location,
                    change,
                    SimulationEventCause::HouseholdConflict,
                    Some(event_id),
                );
            }
            if let Some(dissolution) =
                partnerships::try_dissolve(&mut self.entities, attempt.actor_id, attempt.target_id)
            {
                self.record_partnership_dissolution(
                    dissolution,
                    actor_location,
                    SimulationEventCause::HouseholdConflict,
                    Some(event_id),
                );
            }
            let actor_affinity = self
                .entities
                .binary_search_by_key(&attempt.actor_id, |entity| entity.id)
                .ok()
                .and_then(|index| {
                    self.entities[index]
                        .mind
                        .memory
                        .affinity_to(attempt.target_id)
                })
                .unwrap_or(0);
            if actor_affinity <= autonomy::HOUSEHOLD_EXIT_AFFINITY_THRESHOLD {
                households::set_member_household(
                    &mut self.entities,
                    &self.households,
                    attempt.actor_id,
                    None,
                );
            }
        }
    }

    fn record_social_interactions(&mut self, interactions: Vec<autonomy::SocialInteraction>) {
        for interaction in interactions {
            let interaction_event_id = self.push_event(PendingSimulationEvent {
                caused_by_event_id: None,
                tick: self.tick,
                location: EventLocation {
                    x: interaction.location.0,
                    y: interaction.location.1,
                },
                actor_id: interaction.actor_id,
                target_id: Some(interaction.target_id),
                related_entity_ids: vec![interaction.actor_id, interaction.target_id],
                kind: SimulationEventKind::Interaction,
                cause: SimulationEventCause::MutualSocialContact,
                details: SimulationEventDetails::Interaction {
                    actor_affinity_delta: interaction.actor_affinity_delta,
                    target_affinity_delta: interaction.target_affinity_delta,
                },
            });
            if let Some(change) = interaction.actor_affinity_change {
                self.record_affinity_change(
                    interaction.actor_id,
                    interaction.actor_location,
                    change,
                    SimulationEventCause::MutualSocialContact,
                    Some(interaction_event_id),
                );
            }
            if let Some(change) = interaction.target_affinity_change {
                self.record_affinity_change(
                    interaction.target_id,
                    interaction.target_location,
                    change,
                    SimulationEventCause::MutualSocialContact,
                    Some(interaction_event_id),
                );
            }
            if let Some(dissolution) = partnerships::try_dissolve(
                &mut self.entities,
                interaction.actor_id,
                interaction.target_id,
            ) {
                self.record_partnership_dissolution(
                    dissolution,
                    interaction.location,
                    SimulationEventCause::MutualSocialContact,
                    Some(interaction_event_id),
                );
            } else if let Some(formation) = partnerships::try_form(
                &mut self.entities,
                &self.genealogy,
                interaction.actor_id,
                interaction.target_id,
            ) {
                households::form_for_partnership(
                    &mut self.entities,
                    &mut self.households,
                    &mut self.next_household_id,
                    formation.actor_id,
                    formation.target_id,
                    self.tick,
                );
                self.push_event(PendingSimulationEvent {
                    caused_by_event_id: Some(interaction_event_id),
                    tick: self.tick,
                    location: EventLocation {
                        x: interaction.location.0,
                        y: interaction.location.1,
                    },
                    actor_id: formation.actor_id,
                    target_id: Some(formation.target_id),
                    related_entity_ids: vec![formation.actor_id, formation.target_id],
                    kind: SimulationEventKind::PartnershipFormed,
                    cause: SimulationEventCause::MutualCommitment,
                    details: SimulationEventDetails::PartnershipFormed {
                        actor_affinity: formation.actor_affinity,
                        target_affinity: formation.target_affinity,
                        compatibility_per_mille: formation.compatibility_per_mille,
                    },
                });
            }
        }
    }

    fn process_food_share_attempts(&mut self, attempts: Vec<autonomy::FoodShareAttempt>) {
        // A03 completo: lógica extraída a `food_sharing::process`. `Simulation`
        // solo orquesta; reduce `mod.rs` y aísla regla de negocio.
        food_sharing::process(self, attempts);
    }

    fn record_partnership_dissolution(
        &mut self,
        dissolution: partnerships::PartnershipDissolution,
        location: (u32, u32),
        cause: SimulationEventCause,
        caused_by_event_id: Option<EventId>,
    ) {
        self.push_event(PendingSimulationEvent {
            caused_by_event_id,
            tick: self.tick,
            location: EventLocation {
                x: location.0,
                y: location.1,
            },
            actor_id: dissolution.actor_id,
            target_id: Some(dissolution.target_id),
            related_entity_ids: vec![dissolution.actor_id, dissolution.target_id],
            kind: SimulationEventKind::PartnershipDissolved,
            cause,
            details: SimulationEventDetails::PartnershipDissolved {
                actor_affinity: dissolution.actor_affinity,
                target_affinity: dissolution.target_affinity,
            },
        });
    }

    fn record_affinity_change(
        &mut self,
        actor_id: u32,
        location: (u32, u32),
        change: autonomy::AffinityChangeRecord,
        cause: SimulationEventCause,
        caused_by_event_id: Option<EventId>,
    ) {
        self.push_event(PendingSimulationEvent {
            caused_by_event_id,
            tick: self.tick,
            location: EventLocation {
                x: location.0,
                y: location.1,
            },
            actor_id,
            target_id: Some(change.target_id),
            related_entity_ids: vec![actor_id, change.target_id],
            kind: SimulationEventKind::AffinityChange,
            cause,
            details: SimulationEventDetails::AffinityChange {
                previous_affinity: change.previous_affinity,
                new_affinity: change.new_affinity,
                delta: change.delta,
            },
        });
    }

    fn record_food_consumptions(&mut self, consumptions: &[(u32, u16)]) {
        for &(entity_id, amount) in consumptions {
            if amount == 0 {
                continue;
            }
            let Ok(index) = self
                .entities
                .binary_search_by_key(&entity_id, |entity| entity.id)
            else {
                continue;
            };
            let entity = &self.entities[index];
            let location = EventLocation {
                x: entity.x,
                y: entity.y,
            };
            self.push_event(PendingSimulationEvent {
                caused_by_event_id: None,
                tick: self.tick,
                location,
                actor_id: entity_id,
                target_id: None,
                related_entity_ids: vec![entity_id],
                kind: SimulationEventKind::Consumption,
                cause: SimulationEventCause::AteFood,
                details: SimulationEventDetails::Consumption { amount },
            });
        }
    }

    fn record_resource_discoveries(&mut self, discoveries: Vec<autonomy::ResourceDiscovery>) {
        for discovery in discoveries {
            self.push_event(PendingSimulationEvent {
                caused_by_event_id: None,
                tick: self.tick,
                location: EventLocation {
                    x: discovery.x,
                    y: discovery.y,
                },
                actor_id: discovery.entity_id,
                target_id: None,
                related_entity_ids: vec![discovery.entity_id],
                kind: SimulationEventKind::Discovery,
                cause: SimulationEventCause::ResourceFound,
                details: SimulationEventDetails::ResourceDiscovery {
                    kind: discovery.kind,
                    amount: discovery.amount,
                },
            });
        }
    }

    fn record_entity_encounters(&mut self, mut encounters: Vec<autonomy::EntityEncounter>) {
        encounters.sort_unstable_by_key(|encounter| {
            (
                encounter.observer_id.min(encounter.other_id),
                encounter.observer_id.max(encounter.other_id),
                encounter.observer_id,
            )
        });
        encounters.dedup_by_key(|encounter| {
            (
                encounter.observer_id.min(encounter.other_id),
                encounter.observer_id.max(encounter.other_id),
            )
        });

        for encounter in encounters {
            let actor_id = encounter.observer_id.min(encounter.other_id);
            let target_id = encounter.observer_id.max(encounter.other_id);
            let previously_known = [(actor_id, target_id), (target_id, actor_id)]
                .into_iter()
                .any(|(observer_id, other_id)| {
                    self.entities
                        .binary_search_by_key(&observer_id, |entity| entity.id)
                        .ok()
                        .and_then(|index| {
                            self.entities[index]
                                .mind
                                .memory
                                .known_entities
                                .binary_search_by_key(&other_id, |known| known.id)
                                .ok()
                                .map(|known_index| {
                                    self.entities[index].mind.memory.known_entities[known_index]
                                        .first_seen_tick
                                        < self.tick
                                })
                        })
                        .unwrap_or(false)
                });
            if previously_known {
                continue;
            }

            self.push_event(PendingSimulationEvent {
                caused_by_event_id: None,
                tick: self.tick,
                location: EventLocation {
                    x: encounter.x,
                    y: encounter.y,
                },
                actor_id,
                target_id: Some(target_id),
                related_entity_ids: vec![actor_id, target_id],
                kind: SimulationEventKind::Encounter,
                cause: SimulationEventCause::FirstEncounter,
                details: SimulationEventDetails::Encounter,
            });
        }
    }

    fn push_event(&mut self, event: PendingSimulationEvent) -> EventId {
        self.recent_events.push(event)
    }

    fn rebuild_population_index(&mut self, world: &Grid) {
        self.population_cache.clear();
        self.spatial_grid.prepare(world.width, world.height);

        for entity in &self.entities {
            let snapshot_index = self.population_cache.len();

            let life_stage = LifeStage::from_age_ticks(entity.age_ticks);
            self.population_cache.push(EntitySnapshot {
                id: entity.id,
                x: entity.x,
                y: entity.y,
                hunger: entity.hunger,
                caregiver_id: entity.caregiver_id,
                household_id: entity.household_id,
                partner_id: entity.partner_id,
                mother_id: entity.mother_id,
                father_id: entity.father_id,
                is_adult: life_stage == LifeStage::Adult,
                is_child: life_stage == LifeStage::Child,
                is_infant: life_stage == LifeStage::Infant,
            });

            self.spatial_grid.insert(snapshot_index, entity.x, entity.y);
        }
    }

    #[cfg(feature = "benchmarks")]
    pub(crate) fn benchmark_rebuild_population_index(&mut self, world: &Grid) -> usize {
        self.rebuild_population_index(world);
        self.population_cache.len()
    }

    #[cfg(feature = "benchmarks")]
    pub(crate) fn benchmark_spatial_query(&self, x: u32, y: u32, radius: u32) -> usize {
        let mut candidates = 0;
        self.spatial_grid
            .visit_candidates(x, y, radius, |_| candidates += 1);
        candidates
    }

    #[cfg(feature = "benchmarks")]
    pub(crate) fn benchmark_autonomy_pass(&mut self, world: &mut Grid) -> u64 {
        let (consumed, world_changed, consumers) = self.execute_autonomy(world, None, None);
        consumed
            .saturating_add(u64::from(world_changed))
            .saturating_add(consumers.len() as u64)
    }

    #[cfg(feature = "benchmarks")]
    pub(crate) fn benchmark_set_entity_positions(
        &mut self,
        world: &Grid,
        positions: &[(u32, u32)],
    ) -> Result<(), String> {
        if positions.len() != self.entities.len() {
            return Err(format!(
                "received {} positions for {} entities",
                positions.len(),
                self.entities.len()
            ));
        }
        let unique: HashSet<_> = positions.iter().copied().collect();
        if unique.len() != positions.len()
            || positions.iter().any(|&(x, y)| {
                !world
                    .get(x, y)
                    .is_some_and(|tile| tile.terrain.is_walkable())
            })
        {
            return Err("benchmark entity positions must be unique and walkable".to_string());
        }
        for (entity, &(x, y)) in self.entities.iter_mut().zip(positions) {
            entity.x = x;
            entity.y = y;
            entity.path.clear();
            entity.path_index = 0;
            entity.activity = EntityActivity::Idle;
            entity.mind.current_goal = None;
            entity.mind.current_plan.clear();
            entity.mind.plan_index = 0;
            entity.mind.visible_entities.clear();
        }
        self.benchmark_rebuild_population_index(world);
        Ok(())
    }

    #[cfg(feature = "benchmarks")]
    pub(crate) fn benchmark_seed_households(
        &mut self,
        world: &Grid,
        pair_count: usize,
        food_per_household: u16,
    ) -> Result<(), String> {
        households::benchmark_seed_households(
            &mut self.entities,
            &mut self.households,
            &mut self.next_household_id,
            pair_count,
            food_per_household,
            self.tick,
        )?;
        self.benchmark_household_invariants(world)
    }

    #[cfg(feature = "benchmarks")]
    pub(crate) fn benchmark_household_invariants(&self, world: &Grid) -> Result<(), String> {
        let active_ids: HashSet<_> = self
            .households
            .iter()
            .filter(|household| household.is_active())
            .map(|household| household.id)
            .collect();
        if !self
            .households
            .windows(2)
            .all(|pair| pair[0].id < pair[1].id)
            || self
                .households
                .last()
                .is_some_and(|household| self.next_household_id <= household.id)
        {
            return Err("benchmark household IDs are not ordered and monotonic".to_string());
        }
        for household in self
            .households
            .iter()
            .filter(|household| household.is_active())
        {
            if !world
                .get(household.residence_x, household.residence_y)
                .is_some_and(|tile| tile.terrain.is_walkable())
            {
                return Err(format!(
                    "household {} has an invalid residence",
                    household.id
                ));
            }
        }
        for entity in &self.entities {
            if entity
                .household_id
                .is_some_and(|household_id| !active_ids.contains(&household_id))
            {
                return Err(format!(
                    "entity {} has invalid household membership",
                    entity.id
                ));
            }
            if let Some(partner_id) = entity.partner_id {
                let partner = self
                    .entities
                    .binary_search_by_key(&partner_id, |candidate| candidate.id)
                    .ok()
                    .map(|index| &self.entities[index])
                    .ok_or_else(|| format!("entity {} has a missing partner", entity.id))?;
                if partner.partner_id != Some(entity.id)
                    || partner.household_id != entity.household_id
                {
                    return Err(format!(
                        "entity {} has an asymmetric partnership",
                        entity.id
                    ));
                }
            }
        }
        Ok(())
    }

    fn record_resource_changes(&mut self, consumed_this_tick: u64, world_changed: bool) {
        if consumed_this_tick > 0 {
            self.food_consumed = self.food_consumed.saturating_add(consumed_this_tick);
        }
        if world_changed {
            self.world_revision = self.world_revision.saturating_add(1);
        }
    }

    fn remove_dead_entities(&mut self) -> Vec<DeathContext> {
        // A04: regla de causa de muerte extraída a `lifecycle::collect_dead_entities`
        let records = lifecycle::collect_dead_entities(&self.entities);
        let deaths: Vec<_> = records
            .iter()
            .map(|record| {
                (
                    DeathContext {
                        entity_id: record.entity_id,
                        household_id: record.household_id,
                        partner_id: record.partner_id,
                        caregiver_id: record.caregiver_id,
                    },
                    record.position.0,
                    record.position.1,
                    record.cause,
                )
            })
            .collect();
        for (death, x, y, cause) in &deaths {
            self.push_event(PendingSimulationEvent {
                caused_by_event_id: None,
                tick: self.tick,
                location: EventLocation { x: *x, y: *y },
                actor_id: death.entity_id,
                target_id: None,
                related_entity_ids: vec![death.entity_id],
                kind: SimulationEventKind::Death,
                cause: *cause,
                details: SimulationEventDetails::Death,
            });
        }
        let population_before_deaths = self.entities.len();
        self.entities.retain(|entity| entity.health > 0.0);
        partnerships::clear_missing_partners(&mut self.entities);
        self.deaths = self
            .deaths
            .saturating_add((population_before_deaths - self.entities.len()) as u64);
        deaths.into_iter().map(|(death, _, _, _)| death).collect()
    }

    fn update_pregnancies(&mut self, world: &Grid) {
        // A04: lifecycle owns PendingBirth → materialización + evento Birth
        lifecycle::apply_births(self, world);
    }

    /// Daily maintenance: cools relationship affinity toward neutral for
    /// relationships without recent interaction.
    ///
    /// One daily O(N + R) maintenance pass, where N is the population size
    /// and R is the total number of known relationships across all entities.
    /// No population-pair scan, no pathfinding, and no per-tick relationship
    /// work. Runs on the same daily cadence as conceptions.
    fn run_daily_relationship_decay(&mut self) {
        if !self.tick.is_multiple_of(TICKS_PER_DAY) {
            return;
        }
        let mut changes = Vec::new();
        for entity in &mut self.entities {
            let actor_id = entity.id;
            let location = (entity.x, entity.y);
            changes.extend(
                entity
                    .mind
                    .memory
                    .decay_relationships(self.tick)
                    .into_iter()
                    .map(|change| (actor_id, location, change)),
            );
        }
        for (actor_id, location, change) in changes {
            self.record_affinity_change(
                actor_id,
                location,
                change,
                SimulationEventCause::RelationshipDecay,
                None,
            );
        }
        let dissolutions = partnerships::dissolve_unhealthy(&mut self.entities);
        for dissolution in dissolutions {
            let location = self
                .entities
                .binary_search_by_key(&dissolution.actor_id, |entity| entity.id)
                .ok()
                .map(|index| (self.entities[index].x, self.entities[index].y))
                .unwrap_or((0, 0));
            self.record_partnership_dissolution(
                dissolution,
                location,
                SimulationEventCause::RelationshipDecay,
                None,
            );
        }
    }

    fn try_daily_conceptions(&mut self) {
        if !self.tick.is_multiple_of(TICKS_PER_DAY) {
            return;
        }
        try_conceptions(
            &mut self.entities,
            self.tick,
            self.seed,
            MAX_HEALTH,
            DAILY_CONCEPTION_THRESHOLD,
        );
    }
}

#[cfg(test)]
fn food_share_willingness(cooperativeness: f32, affinity: i16) -> bool {
    food_sharing::willingness_for_test(cooperativeness, affinity)
}

#[cfg(test)]
fn relationship_food_share_willingness(
    cooperativeness: f32,
    affinity: i16,
    role: autonomy::CloseRelationshipRole,
) -> bool {
    let affinity_factor = ((f32::from(affinity) + 1_000.0) / 2_000.0).clamp(0.0, 1.0);
    let relationship_bonus = match role {
        autonomy::CloseRelationshipRole::CurrentPartner => 0.20,
        autonomy::CloseRelationshipRole::ParentChild => 0.15,
        autonomy::CloseRelationshipRole::Sibling => 0.10,
        autonomy::CloseRelationshipRole::Other => 0.0,
    };
    cooperativeness * 0.7 + affinity_factor * 0.3 + relationship_bonus >= 0.5
}

#[cfg(test)]
mod tests;
