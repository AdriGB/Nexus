use serde::Serialize;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
#[repr(u8)]
pub enum ResourceKind {
    Food = 1,
    Timber = 2,
    Stone = 3,
    Iron = 4,
}

impl ResourceKind {
    pub fn label(&self) -> &'static str {
        match self {
            Self::Food => "Food",
            Self::Timber => "Timber",
            Self::Stone => "Stone",
            Self::Iron => "Iron",
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
pub struct ResourceDeposit {
    pub kind: ResourceKind,
    pub amount: u16,
}

#[derive(Clone, Copy, PartialEq, Eq, Serialize)]
#[repr(u8)]
pub enum Terrain {
    DeepWater = 0,
    ShallowWater = 1,
    Beach = 2,
    Plains = 3,
    Grassland = 4,
    Forest = 5,
    DenseForest = 6,
    Hills = 7,
    Mountain = 8,
    SnowPeak = 9,
    Desert = 10,
    Swamp = 11,
    Tundra = 12,
}

impl Terrain {
    pub fn label(&self) -> &'static str {
        match self {
            Self::DeepWater => "Deep Water",
            Self::ShallowWater => "Shallow Water",
            Self::Beach => "Beach",
            Self::Plains => "Plains",
            Self::Grassland => "Grassland",
            Self::Forest => "Forest",
            Self::DenseForest => "Dense Forest",
            Self::Hills => "Hills",
            Self::Mountain => "Mountain",
            Self::SnowPeak => "Snow Peak",
            Self::Desert => "Desert",
            Self::Swamp => "Swamp",
            Self::Tundra => "Tundra",
        }
    }

    pub fn is_water(&self) -> bool {
        matches!(self, Self::DeepWater | Self::ShallowWater)
    }

    pub fn movement_cost(&self) -> Option<f32> {
        match self {
            Self::DeepWater | Self::ShallowWater | Self::SnowPeak => None,
            Self::Beach => Some(1.2),
            Self::Plains => Some(1.0),
            Self::Grassland => Some(1.1),
            Self::Forest => Some(1.6),
            Self::DenseForest => Some(2.2),
            Self::Hills => Some(2.0),
            Self::Mountain => Some(4.0),
            Self::Desert => Some(1.8),
            Self::Swamp => Some(3.0),
            Self::Tundra => Some(1.7),
        }
    }

    pub fn is_walkable(&self) -> bool {
        self.movement_cost().is_some()
    }
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum RegionKind {
    Land,
    Water,
}

#[derive(Clone, Debug)]
pub struct Region {
    pub kind: RegionKind,
    pub tile_count: u32,
    pub min_x: u32,
    pub min_y: u32,
    pub max_x: u32,
    pub max_y: u32,
    pub touches_border: bool,
}

#[derive(Clone, Serialize)]
pub struct Tile {
    pub terrain: Terrain,
    pub altitude: f64,
    pub moisture: f64,
    pub temperature: f64,
}

pub struct Grid {
    pub width: u32,
    pub height: u32,
    pub tiles: Vec<Tile>,
    pub region_ids: Vec<u32>,
    pub regions: Vec<Region>,
    pub resources: Vec<Option<ResourceDeposit>>,
}

impl Grid {
    pub fn get(&self, x: u32, y: u32) -> Option<&Tile> {
        if x < self.width && y < self.height {
            Some(&self.tiles[(y * self.width + x) as usize])
        } else {
            None
        }
    }

    pub fn region_id_at(&self, x: u32, y: u32) -> Option<u32> {
        if x >= self.width || y >= self.height {
            return None;
        }

        self.region_ids
            .get((y * self.width + x) as usize)
            .copied()
            .filter(|&region_id| region_id != u32::MAX)
    }

    pub fn is_coastal(&self, x: u32, y: u32) -> bool {
        if let Some(tile) = self.get(x, y) {
            if tile.terrain.is_water() {
                return false;
            }
            for (dx, dy) in [(-1i32, 0i32), (1, 0), (0, -1), (0, 1)] {
                let nx = x as i32 + dx;
                let ny = y as i32 + dy;
                if nx >= 0 && ny >= 0 {
                    if let Some(n) = self.get(nx as u32, ny as u32) {
                        if n.terrain.is_water() {
                            return true;
                        }
                    }
                }
            }
        }
        false
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_grid(w: u32, h: u32) -> Grid {
        let tiles = (0..w * h)
            .map(|_| Tile {
                terrain: Terrain::Plains,
                altitude: 0.0,
                moisture: 0.5,
                temperature: 0.5,
            })
            .collect();
        Grid {
            width: w,
            height: h,
            tiles,
            region_ids: Vec::new(),
            regions: Vec::new(),
            resources: vec![None; (w * h) as usize],
        }
    }

    #[test]
    fn tile_count_matches_dimensions() {
        let grid = make_grid(100, 80);
        assert_eq!(grid.tiles.len(), 8000);
    }

    #[test]
    fn valid_coordinates_return_some() {
        let grid = make_grid(10, 10);
        assert!(grid.get(0, 0).is_some());
        assert!(grid.get(9, 9).is_some());
    }

    #[test]
    fn out_of_bounds_returns_none() {
        let grid = make_grid(10, 10);
        assert!(grid.get(10, 0).is_none());
        assert!(grid.get(0, 10).is_none());
        assert!(grid.get(100, 100).is_none());
    }

    #[test]
    fn region_id_at_returns_known_region() {
        let mut grid = make_grid(2, 1);
        grid.region_ids = vec![3, 7];

        assert_eq!(grid.region_id_at(0, 0), Some(3));
        assert_eq!(grid.region_id_at(1, 0), Some(7));
    }

    #[test]
    fn region_id_at_returns_none_when_regions_are_unavailable() {
        let grid = make_grid(2, 1);

        assert_eq!(grid.region_id_at(0, 0), None);
        assert_eq!(grid.region_id_at(2, 0), None);
    }

    #[test]
    fn plains_are_walkable() {
        assert!(Terrain::Plains.is_walkable());
    }

    #[test]
    fn deep_water_is_not_walkable() {
        assert!(!Terrain::DeepWater.is_walkable());
    }

    #[test]
    fn snow_peaks_are_not_walkable() {
        assert!(!Terrain::SnowPeak.is_walkable());
    }

    #[test]
    fn mountains_are_more_expensive_than_plains() {
        assert!(
            Terrain::Mountain.movement_cost().unwrap() > Terrain::Plains.movement_cost().unwrap()
        );
    }
}
