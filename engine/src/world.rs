use serde::Serialize;

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
}

impl Grid {
    pub fn get(&self, x: u32, y: u32) -> Option<&Tile> {
        if x < self.width && y < self.height {
            Some(&self.tiles[(y * self.width + x) as usize])
        } else {
            None
        }
    }

    pub fn tile_count(&self) -> usize {
        self.tiles.len()
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
        Grid { width: w, height: h, tiles }
    }

    #[test]
    fn tile_count_matches_dimensions() {
        let grid = make_grid(100, 80);
        assert_eq!(grid.tile_count(), 8000);
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
}
