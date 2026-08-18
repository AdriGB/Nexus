mod autonomy;
mod config;
mod dependents;
mod entity;
mod events;
mod lifecycle;
mod physiology;
mod pipeline;
mod spatial;
mod time;

use self::autonomy::Mind;
pub(crate) use self::autonomy::{Action, AutonomyProfile, Goal};
use self::config::{FOOD_SEARCH_THRESHOLD, MAX_HEALTH, MAX_POPULATION};
pub use self::entity::{Entity, EntityActivity, LifeStage, Personality, Sex};
pub(crate) use self::events::EntityEventSummary;
#[cfg(test)]
pub(crate) use self::events::EventId;
pub use self::events::{
    EventLocation, SimulationEvent, SimulationEventCause, SimulationEventDetails,
    SimulationEventKind,
};
use self::events::{PendingSimulationEvent, RecentEventHistory};
use self::lifecycle::{
    founder_age_for, lifespan_for, personality_for, process_due_pregnancies, sex_for,
    spawn_candidates, try_conceptions, DAILY_CONCEPTION_THRESHOLD,
};
use self::spatial::{EntitySnapshot, SpatialGrid};
pub(crate) use self::time::years_from_ticks;
use self::time::TICKS_PER_DAY;
use crate::pathfinding::PathfindingWorkspace;
use crate::world::Grid;
use std::collections::HashSet;

pub const INITIAL_POPULATION: u32 = 10;

type AutonomyRunResult = (
    u64,
    Vec<(u32, u16)>,
    Vec<autonomy::ResourceDiscovery>,
    Vec<autonomy::EntityEncounter>,
    Vec<autonomy::SocialInteraction>,
);

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

    pub(crate) fn entity_event_summary(&self, entity_id: u32) -> EntityEventSummary {
        self.recent_events.summary_for(entity_id)
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

    fn update_autonomy(&mut self, world: &mut Grid) -> u64 {
        dependents::snap_infants_to_caregivers(&mut self.entities);
        self.rebuild_population_index(world);
        let (consumed, consumer_ids, discoveries, encounters, interactions) =
            self.run_autonomy(world);
        self.record_resource_discoveries(discoveries);
        self.record_entity_encounters(encounters);
        self.record_food_consumptions(&consumer_ids);
        self.record_social_interactions(interactions);
        dependents::snap_infants_to_caregivers(&mut self.entities);

        for (id, amount) in consumer_ids {
            dependents::feed_infants_of(&mut self.entities, id, amount);
        }

        consumed
    }

    fn run_autonomy(&mut self, world: &mut Grid) -> AutonomyRunResult {
        let tick = self.tick;
        let population_cache = &self.population_cache;
        let spatial_grid = &self.spatial_grid;
        let pathfinding_workspace = &mut self.pathfinding_workspace;

        let mut consumed = 0u64;
        let mut consumer_ids = Vec::new();
        let mut discoveries = Vec::new();
        let mut encounters = Vec::new();

        for entity in self.entities.iter_mut().filter(|entity| {
            entity.health > 0.0 && LifeStage::from_age_ticks(entity.age_ticks) != LifeStage::Infant
        }) {
            let (result, entity_discoveries, entity_encounters) = autonomy::update_entity(
                entity,
                world,
                tick,
                population_cache,
                spatial_grid,
                pathfinding_workspace,
            );
            discoveries.extend(entity_discoveries);
            encounters.extend(entity_encounters);
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

        (
            consumed,
            consumer_ids,
            discoveries,
            encounters,
            interactions,
        )
    }

    fn record_social_interactions(&mut self, interactions: Vec<autonomy::SocialInteraction>) {
        for interaction in interactions {
            self.push_event(PendingSimulationEvent {
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
                );
            }
            if let Some(change) = interaction.target_affinity_change {
                self.record_affinity_change(
                    interaction.target_id,
                    interaction.target_location,
                    change,
                    SimulationEventCause::MutualSocialContact,
                );
            }
        }
    }

    fn record_affinity_change(
        &mut self,
        actor_id: u32,
        location: (u32, u32),
        change: autonomy::AffinityChangeRecord,
        cause: SimulationEventCause,
    ) {
        self.push_event(PendingSimulationEvent {
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

    fn push_event(&mut self, event: PendingSimulationEvent) {
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
            self.push_event(PendingSimulationEvent {
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
                self.push_event(PendingSimulationEvent {
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
mod tests;
