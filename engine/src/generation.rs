use crate::world::{Grid, Terrain, Tile};
use noise::{NoiseFn, Perlin, Seedable};

pub fn generate_world(seed: u32, width: u32, height: u32, sea_level: f64) -> Grid {
    let tile_count = width
        .checked_mul(height)
        .and_then(|n| usize::try_from(n).ok())
        .expect("world dimensions overflow");

    let continent_noise = Perlin::default().set_seed(seed);
    let detail_noise = Perlin::default().set_seed(seed.wrapping_add(1));
    let moisture_noise = Perlin::default().set_seed(seed.wrapping_add(2));
    let temp_noise = Perlin::default().set_seed(seed.wrapping_add(3));

    let mut tiles = Vec::with_capacity(tile_count);

    for y in 0..height {
        for x in 0..width {
            let fx = x as f64;
            let fy = y as f64;

            let continent = fbm(&continent_noise, fx, fy, 0.0018, 4, 0.5, 2.0);
            let detail = fbm(&detail_noise, fx, fy, 0.007, 6, 0.48, 2.1);
            let altitude = continent * 0.58 + detail * 0.42;

            let raw_moisture = fbm(&moisture_noise, fx, fy, 0.005, 4, 0.5, 2.0);
            let moisture = ((raw_moisture + 1.0) / 2.0).clamp(0.0, 1.0);

            let latitude = (fy / height as f64 - 0.5).abs() * 2.0;
            let temp_var = fbm(&temp_noise, fx, fy, 0.0025, 3, 0.4, 2.0) * 0.18;
            let temperature = (1.0 - latitude * 0.85 + temp_var).clamp(0.0, 1.0);

            let terrain = classify_terrain(altitude, moisture, temperature, sea_level);

            tiles.push(Tile {
                terrain,
                altitude,
                moisture,
                temperature,
            });
        }
    }

    Grid {
        width,
        height,
        tiles,
        region_ids: Vec::new(),
        regions: Vec::new(),
        resources: vec![None; tile_count],
        renewable_resources: Vec::new(),
    }
}

fn fbm(
    noise: &Perlin,
    x: f64,
    y: f64,
    base_freq: f64,
    octaves: usize,
    persistence: f64,
    lacunarity: f64,
) -> f64 {
    let mut total = 0.0;
    let mut amplitude = 1.0;
    let mut frequency = base_freq;
    let mut max_amplitude = 0.0;

    for _ in 0..octaves {
        total += noise.get([x * frequency, y * frequency]) * amplitude;
        max_amplitude += amplitude;
        amplitude *= persistence;
        frequency *= lacunarity;
    }

    total / max_amplitude
}

fn classify_terrain(altitude: f64, moisture: f64, temperature: f64, sea_level: f64) -> Terrain {
    if altitude < sea_level - 0.18 {
        return Terrain::DeepWater;
    }
    if altitude < sea_level {
        return Terrain::ShallowWater;
    }
    if altitude < sea_level + 0.025 {
        return Terrain::Beach;
    }
    if altitude > 0.72 {
        return Terrain::SnowPeak;
    }
    if altitude > 0.52 {
        return Terrain::Mountain;
    }
    if altitude > 0.40 {
        return Terrain::Hills;
    }
    if temperature < 0.18 {
        return Terrain::Tundra;
    }
    if moisture < 0.18 && temperature > 0.4 {
        return Terrain::Desert;
    }
    if moisture > 0.72 && altitude < sea_level + 0.08 {
        return Terrain::Swamp;
    }
    if moisture < 0.32 {
        return Terrain::Plains;
    }
    if moisture < 0.48 {
        return Terrain::Grassland;
    }
    if moisture < 0.65 {
        return Terrain::Forest;
    }
    Terrain::DenseForest
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn same_seed_produces_same_world() {
        let g1 = generate_world(42, 64, 64, 0.35);
        let g2 = generate_world(42, 64, 64, 0.35);
        for (a, b) in g1.tiles.iter().zip(g2.tiles.iter()) {
            assert_eq!(a.terrain as u8, b.terrain as u8);
            assert!((a.altitude - b.altitude).abs() < 1e-12);
        }
    }

    #[test]
    fn different_seeds_differ() {
        let g1 = generate_world(1, 64, 64, 0.35);
        let g2 = generate_world(999, 64, 64, 0.35);
        let same = g1
            .tiles
            .iter()
            .zip(g2.tiles.iter())
            .filter(|(a, b)| a.terrain as u8 == b.terrain as u8)
            .count();
        assert!(same < g1.tiles.len());
    }

    #[test]
    fn expected_tile_count() {
        let grid = generate_world(0, 128, 96, 0.35);
        assert_eq!(grid.tiles.len(), 128 * 96);
    }

    #[test]
    fn regions_start_empty() {
        let grid = generate_world(42, 64, 64, 0.35);
        assert!(grid.region_ids.is_empty());
        assert!(grid.regions.is_empty());
        assert_eq!(grid.resources.len(), grid.tiles.len());
    }
}
