mod world;
mod generation;

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
        let grid = generation::generate_world(seed, width, height, sea_level);
        WorldBridge { grid }
    }

    pub fn width(&self) -> u32 { self.grid.width }

    pub fn height(&self) -> u32 { self.grid.height }

    pub fn get_tile_data(&self, vx: i32, vy: i32, vw: i32, vh: i32) -> Vec<u8> {
        let safe_vw = vw.max(0);
        let safe_vh = vh.max(0);
        let mut data = Vec::with_capacity((safe_vw * safe_vh * 4) as usize);

        for y in vy..(vy + safe_vh) {
            for x in vx..(vx + safe_vw) {
                if x >= 0 && y >= 0
                    && (x as u32) < self.grid.width
                    && (y as u32) < self.grid.height
                {
                    let tile = &self.grid.tiles
                        [(y as u32 * self.grid.width + x as u32) as usize];
                    data.push(tile.terrain as u8);
                    data.push(((tile.altitude + 1.0) / 2.0 * 255.0).clamp(0.0, 255.0) as u8);
                    data.push((tile.moisture * 255.0).clamp(0.0, 255.0) as u8);
                    data.push((tile.temperature * 255.0).clamp(0.0, 255.0) as u8);
                } else {
                    data.extend_from_slice(&[0, 0, 0, 0]);
                }
            }
        }
        data
    }

    pub fn tile_info(&self, x: u32, y: u32) -> String {
        match self.grid.get(x, y) {
            Some(tile) => format!(
                r#"{{"terrain":"{}","altitude":{:.6},"moisture":{:.6},"temperature":{:.6},"x":{},"y":{}}}"#,
                tile.terrain.label(), tile.altitude, tile.moisture, tile.temperature, x, y
            ),
            None => "{}".to_string(),
        }
    }
}
