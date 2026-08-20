use super::autonomy::Mind;
use super::Inventory;

/// Five deterministic psychological traits, each in [0.0, 1.0].
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Personality {
    pub curiosity: f32,
    pub sociability: f32,
    pub cooperativeness: f32,
    pub caution: f32,
    pub persistence: f32,
}

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
    Socializing = 6,
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
            Self::Socializing => "Socializing",
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
    pub caregiver_id: Option<u32>,
    pub personality: Personality,
    pub inventory: Inventory,
    pub action_tick: u32,
}

impl Entity {
    pub fn remaining_path_len(&self) -> usize {
        self.path.len().saturating_sub(self.path_index)
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum LifeStage {
    Infant,
    Child,
    Adolescent,
    Adult,
    Elder,
}

impl LifeStage {
    pub fn from_age_ticks(age_ticks: u64) -> Self {
        use super::time::{ADOLESCENT_AGE_END, CHILD_AGE_END, ELDER_AGE_START, INFANT_AGE_END};

        if age_ticks < INFANT_AGE_END {
            Self::Infant
        } else if age_ticks < CHILD_AGE_END {
            Self::Child
        } else if age_ticks < ADOLESCENT_AGE_END {
            Self::Adolescent
        } else if age_ticks < ELDER_AGE_START {
            Self::Adult
        } else {
            Self::Elder
        }
    }

    pub fn label(self) -> &'static str {
        match self {
            Self::Infant => "Infant",
            Self::Child => "Child",
            Self::Adolescent => "Adolescent",
            Self::Adult => "Adult",
            Self::Elder => "Elder",
        }
    }

    pub fn movement_factor(self) -> f32 {
        match self {
            Self::Infant => 0.2,
            Self::Child => 0.5,
            Self::Adolescent => 0.85,
            Self::Adult => 1.0,
            Self::Elder => 0.7,
        }
    }
}
