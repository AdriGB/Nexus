use super::super::entity::{Entity, EntityActivity};
use super::mind::{chunk_index, Action, Goal, Mind, KNOWLEDGE_CHUNK_SIZE};
use crate::pathfinding::{self, PathfindingWorkspace};
use crate::world::Grid;

pub(super) fn plan_exploration(
    entity: &mut Entity,
    world: &Grid,
    tick: u64,
    pathfinding_workspace: &mut PathfindingWorkspace,
) {
    let origin = (entity.x, entity.y);
    entity.mind.memory.prune_exploration_failures(tick);

    let Some(target) = exploration_target(&entity.mind, world, origin, entity.id, tick) else {
        entity.mind.set_plan(Goal::Rest, vec![Action::Wait], tick);
        entity.activity = EntityActivity::Resting;
        return;
    };
    let Some(path) =
        pathfinding::find_path_with_workspace(pathfinding_workspace, world, origin, target)
    else {
        let failed_chunk = chunk_index(world, target.0, target.1);
        entity
            .mind
            .memory
            .mark_exploration_failed(failed_chunk, tick);
        entity.mind.set_plan(Goal::Rest, vec![Action::Wait], tick);
        entity.activity = EntityActivity::Resting;
        return;
    };
    entity.path = path.into_iter().skip(1).collect();
    entity.path_index = 0;
    if entity.path.is_empty() {
        entity.mind.set_plan(Goal::Rest, vec![Action::Wait], tick);
        entity.activity = EntityActivity::Resting;
    } else {
        entity.mind.set_plan(
            Goal::Explore,
            vec![Action::ExploreArea(target.0, target.1)],
            tick,
        );
        entity.activity = EntityActivity::Exploring;
    }
}

pub fn exploration_target(
    mind: &Mind,
    world: &Grid,
    origin: (u32, u32),
    entity_id: u32,
    tick: u64,
) -> Option<(u32, u32)> {
    let chunks_wide = world.width.div_ceil(KNOWLEDGE_CHUNK_SIZE);
    let chunks_high = world.height.div_ceil(KNOWLEDGE_CHUNK_SIZE);
    let origin_chunk = (
        origin.0 / KNOWLEDGE_CHUNK_SIZE,
        origin.1 / KNOWLEDGE_CHUNK_SIZE,
    );
    let origin_region = world.region_id_at(origin.0, origin.1);
    let max_ring = origin_chunk
        .0
        .max(chunks_wide - 1 - origin_chunk.0)
        .max(origin_chunk.1)
        .max(chunks_high - 1 - origin_chunk.1);

    for ring in 1..=max_ring {
        let mut chunks = ring_perimeter(
            origin_chunk.0 as i32,
            origin_chunk.1 as i32,
            ring as i32,
            chunks_wide as i32,
            chunks_high as i32,
        );
        chunks.sort_unstable_by_key(|&(cx, cy)| {
            let index = cy * chunks_wide + cx;
            index.wrapping_add(entity_id.wrapping_mul(2_654_435_761))
        });
        for (cx, cy) in chunks {
            let candidate_chunk = cy * chunks_wide + cx;
            if mind.memory.exploration_on_cooldown(candidate_chunk, tick) {
                continue;
            }

            let x = (cx * KNOWLEDGE_CHUNK_SIZE + KNOWLEDGE_CHUNK_SIZE / 2).min(world.width - 1);
            let y = (cy * KNOWLEDGE_CHUNK_SIZE + KNOWLEDGE_CHUNK_SIZE / 2).min(world.height - 1);
            if mind.memory.remembers_chunk(world, x, y) {
                continue;
            }

            if let Some(target) = walkable_in_chunk(world, cx, cy, origin_region) {
                return Some(target);
            }
        }
    }

    deterministic_wander_target(mind, world, origin, entity_id, tick, origin_region)
}

fn ring_perimeter(
    center_x: i32,
    center_y: i32,
    ring: i32,
    chunks_wide: i32,
    chunks_high: i32,
) -> Vec<(u32, u32)> {
    debug_assert!(ring > 0);

    let mut chunks = Vec::with_capacity((ring as usize).saturating_mul(8));

    for dx in -ring..=ring {
        for y in [center_y - ring, center_y + ring] {
            let x = center_x + dx;

            if x >= 0 && y >= 0 && x < chunks_wide && y < chunks_high {
                chunks.push((x as u32, y as u32));
            }
        }
    }

    for dy in (-ring + 1)..ring {
        for x in [center_x - ring, center_x + ring] {
            let y = center_y + dy;

            if x >= 0 && y >= 0 && x < chunks_wide && y < chunks_high {
                chunks.push((x as u32, y as u32));
            }
        }
    }

    chunks
}

fn walkable_in_chunk(
    world: &Grid,
    chunk_x: u32,
    chunk_y: u32,
    required_region: Option<u32>,
) -> Option<(u32, u32)> {
    let start_x = chunk_x * KNOWLEDGE_CHUNK_SIZE;
    let start_y = chunk_y * KNOWLEDGE_CHUNK_SIZE;
    let end_x = (start_x + KNOWLEDGE_CHUNK_SIZE).min(world.width);
    let end_y = (start_y + KNOWLEDGE_CHUNK_SIZE).min(world.height);
    (start_y..end_y)
        .flat_map(|y| (start_x..end_x).map(move |x| (x, y)))
        .find(|&(x, y)| {
            let walkable = world
                .get(x, y)
                .is_some_and(|tile| tile.terrain.is_walkable());

            if !walkable {
                return false;
            }

            match required_region {
                Some(region_id) => world.region_id_at(x, y) == Some(region_id),
                None => true,
            }
        })
}

fn deterministic_wander_target(
    mind: &Mind,
    world: &Grid,
    origin: (u32, u32),
    entity_id: u32,
    tick: u64,
    required_region: Option<u32>,
) -> Option<(u32, u32)> {
    let offsets = [
        (8i32, 0i32),
        (0, 8),
        (-8, 0),
        (0, -8),
        (6, 6),
        (-6, 6),
        (6, -6),
        (-6, -6),
    ];
    let start = entity_id as usize % offsets.len();
    offsets
        .iter()
        .cycle()
        .skip(start)
        .take(offsets.len())
        .filter_map(|&(dx, dy)| {
            let x = i64::from(origin.0) + i64::from(dx);
            let y = i64::from(origin.1) + i64::from(dy);
            (x >= 0 && y >= 0 && x < i64::from(world.width) && y < i64::from(world.height))
                .then_some((x as u32, y as u32))
        })
        .find(|&(x, y)| {
            let walkable = world
                .get(x, y)
                .is_some_and(|tile| tile.terrain.is_walkable());
            let in_required_region = match required_region {
                Some(region_id) => world.region_id_at(x, y) == Some(region_id),
                None => true,
            };
            let target_chunk = chunk_index(world, x, y);

            walkable
                && in_required_region
                && !mind.memory.exploration_on_cooldown(target_chunk, tick)
        })
}

#[cfg(test)]
mod tests {
    use super::super::mind::{chunk_index, Mind, FAILED_EXPLORATION_RETRY_TICKS};
    use super::*;
    use crate::world::{Terrain, Tile};

    fn plain_grid(width: u32, height: u32) -> Grid {
        Grid {
            width,
            height,
            tiles: (0..width * height)
                .map(|_| Tile {
                    terrain: Terrain::Plains,
                    altitude: 0.0,
                    moisture: 0.5,
                    temperature: 0.5,
                })
                .collect(),
            region_ids: Vec::new(),
            regions: Vec::new(),
            resources: vec![None; (width * height) as usize],
            renewable_resources: Vec::new(),
        }
    }

    #[test]
    fn ring_perimeter_contains_exactly_eight_r_tiles_when_unclipped() {
        let ring = ring_perimeter(5, 5, 3, 20, 20);

        assert_eq!(ring.len(), 24);
        let unique: std::collections::HashSet<_> = ring.iter().copied().collect();
        assert_eq!(unique.len(), ring.len());
    }

    #[test]
    fn ring_perimeter_clips_to_world_without_duplicates() {
        let ring = ring_perimeter(0, 0, 3, 10, 10);

        assert!(ring.iter().all(|&(x, y)| x < 10 && y < 10));
        let unique: std::collections::HashSet<_> = ring.iter().copied().collect();
        assert_eq!(unique.len(), ring.len());
    }

    #[test]
    fn failed_exploration_chunk_is_skipped_until_retry_tick() {
        let world = plain_grid(32, 32);
        let mut mind = Mind::default();
        let origin = (12, 12);
        let entity_id = 1;
        let first = exploration_target(&mind, &world, origin, entity_id, 0).unwrap();
        let failed_chunk = chunk_index(&world, first.0, first.1);

        mind.memory.mark_exploration_failed(failed_chunk, 0);
        mind.memory.mark_exploration_failed(failed_chunk, 0);
        assert_eq!(mind.memory.failed_exploration_count(), 1);

        let during_cooldown = exploration_target(&mind, &world, origin, entity_id, 1).unwrap();
        let after_cooldown = exploration_target(
            &mind,
            &world,
            origin,
            entity_id,
            FAILED_EXPLORATION_RETRY_TICKS,
        )
        .unwrap();

        assert_ne!(
            chunk_index(&world, during_cooldown.0, during_cooldown.1),
            failed_chunk
        );
        assert_eq!(
            chunk_index(&world, after_cooldown.0, after_cooldown.1),
            failed_chunk
        );
    }
}
