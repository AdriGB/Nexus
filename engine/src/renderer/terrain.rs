use crate::world::Grid;

pub fn encode_world_texture(grid: &Grid) -> Vec<u8> {
    let mut data = Vec::with_capacity(grid.tiles.len() * 4);

    for tile in &grid.tiles {
        data.push(tile.terrain as u8);
        data.push(((tile.altitude + 1.0) * 0.5 * 255.0).clamp(0.0, 255.0) as u8);
        data.push((tile.moisture * 255.0).clamp(0.0, 255.0) as u8);
        data.push((tile.temperature * 255.0).clamp(0.0, 255.0) as u8);
    }

    data
}
