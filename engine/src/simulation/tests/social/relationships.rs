use super::*;

#[test]
fn entity_avoids_negative_affinity_target() {
    let mut world = plain_grid(20, 20);
    let mut sim = Simulation {
        entities: vec![default_adult(1, 5, 5), default_adult(2, 7, 5)],
        next_entity_id: 3,
        ..Simulation::default()
    };

    // Entity 1 is sociable
    sim.entities[0].personality.sociability = 1.0;
    sim.entities[0].personality.curiosity = 0.0;
    sim.entities[0].hunger = 0.0;
    sim.entities[0].health = 100.0;

    // Give entity 1 a negative memory of entity 2
    sim.entities[0]
        .mind
        .memory
        .known_entities
        .push(crate::simulation::autonomy::KnownEntity {
            id: 2,
            first_seen_tick: 0,
            last_seen_tick: 0,
            last_seen_x: 7,
            last_seen_y: 5,
            observed_ticks: 1,
            affinity: -500,
            last_interaction_tick: 0,
            interaction_count: 0,
            seek_retry_after_tick: None,
        });

    // Run — entity should NOT socialize with entity 2
    let mut targeted_bad_entity = false;
    for _ in 0..100 {
        sim.step(&mut world);
        // Check if entity 1 is approaching entity 2
        if has_socialize_goal(&sim, 0) {
            if let Some(action) = sim.entities()[0].mind.current_action() {
                if let Some(target_id) = action.target_entity_id() {
                    if target_id == 2 {
                        targeted_bad_entity = true;
                        break;
                    }
                }
            }
        }
    }
    assert!(
        !targeted_bad_entity,
        "entity should not seek an entity with strong negative affinity"
    );
}

#[test]
fn abandoned_relationship_decays_over_simulated_days() {
    let mut world = plain_grid(16, 16);
    let mut sim = Simulation {
        entities: vec![default_adult(1, 5, 5)],
        next_entity_id: 2,
        seed: 42,
        ..Simulation::default()
    };

    // Memory of someone long gone — no partner present to interact with.
    sim.entities[0]
        .mind
        .memory
        .known_entities
        .push(crate::simulation::autonomy::KnownEntity {
            id: 99,
            first_seen_tick: 0,
            last_seen_tick: 0,
            last_seen_x: 5,
            last_seen_y: 5,
            observed_ticks: 10,
            affinity: 300,
            last_interaction_tick: 0,
            interaction_count: 5,
            seek_retry_after_tick: None,
        });

    // We jump the clock directly instead of running ~700 steps, both to
    // keep the test fast and to avoid the entity starving to death long
    // before the decay window opens. One step then lands on day 30.
    sim.tick = crate::simulation::autonomy::RELATIONSHIP_DECAY_START_TICKS - 1;
    sim.step(&mut world);

    // Day 30: 300 -> 299.
    assert_eq!(
        sim.entities()[0].mind.memory.known_entities[0].affinity,
        300 - i16::from(crate::simulation::autonomy::RELATIONSHIP_DECAY_PER_DAY),
        "first daily pass should cool the abandoned relationship"
    );

    // Day 31: one more day, one more daily pass: 299 -> 298.
    sim.tick = crate::simulation::autonomy::RELATIONSHIP_DECAY_START_TICKS + TICKS_PER_DAY - 1;
    sim.step(&mut world);

    assert_eq!(
        sim.entities()[0].mind.memory.known_entities[0].affinity,
        300 - 2 * i16::from(crate::simulation::autonomy::RELATIONSHIP_DECAY_PER_DAY),
        "decay should continue slowly day by day"
    );
}

#[test]
fn repeated_positive_interactions_form_relationship() {
    let mut world = plain_grid(8, 8);
    let mut sim = Simulation {
        entities: vec![default_adult(1, 3, 3), default_adult(2, 3, 4)],
        next_entity_id: 3,
        seed: 42,
        ..Simulation::default()
    };

    // Compatible + cooperative + sociable → max positive delta (+8)
    // per interaction, and interval 12 ticks (shortest cooldown).
    let compatible = Personality {
        curiosity: 0.0,
        sociability: 1.0,
        cooperativeness: 1.0,
        caution: 0.0,
        persistence: 0.5,
    };
    sim.entities[0].personality = compatible;
    sim.entities[1].personality = compatible;

    // Keep both pinned within social radius and non-starving across
    // several interaction windows. No affinity is ever set manually.
    for _ in 0..60 {
        sim.entities[0].x = 3;
        sim.entities[0].y = 3;
        sim.entities[1].x = 3;
        sim.entities[1].y = 4;
        for entity in &mut sim.entities {
            entity.hunger = 0.0;
            entity.health = 100.0;
        }
        sim.step(&mut world);
    }

    let known = sim.entities()[0]
        .mind
        .memory
        .known_entities
        .iter()
        .find(|k| k.id == 2)
        .expect("entity 1 should have stored a relationship with entity 2");

    assert!(
        known.interaction_count >= 3,
        "repeated proximity should build interaction history"
    );
    assert!(
        known.affinity > 0,
        "compatible cooperative partners should form positive affinity"
    );
}

#[test]
fn repeated_negative_interactions_lead_to_avoidance() {
    let mut world = plain_grid(8, 8);
    let mut sim = Simulation {
        entities: vec![default_adult(1, 3, 3), default_adult(2, 3, 4)],
        next_entity_id: 3,
        seed: 42,
        ..Simulation::default()
    };

    // Opposite curiosity/caution + uncooperative, both sociable so the
    // cooldown stays at its shortest (12 ticks). Compat ≈ 0.33 gives
    // about -5 per interaction. Nothing is injected manually.
    sim.entities[0].personality = Personality {
        curiosity: 0.0,
        sociability: 1.0,
        cooperativeness: 0.0,
        caution: 0.0,
        persistence: 0.5,
    };
    sim.entities[1].personality = Personality {
        curiosity: 1.0,
        sociability: 1.0,
        cooperativeness: 0.0,
        caution: 1.0,
        persistence: 0.5,
    };

    // Phase 1 — formation: repeated bad interactions while pinned within
    // social radius, until the stored affinity genuinely crosses -200.
    let mut too_negative = false;
    for _ in 0..900 {
        sim.entities[0].x = 3;
        sim.entities[0].y = 3;
        sim.entities[1].x = 3;
        sim.entities[1].y = 4;
        for entity in &mut sim.entities {
            entity.hunger = 0.0;
            entity.health = 100.0;
        }
        sim.step(&mut world);

        if sim.entities()[0].mind.memory.affinity_to(2).unwrap_or(0) < -200 {
            too_negative = true;
            break;
        }
    }
    assert!(
        too_negative,
        "repeated bad interactions should drive affinity below -200"
    );

    // Force a fresh decision after the relationship became strongly
    // negative: the last interaction may have crossed the threshold
    // in the middle of an existing social plan.
    sim.entities[0].mind.clear_goal();
    sim.entities[0].path.clear();
    sim.entities[0].path_index = 0;

    // Phase 2 — avoidance: separate them beyond perception (and far
    // outside SOCIAL_RADIUS) so no involuntary interaction can resume.
    let interactions_before_avoidance = interaction_count(&sim, 0, 2);

    for _ in 0..100 {
        sim.entities[0].x = 2;
        sim.entities[0].y = 2;
        sim.entities[1].x = 7;
        sim.entities[1].y = 7;

        for entity in &mut sim.entities {
            entity.hunger = 0.0;
            entity.health = 100.0;
        }

        sim.step(&mut world);

        if let Some(target) = sim.entities()[0]
            .mind
            .current_action()
            .and_then(|action| action.target_entity_id())
        {
            assert_ne!(
                target, 2,
                "strongly negative partner must not be sought voluntarily"
            );
        }
    }

    assert_eq!(
        interaction_count(&sim, 0, 2),
        interactions_before_avoidance,
        "separated hostile entities should not resume interaction"
    );
}

#[test]
fn formed_relationship_later_decays_when_abandoned() {
    let mut world = plain_grid(16, 16);
    let mut sim = Simulation {
        entities: vec![default_adult(1, 3, 3), default_adult(2, 3, 4)],
        next_entity_id: 3,
        seed: 42,
        ..Simulation::default()
    };

    let compatible = Personality {
        curiosity: 0.0,
        sociability: 1.0,
        cooperativeness: 1.0,
        caution: 0.0,
        persistence: 0.5,
    };
    sim.entities[0].personality = compatible;
    sim.entities[1].personality = compatible;

    // Form a real positive relationship through repeated interactions.
    for _ in 0..80 {
        sim.entities[0].x = 3;
        sim.entities[0].y = 3;
        sim.entities[1].x = 3;
        sim.entities[1].y = 4;
        for entity in &mut sim.entities {
            entity.hunger = 0.0;
            entity.health = 100.0;
        }
        sim.step(&mut world);
    }

    let formed = sim.entities()[0]
        .mind
        .memory
        .known_entities
        .iter()
        .find(|k| k.id == 2)
        .copied()
        .expect("relationship should have formed");
    assert!(
        formed.affinity > 0,
        "relationship must be positive before decay"
    );
    let formed_affinity = formed.affinity;
    let last_interaction = formed.last_interaction_tick;

    // Abandon it: move the partner far beyond perception so no new
    // interaction renews the relationship.
    sim.entities[1].x = 15;
    sim.entities[1].y = 15;

    // Jump the clock to the first day boundary at/after the 30-day decay
    // window, so the single step lands on a daily decay pass.
    let window_start =
        last_interaction + crate::simulation::autonomy::RELATIONSHIP_DECAY_START_TICKS;
    let boundary = window_start.div_ceil(crate::simulation::time::TICKS_PER_DAY)
        * crate::simulation::time::TICKS_PER_DAY;
    sim.tick = boundary - 1;
    sim.entities[0].x = 3;
    sim.entities[0].y = 3;
    for entity in &mut sim.entities {
        entity.hunger = 0.0;
        entity.health = 100.0;
    }
    sim.step(&mut world);

    let after = sim.entities()[0]
        .mind
        .memory
        .known_entities
        .iter()
        .find(|k| k.id == 2)
        .expect("relationship should still be remembered");
    assert!(
        after.affinity < formed_affinity,
        "a formed relationship should slowly decay when abandoned: {} -> {}",
        formed_affinity,
        after.affinity,
    );
}

#[test]
fn relationships_json_reflects_memory_and_sorts_by_strength() {
    let mut sim = Simulation {
        entities: vec![default_adult(1, 5, 5)],
        next_entity_id: 2,
        seed: 42,
        ..Simulation::default()
    };

    let known = |id: u32, affinity: i16, interactions: u32, cooldown: Option<u64>| {
        crate::simulation::autonomy::KnownEntity {
            id,
            first_seen_tick: 100,
            last_seen_tick: 900,
            last_seen_x: 7,
            last_seen_y: 5,
            observed_ticks: 50,
            affinity,
            last_interaction_tick: 800,
            interaction_count: interactions,
            seek_retry_after_tick: cooldown,
        }
    };

    sim.entities[0].mind.memory.known_entities = vec![
        known(4, 300, 2, None),
        known(2, 500, 1, Some(1_000)),
        known(6, -241, 5, None),
        known(3, 300, 9, None),
        known(5, -500, 1, None),
    ];

    let payload = crate::bridge::entity_relationships_json(&sim.entities()[0]);
    let json: serde_json::Value = serde_json::from_str(&payload).unwrap();
    let list = json.as_array().expect("payload should be a JSON array");

    assert_eq!(list.len(), 5);

    let ids: Vec<u32> = list
        .iter()
        .map(|row| row["id"].as_u64().unwrap() as u32)
        .collect();
    assert_eq!(ids, vec![2, 5, 3, 4, 6]);

    for row in list {
        for key in [
            "id",
            "affinity",
            "interaction_count",
            "first_seen_tick",
            "last_seen_tick",
            "last_interaction_tick",
            "last_seen_x",
            "last_seen_y",
            "observed_ticks",
            "seek_retry_after_tick",
        ] {
            assert!(row.get(key).is_some(), "missing field {key}");
        }
    }

    assert_eq!(list[0]["seek_retry_after_tick"], 1_000);
    assert_eq!(list[1]["seek_retry_after_tick"], serde_json::Value::Null);
    assert_eq!(list[0]["last_seen_x"], 7);
    assert_eq!(list[0]["last_seen_y"], 5);

    let second = crate::bridge::entity_relationships_json(&sim.entities()[0]);
    assert_eq!(payload, second);
}
