use serde::Serialize;

use crate::simulation::{
    self, AutonomyProfile, Entity, LifeStage, Personality, PhaseProfile, PopulationStats,
    Simulation, SimulationEvent, SimulationEventCause, SimulationEventDetails, SimulationEventKind,
};
use crate::world::{Grid, RegionKind};

fn to_json<T: Serialize>(value: &T) -> String {
    serde_json::to_string(value).expect("bridge DTO serialization should not fail")
}

#[derive(Serialize)]
struct PhaseProfileDto {
    physiology_us: u64,
    population_index_us: u64,
    autonomy_us: u64,
    starvation_us: u64,
    resource_changes_us: u64,
    remove_dead_us: u64,
    pregnancies_us: u64,
    conceptions_us: u64,
    total_us: u64,
}

pub(crate) fn phase_profile_json(profile: &PhaseProfile) -> String {
    to_json(&PhaseProfileDto {
        physiology_us: profile.physiology_us,
        population_index_us: profile.population_index_us,
        autonomy_us: profile.autonomy_us,
        starvation_us: profile.starvation_us,
        resource_changes_us: profile.resource_changes_us,
        remove_dead_us: profile.remove_dead_us,
        pregnancies_us: profile.pregnancies_us,
        conceptions_us: profile.conceptions_us,
        total_us: profile.total_us,
    })
}

#[derive(Serialize)]
struct AutonomyProfileDto {
    resource_perception_us: u64,
    entity_perception_us: u64,
    plan_validation_us: u64,
    planning_us: u64,
    action_us: u64,
    sampled_entities: u32,
    planned_entities: u32,
    urgent_interrupts: u32,
    memory_reconciliation_us: u64,
    visible_scan_us: u64,
    known_resources_total: u32,
    known_resources_max: u32,
    visible_resources_seen: u32,
    social_us: u64,
}

pub(crate) fn autonomy_profile_json(profile: &AutonomyProfile) -> String {
    to_json(&AutonomyProfileDto {
        resource_perception_us: profile.resource_perception_us,
        entity_perception_us: profile.entity_perception_us,
        plan_validation_us: profile.plan_validation_us,
        planning_us: profile.planning_us,
        action_us: profile.action_us,
        sampled_entities: profile.sampled_entities,
        planned_entities: profile.planned_entities,
        urgent_interrupts: profile.urgent_interrupts,
        memory_reconciliation_us: profile.memory_reconciliation_us,
        visible_scan_us: profile.visible_scan_us,
        known_resources_total: profile.known_resources_total,
        known_resources_max: profile.known_resources_max,
        visible_resources_seen: profile.visible_resources_seen,
        social_us: profile.social_us,
    })
}

#[derive(Serialize)]
struct PopulationStatsDto {
    population: u32,
    births: u64,
    deaths: u64,
    females: u32,
    males: u32,
    pregnant: u32,
    hungry: u32,
    seeking_food: u32,
    average_hunger: f32,
    food_consumed: u64,
}

pub(crate) fn population_stats_json(stats: PopulationStats) -> String {
    to_json(&PopulationStatsDto {
        population: stats.population,
        births: stats.births,
        deaths: stats.deaths,
        females: stats.females,
        males: stats.males,
        pregnant: stats.pregnant,
        hungry: stats.hungry,
        seeking_food: stats.seeking_food,
        average_hunger: stats.average_hunger,
        food_consumed: stats.food_consumed,
    })
}

#[derive(Serialize)]
struct UtilityScoresDto {
    eat: f32,
    explore: f32,
    rest: f32,
}

#[derive(Serialize)]
struct PersonalityDto {
    curiosity: f32,
    sociability: f32,
    cooperativeness: f32,
    caution: f32,
    persistence: f32,
}

impl From<Personality> for PersonalityDto {
    fn from(personality: Personality) -> Self {
        Self {
            curiosity: personality.curiosity,
            sociability: personality.sociability,
            cooperativeness: personality.cooperativeness,
            caution: personality.caution,
            persistence: personality.persistence,
        }
    }
}

#[derive(Serialize)]
struct EntityInfoDto {
    id: u32,
    x: u32,
    y: u32,
    sex: &'static str,
    hunger: f32,
    health: f32,
    age_ticks: u64,
    age_years: f64,
    lifespan_ticks: u64,
    pregnant: bool,
    pregnancy_due_tick: Option<u64>,
    activity: &'static str,
    remaining_path: usize,
    goal: &'static str,
    action: &'static str,
    goal_age_ticks: u64,
    known_resources: usize,
    known_entities: usize,
    known_chunks: usize,
    visible_entities: usize,
    utilities: UtilityScoresDto,
    movement_credit: f32,
    life_stage: &'static str,
    stage_movement_factor: f32,
    caregiver_id: Option<u32>,
    personality: PersonalityDto,
}

pub(crate) fn entity_info_json(entity: &Entity, tick: u64) -> String {
    let goal = entity
        .mind
        .current_goal
        .map_or("None", simulation::Goal::label);
    let action = entity
        .mind
        .current_action()
        .map_or("None", simulation::Action::label);
    let goal_age_ticks = entity
        .mind
        .current_goal
        .map_or(0, |_| tick.saturating_sub(entity.mind.goal_since_tick));
    let life_stage = LifeStage::from_age_ticks(entity.age_ticks);

    to_json(&EntityInfoDto {
        id: entity.id,
        x: entity.x,
        y: entity.y,
        sex: entity.sex.label(),
        hunger: entity.hunger,
        health: entity.health,
        age_ticks: entity.age_ticks,
        age_years: simulation::years_from_ticks(entity.age_ticks),
        lifespan_ticks: entity.lifespan_ticks,
        pregnant: entity.pregnancy.is_some(),
        pregnancy_due_tick: entity.pregnancy.map(|pregnancy| pregnancy.due_tick),
        activity: entity.activity.label(),
        remaining_path: entity.remaining_path_len(),
        goal,
        action,
        goal_age_ticks,
        known_resources: entity.mind.memory.known_resources.len(),
        known_entities: entity.mind.memory.known_entities.len(),
        known_chunks: entity.mind.memory.known_chunk_count(),
        visible_entities: entity.mind.visible_entities.len(),
        utilities: UtilityScoresDto {
            eat: entity.mind.utility_scores.eat,
            explore: entity.mind.utility_scores.explore,
            rest: entity.mind.utility_scores.rest,
        },
        movement_credit: entity.movement_credit,
        life_stage: life_stage.label(),
        stage_movement_factor: life_stage.movement_factor(),
        caregiver_id: entity.caregiver_id,
        personality: entity.personality.into(),
    })
}

#[derive(Serialize)]
struct KnownRelationshipInfoDto {
    id: u32,
    affinity: i16,
    interaction_count: u32,
    first_seen_tick: u64,
    last_seen_tick: u64,
    last_interaction_tick: u64,
    last_seen_x: u32,
    last_seen_y: u32,
    observed_ticks: u32,
    seek_retry_after_tick: Option<u64>,
}

/// Serializes known relationships ordered by emotional intensity:
/// absolute affinity descending, interaction count descending, then id.
/// Raw ticks let the frontend derive human-readable relative times.
pub(crate) fn entity_relationships_json(entity: &Entity) -> String {
    let mut relationships: Vec<KnownRelationshipInfoDto> = entity
        .mind
        .memory
        .known_entities
        .iter()
        .map(|known| KnownRelationshipInfoDto {
            id: known.id,
            affinity: known.affinity,
            interaction_count: known.interaction_count,
            first_seen_tick: known.first_seen_tick,
            last_seen_tick: known.last_seen_tick,
            last_interaction_tick: known.last_interaction_tick,
            last_seen_x: known.last_seen_x,
            last_seen_y: known.last_seen_y,
            observed_ticks: known.observed_ticks,
            seek_retry_after_tick: known.seek_retry_after_tick,
        })
        .collect();

    relationships.sort_unstable_by_key(|relationship| {
        use std::cmp::Reverse;

        (
            Reverse(relationship.affinity.unsigned_abs()),
            Reverse(relationship.interaction_count),
            relationship.id,
        )
    });

    to_json(&relationships)
}

#[derive(Serialize)]
struct EventLocationDto {
    x: u32,
    y: u32,
}

#[derive(Serialize)]
struct InteractionEventDto {
    id: String,
    tick: String,
    relative_time: String,
    location: EventLocationDto,
    actor_id: u32,
    target_id: u32,
    related_entity_ids: Vec<u32>,
    kind: &'static str,
    cause: &'static str,
    actor_affinity_delta: i16,
    target_affinity_delta: i16,
}

fn relative_event_time(current_tick: u64, event_tick: u64) -> String {
    let elapsed = current_tick.saturating_sub(event_tick);
    if elapsed == 0 {
        "just now".to_string()
    } else if elapsed < 24 {
        format!("{elapsed}h ago")
    } else {
        let days = elapsed / 24;
        if days < 365 {
            format!("{days}d ago")
        } else {
            format!("{}y ago", days / 365)
        }
    }
}

fn interaction_events_json<'a>(
    events: impl DoubleEndedIterator<Item = &'a SimulationEvent>,
    current_tick: u64,
    entity_id: Option<u32>,
) -> String {
    let interactions: Vec<InteractionEventDto> = events
        .rev()
        .filter(|event| {
            entity_id.is_none_or(|id| {
                event.actor_id == id
                    || event.target_id == Some(id)
                    || event.related_entity_ids.contains(&id)
            })
        })
        .filter_map(|event| {
            if event.kind != SimulationEventKind::Interaction
                || event.cause != SimulationEventCause::MutualSocialContact
            {
                return None;
            }
            let target_id = event.target_id?;
            let (actor_affinity_delta, target_affinity_delta) = match event.details {
                SimulationEventDetails::Interaction {
                    actor_affinity_delta,
                    target_affinity_delta,
                } => (actor_affinity_delta, target_affinity_delta),
            };

            Some(InteractionEventDto {
                id: event.id.to_string(),
                tick: event.tick.to_string(),
                relative_time: relative_event_time(current_tick, event.tick),
                location: EventLocationDto {
                    x: event.location.x,
                    y: event.location.y,
                },
                actor_id: event.actor_id,
                target_id,
                related_entity_ids: event.related_entity_ids.clone(),
                kind: "interaction",
                cause: "mutual_social_contact",
                actor_affinity_delta,
                target_affinity_delta,
            })
        })
        .collect();

    to_json(&interactions)
}

/// Serializes the existing bounded event history without mutating simulation state.
/// Filtering happens only when this bridge query is requested.
pub(crate) fn recent_interaction_events_json(
    simulation: &Simulation,
    entity_id: Option<u32>,
) -> String {
    interaction_events_json(simulation.recent_events(), simulation.tick(), entity_id)
}

#[derive(Serialize)]
struct ResourceInfoDto {
    kind: &'static str,
    amount: u16,
}

#[derive(Serialize)]
struct TileInfoDto {
    terrain: &'static str,
    altitude: f64,
    moisture: f64,
    temperature: f64,
    x: u32,
    y: u32,
    region_id: u32,
    region_type: &'static str,
    region_area: u32,
    coastal: bool,
    walkable: bool,
    movement_cost: Option<f32>,
    resource: Option<ResourceInfoDto>,
}

pub(crate) fn tile_info_json(grid: &Grid, x: u32, y: u32) -> String {
    let Some(tile) = grid.get(x, y) else {
        return "{}".to_string();
    };

    let index = (y * grid.width + x) as usize;
    let region_id = grid.region_ids.get(index).copied().unwrap_or(u32::MAX);
    let (region_type, region_area) = if let Some(region) = grid.regions.get(region_id as usize) {
        (
            match region.kind {
                RegionKind::Land => "Land",
                RegionKind::Water => "Water",
            },
            region.tile_count,
        )
    } else {
        ("Unknown", 0)
    };
    let resource = grid
        .resources
        .get(index)
        .and_then(Option::as_ref)
        .map(|deposit| ResourceInfoDto {
            kind: deposit.kind.label(),
            amount: deposit.amount,
        });

    to_json(&TileInfoDto {
        terrain: tile.terrain.label(),
        altitude: tile.altitude,
        moisture: tile.moisture,
        temperature: tile.temperature,
        x,
        y,
        region_id,
        region_type,
        region_area,
        coastal: grid.is_coastal(x, y),
        walkable: tile.terrain.is_walkable(),
        movement_cost: tile.terrain.movement_cost(),
        resource,
    })
}

#[derive(Serialize)]
struct RegionStatsDto {
    land_regions: usize,
    water_regions: usize,
    land_tiles: u32,
    water_tiles: u32,
    total_tiles: u32,
    land_coverage: f64,
    largest_landmass_pct: f64,
    islands: usize,
}

pub(crate) fn region_stats_json(grid: &Grid) -> String {
    let total = (grid.width * grid.height) as f64;
    let land: Vec<_> = grid
        .regions
        .iter()
        .filter(|region| region.kind == RegionKind::Land)
        .collect();
    let water_regions = grid
        .regions
        .iter()
        .filter(|region| region.kind == RegionKind::Water)
        .count();
    let land_tiles = land.iter().map(|region| region.tile_count).sum();
    let water_tiles = total as u32 - land_tiles;
    let largest = land
        .iter()
        .map(|region| region.tile_count)
        .max()
        .unwrap_or(0);
    let islands = land.iter().filter(|region| !region.touches_border).count();

    to_json(&RegionStatsDto {
        land_regions: land.len(),
        water_regions,
        land_tiles,
        water_tiles,
        total_tiles: total as u32,
        land_coverage: land_tiles as f64 / total,
        largest_landmass_pct: largest as f64 / total,
        islands,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::simulation::Simulation;
    use crate::world::{Region, ResourceDeposit, ResourceKind, Terrain, Tile};

    fn test_grid() -> Grid {
        Grid {
            width: 1,
            height: 1,
            tiles: vec![Tile {
                terrain: Terrain::Plains,
                altitude: 0.25,
                moisture: 0.5,
                temperature: 0.75,
            }],
            region_ids: vec![0],
            regions: vec![Region {
                kind: RegionKind::Land,
                tile_count: 1,
                min_x: 0,
                min_y: 0,
                max_x: 0,
                max_y: 0,
                touches_border: false,
            }],
            resources: vec![Some(ResourceDeposit {
                kind: ResourceKind::Food,
                amount: 20,
            })],
        }
    }

    fn payloads() -> (String, String, String, String) {
        let grid = test_grid();
        let simulation = Simulation::with_population(42, &grid, 1);
        let population = population_stats_json(simulation.population_stats());
        let entity = entity_info_json(&simulation.entities()[0], simulation.tick());
        let tile = tile_info_json(&grid, 0, 0);
        let region = region_stats_json(&grid);
        (population, entity, tile, region)
    }

    #[test]
    fn bridge_payloads_are_valid_json() {
        let (population, entity, tile, region) = payloads();

        for payload in [population, entity, tile, region] {
            let _: serde_json::Value = serde_json::from_str(&payload).unwrap();
        }
    }

    #[test]
    fn absent_pregnancy_serializes_due_tick_as_null() {
        let (_, entity, _, _) = payloads();
        let json: serde_json::Value = serde_json::from_str(&entity).unwrap();

        assert_eq!(json["pregnancy_due_tick"], serde_json::Value::Null);
    }

    #[test]
    fn relationships_json_of_empty_memory_serializes_as_empty_array() {
        let grid = test_grid();
        let simulation = Simulation::with_population(42, &grid, 1);

        assert_eq!(
            super::entity_relationships_json(&simulation.entities()[0]),
            "[]"
        );
    }

    fn interaction_event(
        id: u64,
        tick: u64,
        actor_id: u32,
        target_id: u32,
        actor_affinity_delta: i16,
        target_affinity_delta: i16,
    ) -> SimulationEvent {
        SimulationEvent {
            id,
            tick,
            location: crate::simulation::EventLocation { x: 4, y: 7 },
            actor_id,
            target_id: Some(target_id),
            related_entity_ids: vec![actor_id, target_id],
            kind: SimulationEventKind::Interaction,
            cause: SimulationEventCause::MutualSocialContact,
            details: SimulationEventDetails::Interaction {
                actor_affinity_delta,
                target_affinity_delta,
            },
        }
    }

    #[test]
    fn interaction_history_json_is_complete_and_newest_first() {
        let events = [
            interaction_event(7, 24, 1, 2, 4, -1),
            interaction_event(9, 47, 3, 4, 0, 2),
        ];

        let payload: serde_json::Value =
            serde_json::from_str(&interaction_events_json(events.iter(), 48, None)).unwrap();

        assert_eq!(payload[0]["id"], "9");
        assert_eq!(payload[1]["id"], "7");
        assert_eq!(payload[0]["tick"], "47");
        assert_eq!(payload[0]["relative_time"], "1h ago");
        assert_eq!(
            payload[0]["location"],
            serde_json::json!({ "x": 4, "y": 7 })
        );
        assert_eq!(payload[0]["actor_id"], 3);
        assert_eq!(payload[0]["target_id"], 4);
        assert_eq!(payload[0]["related_entity_ids"], serde_json::json!([3, 4]));
        assert_eq!(payload[0]["kind"], "interaction");
        assert_eq!(payload[0]["cause"], "mutual_social_contact");
        assert_eq!(payload[0]["actor_affinity_delta"], 0);
        assert_eq!(payload[0]["target_affinity_delta"], 2);
    }

    #[test]
    fn interaction_history_json_filters_actor_target_and_related_entities() {
        let mut first = interaction_event(1, 10, 1, 2, 1, 1);
        first.related_entity_ids.push(99);
        let events = [first, interaction_event(2, 11, 3, 4, -1, -1)];

        for entity_id in [1, 2, 99] {
            let payload: serde_json::Value =
                serde_json::from_str(&interaction_events_json(events.iter(), 12, Some(entity_id)))
                    .unwrap();
            assert_eq!(payload.as_array().unwrap().len(), 1);
            assert_eq!(payload[0]["id"], "1");
        }

        assert_eq!(interaction_events_json(events.iter(), 12, Some(50)), "[]");
        let empty: [SimulationEvent; 0] = [];
        assert_eq!(interaction_events_json(empty.iter(), 12, None), "[]");
    }

    #[test]
    fn bridge_payloads_keep_the_frontend_shape() {
        let (population, entity, _, _) = payloads();
        let population: serde_json::Value = serde_json::from_str(&population).unwrap();
        let entity: serde_json::Value = serde_json::from_str(&entity).unwrap();

        for key in [
            "id",
            "sex",
            "age_ticks",
            "lifespan_ticks",
            "pregnant",
            "activity",
            "goal",
            "action",
            "utilities",
            "movement_credit",
            "life_stage",
            "stage_movement_factor",
            "caregiver_id",
            "personality",
            "known_resources",
            "known_entities",
            "known_chunks",
            "visible_entities",
        ] {
            assert!(entity.get(key).is_some(), "missing entity field {key}");
        }

        let personality = entity.get("personality").expect("missing personality");
        for key in [
            "curiosity",
            "sociability",
            "cooperativeness",
            "caution",
            "persistence",
        ] {
            assert!(
                personality.get(key).is_some(),
                "missing personality field {key}"
            );
        }
        for key in [
            "population",
            "females",
            "males",
            "pregnant",
            "births",
            "deaths",
        ] {
            assert!(
                population.get(key).is_some(),
                "missing population field {key}"
            );
        }
    }

    #[test]
    fn phase_profile_payload_is_valid_json() {
        let profile = PhaseProfile {
            physiology_us: 1,
            population_index_us: 2,
            autonomy_us: 3,
            starvation_us: 4,
            resource_changes_us: 5,
            remove_dead_us: 6,
            pregnancies_us: 7,
            conceptions_us: 8,
            total_us: 36,
        };

        let payload = phase_profile_json(&profile);
        let json: serde_json::Value = serde_json::from_str(&payload).unwrap();

        assert_eq!(json["autonomy_us"], 3);
        assert_eq!(json["total_us"], 36);
    }

    #[test]
    fn autonomy_profile_payload_is_valid_json() {
        let profile = AutonomyProfile {
            resource_perception_us: 1000,
            entity_perception_us: 500,
            plan_validation_us: 200,
            planning_us: 3000,
            action_us: 100,
            sampled_entities: 500,
            planned_entities: 87,
            urgent_interrupts: 3,
            memory_reconciliation_us: 800,
            visible_scan_us: 200,
            known_resources_total: 15_000,
            known_resources_max: 50,
            visible_resources_seen: 100,
            social_us: 250,
        };

        let payload = autonomy_profile_json(&profile);
        let json: serde_json::Value = serde_json::from_str(&payload).unwrap();

        assert_eq!(json["sampled_entities"], 500);
        assert_eq!(json["planned_entities"], 87);
        assert_eq!(json["known_resources_max"], 50);
        assert_eq!(json["social_us"], 250);
    }
}
