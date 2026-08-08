use super::Entity;
use crate::world::{Grid, ResourceKind};
use std::collections::HashSet;

pub(super) const ADULT_AGE_TICKS: u64 = 200;
pub(super) const REPRODUCTION_COOLDOWN_TICKS: u32 = 300;
const REPRODUCTION_MAX_HUNGER: f32 = 35.0;
const REPRODUCTION_MAX_DISTANCE: u32 = 2;

pub(super) fn can_reproduce(entity: &Entity, max_health: f32) -> bool {
    entity.age_ticks >= ADULT_AGE_TICKS
        && entity.reproduction_cooldown == 0
        && entity.hunger <= REPRODUCTION_MAX_HUNGER
        && entity.health >= max_health * 0.8
}

pub(super) fn within_reproduction_distance(left: &Entity, right: &Entity) -> bool {
    manhattan((left.x, left.y), (right.x, right.y)) <= REPRODUCTION_MAX_DISTANCE
}

pub(super) fn reproduction_positions(
    entities: &mut [Entity],
    world: &Grid,
    max_health: f32,
) -> Vec<(u32, u32)> {
    if entities.len() < 2 {
        return Vec::new();
    }
    let mut paired = vec![false; entities.len()];
    let mut occupied: HashSet<_> = entities.iter().map(|entity| (entity.x, entity.y)).collect();
    let mut plans = Vec::new();

    for left in 0..entities.len() {
        if paired[left] || !can_reproduce(&entities[left], max_health) {
            continue;
        }
        for right in (left + 1)..entities.len() {
            if paired[right]
                || !can_reproduce(&entities[right], max_health)
                || !within_reproduction_distance(&entities[left], &entities[right])
            {
                continue;
            }
            let parent_position = (entities[left].x, entities[left].y);
            if let Some(position) = adjacent_birth_position(world, parent_position, &occupied) {
                occupied.insert(position);
                paired[left] = true;
                paired[right] = true;
                plans.push((left, right, position));
            }
            break;
        }
    }

    for &(left, right, _) in &plans {
        entities[left].reproduction_cooldown = REPRODUCTION_COOLDOWN_TICKS;
        entities[right].reproduction_cooldown = REPRODUCTION_COOLDOWN_TICKS;
    }
    plans.into_iter().map(|(_, _, position)| position).collect()
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
