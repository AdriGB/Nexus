use crate::pathfinding;
use crate::world::{Grid, ResourceKind};

const MAX_HUNGER: f32 = 100.0;
const HUNGER_PER_TICK: f32 = 1.0;
const FOOD_SEARCH_THRESHOLD: f32 = 60.0;
const FOOD_SEARCH_RADIUS: u32 = 30;
const FOOD_CONSUMED_PER_MEAL: u16 = 10;
const HUNGER_REDUCTION_PER_MEAL: f32 = 50.0;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[repr(u32)]
pub enum EntityActivity {
    Idle = 0,
    SeekingFood = 1,
    Moving = 2,
}

impl EntityActivity {
    pub fn label(self) -> &'static str {
        match self {
            Self::Idle => "Idle",
            Self::SeekingFood => "Seeking food",
            Self::Moving => "Moving",
        }
    }
}

#[derive(Clone, Debug)]
pub struct Entity {
    pub id: u32,
    pub x: u32,
    pub y: u32,
    pub hunger: f32,
    pub path: Vec<(u32, u32)>,
    pub path_index: usize,
    pub activity: EntityActivity,
}

impl Entity {
    pub fn remaining_path_len(&self) -> usize {
        self.path.len().saturating_sub(self.path_index)
    }
}

#[derive(Clone, Debug)]
pub struct Simulation {
    tick: u64,
    paused: bool,
    entities: Vec<Entity>,
    next_entity_id: u32,
    world_revision: u64,
}

impl Default for Simulation {
    fn default() -> Self {
        Self {
            tick: 0,
            paused: true,
            entities: Vec::new(),
            next_entity_id: 1,
            world_revision: 0,
        }
    }
}

impl Simulation {
    pub fn with_first_entity(world: &Grid) -> Self {
        let mut simulation = Self::default();
        simulation.spawn_on_walkable_terrain(world);
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

    pub fn spawn_on_walkable_terrain(&mut self, world: &Grid) -> Option<u32> {
        let (x, y) = find_spawn_position(world)?;
        let id = self.next_entity_id;
        self.next_entity_id = self.next_entity_id.saturating_add(1);
        self.entities.push(Entity {
            id,
            x,
            y,
            hunger: 0.0,
            path: Vec::new(),
            path_index: 0,
            activity: EntityActivity::Idle,
        });
        Some(id)
    }

    fn step_world(&mut self, world: &mut Grid) {
        self.tick = self.tick.saturating_add(1);
        let mut world_changed = false;

        for entity in &mut self.entities {
            entity.hunger = (entity.hunger + HUNGER_PER_TICK).min(MAX_HUNGER);

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
                    world_changed |= consume_food(entity, world);
                }
                continue;
            }

            if entity.hunger >= FOOD_SEARCH_THRESHOLD {
                if consume_food(entity, world) {
                    world_changed = true;
                } else if let Some(path) = find_path_to_food(world, (entity.x, entity.y)) {
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

        if world_changed {
            self.world_revision = self.world_revision.saturating_add(1);
        }
    }
}

fn find_spawn_position(world: &Grid) -> Option<(u32, u32)> {
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

    for food in food_tiles.into_iter().take(64) {
        for radius in 6..=12i32 {
            for dx in -radius..=radius {
                let dy = radius - dx.abs();
                for signed_dy in [dy, -dy] {
                    let x = food.0 as i64 + i64::from(dx);
                    let y = food.1 as i64 + i64::from(signed_dy);
                    if x < 0 || y < 0 || x >= i64::from(world.width) || y >= i64::from(world.height)
                    {
                        continue;
                    }
                    let candidate = (x as u32, y as u32);
                    if world
                        .get(candidate.0, candidate.1)
                        .is_some_and(|tile| tile.terrain.is_walkable())
                        && pathfinding::find_path(world, candidate, food).is_some()
                    {
                        return Some(candidate);
                    }
                }
            }
        }
    }

    world
        .tiles
        .iter()
        .enumerate()
        .filter(|(_, tile)| tile.terrain.is_walkable())
        .min_by_key(|(index, _)| {
            let index = *index as u32;
            manhattan((index % world.width, index / world.width), center)
        })
        .map(|(index, _)| {
            let index = index as u32;
            (index % world.width, index / world.width)
        })
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

fn consume_food(entity: &mut Entity, world: &mut Grid) -> bool {
    let index = (entity.y * world.width + entity.x) as usize;
    let Some(slot) = world.resources.get_mut(index) else {
        return false;
    };
    let Some(deposit) = slot.as_mut() else {
        return false;
    };
    if deposit.kind != ResourceKind::Food {
        return false;
    }

    let consumed = deposit.amount.min(FOOD_CONSUMED_PER_MEAL);
    deposit.amount -= consumed;
    entity.hunger = (entity.hunger - HUNGER_REDUCTION_PER_MEAL).max(0.0);
    if deposit.amount == 0 {
        *slot = None;
    }
    true
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

    fn simulation_with_entity(x: u32, y: u32, hunger: f32) -> Simulation {
        Simulation {
            entities: vec![Entity {
                id: 1,
                x,
                y,
                hunger,
                path: Vec::new(),
                path_index: 0,
                activity: EntityActivity::Idle,
            }],
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
    fn first_entity_spawns_on_walkable_terrain_with_stable_id() {
        let world = grid_from_rows(&["###", "#P#", "###"]);
        let simulation = Simulation::with_first_entity(&world);
        let entity = &simulation.entities()[0];
        assert_eq!(entity.id, 1);
        assert_eq!((entity.x, entity.y), (1, 1));
        assert!(world.get(entity.x, entity.y).unwrap().terrain.is_walkable());
    }

    #[test]
    fn first_entity_spawns_near_reachable_food_when_available() {
        let world = grid_from_rows(&["PPPPPPPPF"]);
        let simulation = Simulation::with_first_entity(&world);
        let entity = &simulation.entities()[0];
        let distance = manhattan((entity.x, entity.y), (8, 0));
        assert!((6..=12).contains(&distance));
        assert!(pathfinding::find_path(&world, (entity.x, entity.y), (8, 0)).is_some());
    }

    #[test]
    fn paused_simulation_does_not_change_world_or_entity() {
        let mut world = grid_from_rows(&["PF"]);
        let mut simulation = simulation_with_entity(0, 0, 59.0);
        simulation.advance(10, &mut world);
        assert_eq!(simulation.tick(), 0);
        assert_eq!(simulation.entities()[0].hunger, 59.0);
        assert_eq!(world.resources[1].unwrap().amount, 20);
    }

    #[test]
    fn manual_step_works_while_paused() {
        let mut world = grid_from_rows(&["P"]);
        let mut simulation = simulation_with_entity(0, 0, 0.0);
        assert_eq!(simulation.step(&mut world), 1);
        assert_eq!(simulation.entities()[0].hunger, 1.0);
        assert!(simulation.is_paused());
    }

    #[test]
    fn entity_calculates_path_once_and_follows_it() {
        let mut world = grid_from_rows(&["PPPPP", "P###F", "PPPPP"]);
        let mut simulation = simulation_with_entity(0, 1, 59.0);

        simulation.step(&mut world);
        let original_path = simulation.entities()[0].path.clone();
        assert!(!original_path.is_empty());
        simulation.step(&mut world);
        let entity = &simulation.entities()[0];
        assert_eq!(entity.path, original_path);
        assert_eq!(entity.path_index, 1);
        assert!(world.get(entity.x, entity.y).unwrap().terrain.is_walkable());
    }

    #[test]
    fn entity_path_prefers_cheaper_terrain() {
        let mut world = grid_from_rows(&["PPPPP", "PMMMF", "PPPPP"]);
        let mut simulation = simulation_with_entity(0, 1, 59.0);
        simulation.step(&mut world);

        assert!(simulation.entities()[0].path.iter().all(|&(x, y)| world
            .get(x, y)
            .unwrap()
            .terrain
            != Terrain::Mountain));
    }

    #[test]
    fn hungry_entity_reaches_and_consumes_food() {
        let mut world = grid_from_rows(&["PPPPP", "P###F", "PPPPP"]);
        let mut simulation = simulation_with_entity(0, 1, 59.0);

        for _ in 0..10 {
            simulation.step(&mut world);
        }

        let entity = &simulation.entities()[0];
        assert_eq!((entity.x, entity.y), (4, 1));
        assert!(entity.hunger < FOOD_SEARCH_THRESHOLD);
        assert_eq!(world.resources[9].unwrap().amount, 10);
        assert_eq!(simulation.world_revision(), 1);
    }

    #[test]
    fn depleted_food_deposit_is_removed() {
        let mut world = grid_from_rows(&["F"]);
        world.resources[0].as_mut().unwrap().amount = 7;
        let mut simulation = simulation_with_entity(0, 0, 60.0);
        simulation.step(&mut world);
        assert!(world.resources[0].is_none());
    }

    #[test]
    fn automatic_advance_respects_resume_and_pause() {
        let mut world = grid_from_rows(&["P"]);
        let mut simulation = simulation_with_entity(0, 0, 0.0);
        simulation.resume();
        assert_eq!(simulation.advance(3, &mut world), 3);
        simulation.pause();
        assert_eq!(simulation.advance(8, &mut world), 3);
    }
}
