use super::super::autonomy::DecisionContext;
use super::super::entity::{Entity, Personality};
use super::super::time::{TICKS_PER_DAY, TICKS_PER_YEAR};
use super::super::Simulation;
use super::support::*;

fn default_adult(id: u32, x: u32, y: u32) -> Entity {
    let mut entity = entity(id, x, y, 0.0);

    entity.age_ticks = 25 * TICKS_PER_YEAR;
    entity.personality = Personality {
        curiosity: 0.5,
        sociability: 0.5,
        cooperativeness: 0.5,
        caution: 0.5,
        persistence: 0.5,
    };

    entity
}

fn interaction_count(simulation: &Simulation, observer: usize, target_id: u32) -> u32 {
    simulation.entities()[observer]
        .mind
        .memory
        .known_entities
        .iter()
        .find(|known| known.id == target_id)
        .map_or(0, |known| known.interaction_count)
}

fn affinity(simulation: &Simulation, observer: usize, target_id: u32) -> i16 {
    simulation.entities()[observer]
        .mind
        .memory
        .known_entities
        .iter()
        .find(|known| known.id == target_id)
        .expect("relationship should exist")
        .affinity
}

#[test]
fn nearby_entities_interact() {
    let mut world = plain_grid(10, 10);
    let mut sim = Simulation {
        entities: vec![default_adult(1, 5, 5), default_adult(2, 6, 5)],
        next_entity_id: 3,
        ..Simulation::default()
    };

    sim.step(&mut world);

    let known = sim.entities()[0]
        .mind
        .memory
        .known_entities
        .iter()
        .find(|known| known.id == 2)
        .expect("entity 1 should know entity 2");

    assert_eq!(known.interaction_count, 1);
    assert!(known.last_interaction_tick > 0);
    assert_eq!(known.affinity, 4);
}

#[test]
fn distant_entities_do_not_interact() {
    let mut world = plain_grid(64, 64);
    let mut sim = Simulation {
        entities: vec![default_adult(1, 10, 10), default_adult(2, 10, 13)],
        next_entity_id: 3,
        ..Simulation::default()
    };

    sim.step(&mut world);

    let known = sim.entities()[0]
        .mind
        .memory
        .known_entities
        .iter()
        .find(|known| known.id == 2)
        .expect("entity should be perceived but outside social radius");

    assert_eq!(known.interaction_count, 0);
    assert_eq!(known.affinity, 0);
}

#[test]
fn interaction_respects_cooldown() {
    let mut world = plain_grid(5, 5);
    let mut sim = Simulation {
        entities: vec![default_adult(1, 2, 2), default_adult(2, 2, 3)],
        next_entity_id: 3,
        ..Simulation::default()
    };

    for entity in &mut sim.entities {
        entity.personality.sociability = 1.0;
    }

    sim.step(&mut world);
    let first_count = interaction_count(&sim, 0, 2);
    assert!(first_count >= 1, "should have interacted on first step");

    sim.step(&mut world);
    assert_eq!(
        interaction_count(&sim, 0, 2),
        first_count,
        "cooldown should prevent immediate re-interaction"
    );
}

#[test]
fn high_sociability_interacts_more_frequently() {
    let mut world_high = plain_grid(5, 5);
    let mut world_low = plain_grid(5, 5);

    let mut sim_high = Simulation {
        entities: vec![default_adult(1, 2, 2), default_adult(2, 2, 3)],
        next_entity_id: 3,
        ..Simulation::default()
    };
    sim_high.entities[0].personality.sociability = 1.0;
    sim_high.entities[1].personality.sociability = 1.0;

    let mut sim_low = Simulation {
        entities: vec![default_adult(1, 2, 2), default_adult(2, 2, 3)],
        next_entity_id: 3,
        ..Simulation::default()
    };
    sim_low.entities[0].personality.sociability = 0.0;
    sim_low.entities[1].personality.sociability = 0.0;

    for _ in 0..150 {
        for simulation in [&mut sim_high, &mut sim_low] {
            simulation.entities[0].x = 2;
            simulation.entities[0].y = 2;
            simulation.entities[1].x = 2;
            simulation.entities[1].y = 3;

            for entity in &mut simulation.entities {
                entity.hunger = 0.0;
                entity.health = 100.0;
            }
        }

        sim_high.step(&mut world_high);
        sim_low.step(&mut world_low);
    }

    let high_count = interaction_count(&sim_high, 0, 2);
    let low_count = interaction_count(&sim_low, 0, 2);

    assert!(
        high_count > low_count,
        "high sociability should interact more often: high={high_count}, low={low_count}"
    );
}

#[test]
fn cooperativeness_affects_partner_affinity() {
    let mut world_cooperative = plain_grid(5, 5);
    let mut world_uncooperative = plain_grid(5, 5);

    let mut sim_cooperative = Simulation {
        entities: vec![default_adult(1, 2, 2), default_adult(2, 2, 3)],
        next_entity_id: 3,
        ..Simulation::default()
    };
    sim_cooperative.entities[1].personality.cooperativeness = 1.0;

    let mut sim_uncooperative = Simulation {
        entities: vec![default_adult(1, 2, 2), default_adult(2, 2, 3)],
        next_entity_id: 3,
        ..Simulation::default()
    };
    sim_uncooperative.entities[1].personality.cooperativeness = 0.0;

    sim_cooperative.step(&mut world_cooperative);
    sim_uncooperative.step(&mut world_uncooperative);

    let cooperative_affinity = affinity(&sim_cooperative, 0, 2);
    let uncooperative_affinity = affinity(&sim_uncooperative, 0, 2);

    assert!(
        cooperative_affinity > uncooperative_affinity,
        "cooperative partner should yield higher affinity: cooperative={cooperative_affinity}, uncooperative={uncooperative_affinity}"
    );
}

#[test]
fn relationships_can_be_asymmetric() {
    let mut world = plain_grid(5, 5);
    let mut sim = Simulation {
        entities: vec![default_adult(1, 2, 2), default_adult(2, 2, 3)],
        next_entity_id: 3,
        ..Simulation::default()
    };

    sim.entities[0].personality.cooperativeness = 1.0;
    sim.entities[1].personality.cooperativeness = 0.0;

    sim.step(&mut world);

    let a_to_b = affinity(&sim, 0, 2);
    let b_to_a = affinity(&sim, 1, 1);

    assert!(
        b_to_a > a_to_b,
        "relationship should be asymmetric: A-to-B={a_to_b}, B-to-A={b_to_a}"
    );
}

// ── Socialization goal tests ──────────────────────────────────────────────

use super::super::autonomy::Goal;

fn has_socialize_goal(sim: &Simulation, entity_index: usize) -> bool {
    sim.entities()[entity_index].mind.current_goal == Some(Goal::Socialize)
}

#[test]
fn highly_sociable_entity_chooses_socialize_goal() {
    let mut world = plain_grid(20, 20);
    let mut sim = Simulation {
        entities: vec![default_adult(1, 5, 5), default_adult(2, 7, 5)],
        next_entity_id: 3,
        ..Simulation::default()
    };

    // Entity 1 is highly sociable
    sim.entities[0].personality.sociability = 1.0;
    sim.entities[0].personality.curiosity = 0.0; // suppress explore
    sim.entities[0].hunger = 0.0;
    sim.entities[0].health = 100.0;

    // Entity 2 is nearby but not adjacent
    sim.entities[1].personality.sociability = 1.0;

    // Run a few ticks — entity should eventually pick Socialize
    let mut socialized = false;
    for _ in 0..50 {
        sim.step(&mut world);
        if has_socialize_goal(&sim, 0) {
            socialized = true;
            break;
        }
    }
    assert!(
        socialized,
        "highly sociable entity should choose Socialize goal"
    );
}

#[test]
fn unsociable_entity_rarely_socializes() {
    let mut world = plain_grid(20, 20);
    // Scatter food so entities don't starve to death
    for x in 0..20u32 {
        for y in 0..20u32 {
            if (x + y) % 3 == 0 {
                let idx = (y * world.width + x) as usize;
                world.resources[idx] = Some(super::super::super::world::ResourceDeposit {
                    kind: super::super::super::world::ResourceKind::Food,
                    amount: 500,
                });
            }
        }
    }
    let mut sim = Simulation {
        entities: vec![default_adult(1, 5, 5), default_adult(2, 7, 5)],
        next_entity_id: 3,
        ..Simulation::default()
    };

    // Entity 1 is unsociable
    sim.entities[0].personality.sociability = 0.0;
    sim.entities[0].personality.curiosity = 0.5;
    sim.entities[0].hunger = 0.0;
    sim.entities[0].health = 100.0;

    // Run many ticks — socialize should be rare
    let mut socialize_count = 0;
    for _ in 0..200 {
        sim.step(&mut world);
        if has_socialize_goal(&sim, 0) {
            socialize_count += 1;
        }
    }
    assert!(
        socialize_count < 20,
        "unsociable entity should rarely socialize: count={socialize_count}"
    );
}

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
        .push(super::super::autonomy::KnownEntity {
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
fn socialize_utility_increases_with_positive_affinity() {
    use super::super::entity::Personality;

    let mut mind = super::super::autonomy::Mind::default();
    let personality = Personality {
        curiosity: 0.0,
        sociability: 0.8,
        cooperativeness: 0.5,
        caution: 0.5,
        persistence: 0.5,
    };

    // No known entities — baseline socialize
    let _ = super::super::autonomy::evaluate_goals(
        &mut mind,
        10.0,
        100.0,
        25 * super::super::time::TICKS_PER_YEAR,
        &personality,
        None,
        DecisionContext {
            tick: 0,
            origin: (0, 0),
        },
    );
    let score_no_affinity = mind.utility_scores.socialize;

    // Add positive affinity entities
    mind.memory
        .known_entities
        .push(super::super::autonomy::KnownEntity {
            id: 10,
            first_seen_tick: 0,
            last_seen_tick: 0,
            last_seen_x: 0,
            last_seen_y: 0,
            observed_ticks: 5,
            affinity: 500,
            last_interaction_tick: 0,
            interaction_count: 3,
            seek_retry_after_tick: None,
        });
    mind.memory
        .known_entities
        .push(super::super::autonomy::KnownEntity {
            id: 11,
            first_seen_tick: 0,
            last_seen_tick: 0,
            last_seen_x: 0,
            last_seen_y: 0,
            observed_ticks: 5,
            affinity: 300,
            last_interaction_tick: 0,
            interaction_count: 2,
            seek_retry_after_tick: None,
        });
    // Socialize utility requires visible candidates
    mind.visible_entities = vec![10, 11];

    let _ = super::super::autonomy::evaluate_goals(
        &mut mind,
        10.0,
        100.0,
        25 * super::super::time::TICKS_PER_YEAR,
        &personality,
        None,
        DecisionContext {
            tick: 0,
            origin: (0, 0),
        },
    );
    let score_with_affinity = mind.utility_scores.socialize;

    assert!(
        score_with_affinity > score_no_affinity,
        "socialize utility should increase with positive affinity: no_affinity={score_no_affinity}, with_affinity={score_with_affinity}"
    );
}

// ── Knowledge-bounded social pursuit tests ───────────────────────────────

use super::super::autonomy::Action;

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
        if distance_to_last_seen <= super::super::autonomy::SOCIAL_RADIUS {
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
        super::super::entity::EntityActivity::Socializing,
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
fn socialize_utility_is_zero_without_candidates() {
    use super::super::entity::Personality;

    let mut mind = super::super::autonomy::Mind::default();
    let personality = Personality {
        curiosity: 0.0,
        sociability: 1.0,
        cooperativeness: 0.5,
        caution: 0.0,
        persistence: 0.0,
    };

    // No visible entities at all — socialize should be 0
    mind.visible_entities.clear();

    let _ = super::super::autonomy::evaluate_goals(
        &mut mind,
        0.0,   // no hunger
        100.0, // full health
        25 * super::super::time::TICKS_PER_YEAR,
        &personality,
        None,
        DecisionContext {
            tick: 0,
            origin: (0, 0),
        },
    );

    assert_eq!(
        mind.utility_scores.socialize, 0.0,
        "socialize utility should be 0 with no visible candidates"
    );
}

#[test]
fn interact_does_not_reacquire_hidden_target() {
    use super::super::entity::EntityActivity;

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
    use super::super::autonomy::KnownEntity;

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
fn socialize_utility_nonzero_with_positive_memory_no_visible() {
    use super::super::entity::Personality;

    let mut mind = super::super::autonomy::Mind::default();
    let personality = Personality {
        curiosity: 0.0,
        sociability: 0.8,
        cooperativeness: 0.5,
        caution: 0.5,
        persistence: 0.5,
    };

    // Add two high-affinity known entities (not visible)
    mind.memory
        .known_entities
        .push(super::super::autonomy::KnownEntity {
            id: 10,
            first_seen_tick: 0,
            last_seen_tick: 0,
            last_seen_x: 0,
            last_seen_y: 0,
            observed_ticks: 5,
            affinity: 500,
            last_interaction_tick: 0,
            interaction_count: 3,
            seek_retry_after_tick: None,
        });
    mind.memory
        .known_entities
        .push(super::super::autonomy::KnownEntity {
            id: 11,
            first_seen_tick: 0,
            last_seen_tick: 0,
            last_seen_x: 0,
            last_seen_y: 0,
            observed_ticks: 5,
            affinity: 400,
            last_interaction_tick: 0,
            interaction_count: 2,
            seek_retry_after_tick: None,
        });

    // No visible entities
    mind.visible_entities.clear();

    let _ = super::super::autonomy::evaluate_goals(
        &mut mind,
        0.0,   // no hunger
        100.0, // full health
        25 * super::super::time::TICKS_PER_YEAR,
        &personality,
        None,
        DecisionContext {
            tick: 0,
            origin: (0, 0),
        },
    );

    assert!(
        mind.utility_scores.socialize > 0.0,
        "socialize utility should be > 0 with high-affinity memories even without visible candidates: score={}",
        mind.utility_scores.socialize
    );

    // Utility should be reduced compared to having visible candidates
    mind.visible_entities = vec![10, 11];
    let _ = super::super::autonomy::evaluate_goals(
        &mut mind,
        0.0,
        100.0,
        25 * super::super::time::TICKS_PER_YEAR,
        &personality,
        None,
        DecisionContext {
            tick: 0,
            origin: (0, 0),
        },
    );
    let visible_score = mind.utility_scores.socialize;

    // Now clear visibility again and check memory-only score
    mind.visible_entities.clear();
    let _ = super::super::autonomy::evaluate_goals(
        &mut mind,
        0.0,
        100.0,
        25 * super::super::time::TICKS_PER_YEAR,
        &personality,
        None,
        DecisionContext {
            tick: 0,
            origin: (0, 0),
        },
    );
    let memory_only_score = mind.utility_scores.socialize;

    assert!(
        visible_score > memory_only_score,
        "visible candidates should yield higher utility than memory-only: visible={visible_score}, memory={memory_only_score}"
    );
}

#[test]
fn seeker_abandons_at_stale_last_seen_position() {
    use super::super::entity::EntityActivity;

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
    use super::super::autonomy::KnownEntity;

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
        .push(super::super::autonomy::KnownEntity {
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
    sim.tick = super::super::autonomy::RELATIONSHIP_DECAY_START_TICKS - 1;
    sim.step(&mut world);

    // Day 30: 300 -> 299.
    assert_eq!(
        sim.entities()[0].mind.memory.known_entities[0].affinity,
        300 - i16::from(super::super::autonomy::RELATIONSHIP_DECAY_PER_DAY),
        "first daily pass should cool the abandoned relationship"
    );

    // Day 31: one more day, one more daily pass: 299 -> 298.
    sim.tick = super::super::autonomy::RELATIONSHIP_DECAY_START_TICKS + TICKS_PER_DAY - 1;
    sim.step(&mut world);

    assert_eq!(
        sim.entities()[0].mind.memory.known_entities[0].affinity,
        300 - 2 * i16::from(super::super::autonomy::RELATIONSHIP_DECAY_PER_DAY),
        "decay should continue slowly day by day"
    );
}
