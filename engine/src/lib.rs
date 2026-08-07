mod generation;
mod regions;
mod world;

use wasm_bindgen::prelude::*;

#[wasm_bindgen(start)]
pub fn start() {
    console_error_panic_hook::set_once();
}

#[wasm_bindgen]
pub struct WorldBridge {
    grid: world::Grid,
}

#[wasm_bindgen]
impl WorldBridge {
    #[wasm_bindgen(constructor)]
    pub fn new(seed: u32, width: u32, height: u32, sea_level: f64) -> WorldBridge {
        let mut grid = generation::generate_world(seed, width, height, sea_level);
        regions::detect_regions(&mut grid);
        WorldBridge { grid }
    }

    pub fn width(&self) -> u32 {
        self.grid.width
    }

    pub fn height(&self) -> u32 {
        self.grid.height
    }

    pub fn get_tile_data(&self, vx: i32, vy: i32, vw: i32, vh: i32) -> Vec<u8> {
        let safe_vw = vw.max(0);
        let safe_vh = vh.max(0);
        let mut data = Vec::with_capacity((safe_vw * safe_vh * 4) as usize);

        for y in vy..(vy + safe_vh) {
            for x in vx..(vx + safe_vw) {
                if x >= 0 && y >= 0 && (x as u32) < self.grid.width && (y as u32) < self.grid.height
                {
                    let tile = &self.grid.tiles[(y as u32 * self.grid.width + x as u32) as usize];
                    data.push(tile.terrain as u8);
                    data.push(((tile.altitude + 1.0) / 2.0 * 255.0).clamp(0.0, 255.0) as u8);
                    data.push((tile.moisture * 255.0).clamp(0.0, 255.0) as u8);
                    data.push((tile.temperature * 255.0).clamp(0.0, 255.0) as u8);
                } else {
                    data.push(255);
                    data.extend_from_slice(&[0, 0, 0]);
                }
            }
        }
        data
    }

    pub fn tile_info(&self, x: u32, y: u32) -> String {
        match self.grid.get(x, y) {
            Some(tile) => {
                let idx = (y * self.grid.width + x) as usize;
                let rid = if idx < self.grid.region_ids.len() {
                    self.grid.region_ids[idx]
                } else {
                    u32::MAX
                };
                let (r_kind, r_area, _r_border) =
                    if rid != u32::MAX && (rid as usize) < self.grid.regions.len() {
                        let r = &self.grid.regions[rid as usize];
                        (
                            match r.kind {
                                world::RegionKind::Land => "Land",
                                world::RegionKind::Water => "Water",
                            },
                            r.tile_count,
                            r.touches_border,
                        )
                    } else {
                        ("Unknown", 0, false)
                    };
                let coastal = self.grid.is_coastal(x, y);
                format!(
                    concat!(
                        r#"{{"terrain":"{}","#,
                        r#""altitude":{:.6},"#,
                        r#""moisture":{:.6},"#,
                        r#""temperature":{:.6},"#,
                        r#""x":{},"y":{},"#,
                        r#""region_id":{},"#,
                        r#""region_type":"{}","#,
                        r#""region_area":{},"#,
                        r#""coastal":{}}}"#,
                    ),
                    tile.terrain.label(),
                    tile.altitude,
                    tile.moisture,
                    tile.temperature,
                    x,
                    y,
                    rid,
                    r_kind,
                    r_area,
                    coastal,
                )
            }
            None => "{}".to_string(),
        }
    }

    pub fn region_stats(&self) -> String {
        let total = (self.grid.width * self.grid.height) as f64;
        let land: Vec<_> = self
            .grid
            .regions
            .iter()
            .filter(|r| r.kind == world::RegionKind::Land)
            .collect();
        let water_count = self
            .grid
            .regions
            .iter()
            .filter(|r| r.kind == world::RegionKind::Water)
            .count();
        let land_tiles: u32 = land.iter().map(|r| r.tile_count).sum();
        let water_tiles: u32 = total as u32 - land_tiles;
        let largest = land.iter().map(|r| r.tile_count).max().unwrap_or(0);
        let islands = land.iter().filter(|r| !r.touches_border).count();

        format!(
            concat!(
                r#"{{"land_regions":{},"#,
                r#""water_regions":{},"#,
                r#""land_tiles":{},"#,
                r#""water_tiles":{},"#,
                r#""total_tiles":{},"#,
                r#""land_coverage":{:.4},"#,
                r#""largest_landmass_pct":{:.4},"#,
                r#""islands":{}}}"#,
            ),
            land.len(),
            water_count,
            land_tiles,
            water_tiles,
            total as u32,
            land_tiles as f64 / total,
            largest as f64 / total,
            islands,
        )
    }
}
