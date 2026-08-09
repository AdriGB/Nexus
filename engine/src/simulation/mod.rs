mod autonomy;
mod config;
mod entity;
mod lifecycle;
mod spatial;
mod time;

use self::autonomy::Mind;
pub(crate) use self::autonomy::{Action, Goal};
use self::config::{
    FOOD_SEARCH_THRESHOLD, HUNGER_PER_TICK, MAX_HEALTH, MAX_HUNGER, MAX_POPULATION,
    STARVATION_DAMAGE_PER_TICK,
};
pub use self::entity::{Entity, EntityActivity, Sex};
use self::lifecycle::{
    founder_age_for, lifespan_for, process_due_pregnancies, sex_for, spawn_candidates,
    try_conceptions, DAILY_CONCEPTION_THRESHOLD,
};
use self::spatial::{EntitySnapshot, SpatialGrid};
pub(crate) use self::time::years_from_ticks;
use self::time::TICKS_PER_DAY;
use crate::pathfinding::PathfindingWorkspace;
use crate::world::Grid;
use std::collections::HashSet;
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

        let start = Instant::now();
        self.rebuild_population_index(world);
        let population_index_us = start.elapsed().as_micros() as u64;

        let start = Instant::now();
        let consumed_this_tick = self.run_autonomy(world);
        let autonomy_us = start.elapsed().as_micros() as u64;

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
        self.update_pregnancies(world);
        let pregnancies_us = start.elapsed().as_micros() as u64;

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
        });
        Some(id)
    }

    fn step_world(&mut self, world: &mut Grid) {
        // Tick order is intentional:
        // 1. advance time and physiology
        // 2. perceive, decide, and act
        // 3. resolve immediate consequences
        // 4. remove dead entities
        // 5. resolve births
        // 6. attempt scheduled conceptions
        self.tick = self.tick.saturating_add(1);
        self.update_physiology();
        let consumed_this_tick = self.update_autonomy(world);
        self.resolve_starvation();
        self.record_resource_changes(consumed_this_tick);
        self.remove_dead_entities();
        self.update_pregnancies(world);
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
        self.rebuild_population_index(world);
        self.run_autonomy(world)
    }

    fn run_autonomy(&mut self, world: &mut Grid) -> u64 {
        let tick = self.tick;
        let population_cache = &self.population_cache;
        let spatial_grid = &self.spatial_grid;
        let pathfinding_workspace = &mut self.pathfinding_workspace;

        let mut consumed = 0u64;

        for entity in self
            .entities
            .iter_mut()
            .filter(|entity| entity.health > 0.0)
        {
            consumed += u64::from(autonomy::update_entity(
                entity,
                world,
                tick,
                population_cache,
                spatial_grid,
                pathfinding_workspace,
            ));
        }

        consumed
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
        let population_before_deaths = self.entities.len();
        self.entities.retain(|entity| entity.health > 0.0);
        self.deaths = self
            .deaths
            .saturating_add((population_before_deaths - self.entities.len()) as u64);
    }

    fn update_pregnancies(&mut self, world: &Grid) {
        let capacity = MAX_POPULATION.saturating_sub(self.entities.len());
        let positions = process_due_pregnancies(&mut self.entities, world, self.tick, capacity);
        for position in positions {
            if self.push_newborn(position).is_some() {
                self.births = self.births.saturating_add(1);
            }
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
mod tests;
