use super::*;

#[test]
fn socialize_goal_moves_entity_toward_target() {
    let mut world = plain_grid(20, 20);
    let mut sim = Simulation {
        entities: vec![default_adult(1, 5, 5), default_adult(2, 10, 5)],
        next_entity_id: 3,
        ..Simulation::default()
    };

    // Force socialize goal and suppress other goals
    for entity in &mut sim.entities {
        entity.personality.sociability = 1.0;
        entity.personality.curiosity = 0.0;
        entity.hunger = 0.0;
        entity.health = 100.0;
    }

    let initial_distance = sim.entities()[0].x.abs_diff(sim.entities()[1].x)
        + sim.entities()[0].y.abs_diff(sim.entities()[1].y);

    // Run until entity 1 moves or socializes
    let mut moved_closer = false;
    for _ in 0..100 {
        sim.step(&mut world);
        let current_distance = sim.entities()[0].x.abs_diff(sim.entities()[1].x)
            + sim.entities()[0].y.abs_diff(sim.entities()[1].y);
        if current_distance < initial_distance {
            moved_closer = true;
            break;
        }
    }
    assert!(
        moved_closer,
        "entity should move toward socialization target"
    );
}

#[test]
fn approach_uses_visible_target_position() {
    let mut world = plain_grid(20, 20);
    let mut sim = Simulation {
        entities: vec![default_adult(1, 5, 5), default_adult(2, 8, 5)],
        next_entity_id: 3,
        ..Simulation::default()
    };

    // Suppress all goals except socialize
    for entity in &mut sim.entities {
        entity.personality.sociability = 1.0;
        entity.personality.curiosity = 0.0;
        entity.hunger = 0.0;
        entity.health = 100.0;
    }

    // Run until entity 1 plans to approach entity 2
    let mut found_approach = false;
    for _ in 0..50 {
        sim.step(&mut world);
        if let Some(Action::ApproachEntity(2)) = sim.entities()[0].mind.current_action() {
            found_approach = true;
            break;
        }
    }
    assert!(
        found_approach,
        "entity should plan to approach visible target"
    );

    // Entity 2 should be in visible_entities (it's nearby)
    assert!(
        sim.entities()[0].mind.visible_entities.contains(&2),
        "target should be visible during approach"
    );
}

#[test]
fn approach_uses_last_seen_position_when_target_not_visible() {
    let mut world = plain_grid(64, 64);
    let mut sim = Simulation {
        entities: vec![default_adult(1, 5, 5), default_adult(2, 8, 5)],
        next_entity_id: 3,
        ..Simulation::default()
    };

    // Suppress all goals except socialize
    for entity in &mut sim.entities {
        entity.personality.sociability = 1.0;
        entity.personality.curiosity = 0.0;
        entity.hunger = 0.0;
        entity.health = 100.0;
    }

    // Step once so entity 1 perceives entity 2 and remembers it
    sim.step(&mut world);
    assert!(
        sim.entities()[0]
            .mind
            .memory
            .known_entities
            .iter()
            .any(|k| k.id == 2),
        "entity 1 should remember entity 2"
    );

    // Teleport entity 2 far away (beyond perception radius)
    sim.entities[1].x = 50;
    sim.entities[1].y = 50;

    // Step again — entity 1 should plan approach using last_seen position
    sim.step(&mut world);

    if let Some(Action::ApproachEntity(target_id)) = sim.entities()[0].mind.current_action() {
        assert_eq!(target_id, 2);
        // Entity 1 should be heading toward (8, 5), the last known position
        // NOT toward (50, 50), the real position
        let path_target = sim.entities()[0].path.last().copied();
        if let Some(dest) = path_target {
            let dist_to_last_seen = dest.0.abs_diff(8) + dest.1.abs_diff(5);
            let dist_to_real = dest.0.abs_diff(50) + dest.1.abs_diff(50);
            assert!(
                dist_to_last_seen < dist_to_real,
                "should head toward last_seen (8,5) not real (50,50): dest={:?}",
                dest
            );
        }
    }
}

#[test]
fn approach_does_not_track_hidden_target() {
    // The key anti-omniscience test:
    // A sees B at (5,5) on tick 1.
    // B teleports to (15,15) on tick 2 (beyond perception).
    // A must follow (5,5), NOT (15,15).
    let mut world = plain_grid(64, 64);
    let mut sim = Simulation {
        entities: vec![default_adult(1, 3, 5), default_adult(2, 5, 5)],
        next_entity_id: 3,
        ..Simulation::default()
    };

    for entity in &mut sim.entities {
        entity.personality.sociability = 1.0;
        entity.personality.curiosity = 0.0;
        entity.hunger = 0.0;
        entity.health = 100.0;
    }

    // Tick 1: A sees B at (5,5)
    sim.step(&mut world);
    let known = sim.entities()[0]
        .mind
        .memory
        .known_entities
        .iter()
        .find(|k| k.id == 2)
        .expect("should remember entity 2");
    assert_eq!((known.last_seen_x, known.last_seen_y), (5, 5));

    // B moves far away (beyond perception radius of 6)
    sim.entities[1].x = 15;
    sim.entities[1].y = 15;

    // Tick 2: A should still head toward (5,5)
    sim.step(&mut world);

    // Verify A's memory still says (5,5) — not (15,15)
    let known = sim.entities()[0]
        .mind
        .memory
        .known_entities
        .iter()
        .find(|k| k.id == 2)
        .expect("should still remember entity 2");
    assert_eq!(
        (known.last_seen_x, known.last_seen_y),
        (5, 5),
        "memory should retain last seen position, not track real position"
    );
}

#[test]
fn approach_abandons_search_at_stale_last_seen_position() {
    // A approaches B's last known position.
    // A arrives but B is not there and not visible.
    // A should abandon Socialize immediately, not stand forever.
    let mut world = plain_grid(32, 32);
    let mut sim = Simulation {
        entities: vec![default_adult(1, 5, 5), default_adult(2, 10, 5)],
        next_entity_id: 3,
        ..Simulation::default()
    };

    for entity in &mut sim.entities {
        entity.personality.sociability = 1.0;
        entity.personality.curiosity = 0.0;
        entity.hunger = 0.0;
        entity.health = 100.0;
    }

    // Let A perceive B, then pin the state under test: an active approach
    // toward B's last observed position.
    sim.step(&mut world);
    sim.entities[0].x = 5;
    sim.entities[0].y = 5;
    sim.entities[0].path.clear();
    sim.entities[0].path_index = 0;
    sim.entities[0].movement_credit = 0.0;
    let tick = sim.tick();
    sim.entities[0].mind.set_plan(
        Goal::Socialize,
        vec![Action::ApproachEntity(2), Action::Interact(2)],
        tick,
    );

    // B disappears (teleport far away or "die")
    sim.entities[1].x = 30;
    sim.entities[1].y = 30;

    // Run until A reaches within SOCIAL_RADIUS of last known position (10, 5)
    let mut arrived = false;
    for _ in 0..200 {
        for entity in &mut sim.entities {
            entity.hunger = 0.0;
            entity.health = 100.0;
        }
        sim.entities[1].x = 30;
        sim.entities[1].y = 30;

        sim.step(&mut world);
        let distance_to_last_seen =
            sim.entities()[0].x.abs_diff(10) + sim.entities()[0].y.abs_diff(5);
        if distance_to_last_seen <= crate::simulation::autonomy::SOCIAL_RADIUS {
            arrived = true;
            // Entering the radius can consume the movement portion of this
            // tick. The next action update must notice the absent target and
            // invalidate the stale social plan.
            sim.step(&mut world);
            break;
        }
    }
    assert!(
        arrived,
        "entity should reach vicinity of last known position"
    );

    // Immediately after arriving, A should NOT be Socializing
    // and should NOT have Interact(2) as current action
    let activity = sim.entities()[0].activity;
    let current_action = sim.entities()[0].mind.current_action();
    let current_goal = sim.entities()[0].mind.current_goal;

    assert_ne!(
        activity,
        crate::simulation::entity::EntityActivity::Socializing,
        "entity should NOT be Socializing when target is not visible at last_seen"
    );
    assert_ne!(
        current_action,
        Some(Action::Interact(2)),
        "entity should NOT have Interact(2) as current action"
    );
    assert_ne!(
        current_goal,
        Some(Goal::Socialize),
        "entity should abandon Socialize goal immediately"
    );
}

#[test]
fn interact_requires_target_in_social_radius() {
    let mut world = plain_grid(20, 20);
    let mut sim = Simulation {
        entities: vec![default_adult(1, 5, 5), default_adult(2, 8, 5)],
        next_entity_id: 3,
        ..Simulation::default()
    };

    for entity in &mut sim.entities {
        entity.personality.sociability = 1.0;
        entity.personality.curiosity = 0.0;
        entity.hunger = 0.0;
        entity.health = 100.0;
    }

    // Force a plan with Interact(2) while target is visible but far away
    // Distance = 3: within perception (6) but beyond SOCIAL_RADIUS (2)
    sim.entities[0]
        .mind
        .set_plan(Goal::Socialize, vec![Action::Interact(2)], 0);

    sim.step(&mut world);

    // After executing Interact with target visible but out of range,
    // entity should replan to ApproachEntity, not stay on Interact or Socialize
    let current_action = sim.entities()[0].mind.current_action();
    assert_eq!(
        current_action,
        Some(Action::ApproachEntity(2)),
        "should replan to ApproachEntity when target is visible but outside SOCIAL_RADIUS"
    );
}

#[test]
fn interact_does_not_reacquire_hidden_target() {
    use crate::simulation::entity::EntityActivity;

    let mut world = plain_grid(64, 64);
    let mut sim = Simulation {
        entities: vec![default_adult(1, 5, 5), default_adult(2, 8, 5)],
        next_entity_id: 3,
        ..Simulation::default()
    };

    for entity in &mut sim.entities {
        entity.personality.sociability = 1.0;
        entity.personality.curiosity = 0.0;
        entity.hunger = 0.0;
        entity.health = 100.0;
    }

    // Step once so A perceives B at (8, 5) and remembers it
    sim.step(&mut world);
    assert!(
        sim.entities()[0].mind.visible_entities.contains(&2),
        "A should see B initially"
    );

    // Force a plan: Socialize -> [ApproachEntity(2), Interact(2)]
    sim.entities[0].mind.set_plan(
        Goal::Socialize,
        vec![Action::ApproachEntity(2), Action::Interact(2)],
        0,
    );

    // B moves to (15, 15) and becomes invisible to A
    sim.entities[1].x = 15;
    sim.entities[1].y = 15;

    // Clear A's visibility so B is not visible
    sim.entities[0].mind.visible_entities.clear();

    // Manually move A to B's last known position (8, 5)
    sim.entities[0].x = 8;
    sim.entities[0].y = 5;

    // Advance to Interact action
    sim.entities[0].mind.advance_action();
    assert_eq!(
        sim.entities()[0].mind.current_action(),
        Some(Action::Interact(2)),
        "should be on Interact(2)"
    );

    // Execute one step — Interact should NOT reacquire B at (15, 15)
    sim.step(&mut world);

    // Verify A did NOT create a path toward the real position (15, 15)
    let path_target = sim.entities()[0].path.last().copied();
    if let Some(dest) = path_target {
        let dist_to_real = dest.0.abs_diff(15) + dest.1.abs_diff(15);
        assert!(
            dist_to_real > 5,
            "A should NOT path toward B's real position (15, 15)"
        );
    }

    // Verify A abandoned the Socialize goal (no longer pursuing B)
    let activity = sim.entities()[0].activity;
    assert_ne!(
        activity,
        EntityActivity::Moving,
        "A should not be Moving toward hidden B"
    );
    assert_ne!(
        activity,
        EntityActivity::Socializing,
        "A should not be Socializing with hidden B"
    );
}

// ── Memory-based social seeking tests ─────────────────────────────────────

#[test]
fn entity_seeks_remembered_high_affinity_target() {
    use crate::simulation::autonomy::KnownEntity;

    let mut world = plain_grid(64, 64);
    let mut sim = Simulation {
        entities: vec![default_adult(1, 5, 5), default_adult(2, 50, 50)],
        next_entity_id: 3,
        ..Simulation::default()
    };

    // Entity 1 is highly sociable
    sim.entities[0].personality.sociability = 1.0;
    sim.entities[0].personality.curiosity = 0.0;
    sim.entities[0].hunger = 0.0;
    sim.entities[0].health = 100.0;

    // Entity 2 is far away (beyond perception radius of 6)
    // Distance = 90, so entity 1 cannot see entity 2

    // Manually add a high-affinity memory of entity 2
    sim.entities[0]
        .mind
        .memory
        .known_entities
        .push(KnownEntity {
            id: 2,
            first_seen_tick: 0,
            last_seen_tick: 0,
            last_seen_x: 50,
            last_seen_y: 50,
            observed_ticks: 10,
            affinity: 500, // Strong positive affinity
            last_interaction_tick: 0,
            interaction_count: 5,
            seek_retry_after_tick: None,
        });

    // Clear any visible entities to force memory-based decision
    sim.entities[0].mind.visible_entities.clear();

    // Run until entity 1 decides to seek entity 2 from memory
    let mut found_seek = false;
    for _ in 0..100 {
        sim.step(&mut world);

        // Check if entity 1 has Socialize goal and is approaching entity 2
        if sim.entities()[0].mind.current_goal == Some(Goal::Socialize) {
            if let Some(Action::ApproachEntity(2)) = sim.entities()[0].mind.current_action() {
                found_seek = true;
                break;
            }
        }
    }

    assert!(
        found_seek,
        "entity should seek remembered high-affinity target even when not visible"
    );

    // Verify entity 1 is moving toward last_seen position (50, 50)
    let path_target = sim.entities()[0].path.last().copied();
    assert!(
        path_target.is_some(),
        "entity should have a path toward last_seen position"
    );
}

#[test]
fn seeker_abandons_at_stale_last_seen_position() {
    use crate::simulation::entity::EntityActivity;

    let mut world = plain_grid(64, 64);
    let mut sim = Simulation {
        entities: vec![default_adult(1, 5, 5), default_adult(2, 10, 5)],
        next_entity_id: 3,
        ..Simulation::default()
    };

    for entity in &mut sim.entities {
        entity.personality.sociability = 1.0;
        entity.personality.curiosity = 0.0;
        entity.hunger = 0.0;
        entity.health = 100.0;
    }

    // Step once so entity 1 perceives entity 2 at (10, 5)
    sim.step(&mut world);
    assert!(
        sim.entities()[0].mind.visible_entities.contains(&2),
        "entity 1 should see entity 2 initially"
    );

    // Entity 2 moves far away to (30, 30) and becomes invisible
    sim.entities[1].x = 30;
    sim.entities[1].y = 30;

    // Force a social plan targeting entity 2
    sim.entities[0].mind.set_plan(
        Goal::Socialize,
        vec![Action::ApproachEntity(2), Action::Interact(2)],
        0,
    );

    // Manually move entity 1 to the last_seen position of entity 2
    sim.entities[0].x = 10;
    sim.entities[0].y = 5;
    sim.entities[0].path.clear();
    sim.entities[0].path_index = 0;

    // Clear visibility so entity 2 is not visible
    sim.entities[0].mind.visible_entities.clear();

    // Advance to Interact action
    sim.entities[0].mind.advance_action();
    assert_eq!(
        sim.entities()[0].mind.current_action(),
        Some(Action::Interact(2)),
        "should be on Interact(2)"
    );

    // Execute step — Interact should detect target is not visible and abandon
    sim.step(&mut world);

    // Verify entity 1 abandoned the goal
    let activity = sim.entities()[0].activity;
    let current_goal = sim.entities()[0].mind.current_goal;
    let current_action = sim.entities()[0].mind.current_action();

    assert_ne!(
        activity,
        EntityActivity::Socializing,
        "entity should NOT be Socializing when target is not visible at last_seen"
    );
    assert_ne!(
        current_goal,
        Some(Goal::Socialize),
        "entity should abandon Socialize goal when target not found"
    );
    assert_ne!(
        current_action,
        Some(Action::Interact(2)),
        "entity should NOT stay on Interact(2) when target is hidden"
    );
}

#[test]
fn low_affinity_memory_does_not_trigger_seek() {
    use crate::simulation::autonomy::KnownEntity;

    let mut world = plain_grid(64, 64);
    let mut sim = Simulation {
        entities: vec![default_adult(1, 5, 5)],
        next_entity_id: 2,
        ..Simulation::default()
    };

    sim.entities[0].personality.sociability = 1.0;
    sim.entities[0].personality.curiosity = 0.0;
    sim.entities[0].hunger = 0.0;
    sim.entities[0].health = 100.0;

    // Add a low-affinity memory (below threshold of 100)
    sim.entities[0]
        .mind
        .memory
        .known_entities
        .push(KnownEntity {
            id: 99,
            first_seen_tick: 0,
            last_seen_tick: 0,
            last_seen_x: 50,
            last_seen_y: 50,
            observed_ticks: 3,
            affinity: 50, // Below threshold
            last_interaction_tick: 0,
            interaction_count: 1,
            seek_retry_after_tick: None,
        });

    // No visible entities
    sim.entities[0].mind.visible_entities.clear();

    // Run many ticks — should NOT seek the low-affinity target
    let mut sought_low_affinity = false;
    for _ in 0..100 {
        sim.step(&mut world);

        if sim.entities()[0].mind.current_goal == Some(Goal::Socialize) {
            if let Some(Action::ApproachEntity(99)) = sim.entities()[0].mind.current_action() {
                sought_low_affinity = true;
                break;
            }
        }
    }

    assert!(
        !sought_low_affinity,
        "entity should NOT seek low-affinity remembered target (affinity <= 100)"
    );
}
