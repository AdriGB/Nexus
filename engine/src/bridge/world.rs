use serde::Serialize;

use super::to_json;
use crate::world::{Grid, RegionKind};

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
