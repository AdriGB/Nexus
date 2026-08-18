use serde::Serialize;

mod entities;
mod events;
mod profiles;
mod world;

pub(crate) use entities::{entity_info_json, entity_relationships_json};
pub(crate) use events::{recent_events_json, recent_interaction_events_json};
pub(crate) use profiles::{autonomy_profile_json, phase_profile_json, population_stats_json};
pub(crate) use world::{region_stats_json, tile_info_json};

#[cfg(test)]
use crate::simulation::{
    AutonomyProfile, EventId, PhaseProfile, SimulationEvent, SimulationEventCause,
    SimulationEventDetails, SimulationEventKind,
};
#[cfg(test)]
use crate::world::{Grid, RegionKind};
#[cfg(test)]
use events::simulation_events_json;

pub(super) fn to_json<T: Serialize>(value: &T) -> String {
    serde_json::to_string(value).expect("bridge DTO serialization should not fail")
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
            id: EventId::new(id),
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
            serde_json::from_str(&simulation_events_json(events.iter(), 48, None)).unwrap();

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
                serde_json::from_str(&simulation_events_json(events.iter(), 12, Some(entity_id)))
                    .unwrap();
            assert_eq!(payload.as_array().unwrap().len(), 1);
            assert_eq!(payload[0]["id"], "1");
        }

        assert_eq!(simulation_events_json(events.iter(), 12, Some(50)), "[]");
        let empty: [SimulationEvent; 0] = [];
        assert_eq!(simulation_events_json(empty.iter(), 12, None), "[]");
    }

    #[test]
    fn lifecycle_events_json_handles_optional_participants_and_causes() {
        let events = [
            SimulationEvent {
                id: EventId::new(10),
                tick: 20,
                location: crate::simulation::EventLocation { x: 2, y: 3 },
                actor_id: 1,
                target_id: None,
                related_entity_ids: vec![1, 5],
                kind: SimulationEventKind::Birth,
                cause: SimulationEventCause::Born,
                details: SimulationEventDetails::Birth { child_id: 5 },
            },
            SimulationEvent {
                id: EventId::new(11),
                tick: 21,
                location: crate::simulation::EventLocation { x: 4, y: 6 },
                actor_id: 9,
                target_id: None,
                related_entity_ids: vec![9],
                kind: SimulationEventKind::Death,
                cause: SimulationEventCause::NaturalDeath,
                details: SimulationEventDetails::Death,
            },
        ];

        let payload: serde_json::Value =
            serde_json::from_str(&simulation_events_json(events.iter(), 21, None)).unwrap();
        assert_eq!(payload[0]["kind"], "death");
        assert_eq!(payload[0]["cause"], "natural_death");
        assert_eq!(payload[0]["target_id"], serde_json::Value::Null);
        assert_eq!(payload[1]["kind"], "birth");
        assert_eq!(payload[1]["cause"], "born");
        assert_eq!(payload[1]["child_id"], 5);

        let newborn: serde_json::Value =
            serde_json::from_str(&simulation_events_json(events.iter(), 21, Some(5))).unwrap();
        assert_eq!(newborn.as_array().unwrap().len(), 1);
        assert_eq!(newborn[0]["id"], "10");
    }

    #[test]
    fn consumption_event_json_includes_amount_and_filters_by_consumer() {
        let events = [SimulationEvent {
            id: EventId::new(12),
            tick: 30,
            location: crate::simulation::EventLocation { x: 6, y: 8 },
            actor_id: 4,
            target_id: None,
            related_entity_ids: vec![4],
            kind: SimulationEventKind::Consumption,
            cause: SimulationEventCause::AteFood,
            details: SimulationEventDetails::Consumption { amount: 9 },
        }];

        let payload: serde_json::Value =
            serde_json::from_str(&simulation_events_json(events.iter(), 30, Some(4))).unwrap();
        assert_eq!(payload[0]["kind"], "consumption");
        assert_eq!(payload[0]["cause"], "ate_food");
        assert_eq!(payload[0]["amount"], 9);
        assert_eq!(payload[0]["target_id"], serde_json::Value::Null);
        assert_eq!(simulation_events_json(events.iter(), 30, Some(5)), "[]");
    }

    #[test]
    fn discovery_event_json_includes_resource_observation() {
        let events = [SimulationEvent {
            id: EventId::new(13),
            tick: 31,
            location: crate::simulation::EventLocation { x: 7, y: 9 },
            actor_id: 6,
            target_id: None,
            related_entity_ids: vec![6],
            kind: SimulationEventKind::Discovery,
            cause: SimulationEventCause::ResourceFound,
            details: SimulationEventDetails::ResourceDiscovery {
                kind: ResourceKind::Stone,
                amount: 17,
            },
        }];

        let payload: serde_json::Value =
            serde_json::from_str(&simulation_events_json(events.iter(), 31, Some(6))).unwrap();
        assert_eq!(payload[0]["kind"], "discovery");
        assert_eq!(payload[0]["cause"], "resource_found");
        assert_eq!(payload[0]["resource_kind"], "stone");
        assert_eq!(payload[0]["amount"], 17);
        assert_eq!(simulation_events_json(events.iter(), 31, Some(5)), "[]");
    }

    #[test]
    fn encounter_event_json_includes_both_entities() {
        let events = [SimulationEvent {
            id: EventId::new(14),
            tick: 32,
            location: crate::simulation::EventLocation { x: 2, y: 5 },
            actor_id: 3,
            target_id: Some(9),
            related_entity_ids: vec![3, 9],
            kind: SimulationEventKind::Encounter,
            cause: SimulationEventCause::FirstEncounter,
            details: SimulationEventDetails::Encounter,
        }];

        let payload: serde_json::Value =
            serde_json::from_str(&simulation_events_json(events.iter(), 32, Some(9))).unwrap();
        assert_eq!(payload[0]["kind"], "encounter");
        assert_eq!(payload[0]["cause"], "first_encounter");
        assert_eq!(payload[0]["actor_id"], 3);
        assert_eq!(payload[0]["target_id"], 9);
        assert_eq!(simulation_events_json(events.iter(), 32, Some(8)), "[]");
    }

    #[test]
    fn affinity_change_event_json_serializes_and_filters_both_entities() {
        let events = [SimulationEvent {
            id: EventId::new(15),
            tick: 33,
            location: crate::simulation::EventLocation { x: 3, y: 6 },
            actor_id: 1,
            target_id: Some(2),
            related_entity_ids: vec![1, 2],
            kind: SimulationEventKind::AffinityChange,
            cause: SimulationEventCause::RelationshipDecay,
            details: SimulationEventDetails::AffinityChange {
                previous_affinity: 100,
                new_affinity: 99,
                delta: -1,
            },
        }];

        for entity_id in [1, 2] {
            let payload: serde_json::Value =
                serde_json::from_str(&simulation_events_json(events.iter(), 33, Some(entity_id)))
                    .unwrap();
            assert_eq!(payload[0]["kind"], "affinity_change");
            assert_eq!(payload[0]["cause"], "relationship_decay");
            assert_eq!(payload[0]["previous_affinity"], 100);
            assert_eq!(payload[0]["new_affinity"], 99);
            assert_eq!(payload[0]["delta"], -1);
        }
        assert_eq!(simulation_events_json(events.iter(), 33, Some(99)), "[]");
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
