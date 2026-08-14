mod autonomy;
mod config;
mod entity;
mod events;
mod lifecycle;
mod spatial;
mod time;

use self::autonomy::Mind;
pub(crate) use self::autonomy::{Action, AutonomyProfile, Goal};
use self::config::{
    FOOD_CONSUMED_PER_MEAL, FOOD_SEARCH_THRESHOLD, HUNGER_PER_TICK, HUNGER_REDUCTION_PER_MEAL,
    MAX_HEALTH, MAX_HUNGER, MAX_POPULATION, STARVATION_DAMAGE_PER_TICK,
};
pub use self::entity::{Entity, EntityActivity, LifeStage, Personality, Sex};
use self::events::RecentEventHistory;
pub use self::events::{
    EventLocation, SimulationEvent, SimulationEventCause, SimulationEventDetails,
    SimulationEventKind,
};
use self::lifecycle::{
    founder_age_for, lifespan_for, personality_for, process_due_pregnancies, sex_for,
    spawn_candidates, try_conceptions, DAILY_CONCEPTION_THRESHOLD,
};
use self::spatial::{EntitySnapshot, SpatialGrid};
pub(crate) use self::time::years_from_ticks;
use self::time::TICKS_PER_DAY;
use crate::pathfinding::PathfindingWorkspace;
use crate::world::Grid;
use std::collections::{HashMap, HashSet};
use web_time::Instant;

pub const INITIAL_POPULATION: u32 = 10;

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

#[derive(Clone, Debug, Default)]
pub(crate) struct PhaseProfile {
    pub physiology_us: u64,
    pub population_index_us: u64,
    pub autonomy_us: u64,
    pub starvation_us: u64,
    pub resource_changes_us: u64,
    pub remove_dead_us: u64,
    pub pregnancies_us: u64,
    pub conceptions_us: u64,
    pub total_us: u64,
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
    next_event_id: u64,
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
            next_event_id: 1,
        }
    }
}

impl Simulation {
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

    pub(crate) fn recent_events(&self) -> impl DoubleEndedIterator<Item = &SimulationEvent> {
        self.recent_events.iter()
    }

    pub fn world_revision(&self) -> u64 {
        self.world_revision
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

    pub fn advance(&mut self, ticks: u32, world: &mut Grid) -> u64 {
        if !self.paused {
            for _ in 0..ticks {
                self.step_world(world);
            }
        }
        self.tick
    }

    pub fn step(&mut self, world: &mut Grid) -> u64 {
        self.step_world(world);
        self.tick
    }

    pub(crate) fn profile_step(&mut self, world: &mut Grid) -> PhaseProfile {
        let total_start = Instant::now();

        self.tick = self.tick.saturating_add(1);

        let start = Instant::now();
        self.update_physiology();
        let physiology_us = start.elapsed().as_micros() as u64;

        self.clear_graduated_caregivers();
        self.snap_infants_to_caregivers();

        let start = Instant::now();
        self.rebuild_population_index(world);
        let population_index_us = start.elapsed().as_micros() as u64;

        let start = Instant::now();
        let (consumed_this_tick, consumer_ids, interactions) = self.run_autonomy(world);
        self.record_food_consumptions(&consumer_ids);
        self.record_social_interactions(interactions);
        let autonomy_us = start.elapsed().as_micros() as u64;

        self.snap_infants_to_caregivers();
        for (id, amount) in consumer_ids {
            self.feed_infants_of(id, amount);
        }

        let start = Instant::now();
        self.resolve_starvation();
        let starvation_us = start.elapsed().as_micros() as u64;

        let start = Instant::now();
        self.record_resource_changes(consumed_this_tick);
        let resource_changes_us = start.elapsed().as_micros() as u64;

        let start = Instant::now();
        self.remove_dead_entities();
        let remove_dead_us = start.elapsed().as_micros() as u64;

        let start = Instant::now();
        self.reassign_orphaned_dependents(world);
        self.update_pregnancies(world);
        self.snap_infants_to_caregivers();
        let pregnancies_us = start.elapsed().as_micros() as u64;

        self.run_daily_relationship_decay();

        let start = Instant::now();
        self.try_daily_conceptions();
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

    pub(crate) fn profile_autonomy_step(&mut self, world: &mut Grid) -> AutonomyProfile {
        self.tick = self.tick.saturating_add(1);
        self.update_physiology();
        self.clear_graduated_caregivers();
        self.snap_infants_to_caregivers();
        self.rebuild_population_index(world);

        let tick = self.tick;
        let population_cache = &self.population_cache;
        let spatial_grid = &self.spatial_grid;
        let pathfinding_workspace = &mut self.pathfinding_workspace;

        let (consumed_this_tick, profile, consumer_ids, interactions) = autonomy::profile_autonomy(
            &mut self.entities,
            world,
            tick,
            population_cache,
            spatial_grid,
            pathfinding_workspace,
        );
        self.record_food_consumptions(&consumer_ids);
        self.record_social_interactions(interactions);

        self.snap_infants_to_caregivers();
        for (id, amount) in consumer_ids {
            self.feed_infants_of(id, amount);
        }

        self.resolve_starvation();
        self.record_resource_changes(consumed_this_tick);
        self.remove_dead_entities();
        self.reassign_orphaned_dependents(world);
        self.update_pregnancies(world);
        // Covers both newly reassigned infants
        // and newborns assigned to their mother.
        self.snap_infants_to_caregivers();
        self.run_daily_relationship_decay();
        self.try_daily_conceptions();

        profile
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

    fn push_newborn(&mut self, position: (u32, u32)) -> Option<u32> {
        self.push_entity(position, 0)
    }

    fn push_entity(&mut self, (x, y): (u32, u32), age_ticks: u64) -> Option<u32> {
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
            caregiver_id: None,
            personality: personality_for(self.seed, id),
        });
        Some(id)
    }

    fn step_world(&mut self, world: &mut Grid) {
        self.tick = self.tick.saturating_add(1);
        self.update_physiology();
        self.clear_graduated_caregivers();
        let consumed_this_tick = self.update_autonomy(world);
        self.resolve_starvation();
        self.record_resource_changes(consumed_this_tick);
        self.remove_dead_entities();
        self.reassign_orphaned_dependents(world);
        self.update_pregnancies(world);
        self.snap_infants_to_caregivers();
        self.run_daily_relationship_decay();
        self.try_daily_conceptions();
    }

    fn update_physiology(&mut self) {
        for entity in self
            .entities
            .iter_mut()
            .filter(|entity| entity.health > 0.0)
        {
            entity.age_ticks = entity.age_ticks.saturating_add(1);
            entity.hunger = (entity.hunger + HUNGER_PER_TICK).min(MAX_HUNGER);
            if entity.age_ticks >= entity.lifespan_ticks {
                entity.health = 0.0;
            }
        }
    }

    fn update_autonomy(&mut self, world: &mut Grid) -> u64 {
        self.snap_infants_to_caregivers();
        self.rebuild_population_index(world);
        let (consumed, consumer_ids, interactions) = self.run_autonomy(world);
        self.record_food_consumptions(&consumer_ids);
        self.record_social_interactions(interactions);
        self.snap_infants_to_caregivers();

        for (id, amount) in consumer_ids {
            self.feed_infants_of(id, amount);
        }

        consumed
    }

    fn run_autonomy(
        &mut self,
        world: &mut Grid,
    ) -> (u64, Vec<(u32, u16)>, Vec<autonomy::SocialInteraction>) {
        let tick = self.tick;
        let population_cache = &self.population_cache;
        let spatial_grid = &self.spatial_grid;
        let pathfinding_workspace = &mut self.pathfinding_workspace;

        let mut consumed = 0u64;
        let mut consumer_ids = Vec::new();

        for entity in self.entities.iter_mut().filter(|entity| {
            entity.health > 0.0 && LifeStage::from_age_ticks(entity.age_ticks) != LifeStage::Infant
        }) {
            let result = autonomy::update_entity(
                entity,
                world,
                tick,
                population_cache,
                spatial_grid,
                pathfinding_workspace,
            );
            if result > 0 {
                consumer_ids.push((entity.id, result));
            }
            consumed += u64::from(result);
        }

        let interactions = autonomy::process_social_interactions(
            &mut self.entities,
            &self.population_cache,
            self.tick,
        );

        (consumed, consumer_ids, interactions)
    }

    fn record_social_interactions(&mut self, interactions: Vec<autonomy::SocialInteraction>) {
        for interaction in interactions {
            self.push_event(SimulationEvent {
                id: 0,
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
        }
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
            self.push_event(SimulationEvent {
                id: 0,
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

    fn push_event(&mut self, mut event: SimulationEvent) {
        event.id = self.next_event_id;
        self.next_event_id = self
            .next_event_id
            .checked_add(1)
            .expect("simulation event id space exhausted");
        self.recent_events.push(event);
    }

    fn rebuild_population_index(&mut self, world: &Grid) {
        self.population_cache.clear();
        self.spatial_grid.prepare(world.width, world.height);

        for entity in &self.entities {
            let snapshot_index = self.population_cache.len();

            self.population_cache.push(EntitySnapshot {
                id: entity.id,
                x: entity.x,
                y: entity.y,
            });

            self.spatial_grid.insert(snapshot_index, entity.x, entity.y);
        }
    }

    fn resolve_starvation(&mut self) {
        for entity in self
            .entities
            .iter_mut()
            .filter(|entity| entity.health > 0.0)
        {
            if entity.hunger >= MAX_HUNGER {
                entity.health = (entity.health - STARVATION_DAMAGE_PER_TICK).max(0.0);
                entity.activity = EntityActivity::Starving;
            }
        }
    }

    fn record_resource_changes(&mut self, consumed_this_tick: u64) {
        if consumed_this_tick > 0 {
            self.food_consumed = self.food_consumed.saturating_add(consumed_this_tick);
            self.world_revision = self.world_revision.saturating_add(1);
        }
    }

    fn remove_dead_entities(&mut self) {
        let deaths: Vec<_> = self
            .entities
            .iter()
            .filter(|entity| entity.health <= 0.0)
            .map(|entity| {
                let cause = if entity.age_ticks >= entity.lifespan_ticks {
                    SimulationEventCause::NaturalDeath
                } else {
                    SimulationEventCause::Starvation
                };
                (entity.id, entity.x, entity.y, cause)
            })
            .collect();
        for (entity_id, x, y, cause) in deaths {
            self.push_event(SimulationEvent {
                id: 0,
                tick: self.tick,
                location: EventLocation { x, y },
                actor_id: entity_id,
                target_id: None,
                related_entity_ids: vec![entity_id],
                kind: SimulationEventKind::Death,
                cause,
                details: SimulationEventDetails::Death,
            });
        }
        let population_before_deaths = self.entities.len();
        self.entities.retain(|entity| entity.health > 0.0);
        self.deaths = self
            .deaths
            .saturating_add((population_before_deaths - self.entities.len()) as u64);
    }

    fn update_pregnancies(&mut self, world: &Grid) {
        let capacity = MAX_POPULATION.saturating_sub(self.entities.len());
        let births = process_due_pregnancies(&mut self.entities, world, self.tick, capacity);
        for (position, mother_id) in births {
            if let Some(child_id) = self.push_newborn(position) {
                if let Some(child) = self.entities.last_mut() {
                    child.caregiver_id = Some(mother_id);
                }
                self.births = self.births.saturating_add(1);
                self.push_event(SimulationEvent {
                    id: 0,
                    tick: self.tick,
                    location: EventLocation {
                        x: position.0,
                        y: position.1,
                    },
                    actor_id: mother_id,
                    target_id: None,
                    related_entity_ids: vec![mother_id, child_id],
                    kind: SimulationEventKind::Birth,
                    cause: SimulationEventCause::Born,
                    details: SimulationEventDetails::Birth { child_id },
                });
            }
        }
    }

    fn snap_infants_to_caregivers(&mut self) {
        let positions: HashMap<u32, (u32, u32)> = self
            .entities
            .iter()
            .filter(|entity| entity.health > 0.0)
            .map(|entity| (entity.id, (entity.x, entity.y)))
            .collect();

        for entity in &mut self.entities {
            if entity.health <= 0.0
                || LifeStage::from_age_ticks(entity.age_ticks) != LifeStage::Infant
            {
                continue;
            }
            if let Some(position) = entity
                .caregiver_id
                .and_then(|caregiver_id| positions.get(&caregiver_id).copied())
            {
                entity.x = position.0;
                entity.y = position.1;
            }
        }
    }

    fn feed_infants_of(&mut self, consumer_id: u32, consumed: u16) {
        let meal_fraction = f32::from(consumed) / f32::from(FOOD_CONSUMED_PER_MEAL);
        for entity in &mut self.entities {
            if entity.health > 0.0
                && LifeStage::from_age_ticks(entity.age_ticks) == LifeStage::Infant
                && entity.caregiver_id == Some(consumer_id)
            {
                entity.hunger =
                    (entity.hunger - HUNGER_REDUCTION_PER_MEAL * meal_fraction).max(0.0);
            }
        }
    }

    fn clear_graduated_caregivers(&mut self) {
        for entity in &mut self.entities {
            if entity.health <= 0.0 {
                continue;
            }

            if matches!(
                LifeStage::from_age_ticks(entity.age_ticks),
                LifeStage::Infant | LifeStage::Child
            ) {
                continue;
            }

            if entity.caregiver_id.take().is_some()
                && entity.mind.current_goal == Some(Goal::Follow)
            {
                entity.mind.clear_goal();
                entity.path.clear();
                entity.path_index = 0;
                entity.movement_credit = 0.0;
            }
        }
    }

    fn reassign_orphaned_dependents(&mut self, world: &Grid) {
        let alive: HashSet<u32> = self.entities.iter().map(|entity| entity.id).collect();
        let needs_reassignment: Vec<usize> = self
            .entities
            .iter()
            .enumerate()
            .filter(|(_, entity)| {
                matches!(
                    LifeStage::from_age_ticks(entity.age_ticks),
                    LifeStage::Infant | LifeStage::Child
                ) && entity
                    .caregiver_id
                    .is_none_or(|caregiver_id| !alive.contains(&caregiver_id))
            })
            .map(|(index, _)| index)
            .collect();

        for index in needs_reassignment {
            let position = (self.entities[index].x, self.entities[index].y);
            let new_caregiver = self.find_nearest_caregiver(position, world);

            if self.entities[index].caregiver_id != new_caregiver {
                let entity = &mut self.entities[index];
                entity.caregiver_id = new_caregiver;
                entity.mind.clear_goal();
                entity.path.clear();
                entity.path_index = 0;
                entity.movement_credit = 0.0;
            }
        }
    }

    fn find_nearest_caregiver(&self, position: (u32, u32), world: &Grid) -> Option<u32> {
        let dependent_region = world.region_id_at(position.0, position.1);

        self.entities
            .iter()
            .filter(|entity| {
                entity.health > 0.0
                    && caregiver_priority(LifeStage::from_age_ticks(entity.age_ticks)).is_some()
            })
            .filter(|entity| {
                let caregiver_region = world.region_id_at(entity.x, entity.y);
                match (dependent_region, caregiver_region) {
                    (Some(left), Some(right)) => left == right,
                    (None, None) => true,
                    _ => false,
                }
            })
            .min_by_key(|entity| {
                (
                    caregiver_priority(LifeStage::from_age_ticks(entity.age_ticks)).unwrap(),
                    entity.x.abs_diff(position.0) + entity.y.abs_diff(position.1),
                    entity.id,
                )
            })
            .map(|entity| entity.id)
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
        for entity in &mut self.entities {
            entity.mind.memory.decay_relationships(self.tick);
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

fn caregiver_priority(stage: LifeStage) -> Option<u8> {
    match stage {
        LifeStage::Adult => Some(0),
        LifeStage::Elder => Some(1),
        _ => None,
    }
}

#[cfg(test)]
mod tests;
