use crate::world::{Grid, ResourceDeposit, ResourceKind, Terrain};

const RESOURCE_SEED_SALT: u64 = 0x7265_736f_7572_6365;

pub fn generate_resources(world_seed: u32, grid: &mut Grid) {
    let resource_seed = mix64(u64::from(world_seed) ^ RESOURCE_SEED_SALT);
    grid.resources = grid
        .tiles
        .iter()
        .enumerate()
        .map(|(index, tile)| {
            let sample = mix64(resource_seed ^ (index as u64).wrapping_mul(0x9e37_79b9_7f4a_7c15));
            deposit_for(tile.terrain, sample)
        })
        .collect();
}

fn deposit_for(terrain: Terrain, sample: u64) -> Option<ResourceDeposit> {
    let roll = (sample >> 48) as u16;
    let amount_roll = sample as u16;
    let threshold = |probability: f32| (probability * f32::from(u16::MAX)) as u16;

    let (kind, minimum, maximum) = match terrain {
        Terrain::Forest if roll < threshold(0.48) => (ResourceKind::Timber, 280, 820),
        Terrain::DenseForest if roll < threshold(0.68) => (ResourceKind::Timber, 420, 1_100),
        Terrain::Plains if roll < threshold(0.24) => (ResourceKind::Food, 120, 520),
        Terrain::Grassland if roll < threshold(0.38) => (ResourceKind::Food, 180, 720),
        Terrain::Hills if roll < threshold(0.10) => (ResourceKind::Iron, 25, 150),
        Terrain::Hills if roll < threshold(0.40) => (ResourceKind::Stone, 160, 620),
        Terrain::Mountain if roll < threshold(0.22) => (ResourceKind::Iron, 40, 220),
        Terrain::Mountain if roll < threshold(0.72) => (ResourceKind::Stone, 260, 900),
        Terrain::Desert if roll < threshold(0.05) => (ResourceKind::Stone, 80, 300),
        Terrain::Swamp if roll < threshold(0.20) => (ResourceKind::Food, 90, 360),
        Terrain::Tundra if roll < threshold(0.08) => (ResourceKind::Food, 60, 220),
        _ => return None,
    };

    Some(ResourceDeposit {
        kind,
        amount: amount_in_range(minimum, maximum, amount_roll),
    })
}

fn amount_in_range(minimum: u16, maximum: u16, sample: u16) -> u16 {
    let span = u32::from(maximum - minimum) + 1;
    minimum + (u32::from(sample) * span / (u32::from(u16::MAX) + 1)) as u16
}

fn mix64(mut value: u64) -> u64 {
    value ^= value >> 30;
    value = value.wrapping_mul(0xbf58_476d_1ce4_e5b9);
    value ^= value >> 27;
    value = value.wrapping_mul(0x94d0_49bb_1331_11eb);
    value ^ (value >> 31)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::world::Tile;

    fn grid_of(terrain: Terrain, count: usize) -> Grid {
        Grid {
            width: count as u32,
            height: 1,
            tiles: vec![
                Tile {
                    terrain,
                    altitude: 0.5,
                    moisture: 0.5,
                    temperature: 0.5,
                };
                count
            ],
            region_ids: Vec::new(),
            regions: Vec::new(),
            resources: Vec::new(),
        }
    }

    #[test]
    fn resource_generation_is_deterministic() {
        let mut first = grid_of(Terrain::Forest, 512);
        let mut second = grid_of(Terrain::Forest, 512);
        generate_resources(42, &mut first);
        generate_resources(42, &mut second);
        assert_eq!(first.resources, second.resources);
    }

    #[test]
    fn different_seeds_change_resources() {
        let mut first = grid_of(Terrain::Grassland, 512);
        let mut second = grid_of(Terrain::Grassland, 512);
        generate_resources(1, &mut first);
        generate_resources(999, &mut second);
        assert_ne!(first.resources, second.resources);
    }

    #[test]
    fn ocean_has_no_land_resources() {
        for terrain in [Terrain::DeepWater, Terrain::ShallowWater] {
            let mut grid = grid_of(terrain, 512);
            generate_resources(42, &mut grid);
            assert!(grid.resources.iter().all(Option::is_none));
        }
    }

    #[test]
    fn forests_can_generate_timber() {
        let mut grid = grid_of(Terrain::Forest, 512);
        generate_resources(42, &mut grid);
        assert!(grid
            .resources
            .iter()
            .flatten()
            .any(|deposit| { deposit.kind == ResourceKind::Timber && deposit.amount > 0 }));
    }

    #[test]
    fn mountains_can_generate_stone_and_iron() {
        let mut grid = grid_of(Terrain::Mountain, 1_024);
        generate_resources(42, &mut grid);
        assert!(grid
            .resources
            .iter()
            .flatten()
            .any(|deposit| deposit.kind == ResourceKind::Stone));
        assert!(grid
            .resources
            .iter()
            .flatten()
            .any(|deposit| deposit.kind == ResourceKind::Iron));
    }

    #[test]
    fn resource_count_matches_world_size() {
        let mut grid = grid_of(Terrain::Plains, 4_096);
        generate_resources(42, &mut grid);
        assert_eq!(grid.resources.len(), grid.tiles.len());
    }
}
