mod autonomy;
mod entity;
mod lifecycle;

use self::autonomy::Mind;
pub(crate) use self::autonomy::{Action, Goal};
pub use self::entity::{Entity, EntityActivity};
use self::lifecycle::{reproduction_positions, spawn_candidates, REPRODUCTION_COOLDOWN_TICKS};
use crate::world::Grid;
use std::collections::HashSet;

pub const INITIAL_POPULATION: u32 = 10;
const MAX_POPULATION: usize = 10_000;
const MAX_HUNGER: f32 = 100.0;
const MAX_HEALTH: f32 = 100.0;
const HUNGER_PER_TICK: f32 = 1.0;
const FOOD_SEARCH_THRESHOLD: f32 = 60.0;
const STARVATION_DAMAGE_PER_TICK: f32 = 2.0;

#[derive(Clone, Copy, Debug)]
pub struct PopulationStats {
    pub population: u32,
    pub births: u64,
    pub deaths: u64,
    pub hungry: u32,
    pub seeking_food: u32,
    pub average_hunger: f32,
    pub food_consumed: u64,
}

#[derive(Clone, Debug)]
pub struct Simulation {
    tick: u64,
    paused: bool,
    entities: Vec<Entity>,
    next_entity_id: u32,
    world_revision: u64,
    births: u64,
    deaths: u64,
    food_consumed: u64,
}

impl Default for Simulation {
    fn default() -> Self {
        Self {
            tick: 0,
            paused: true,
            entities: Vec::new(),
            next_entity_id: 1,
            world_revision: 0,
            births: 0,
            deaths: 0,
            food_consumed: 0,
        }
    }
}

impl Simulation {
    pub fn with_population(world: &Grid, count: u32) -> Self {
        let mut simulation = Self::default();
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
            if self.push_entity(position, 0.0, 0).is_some() {
                spawned += 1;
            }
        }
        spawned
    }

    fn push_entity(
        &mut self,
        (x, y): (u32, u32),
        hunger: f32,
        reproduction_cooldown: u32,
    ) -> Option<u32> {
        let id = self.next_entity_id;
        self.next_entity_id = self.next_entity_id.checked_add(1)?;
        self.entities.push(Entity {
            id,
            x,
            y,
            hunger,
            health: MAX_HEALTH,
            age_ticks: 0,
            path: Vec::new(),
            path_index: 0,
            activity: EntityActivity::Idle,
            mind: Mind::default(),
            reproduction_cooldown,
        });
        Some(id)
    }

    fn step_world(&mut self, world: &mut Grid) {
        self.tick = self.tick.saturating_add(1);
        self.update_physiology();
        let consumed_this_tick = self.update_autonomy(world);
        self.resolve_starvation();
        self.record_resource_changes(consumed_this_tick);
        self.remove_dead_entities();
        self.reproduce(world);
    }

    fn update_physiology(&mut self) {
        for entity in self
            .entities
            .iter_mut()
            .filter(|entity| entity.health > 0.0)
        {
            entity.age_ticks = entity.age_ticks.saturating_add(1);
            entity.reproduction_cooldown = entity.reproduction_cooldown.saturating_sub(1);
            entity.hunger = (entity.hunger + HUNGER_PER_TICK).min(MAX_HUNGER);
        }
    }

    fn update_autonomy(&mut self, world: &mut Grid) -> u64 {
        let population_snapshot: Vec<_> = self
            .entities
            .iter()
            .map(|entity| (entity.id, (entity.x, entity.y)))
            .collect();
        self.entities
            .iter_mut()
            .filter(|entity| entity.health > 0.0)
            .map(|entity| {
                u64::from(autonomy::update_entity(
                    entity,
                    world,
                    self.tick,
                    &population_snapshot,
                ))
            })
            .sum()
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

    fn reproduce(&mut self, world: &Grid) {
        if self.entities.len() >= MAX_POPULATION {
            return;
        }
        for position in reproduction_positions(&mut self.entities, world, MAX_HEALTH) {
            if self.entities.len() >= MAX_POPULATION {
                break;
            }
            if self
                .push_entity(position, 0.0, REPRODUCTION_COOLDOWN_TICKS)
                .is_some()
            {
                self.births = self.births.saturating_add(1);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::autonomy::URGENT_HUNGER_THRESHOLD;
    use super::lifecycle::ADULT_AGE_TICKS;
    use super::*;
    use crate::world::{ResourceDeposit, ResourceKind, Terrain, Tile};

    fn grid_from_rows(rows: &[&str]) -> Grid {
        let height = rows.len() as u32;
        let width = rows.first().map_or(0, |row| row.len()) as u32;
        let tiles = rows
            .iter()
            .flat_map(|row| row.chars())
            .map(|symbol| Tile {
                terrain: match symbol {
                    'P' | 'F' => Terrain::Plains,
                    'M' => Terrain::Mountain,
                    '#' => Terrain::DeepWater,
                    _ => panic!("unknown terrain symbol: {symbol}"),
                },
                altitude: 0.0,
                moisture: 0.5,
                temperature: 0.5,
            })
            .collect();
        let resources = rows
            .iter()
            .flat_map(|row| row.chars())
            .map(|symbol| {
                (symbol == 'F').then_some(ResourceDeposit {
                    kind: ResourceKind::Food,
                    amount: 20,
                })
            })
            .collect();
        Grid {
            width,
            height,
            tiles,
            region_ids: Vec::new(),
            regions: Vec::new(),
            resources,
        }
    }

    fn plain_grid(width: u32, height: u32) -> Grid {
        let row = "P".repeat(width as usize);
        let rows: Vec<_> = (0..height).map(|_| row.as_str()).collect();
        grid_from_rows(&rows)
    }

    fn entity(id: u32, x: u32, y: u32, hunger: f32) -> Entity {
        Entity {
            id,
            x,
            y,
            hunger,
            health: MAX_HEALTH,
            age_ticks: 0,
            path: Vec::new(),
            path_index: 0,
            activity: EntityActivity::Idle,
            mind: Mind::default(),
            reproduction_cooldown: 0,
        }
    }

    fn simulation_with_entity(x: u32, y: u32, hunger: f32) -> Simulation {
        Simulation {
            entities: vec![entity(1, x, y, hunger)],
            next_entity_id: 2,
            ..Simulation::default()
        }
    }

    #[test]
    fn simulation_starts_paused_at_tick_zero() {
        let simulation = Simulation::default();
        assert_eq!(simulation.tick(), 0);
        assert!(simulation.is_paused());
    }

    #[test]
    fn spawns_multiple_entities_with_unique_ids_and_positions() {
        let world = plain_grid(10, 10);
        let simulation = Simulation::with_population(&world, 10);
        let ids: HashSet<_> = simulation
            .entities()
            .iter()
            .map(|entity| entity.id)
            .collect();
        let positions: HashSet<_> = simulation
            .entities()
            .iter()
            .map(|entity| (entity.x, entity.y))
            .collect();
        assert_eq!(ids.len(), 10);
        assert_eq!(positions.len(), 10);
    }

    #[test]
    fn paused_simulation_does_not_change_entities() {
        let mut world = grid_from_rows(&["PF"]);
        let mut simulation = simulation_with_entity(0, 0, 59.0);
        simulation.advance(10, &mut world);
        assert_eq!(simulation.tick(), 0);
        assert_eq!(simulation.entities()[0].hunger, 59.0);
    }

    #[test]
    fn entity_stores_and_follows_unsmoothed_path() {
        let mut world = grid_from_rows(&["PPPPP", "P###F", "PPPPP"]);
        let mut simulation = simulation_with_entity(0, 1, 59.0);
        simulation.step(&mut world);
        let original_path = simulation.entities()[0].path.clone();
        assert!(original_path.len() > 2);
        assert_eq!(simulation.entities()[0].path_index, 1);
        simulation.step(&mut world);
        assert_eq!(simulation.entities()[0].path, original_path);
        assert_eq!(simulation.entities()[0].path_index, 2);
    }

    #[test]
    fn competing_entities_consume_a_finite_deposit_once() {
        let mut world = grid_from_rows(&["F"]);
        world.resources[0].as_mut().unwrap().amount = 10;
        let mut simulation = Simulation {
            entities: vec![entity(1, 0, 0, 60.0), entity(2, 0, 0, 60.0)],
            next_entity_id: 3,
            ..Simulation::default()
        };
        simulation.step(&mut world);

        assert!(world.resources[0].is_none());
        assert_eq!(simulation.food_consumed, 10);
        assert!(simulation.entities()[0].hunger < simulation.entities()[1].hunger);
        assert_eq!(simulation.world_revision(), 1);
    }

    #[test]
    fn starving_entity_loses_health_and_dies() {
        let mut world = grid_from_rows(&["P"]);
        let mut starving = entity(1, 0, 0, MAX_HUNGER);
        starving.health = STARVATION_DAMAGE_PER_TICK;
        let mut simulation = Simulation {
            entities: vec![starving],
            next_entity_id: 2,
            ..Simulation::default()
        };
        simulation.step(&mut world);
        assert!(simulation.entities().is_empty());
        assert_eq!(simulation.population_stats().deaths, 1);
    }

    #[test]
    fn entity_ids_are_never_reused_after_death() {
        let mut world = plain_grid(3, 1);
        let mut simulation = Simulation::with_population(&world, 2);
        simulation.entities[0].health = 0.0;
        simulation.step(&mut world);
        assert_eq!(simulation.spawn_entities(&world, 1), 1);
        let ids: Vec<_> = simulation
            .entities()
            .iter()
            .map(|entity| entity.id)
            .collect();
        assert_eq!(ids, vec![2, 3]);
    }

    #[test]
    fn age_increases_once_per_tick() {
        let mut world = grid_from_rows(&["P"]);
        let mut simulation = simulation_with_entity(0, 0, 0.0);
        simulation.step(&mut world);
        assert_eq!(simulation.entities()[0].age_ticks, 1);
    }

    #[test]
    fn eligible_nearby_adults_reproduce_with_cooldown() {
        let mut world = plain_grid(4, 4);
        let mut left = entity(1, 1, 1, 0.0);
        let mut right = entity(2, 2, 1, 0.0);
        left.age_ticks = ADULT_AGE_TICKS;
        right.age_ticks = ADULT_AGE_TICKS;
        let mut simulation = Simulation {
            entities: vec![left, right],
            next_entity_id: 3,
            ..Simulation::default()
        };
        simulation.step(&mut world);
        assert_eq!(simulation.entities().len(), 3);
        assert_eq!(simulation.population_stats().births, 1);
        simulation.step(&mut world);
        assert_eq!(simulation.entities().len(), 3);
    }

    #[test]
    fn population_stats_report_pressure_and_consumption() {
        let mut world = grid_from_rows(&["F"]);
        let mut simulation = simulation_with_entity(0, 0, 60.0);
        simulation.step(&mut world);
        let stats = simulation.population_stats();
        assert_eq!(stats.population, 1);
        assert_eq!(stats.food_consumed, 10);
        assert!(stats.average_hunger < FOOD_SEARCH_THRESHOLD);
        assert_eq!(
            simulation.entities()[0].mind.memory.known_resources[0].estimated_amount,
            10
        );
    }

    #[test]
    fn distant_food_is_not_known_without_perception() {
        let mut world = grid_from_rows(&["PPPPPPPPPPPPPPPPPPPF"]);
        let mut simulation = simulation_with_entity(0, 0, 90.0);
        simulation.step(&mut world);

        let entity = &simulation.entities()[0];
        assert!(entity.mind.memory.known_resources.is_empty());
        assert_eq!(entity.mind.current_goal, Some(Goal::Explore));
    }

    #[test]
    fn entity_remembers_seen_food_and_interrupts_exploration_when_hungry() {
        let mut world = grid_from_rows(&["PPPPPFPPPPPPPPPP"]);
        let mut simulation = simulation_with_entity(0, 0, 0.0);
        simulation.step(&mut world);
        assert_eq!(
            simulation.entities()[0].mind.memory.known_resources.len(),
            1
        );
        assert_eq!(
            simulation.entities()[0].mind.current_goal,
            Some(Goal::Explore)
        );

        simulation.entities[0].hunger = URGENT_HUNGER_THRESHOLD;
        simulation.step(&mut world);
        assert_eq!(simulation.entities()[0].mind.current_goal, Some(Goal::Eat));
        assert!(simulation.entities()[0]
            .mind
            .current_plan
            .iter()
            .any(|action| matches!(action, Action::Consume(ResourceKind::Food))));
    }

    #[test]
    fn exploration_goal_is_retained_while_its_plan_is_viable() {
        let mut world = plain_grid(32, 8);
        let mut simulation = simulation_with_entity(0, 0, 0.0);
        simulation.step(&mut world);
        let goal_since = simulation.entities()[0].mind.goal_since_tick;
        assert_eq!(
            simulation.entities()[0].mind.current_goal,
            Some(Goal::Explore)
        );

        simulation.step(&mut world);
        assert_eq!(
            simulation.entities()[0].mind.current_goal,
            Some(Goal::Explore)
        );
        assert_eq!(simulation.entities()[0].mind.goal_since_tick, goal_since);
    }

    #[test]
    fn stale_resource_memory_is_forgotten() {
        let mut world = grid_from_rows(&["FPPPPPPPPPPPPPPPPPPP"]);
        let mut observer = entity(1, 0, 0, 0.0);
        autonomy::perceive(&mut observer.mind, &world, (0, 0), 0);
        assert_eq!(observer.mind.memory.known_resources.len(), 1);

        autonomy::perceive(&mut observer.mind, &world, (19, 0), 3_000);
        assert!(observer.mind.memory.known_resources.is_empty());
        world.resources[0] = None;
    }

    #[test]
    fn unreachable_food_is_temporarily_avoided() {
        let mut world = grid_from_rows(&["P#F"]);
        let mut simulation = simulation_with_entity(0, 0, 90.0);
        simulation.step(&mut world);

        let remembered = &simulation.entities()[0].mind.memory.known_resources[0];
        assert_eq!(remembered.failed_attempts, 1);
        assert!(remembered.avoid_until_tick > simulation.tick());
        assert_ne!(simulation.entities()[0].mind.current_goal, Some(Goal::Eat));
    }

    #[test]
    fn false_food_memory_is_corrected_when_the_target_becomes_visible() {
        let mut world = plain_grid(6, 1);
        let mut observer = entity(1, 0, 0, 90.0);
        observer.mind.perception_radius = 1;
        observer
            .mind
            .memory
            .known_resources
            .push(autonomy::KnownResource {
                x: 4,
                y: 0,
                kind: ResourceKind::Food,
                last_seen_tick: 0,
                estimated_amount: 20,
                failed_attempts: 0,
                avoid_until_tick: 0,
            });
        let mut simulation = Simulation {
            entities: vec![observer],
            next_entity_id: 2,
            ..Simulation::default()
        };

        for _ in 0..4 {
            simulation.step(&mut world);
        }
        assert!(simulation.entities()[0]
            .mind
            .memory
            .known_resources
            .is_empty());
        assert_ne!(simulation.entities()[0].mind.current_goal, Some(Goal::Eat));
        assert_eq!(simulation.food_consumed, 0);
    }

    #[test]
    fn local_perception_reports_only_nearby_entities() {
        let mut world = plain_grid(20, 1);
        let mut simulation = Simulation {
            entities: vec![
                entity(1, 0, 0, 0.0),
                entity(2, 3, 0, 0.0),
                entity(3, 15, 0, 0.0),
            ],
            next_entity_id: 4,
            ..Simulation::default()
        };
        simulation.step(&mut world);
        assert_eq!(simulation.entities()[0].mind.visible_entities, vec![2]);
    }

    #[test]
    fn handles_10_100_and_1000_entity_populations() {
        for count in [10, 100, 1_000] {
            let mut world = plain_grid(40, 25);
            let mut simulation = Simulation::with_population(&world, count);
            assert_eq!(simulation.entities().len(), count as usize);
            simulation.resume();
            simulation.advance(10, &mut world);
            assert_eq!(simulation.entities().len(), count as usize);
            assert_eq!(simulation.tick(), 10);
        }
    }
}
