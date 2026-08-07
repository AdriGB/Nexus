mod generation;
mod pathfinding;
mod regions;
#[cfg(target_arch = "wasm32")]
mod renderer;
mod resources;
mod simulation;
mod world;

use wasm_bindgen::prelude::*;

#[wasm_bindgen(start)]
pub fn start() {
    console_error_panic_hook::set_once();
}

#[wasm_bindgen]
pub struct WorldBridge {
    grid: world::Grid,
    simulation: simulation::Simulation,
}

#[cfg(target_arch = "wasm32")]
#[wasm_bindgen]
pub struct GpuRenderer {
    state: renderer::GpuState,
}

#[cfg(target_arch = "wasm32")]
#[wasm_bindgen]
impl GpuRenderer {
    #[wasm_bindgen(js_name = create)]
    pub async fn create(canvas_id: String) -> Result<GpuRenderer, JsValue> {
        let state = renderer::GpuState::new(&canvas_id).await?;
        Ok(GpuRenderer { state })
    }

    pub fn resize(&mut self, width: u32, height: u32, dpr: f32) {
        self.state.resize(width, height, dpr);
    }

    pub fn upload_world(&mut self, world: &WorldBridge) {
        self.state.upload_world(&world.grid);
    }

    pub fn upload_route(&mut self, coordinates: Vec<u32>) {
        self.state.upload_route(&coordinates);
    }

    pub fn upload_entities(&mut self, world: &WorldBridge) {
        self.state.upload_entities(&world.simulation);
    }

    pub fn upload_resources(&mut self, world: &WorldBridge) {
        self.state.upload_resources(&world.grid);
    }

    #[allow(clippy::too_many_arguments)]
    pub fn render(
        &mut self,
        pan_x: f32,
        pan_y: f32,
        zoom: f32,
        hover_x: i32,
        hover_y: i32,
        selected_x: i32,
        selected_y: i32,
        show_grid: bool,
        show_resources: bool,
    ) -> Result<(), JsValue> {
        self.state.render(
            pan_x,
            pan_y,
            zoom,
            hover_x,
            hover_y,
            selected_x,
            selected_y,
            show_grid,
            show_resources,
        )
    }
}

#[wasm_bindgen]
impl WorldBridge {
    #[wasm_bindgen(constructor)]
    pub fn new(seed: u32, width: u32, height: u32, sea_level: f64) -> WorldBridge {
        let mut grid = generation::generate_world(seed, width, height, sea_level);
        resources::generate_resources(seed, &mut grid);
        regions::detect_regions(&mut grid);
        let simulation =
            simulation::Simulation::with_population(&grid, simulation::INITIAL_POPULATION);
        WorldBridge { grid, simulation }
    }

    pub fn width(&self) -> u32 {
        self.grid.width
    }

    pub fn height(&self) -> u32 {
        self.grid.height
    }

    pub fn simulation_tick(&self) -> u64 {
        self.simulation.tick()
    }

    pub fn simulation_is_paused(&self) -> bool {
        self.simulation.is_paused()
    }

    pub fn simulation_advance(&mut self, ticks: u32) -> u64 {
        self.simulation.advance(ticks, &mut self.grid)
    }

    pub fn simulation_step(&mut self) -> u64 {
        self.simulation.step(&mut self.grid)
    }

    pub fn simulation_pause(&mut self) {
        self.simulation.pause();
    }

    pub fn simulation_resume(&mut self) {
        self.simulation.resume();
    }

    pub fn simulation_world_revision(&self) -> u64 {
        self.simulation.world_revision()
    }

    pub fn entity_count(&self) -> u32 {
        self.simulation.entities().len() as u32
    }

    pub fn spawn_entities(&mut self, count: u32) -> u32 {
        self.simulation.spawn_entities(&self.grid, count)
    }

    pub fn population_stats(&self) -> String {
        let stats = self.simulation.population_stats();
        format!(
            concat!(
                r#"{{"population":{},"births":{},"deaths":{},"#,
                r#""hungry":{},"seeking_food":{},"average_hunger":{:.2},"#,
                r#""food_consumed":{}}}"#,
            ),
            stats.population,
            stats.births,
            stats.deaths,
            stats.hungry,
            stats.seeking_food,
            stats.average_hunger,
            stats.food_consumed,
        )
    }

    pub fn first_entity_info(&self) -> String {
        self.simulation
            .entities()
            .first()
            .map_or_else(|| "{}".to_string(), format_entity_info)
    }

    pub fn entity_info(&self, id: u32) -> String {
        let Some(entity) = self
            .simulation
            .entities()
            .iter()
            .find(|entity| entity.id == id)
        else {
            return "{}".to_string();
        };

        format_entity_info(entity)
    }

    pub fn find_path(&self, start_x: u32, start_y: u32, goal_x: u32, goal_y: u32) -> Vec<u32> {
        pathfinding::find_path(&self.grid, (start_x, start_y), (goal_x, goal_y))
            .map(|path| pathfinding::smooth_path(&self.grid, path))
            .unwrap_or_default()
            .into_iter()
            .flat_map(|(x, y)| [x, y])
            .collect()
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
                let movement_cost = tile
                    .terrain
                    .movement_cost()
                    .map_or_else(|| "null".to_string(), |cost| format!("{cost:.1}"));
                let resource = self
                    .grid
                    .resources
                    .get(idx)
                    .and_then(Option::as_ref)
                    .map_or_else(
                        || "null".to_string(),
                        |deposit| {
                            format!(
                                r#"{{"kind":"{}","amount":{}}}"#,
                                deposit.kind.label(),
                                deposit.amount
                            )
                        },
                    );
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
                        r#""coastal":{},"#,
                        r#""walkable":{},"#,
                        r#""movement_cost":{},"#,
                        r#""resource":{}}}"#,
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
                    tile.terrain.is_walkable(),
                    movement_cost,
                    resource,
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

fn format_entity_info(entity: &simulation::Entity) -> String {
    format!(
        concat!(
            r#"{{"id":{},"x":{},"y":{},"hunger":{:.2},"health":{:.2},"#,
            r#""age_ticks":{},"activity":"{}","remaining_path":{}}}"#,
        ),
        entity.id,
        entity.x,
        entity.y,
        entity.hunger,
        entity.health,
        entity.age_ticks,
        entity.activity.label(),
        entity.remaining_path_len(),
    )
}
