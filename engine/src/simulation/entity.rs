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
    pub mother_id: Option<u32>,
    pub father_id: Option<u32>,
    pub caregiver_id: Option<u32>,
    pub partner_id: Option<u32>,
    pub household_id: Option<u32>,
    pub personality: Personality,
    pub inventory: Inventory,
    pub action_tick: u32,
}

impl Entity {
    pub fn remaining_path_len(&self) -> usize {
        self.path.len().saturating_sub(self.path_index)
    }

    pub(in crate::simulation) fn hash_state(&self, hasher: &mut super::state_hash::StateHasher) {
        hasher.write_u32(self.id);
        hasher.write_u32(self.x);
        hasher.write_u32(self.y);
        hasher.write_u32(match self.sex {
            Sex::Female => 0,
            Sex::Male => 1,
        });
        hasher.write_u64(self.lifespan_ticks);
        hasher.write_f32(self.hunger);
        hasher.write_f32(self.health);
        hasher.write_u64(self.age_ticks);
        hasher.write_usize(self.path.len());
        hasher.write_usize(self.path_index);
        for &(px, py) in &self.path {
            hasher.write_u32(px);
            hasher.write_u32(py);
        }
        hasher.write_u32(self.activity as u32);
        if let Some(pregnancy) = self.pregnancy {
            hasher.write_bool(true);
            hasher.write_u32(pregnancy.father_id);
            hasher.write_u64(pregnancy.conceived_tick);
            hasher.write_u64(pregnancy.due_tick);
        } else {
            hasher.write_bool(false);
        }
        hasher.write_u64(self.postpartum_until_tick);
        hasher.write_f32(self.movement_credit);
        hasher.write_opt_u32(self.mother_id);
        hasher.write_opt_u32(self.father_id);
        hasher.write_opt_u32(self.caregiver_id);
        hasher.write_opt_u32(self.partner_id);
        hasher.write_opt_u32(self.household_id);

        // Personality
        hasher.write_f32(self.personality.curiosity);
        hasher.write_f32(self.personality.sociability);
        hasher.write_f32(self.personality.cooperativeness);
        hasher.write_f32(self.personality.caution);
        hasher.write_f32(self.personality.persistence);

        // Inventory
        hasher.write_u16(self.inventory.capacity());
        for &amt in self.inventory.amounts() {
            hasher.write_u16(amt);
        }

        hasher.write_u32(self.action_tick);

        // Mind
        self.mind.hash_state(hasher);
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
