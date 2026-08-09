use super::autonomy::Mind;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Sex {
    Female,
    Male,
}

impl Sex {
    pub fn label(self) -> &'static str {
        match self {
            Self::Female => "Female",
            Self::Male => "Male",
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Pregnancy {
    pub father_id: u32,
    pub conceived_tick: u64,
    pub due_tick: u64,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[repr(u32)]
pub enum EntityActivity {
    Idle = 0,
    SeekingFood = 1,
    Moving = 2,
    Starving = 3,
    Exploring = 4,
    Resting = 5,
}

impl EntityActivity {
    pub fn label(self) -> &'static str {
        match self {
            Self::Idle => "Idle",
            Self::SeekingFood => "Seeking food",
            Self::Moving => "Moving",
            Self::Starving => "Starving",
            Self::Exploring => "Exploring",
            Self::Resting => "Resting",
        }
    }
}

#[derive(Clone, Debug)]
pub struct Entity {
    pub id: u32,
    pub x: u32,
    pub y: u32,
    pub sex: Sex,
    pub lifespan_ticks: u64,
    pub hunger: f32,
    pub health: f32,
    pub age_ticks: u64,
    pub path: Vec<(u32, u32)>,
    pub path_index: usize,
    pub activity: EntityActivity,
    pub mind: Mind,
    pub pregnancy: Option<Pregnancy>,
    pub postpartum_until_tick: u64,
    pub movement_credit: f32,
}

impl Entity {
    pub fn remaining_path_len(&self) -> usize {
        self.path.len().saturating_sub(self.path_index)
    }
}
