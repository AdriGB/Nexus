use serde::Serialize;

use crate::simulation::{self, Entity, PopulationStats};
use crate::world::{Grid, RegionKind};

fn to_json<T: Serialize>(value: &T) -> String {
    serde_json::to_string(value).expect("bridge DTO serialization should not fail")
}

#[derive(Serialize)]
struct PopulationStatsDto {
    population: u32,
    births: u64,
    deaths: u64,
    females: u32,
    males: u32,
    pregnant: u32,
    hungry: u32,
    seeking_food: u32,
    average_hunger: f32,
    food_consumed: u64,
}

pub(crate) fn population_stats_json(stats: PopulationStats) -> String {
    to_json(&PopulationStatsDto {
        population: stats.population,
        births: stats.births,
        deaths: stats.deaths,
        females: stats.females,
        males: stats.males,
        pregnant: stats.pregnant,
        hungry: stats.hungry,
        seeking_food: stats.seeking_food,
        average_hunger: stats.average_hunger,
        food_consumed: stats.food_consumed,
    })
}

#[derive(Serialize)]
struct UtilityScoresDto {
    eat: f32,
    explore: f32,
    rest: f32,
}

#[derive(Serialize)]
struct EntityInfoDto {
    id: u32,
    x: u32,
    y: u32,
    sex: &'static str,
    hunger: f32,
    health: f32,
    age_ticks: u64,
    age_years: f64,
    lifespan_ticks: u64,
    pregnant: bool,
    pregnancy_due_tick: Option<u64>,
    activity: &'static str,
    remaining_path: usize,
    goal: &'static str,
    action: &'static str,
    goal_age_ticks: u64,
    known_resources: usize,
    known_chunks: usize,
    visible_entities: usize,
    utilities: UtilityScoresDto,
}

pub(crate) fn entity_info_json(entity: &Entity, tick: u64) -> String {
    let goal = entity
        .mind
        .current_goal
        .map_or("None", simulation::Goal::label);
    let action = entity
        .mind
        .current_action()
        .map_or("None", simulation::Action::label);
    let goal_age_ticks = entity
        .mind
        .current_goal
        .map_or(0, |_| tick.saturating_sub(entity.mind.goal_since_tick));

    to_json(&EntityInfoDto {
        id: entity.id,
        x: entity.x,
        y: entity.y,
        sex: entity.sex.label(),
        hunger: entity.hunger,
        health: entity.health,
        age_ticks: entity.age_ticks,
        age_years: simulation::years_from_ticks(entity.age_ticks),
        lifespan_ticks: entity.lifespan_ticks,
        pregnant: entity.pregnancy.is_some(),
        pregnancy_due_tick: entity.pregnancy.map(|pregnancy| pregnancy.due_tick),
        activity: entity.activity.label(),
        remaining_path: entity.remaining_path_len(),
        goal,
        action,
        goal_age_ticks,
        known_resources: entity.mind.memory.known_resources.len(),
        known_chunks: entity.mind.memory.known_chunk_count(),
        visible_entities: entity.mind.visible_entities.len(),
        utilities: UtilityScoresDto {
            eat: entity.mind.utility_scores.eat,
            explore: entity.mind.utility_scores.explore,
            rest: entity.mind.utility_scores.rest,
        },
    })
}

#[derive(Serialize)]
struct ResourceInfoDto {
    kind: &'static str,
    amount: u16,
}

#[derive(Serialize)]
struct TileInfoDto {
    terrain: &'static str,
    altitude: f64,
    moisture: f64,
    temperature: f64,
    x: u32,
    y: u32,
    region_id: u32,
    region_type: &'static str,
    region_area: u32,
    coastal: bool,
    walkable: bool,
    movement_cost: Option<f32>,
    resource: Option<ResourceInfoDto>,
}

pub(crate) fn tile_info_json(grid: &Grid, x: u32, y: u32) -> String {
    let Some(tile) = grid.get(x, y) else {
        return "{}".to_string();
    };

    let index = (y * grid.width + x) as usize;
    let region_id = grid.region_ids.get(index).copied().unwrap_or(u32::MAX);
    let (region_type, region_area) = if let Some(region) = grid.regions.get(region_id as usize) {
        (
            match region.kind {
                RegionKind::Land => "Land",
                RegionKind::Water => "Water",
            },
            region.tile_count,
        )
    } else {
        ("Unknown", 0)
    };
    let resource = grid
        .resources
        .get(index)
        .and_then(Option::as_ref)
        .map(|deposit| ResourceInfoDto {
            kind: deposit.kind.label(),
            amount: deposit.amount,
        });

    to_json(&TileInfoDto {
        terrain: tile.terrain.label(),
        altitude: tile.altitude,
        moisture: tile.moisture,
        temperature: tile.temperature,
        x,
        y,
        region_id,
        region_type,
        region_area,
        coastal: grid.is_coastal(x, y),
        walkable: tile.terrain.is_walkable(),
        movement_cost: tile.terrain.movement_cost(),
        resource,
    })
}

#[derive(Serialize)]
struct RegionStatsDto {
    land_regions: usize,
    water_regions: usize,
    land_tiles: u32,
    water_tiles: u32,
    total_tiles: u32,
    land_coverage: f64,
    largest_landmass_pct: f64,
    islands: usize,
}

pub(crate) fn region_stats_json(grid: &Grid) -> String {
    let total = (grid.width * grid.height) as f64;
    let land: Vec<_> = grid
        .regions
        .iter()
        .filter(|region| region.kind == RegionKind::Land)
        .collect();
    let water_regions = grid
        .regions
        .iter()
        .filter(|region| region.kind == RegionKind::Water)
        .count();
    let land_tiles = land.iter().map(|region| region.tile_count).sum();
    let water_tiles = total as u32 - land_tiles;
    let largest = land
        .iter()
        .map(|region| region.tile_count)
        .max()
        .unwrap_or(0);
    let islands = land.iter().filter(|region| !region.touches_border).count();

    to_json(&RegionStatsDto {
        land_regions: land.len(),
        water_regions,
        land_tiles,
        water_tiles,
        total_tiles: total as u32,
        land_coverage: land_tiles as f64 / total,
        largest_landmass_pct: largest as f64 / total,
        islands,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::simulation::Simulation;
    use crate::world::{Region, ResourceDeposit, ResourceKind, Terrain, Tile};

    fn test_grid() -> Grid {
        Grid {
            width: 1,
            height: 1,
            tiles: vec![Tile {
                terrain: Terrain::Plains,
                altitude: 0.25,
                moisture: 0.5,
                temperature: 0.75,
            }],
            region_ids: vec![0],
            regions: vec![Region {
                kind: RegionKind::Land,
                tile_count: 1,
                min_x: 0,
                min_y: 0,
                max_x: 0,
                max_y: 0,
                touches_border: false,
            }],
            resources: vec![Some(ResourceDeposit {
                kind: ResourceKind::Food,
                amount: 20,
            })],
        }
    }

    fn payloads() -> (String, String, String, String) {
        let grid = test_grid();
        let simulation = Simulation::with_population(42, &grid, 1);
        let population = population_stats_json(simulation.population_stats());
        let entity = entity_info_json(&simulation.entities()[0], simulation.tick());
        let tile = tile_info_json(&grid, 0, 0);
        let region = region_stats_json(&grid);
        (population, entity, tile, region)
    }

    #[test]
    fn bridge_payloads_are_valid_json() {
        let (population, entity, tile, region) = payloads();

        for payload in [population, entity, tile, region] {
            let _: serde_json::Value = serde_json::from_str(&payload).unwrap();
        }
    }

    #[test]
    fn absent_pregnancy_serializes_due_tick_as_null() {
        let (_, entity, _, _) = payloads();
        let json: serde_json::Value = serde_json::from_str(&entity).unwrap();

        assert_eq!(json["pregnancy_due_tick"], serde_json::Value::Null);
    }

    #[test]
    fn bridge_payloads_keep_the_frontend_shape() {
        let (population, entity, _, _) = payloads();
        let population: serde_json::Value = serde_json::from_str(&population).unwrap();
        let entity: serde_json::Value = serde_json::from_str(&entity).unwrap();

        for key in [
            "id",
            "sex",
            "age_ticks",
            "lifespan_ticks",
            "pregnant",
            "activity",
            "goal",
            "action",
            "utilities",
        ] {
            assert!(entity.get(key).is_some(), "missing entity field {key}");
        }
        for key in [
            "population",
            "females",
            "males",
            "pregnant",
            "births",
            "deaths",
        ] {
            assert!(
                population.get(key).is_some(),
                "missing population field {key}"
            );
        }
    }
}
