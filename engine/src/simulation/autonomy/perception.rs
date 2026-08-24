use super::super::spatial::{EntitySnapshot, SpatialGrid};
use super::mind::{manhattan, KnownEntity, KnownResource, Mind, KNOWLEDGE_CHUNK_SIZE};
use crate::world::{Grid, ResourceKind};

const RESOURCE_MEMORY_TTL: u64 = 2_000;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(in crate::simulation) struct ResourceDiscovery {
    pub entity_id: u32,
    pub x: u32,
    pub y: u32,
    pub kind: ResourceKind,
    pub amount: u16,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(in crate::simulation) struct EntityEncounter {
    pub observer_id: u32,
    pub other_id: u32,
    pub x: u32,
    pub y: u32,
}

fn resource_key(x: u32, y: u32, kind: ResourceKind) -> (u32, u32, u8) {
    (y, x, kind as u8)
}

fn remember_resource(
    mind: &mut Mind,
    x: u32,
    y: u32,
    kind: ResourceKind,
    amount: u16,
    tick: u64,
) -> bool {
    let key = resource_key(x, y, kind);

    match mind
        .memory
        .known_resources
        .binary_search_by_key(&key, |known| resource_key(known.x, known.y, known.kind))
    {
        Ok(index) => {
            let known = &mut mind.memory.known_resources[index];
            known.last_seen_tick = tick;
            known.estimated_amount = amount;
            known.failed_attempts = 0;
            known.avoid_until_tick = 0;
            false
        }
        Err(index) => {
            mind.memory.known_resources.insert(
                index,
                KnownResource {
                    x,
                    y,
                    kind,
                    last_seen_tick: tick,
                    estimated_amount: amount,
                    failed_attempts: 0,
                    avoid_until_tick: 0,
                },
            );
            true
        }
    }
}

fn remember_visible_chunks(mind: &mut Mind, world: &Grid, position: (u32, u32)) {
    let radius = mind.perception_radius;

    let min_x = position.0.saturating_sub(radius);
    let max_x = position.0.saturating_add(radius).min(world.width - 1);
    let min_y = position.1.saturating_sub(radius);
    let max_y = position.1.saturating_add(radius).min(world.height - 1);

    let min_chunk_x = min_x / KNOWLEDGE_CHUNK_SIZE;
    let max_chunk_x = max_x / KNOWLEDGE_CHUNK_SIZE;
    let min_chunk_y = min_y / KNOWLEDGE_CHUNK_SIZE;
    let max_chunk_y = max_y / KNOWLEDGE_CHUNK_SIZE;

    let chunks_wide = world.width.div_ceil(KNOWLEDGE_CHUNK_SIZE);

    for chunk_y in min_chunk_y..=max_chunk_y {
        for chunk_x in min_chunk_x..=max_chunk_x {
            let chunk_min_x = chunk_x * KNOWLEDGE_CHUNK_SIZE;
            let chunk_min_y = chunk_y * KNOWLEDGE_CHUNK_SIZE;
            let chunk_max_x = (chunk_min_x + KNOWLEDGE_CHUNK_SIZE - 1).min(world.width - 1);
            let chunk_max_y = (chunk_min_y + KNOWLEDGE_CHUNK_SIZE - 1).min(world.height - 1);

            let nearest_x = position.0.clamp(chunk_min_x, chunk_max_x);
            let nearest_y = position.1.clamp(chunk_min_y, chunk_max_y);

            if manhattan(position, (nearest_x, nearest_y)) <= radius {
                let index = chunk_y * chunks_wide + chunk_x;
                mind.memory.known_chunks.insert(index);
            }
        }
    }
}

fn remember_entity(mind: &mut Mind, other: EntitySnapshot, tick: u64) -> bool {
    match mind
        .memory
        .known_entities
        .binary_search_by_key(&other.id, |known| known.id)
    {
        Ok(index) => {
            let known = &mut mind.memory.known_entities[index];
            known.last_seen_tick = tick;
            known.last_seen_x = other.x;
            known.last_seen_y = other.y;
            known.observed_ticks = known.observed_ticks.saturating_add(1);
            known.clear_seek_cooldown();
            false
        }
        Err(index) => {
            mind.memory.known_entities.insert(
                index,
                KnownEntity {
                    id: other.id,
                    first_seen_tick: tick,
                    last_seen_tick: tick,
                    last_seen_x: other.x,
                    last_seen_y: other.y,
                    observed_ticks: 1,
                    affinity: super::mind::NEUTRAL_AFFINITY,
                    last_interaction_tick: 0,
                    interaction_count: 0,
                    seek_retry_after_tick: None,
                },
            );
            true
        }
    }
}

pub fn perceive(
    mind: &mut Mind,
    entity_id: u32,
    world: &Grid,
    position: (u32, u32),
    tick: u64,
) -> Vec<ResourceDiscovery> {
    reconcile_resource_memory(mind, world, position, tick);
    scan_visible_resources(mind, entity_id, world, position, tick).1
}

pub(super) fn reconcile_resource_memory(
    mind: &mut Mind,
    world: &Grid,
    position: (u32, u32),
    tick: u64,
) {
    mind.memory
        .known_resources
        .retain(|known| tick.saturating_sub(known.last_seen_tick) <= RESOURCE_MEMORY_TTL);

    let radius = mind.perception_radius;
    let min_y = position.1.saturating_sub(radius);
    let max_y = position
        .1
        .saturating_add(radius)
        .min(world.height.saturating_sub(1));

    let start = mind
        .memory
        .known_resources
        .partition_point(|known| known.y < min_y);
    let end = mind
        .memory
        .known_resources
        .partition_point(|known| known.y <= max_y);

    let mut depleted = Vec::new();

    for index in start..end {
        let known = mind.memory.known_resources[index];

        if manhattan(position, (known.x, known.y)) > radius {
            continue;
        }

        let world_index = (known.y * world.width + known.x) as usize;

        if world.resources.get(world_index).is_none_or(Option::is_none) {
            depleted.push(index);
        }
    }

    for index in depleted.into_iter().rev() {
        mind.memory.known_resources.remove(index);
    }
}

pub(super) fn scan_visible_resources(
    mind: &mut Mind,
    entity_id: u32,
    world: &Grid,
    position: (u32, u32),
    tick: u64,
) -> (u32, Vec<ResourceDiscovery>) {
    let radius = mind.perception_radius as i64;
    let min_x = (i64::from(position.0) - radius).max(0) as u32;
    let max_x = (i64::from(position.0) + radius).min(i64::from(world.width) - 1) as u32;
    let min_y = (i64::from(position.1) - radius).max(0) as u32;
    let max_y = (i64::from(position.1) + radius).min(i64::from(world.height) - 1) as u32;

    remember_visible_chunks(mind, world, position);

    let mut visible_count = 0u32;
    let mut discoveries = Vec::new();
    for y in min_y..=max_y {
        for x in min_x..=max_x {
            if manhattan(position, (x, y)) > mind.perception_radius {
                continue;
            }
            let deposit = world.resources[(y * world.width + x) as usize];
            if let Some(deposit) = deposit {
                if remember_resource(mind, x, y, deposit.kind, deposit.amount, tick) {
                    discoveries.push(ResourceDiscovery {
                        entity_id,
                        x,
                        y,
                        kind: deposit.kind,
                        amount: deposit.amount,
                    });
                }
                visible_count += 1;
            }
        }
    }
    (visible_count, discoveries)
}

pub(super) fn perceive_entities(
    mind: &mut Mind,
    entity_id: u32,
    position: (u32, u32),
    tick: u64,
    population: &[EntitySnapshot],
    spatial_grid: &SpatialGrid,
) -> Vec<EntityEncounter> {
    mind.visible_entities.clear();
    let mut encounters = Vec::new();

    spatial_grid.visit_candidates(
        position.0,
        position.1,
        mind.perception_radius,
        |snapshot_index| {
            let other = population[snapshot_index];

            if other.id == entity_id {
                return;
            }

            if manhattan(position, (other.x, other.y)) <= mind.perception_radius {
                mind.visible_entities.push(other.id);
                if remember_entity(mind, other, tick) {
                    encounters.push(EntityEncounter {
                        observer_id: entity_id,
                        other_id: other.id,
                        x: position.0,
                        y: position.1,
                    });
                }
            }
        },
    );

    mind.visible_entities.sort_unstable();
    encounters
}

#[cfg(test)]
mod tests {
    use super::super::mind::{chunk_index, manhattan, Mind, NEUTRAL_AFFINITY};
    use super::*;
    use crate::world::{ResourceDeposit, ResourceKind, Terrain, Tile};

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
    fn seeing_entity_adds_it_to_memory() {
        let mut mind = Mind::default();
        let snapshot = EntitySnapshot {
            id: 5,
            x: 10,
            y: 20,
            hunger: 0.0,
            caregiver_id: None,
            is_child: false,
            is_infant: false,
        };
        remember_entity(&mut mind, snapshot, 100);

        assert_eq!(mind.memory.known_entities.len(), 1);
        let known = &mind.memory.known_entities[0];
        assert_eq!(known.id, 5);
        assert_eq!(known.first_seen_tick, 100);
        assert_eq!(known.last_seen_tick, 100);
        assert_eq!(known.last_seen_x, 10);
        assert_eq!(known.last_seen_y, 20);
        assert_eq!(known.observed_ticks, 1);
        assert_eq!(known.affinity, NEUTRAL_AFFINITY);
        assert_eq!(known.last_interaction_tick, 0);
        assert_eq!(known.interaction_count, 0);
    }

    #[test]
    fn seeing_same_entity_updates_existing_memory() {
        let mut mind = Mind::default();
        let snapshot_a = EntitySnapshot {
            id: 5,
            x: 10,
            y: 20,
            hunger: 0.0,
            caregiver_id: None,
            is_child: false,
            is_infant: false,
        };
        remember_entity(&mut mind, snapshot_a, 100);

        let snapshot_b = EntitySnapshot {
            id: 5,
            x: 15,
            y: 25,
            hunger: 0.0,
            caregiver_id: None,
            is_child: false,
            is_infant: false,
        };
        remember_entity(&mut mind, snapshot_b, 200);

        assert_eq!(mind.memory.known_entities.len(), 1);
        let known = &mind.memory.known_entities[0];
        assert_eq!(known.first_seen_tick, 100);
        assert_eq!(known.last_seen_tick, 200);
        assert_eq!(known.last_seen_x, 15);
        assert_eq!(known.last_seen_y, 25);
        assert_eq!(known.observed_ticks, 2);
    }

    #[test]
    fn seeing_entity_again_preserves_affinity() {
        let mut mind = Mind::default();

        let snapshot = EntitySnapshot {
            id: 3,
            x: 10,
            y: 20,
            hunger: 0.0,
            caregiver_id: None,
            is_child: false,
            is_infant: false,
        };
        remember_entity(&mut mind, snapshot, 100);

        assert_eq!(mind.memory.affinity_to(3), Some(NEUTRAL_AFFINITY));
        assert!(mind.memory.adjust_affinity(3, 300));
        assert_eq!(mind.memory.affinity_to(3), Some(300));

        let snapshot_again = EntitySnapshot {
            id: 3,
            x: 12,
            y: 22,
            hunger: 0.0,
            caregiver_id: None,
            is_child: false,
            is_infant: false,
        };
        remember_entity(&mut mind, snapshot_again, 200);

        assert_eq!(mind.memory.affinity_to(3), Some(300));
        assert_eq!(mind.memory.known_entities.len(), 1);
        assert_eq!(mind.memory.known_entities[0].observed_ticks, 2);
    }

    #[test]
    fn known_entities_remain_sorted_by_id() {
        let mut mind = Mind::default();

        for id in [30, 10, 50, 20, 40] {
            let snapshot = EntitySnapshot {
                id,
                x: 0,
                y: 0,
                hunger: 0.0,
                caregiver_id: None,
                is_child: false,
                is_infant: false,
            };
            remember_entity(&mut mind, snapshot, 0);
        }

        let ids: Vec<u32> = mind
            .memory
            .known_entities
            .iter()
            .map(|known| known.id)
            .collect();
        assert_eq!(ids, vec![10, 20, 30, 40, 50]);
    }

    #[test]
    fn batched_visible_chunks_match_tile_by_tile_reference() {
        let world = plain_grid(32, 32);

        for position in [(1, 1), (7, 7), (8, 8), (15, 15), (30, 30)] {
            let mut mind = Mind::default();
            let radius = mind.perception_radius;
            let mut expected = std::collections::HashSet::new();

            for y in 0..world.height {
                for x in 0..world.width {
                    if manhattan(position, (x, y)) <= radius {
                        expected.insert(chunk_index(&world, x, y));
                    }
                }
            }

            remember_visible_chunks(&mut mind, &world, position);

            assert_eq!(
                mind.memory.known_chunks, expected,
                "mismatch at position {:?}",
                position,
            );
        }
    }

    #[test]
    fn perception_forgets_visible_depleted_resource() {
        let mut world = plain_grid(16, 16);
        let position = (5, 5);
        let index = (position.1 * world.width + position.0) as usize;

        world.resources[index] = Some(ResourceDeposit {
            kind: ResourceKind::Food,
            amount: 100,
        });

        let mut mind = Mind::default();
        perceive(&mut mind, 1, &world, position, 10);

        assert!(mind
            .memory
            .known_resources
            .iter()
            .any(|known| (known.x, known.y) == position));

        world.resources[index] = None;
        perceive(&mut mind, 1, &world, position, 11);

        assert!(!mind
            .memory
            .known_resources
            .iter()
            .any(|known| (known.x, known.y) == position));
    }

    #[test]
    fn perception_reports_only_new_resource_memories() {
        let mut world = plain_grid(8, 8);
        let position = (3, 3);
        let index = (position.1 * world.width + position.0) as usize;
        world.resources[index] = Some(ResourceDeposit {
            kind: ResourceKind::Timber,
            amount: 42,
        });
        let mut mind = Mind::default();

        let first = perceive(&mut mind, 7, &world, position, 10);
        let refreshed = perceive(&mut mind, 7, &world, position, 11);

        assert_eq!(
            first,
            vec![ResourceDiscovery {
                entity_id: 7,
                x: 3,
                y: 3,
                kind: ResourceKind::Timber,
                amount: 42,
            }]
        );
        assert!(refreshed.is_empty());
    }

    #[test]
    fn perception_keeps_resource_outside_current_view() {
        let mut world = plain_grid(32, 32);
        let resource_position = (20, 20);
        let index = (resource_position.1 * world.width + resource_position.0) as usize;

        world.resources[index] = Some(ResourceDeposit {
            kind: ResourceKind::Food,
            amount: 100,
        });

        let mut mind = Mind::default();
        perceive(&mut mind, 1, &world, resource_position, 10);

        world.resources[index] = None;
        perceive(&mut mind, 1, &world, (5, 5), 11);

        assert!(mind
            .memory
            .known_resources
            .iter()
            .any(|known| (known.x, known.y) == resource_position));
    }

    #[test]
    fn perception_refreshes_visible_resource() {
        let mut world = plain_grid(16, 16);
        let position = (5, 5);
        let index = (position.1 * world.width + position.0) as usize;

        world.resources[index] = Some(ResourceDeposit {
            kind: ResourceKind::Food,
            amount: 100,
        });

        let mut mind = Mind::default();
        perceive(&mut mind, 1, &world, position, 10);

        world.resources[index] = Some(ResourceDeposit {
            kind: ResourceKind::Food,
            amount: 40,
        });
        perceive(&mut mind, 1, &world, position, 25);

        let known = mind
            .memory
            .known_resources
            .iter()
            .find(|known| {
                (known.x, known.y, known.kind) == (position.0, position.1, ResourceKind::Food)
            })
            .unwrap();

        assert_eq!(known.estimated_amount, 40);
        assert_eq!(known.last_seen_tick, 25);
    }

    #[test]
    fn remember_resource_keeps_memory_sorted_and_updates_existing() {
        let mut mind = Mind::default();

        remember_resource(&mut mind, 20, 10, ResourceKind::Food, 50, 1);
        remember_resource(&mut mind, 2, 3, ResourceKind::Stone, 30, 1);
        remember_resource(&mut mind, 8, 3, ResourceKind::Timber, 40, 1);

        let keys: Vec<_> = mind
            .memory
            .known_resources
            .iter()
            .map(|known| resource_key(known.x, known.y, known.kind))
            .collect();

        assert!(keys.windows(2).all(|pair| pair[0] < pair[1]));

        remember_resource(&mut mind, 2, 3, ResourceKind::Stone, 99, 42);

        assert_eq!(mind.memory.known_resources.len(), 3);

        let known = mind
            .memory
            .known_resources
            .iter()
            .find(|known| known.x == 2 && known.y == 3 && known.kind == ResourceKind::Stone)
            .unwrap();

        assert_eq!(known.estimated_amount, 99);
        assert_eq!(known.last_seen_tick, 42);
    }
}
