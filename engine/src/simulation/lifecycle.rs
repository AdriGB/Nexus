use super::entity::{Entity, LifeStage, Personality, Pregnancy, Sex};
use super::time::{
    BASE_LIFESPAN_TICKS, FEMALE_REPRODUCTIVE_AGE_END, FOUNDER_AGE_MAX, FOUNDER_AGE_MIN,
    GESTATION_TICKS, LIFESPAN_VARIATION_TICKS, MALE_REPRODUCTIVE_AGE_END, POSTPARTUM_TICKS,
    REPRODUCTIVE_AGE_START,
};
use crate::world::{Grid, ResourceKind};
use std::collections::HashSet;

const REPRODUCTION_MAX_HUNGER: f32 = 35.0;
const REPRODUCTION_MAX_DISTANCE: u32 = 2;
const REPRODUCTION_MIN_AFFINITY: i16 = -200;
pub(super) const DAILY_CONCEPTION_SCALE: u64 = 10_000;
pub(super) const DAILY_CONCEPTION_THRESHOLD: u64 = 100;

const SEX_SALT: u64 = 0x19d6_7a4e_2f91_b5c3;
const LIFESPAN_SALT: u64 = 0xa7c5_31e8_42d9_f60b;
const FOUNDER_AGE_SALT: u64 = 0x62e4_b19f_d03a_875c;
const CURIOSITY_SALT: u64 = 0xa3f1_7b2e_94c0_d685;
const SOCIABILITY_SALT: u64 = 0xd74e_c058_31ab_f926;
const COOPERATION_SALT: u64 = 0x5e83_d6a1_b7f4_2c09;
const CAUTION_SALT: u64 = 0x2b9c_4f87_e5d1_a364;
const PERSISTENCE_SALT: u64 = 0xf16a_8d3c_50b7_e942;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) struct PendingBirth {
    pub position: (u32, u32),
    pub mother_id: u32,
    pub father_id: u32,
}

pub(super) fn sex_for(seed: u64, id: u32) -> Sex {
    if entity_random(seed, id, SEX_SALT) & 1 == 0 {
        Sex::Female
    } else {
        Sex::Male
    }
}

pub(super) fn lifespan_for(seed: u64, id: u32) -> u64 {
    let minimum = BASE_LIFESPAN_TICKS - LIFESPAN_VARIATION_TICKS;
    let span = LIFESPAN_VARIATION_TICKS * 2 + 1;
    minimum + entity_random(seed, id, LIFESPAN_SALT) % span
}

pub(super) fn founder_age_for(seed: u64, id: u32) -> u64 {
    let span = FOUNDER_AGE_MAX - FOUNDER_AGE_MIN + 1;
    FOUNDER_AGE_MIN + entity_random(seed, id, FOUNDER_AGE_SALT) % span
}

/// Generates the five personality traits for an entity.
pub(super) fn personality_for(seed: u64, id: u32) -> Personality {
    Personality {
        curiosity: trait_value(seed, id, CURIOSITY_SALT),
        sociability: trait_value(seed, id, SOCIABILITY_SALT),
        cooperativeness: trait_value(seed, id, COOPERATION_SALT),
        caution: trait_value(seed, id, CAUTION_SALT),
        persistence: trait_value(seed, id, PERSISTENCE_SALT),
    }
}

/// Converts a deterministic hash to an f32 in [0.0, 1.0].
fn trait_value(seed: u64, id: u32, salt: u64) -> f32 {
    let raw = entity_random(seed, id, salt);
    (raw as f64 / u64::MAX as f64) as f32
}

pub(super) fn female_is_fertile(entity: &Entity, tick: u64, max_health: f32) -> bool {
    let stage = LifeStage::from_age_ticks(entity.age_ticks);

    entity.sex == Sex::Female
        && matches!(stage, LifeStage::Adult | LifeStage::Elder)
        && entity.age_ticks >= REPRODUCTIVE_AGE_START
        && entity.age_ticks < FEMALE_REPRODUCTIVE_AGE_END
        && entity.pregnancy.is_none()
        && tick >= entity.postpartum_until_tick
        && entity.hunger <= REPRODUCTION_MAX_HUNGER
        && entity.health >= max_health * 0.8
}

pub(super) fn male_is_fertile(entity: &Entity, max_health: f32) -> bool {
    let stage = LifeStage::from_age_ticks(entity.age_ticks);

    entity.sex == Sex::Male
        && matches!(stage, LifeStage::Adult | LifeStage::Elder)
        && entity.age_ticks >= REPRODUCTIVE_AGE_START
        && entity.age_ticks < MALE_REPRODUCTIVE_AGE_END
        && entity.hunger <= REPRODUCTION_MAX_HUNGER
        && entity.health >= max_health * 0.8
}

fn relationship_score(female: &Entity, male: &Entity) -> (i16, i32) {
    // Bilateral willingness: each individual contributes only the
    // affinity stored in their own memory. Neither individual gains
    // access to the other's memory.
    let mother = female.mind.memory.affinity_to(male.id).unwrap_or(0);
    let father = male.mind.memory.affinity_to(female.id).unwrap_or(0);

    // Reciprocity first: the weaker side sets the floor.
    // Joint affinity breaks ties between equally reciprocal pairs.
    (mother.min(father), i32::from(mother) + i32::from(father))
}

/// Selects the reproduction partner by, in order:
/// 1. strongest reciprocity (higher minimum of the two affinities),
/// 2. highest total affinity,
/// 3. smallest distance,
/// 4. lowest entity id.
///
/// Both individuals must meet [`REPRODUCTION_MIN_AFFINITY`] in their own
/// memories. Unknown relationships contribute a neutral zero, so the
/// previous closest-male-by-id behavior remains the fallback.
pub(super) fn select_reproduction_partner(
    female: &Entity,
    entities: &[Entity],
    max_health: f32,
) -> Option<u32> {
    let female_position = (female.x, female.y);

    let eligible = |candidate: &&Entity| {
        male_is_fertile(candidate, max_health)
            && manhattan(female_position, (candidate.x, candidate.y)) <= REPRODUCTION_MAX_DISTANCE
            && female.mind.memory.affinity_to(candidate.id).unwrap_or(0)
                >= REPRODUCTION_MIN_AFFINITY
            && candidate.mind.memory.affinity_to(female.id).unwrap_or(0)
                >= REPRODUCTION_MIN_AFFINITY
    };

    if let Some(partner_id) = female.partner_id {
        if let Some(partner) = entities
            .iter()
            .find(|candidate| candidate.id == partner_id)
            .filter(eligible)
        {
            return Some(partner.id);
        }
    }

    entities
        .iter()
        .filter(eligible)
        .max_by_key(|candidate| {
            use std::cmp::Reverse;

            let (mutual_affinity, total_affinity) = relationship_score(female, candidate);

            (
                mutual_affinity,
                total_affinity,
                Reverse(manhattan(female_position, (candidate.x, candidate.y))),
                Reverse(candidate.id),
            )
        })
        .map(|candidate| candidate.id)
}

pub(super) fn try_conceptions(
    entities: &mut [Entity],
    tick: u64,
    seed: u64,
    max_health: f32,
    threshold: u64,
) -> u32 {
    let mut conceptions = 0;
    for female_index in 0..entities.len() {
        if !female_is_fertile(&entities[female_index], tick, max_health) {
            continue;
        }
        let female_id = entities[female_index].id;
        let Some(father_id) =
            select_reproduction_partner(&entities[female_index], entities, max_health)
        else {
            continue;
        };
        if conception_roll(seed, female_id, father_id, tick) >= threshold {
            continue;
        }
        entities[female_index].pregnancy = Some(Pregnancy {
            father_id,
            conceived_tick: tick,
            due_tick: tick.saturating_add(GESTATION_TICKS),
        });
        conceptions += 1;
    }
    conceptions
}

pub(super) fn process_due_pregnancies(
    entities: &mut [Entity],
    world: &Grid,
    tick: u64,
    max_births: usize,
) -> Vec<PendingBirth> {
    let mut occupied: HashSet<_> = entities.iter().map(|entity| (entity.x, entity.y)).collect();
    let mut births = Vec::new();
    for mother in entities.iter_mut() {
        if births.len() >= max_births {
            break;
        }
        let Some(pregnancy) = mother.pregnancy else {
            continue;
        };
        if pregnancy.due_tick > tick {
            continue;
        }
        let Some(position) = adjacent_birth_position(world, (mother.x, mother.y), &occupied) else {
            continue;
        };
        occupied.insert(position);
        mother.pregnancy = None;
        mother.postpartum_until_tick = tick.saturating_add(POSTPARTUM_TICKS);
        births.push(PendingBirth {
            position,
            mother_id: mother.id,
            father_id: pregnancy.father_id,
        });
    }
    births
}

pub(super) fn conception_roll(seed: u64, female_id: u32, male_id: u32, tick: u64) -> u64 {
    let value = seed
        ^ u64::from(female_id).wrapping_mul(0x9e37_79b9_7f4a_7c15)
        ^ u64::from(male_id).wrapping_mul(0xbf58_476d_1ce4_e5b9)
        ^ tick.wrapping_mul(0x94d0_49bb_1331_11eb);
    mix64(value) % DAILY_CONCEPTION_SCALE
}

fn entity_random(seed: u64, id: u32, salt: u64) -> u64 {
    mix64(seed ^ u64::from(id).wrapping_mul(0x9e37_79b9_7f4a_7c15) ^ salt)
}

fn mix64(mut value: u64) -> u64 {
    value ^= value >> 30;
    value = value.wrapping_mul(0xbf58_476d_1ce4_e5b9);
    value ^= value >> 27;
    value = value.wrapping_mul(0x94d0_49bb_1331_11eb);
    value ^ (value >> 31)
}

pub(super) fn adjacent_birth_position(
    world: &Grid,
    origin: (u32, u32),
    occupied: &HashSet<(u32, u32)>,
) -> Option<(u32, u32)> {
    (-1..=1)
        .flat_map(|dy| (-1..=1).map(move |dx| (dx, dy)))
        .filter(|&(dx, dy)| dx != 0 || dy != 0)
        .filter_map(|(dx, dy)| {
            let x = i64::from(origin.0) + i64::from(dx);
            let y = i64::from(origin.1) + i64::from(dy);
            (x >= 0 && y >= 0 && x < i64::from(world.width) && y < i64::from(world.height))
                .then_some((x as u32, y as u32))
        })
        .find(|&(x, y)| {
            !occupied.contains(&(x, y))
                && world
                    .get(x, y)
                    .is_some_and(|tile| tile.terrain.is_walkable())
        })
}

pub(super) fn spawn_candidates(world: &Grid) -> Vec<(u32, u32)> {
    let center = (world.width / 2, world.height / 2);
    let mut food_tiles: Vec<_> = world
        .resources
        .iter()
        .enumerate()
        .filter_map(|(index, deposit)| {
            let deposit = deposit.as_ref()?;
            (deposit.kind == ResourceKind::Food && deposit.amount > 0).then(|| {
                let index = index as u32;
                (index % world.width, index / world.width)
            })
        })
        .collect();
    food_tiles.sort_unstable_by_key(|&coordinate| manhattan(coordinate, center));

    let mut seen = HashSet::new();
    let mut candidates = Vec::new();
    for food in food_tiles {
        for radius in 6..=12i32 {
            for dx in -radius..=radius {
                let dy = radius - dx.abs();
                for signed_dy in [dy, -dy] {
                    let x = i64::from(food.0) + i64::from(dx);
                    let y = i64::from(food.1) + i64::from(signed_dy);
                    if x < 0 || y < 0 || x >= i64::from(world.width) || y >= i64::from(world.height)
                    {
                        continue;
                    }
                    let candidate = (x as u32, y as u32);
                    if seen.insert(candidate)
                        && world
                            .get(candidate.0, candidate.1)
                            .is_some_and(|tile| tile.terrain.is_walkable())
                    {
                        candidates.push(candidate);
                    }
                }
            }
        }
    }

    let mut fallback: Vec<_> = world
        .tiles
        .iter()
        .enumerate()
        .filter(|(_, tile)| tile.terrain.is_walkable())
        .map(|(index, _)| {
            let index = index as u32;
            (index % world.width, index / world.width)
        })
        .collect();
    fallback.sort_unstable_by_key(|&coordinate| manhattan(coordinate, center));
    candidates.extend(
        fallback
            .into_iter()
            .filter(|position| seen.insert(*position)),
    );
    candidates
}

fn manhattan(left: (u32, u32), right: (u32, u32)) -> u32 {
    left.0.abs_diff(right.0) + left.1.abs_diff(right.1)
}
