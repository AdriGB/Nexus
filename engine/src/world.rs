use serde::Serialize;

/// Tipos de terreno del mundo
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
    pub fn from_u8(v: u8) -> Self {
        match v {
            0 => Self::DeepWater,
            1 => Self::ShallowWater,
            2 => Self::Beach,
            3 => Self::Plains,
            4 => Self::Grassland,
            5 => Self::Forest,
            6 => Self::DenseForest,
            7 => Self::Hills,
            8 => Self::Mountain,
            9 => Self::SnowPeak,
            10 => Self::Desert,
            11 => Self::Swamp,
            12 => Self::Tundra,
            _ => Self::DeepWater,
        }
    }

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

/// Datos de un tile individual
#[derive(Clone, Serialize)]
pub struct Tile {
    pub terrain: Terrain,
    pub altitude: f64,
    pub moisture: f64,
    pub temperature: f64,
}

/// Grid del mundo
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
}
