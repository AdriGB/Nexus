use crate::pathfinding;
use crate::world::{Grid, ResourceKind};
use std::collections::HashSet;

pub const INITIAL_POPULATION: u32 = 10;
const MAX_POPULATION: usize = 10_000;
const MAX_HUNGER: f32 = 100.0;
const MAX_HEALTH: f32 = 100.0;
const HUNGER_PER_TICK: f32 = 1.0;
const FOOD_SEARCH_THRESHOLD: f32 = 60.0;
const FOOD_SEARCH_RADIUS: u32 = 30;
const FOOD_CONSUMED_PER_MEAL: u16 = 10;
const HUNGER_REDUCTION_PER_MEAL: f32 = 50.0;
const STARVATION_DAMAGE_PER_TICK: f32 = 2.0;
const ADULT_AGE_TICKS: u64 = 200;
const REPRODUCTION_COOLDOWN_TICKS: u32 = 300;
const REPRODUCTION_MAX_HUNGER: f32 = 35.0;
const REPRODUCTION_MAX_DISTANCE: u32 = 2;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[repr(u32)]
pub enum EntityActivity {
    Idle = 0,
    SeekingFood = 1,
    Moving = 2,
    Starving = 3,
}

impl EntityActivity {
    pub fn label(self) -> &'static str {
        match self {
            Self::Idle => "Idle",
            Self::SeekingFood => "Seeking food",
            Self::Moving => "Moving",
            Self::Starving => "Starving",
        }
    }
}

#[derive(Clone, Debug)]
pub struct Entity {
    pub id: u32,
    pub x: u32,
    pub y: u32,
    pub hunger: f32,
    pub health: f32,
    pub age_ticks: u64,
    pub path: Vec<(u32, u32)>,
    pub path_index: usize,
    pub activity: EntityActivity,
    reproduction_cooldown: u32,
}

impl Entity {
    pub fn remaining_path_len(&self) -> usize {
        self.path.len().saturating_sub(self.path_index)
    }
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
            reproduction_cooldown,
        });
        Some(id)
    }

    fn step_world(&mut self, world: &mut Grid) {
        self.tick = self.tick.saturating_add(1);
        let mut consumed_this_tick = 0u64;

        for entity in &mut self.entities {
            entity.age_ticks = entity.age_ticks.saturating_add(1);
            entity.reproduction_cooldown = entity.reproduction_cooldown.saturating_sub(1);
            entity.hunger = (entity.hunger + HUNGER_PER_TICK).min(MAX_HUNGER);

            let mut consumed = 0;
            if entity.path_index < entity.path.len() {
                let next = entity.path[entity.path_index];
                entity.x = next.0;
                entity.y = next.1;
                entity.path_index += 1;
                entity.activity = EntityActivity::Moving;

                if entity.path_index == entity.path.len() {
                    entity.path.clear();
                    entity.path_index = 0;
                    entity.activity = EntityActivity::Idle;
                    consumed = consume_food(entity, world);
                }
            } else if entity.hunger >= FOOD_SEARCH_THRESHOLD {
                consumed = consume_food(entity, world);
                if consumed == 0 {
                    if let Some(path) = find_path_to_food(world, (entity.x, entity.y)) {
                        entity.path = path.into_iter().skip(1).collect();
                        entity.path_index = 0;
                        entity.activity = if entity.path.is_empty() {
                            EntityActivity::Idle
                        } else {
                            EntityActivity::SeekingFood
                        };
                    } else {
                        entity.activity = EntityActivity::Idle;
                    }
                }
            }

            consumed_this_tick += u64::from(consumed);
            if entity.hunger >= MAX_HUNGER {
                entity.health = (entity.health - STARVATION_DAMAGE_PER_TICK).max(0.0);
                entity.activity = EntityActivity::Starving;
            }
        }

        if consumed_this_tick > 0 {
            self.food_consumed = self.food_consumed.saturating_add(consumed_this_tick);
            self.world_revision = self.world_revision.saturating_add(1);
        }

        let population_before_deaths = self.entities.len();
        self.entities.retain(|entity| entity.health > 0.0);
        self.deaths = self
            .deaths
            .saturating_add((population_before_deaths - self.entities.len()) as u64);

        self.reproduce(world);
    }

    fn reproduce(&mut self, world: &Grid) {
        if self.entities.len() < 2 || self.entities.len() >= MAX_POPULATION {
            return;
        }

        let mut paired = vec![false; self.entities.len()];
        let mut occupied: HashSet<_> = self
            .entities
            .iter()
            .map(|entity| (entity.x, entity.y))
            .collect();
        let mut plans = Vec::new();

        for left in 0..self.entities.len() {
            if paired[left] || !can_reproduce(&self.entities[left]) {
                continue;
            }
            for right in (left + 1)..self.entities.len() {
                if paired[right]
                    || !can_reproduce(&self.entities[right])
                    || manhattan(
                        (self.entities[left].x, self.entities[left].y),
                        (self.entities[right].x, self.entities[right].y),
                    ) > REPRODUCTION_MAX_DISTANCE
                {
                    continue;
                }
                let parent_position = (self.entities[left].x, self.entities[left].y);
                if let Some(position) = adjacent_birth_position(world, parent_position, &occupied) {
                    occupied.insert(position);
                    paired[left] = true;
                    paired[right] = true;
                    plans.push((left, right, position));
                }
                break;
            }
        }

        for &(left, right, _) in &plans {
            self.entities[left].reproduction_cooldown = REPRODUCTION_COOLDOWN_TICKS;
            self.entities[right].reproduction_cooldown = REPRODUCTION_COOLDOWN_TICKS;
        }
        for (_, _, position) in plans {
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

fn can_reproduce(entity: &Entity) -> bool {
    entity.age_ticks >= ADULT_AGE_TICKS
        && entity.reproduction_cooldown == 0
        && entity.hunger <= REPRODUCTION_MAX_HUNGER
        && entity.health >= MAX_HEALTH * 0.8
}

fn adjacent_birth_position(
    world: &Grid,
    origin: (u32, u32),
    occupied: &HashSet<(u32, u32)>,
) -> Option<(u32, u32)> {
    (-1..=1)
        .flat_map(|dy| (-1..=1).map(move |dx| (dx, dy)))
        .filter(|&(dx, dy)| dx != 0 || dy != 0)
        .filter_map(|(dx, dy)| {
            let x = i64::from(origin.0) + i64::from(dx);
            let y = i64::from(origin.1) + i64::from(dy);
            (x >= 0 && y >= 0 && x < i64::from(world.width) && y < i64::from(world.height))
                .then_some((x as u32, y as u32))
        })
        .find(|&(x, y)| {
            !occupied.contains(&(x, y))
                && world
                    .get(x, y)
                    .is_some_and(|tile| tile.terrain.is_walkable())
        })
}

fn spawn_candidates(world: &Grid) -> Vec<(u32, u32)> {
    let center = (world.width / 2, world.height / 2);
    let mut food_tiles: Vec<_> = world
        .resources
        .iter()
        .enumerate()
        .filter_map(|(index, deposit)| {
            let deposit = deposit.as_ref()?;
            (deposit.kind == ResourceKind::Food && deposit.amount > 0).then(|| {
                let index = index as u32;
                (index % world.width, index / world.width)
            })
        })
        .collect();
    food_tiles.sort_unstable_by_key(|&coordinate| manhattan(coordinate, center));

    let mut seen = HashSet::new();
    let mut candidates = Vec::new();
    for food in food_tiles {
        for radius in 6..=12i32 {
            for dx in -radius..=radius {
                let dy = radius - dx.abs();
                for signed_dy in [dy, -dy] {
                    let x = i64::from(food.0) + i64::from(dx);
                    let y = i64::from(food.1) + i64::from(signed_dy);
                    if x < 0 || y < 0 || x >= i64::from(world.width) || y >= i64::from(world.height)
                    {
                        continue;
                    }
                    let candidate = (x as u32, y as u32);
                    if seen.insert(candidate)
                        && world
                            .get(candidate.0, candidate.1)
                            .is_some_and(|tile| tile.terrain.is_walkable())
                    {
                        candidates.push(candidate);
                    }
                }
            }
        }
    }

    let mut fallback: Vec<_> = world
        .tiles
        .iter()
        .enumerate()
        .filter(|(_, tile)| tile.terrain.is_walkable())
        .map(|(index, _)| {
            let index = index as u32;
            (index % world.width, index / world.width)
        })
        .collect();
    fallback.sort_unstable_by_key(|&coordinate| manhattan(coordinate, center));
    candidates.extend(
        fallback
            .into_iter()
            .filter(|position| seen.insert(*position)),
    );
    candidates
}

fn find_path_to_food(world: &Grid, origin: (u32, u32)) -> Option<Vec<(u32, u32)>> {
    let mut candidates: Vec<_> = world
        .resources
        .iter()
        .enumerate()
        .filter_map(|(index, deposit)| {
            let deposit = deposit.as_ref()?;
            if deposit.kind != ResourceKind::Food || deposit.amount == 0 {
                return None;
            }
            let index = index as u32;
            let coordinate = (index % world.width, index / world.width);
            let distance = manhattan(origin, coordinate);
            (distance <= FOOD_SEARCH_RADIUS).then_some((distance, coordinate))
        })
        .collect();
    candidates.sort_unstable_by_key(|candidate| *candidate);
    candidates
        .into_iter()
        .find_map(|(_, destination)| pathfinding::find_path(world, origin, destination))
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

fn manhattan(left: (u32, u32), right: (u32, u32)) -> u32 {
    left.0.abs_diff(right.0) + left.1.abs_diff(right.1)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::world::{ResourceDeposit, Terrain, Tile};

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
        simulation.step(&mut world);
        assert_eq!(simulation.entities()[0].path, original_path);
        assert_eq!(simulation.entities()[0].path_index, 1);
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
