use super::*;

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
                world.resources[idx] = Some(crate::world::ResourceDeposit {
                    kind: crate::world::ResourceKind::Food,
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
fn socialize_utility_increases_with_positive_affinity() {
    use crate::simulation::entity::Personality;

    let mut mind = crate::simulation::autonomy::Mind::default();
    let personality = Personality {
        curiosity: 0.0,
        sociability: 0.8,
        cooperativeness: 0.5,
        caution: 0.5,
        persistence: 0.5,
    };

    // No known entities — baseline socialize
    let _ = crate::simulation::autonomy::evaluate_goals(
        &mut mind,
        10.0,
        100.0,
        25 * crate::simulation::time::TICKS_PER_YEAR,
        &personality,
        None,
        DecisionContext {
            tick: 0,
            origin: (0, 0),
            food_in_inventory: 0,
            visible_food_need: 0.0,
            best_remembered_social_score: None,
        },
    );
    let score_no_affinity = mind.utility_scores.socialize;

    // Add positive affinity entities
    mind.memory
        .known_entities
        .push(crate::simulation::autonomy::KnownEntity {
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
        .push(crate::simulation::autonomy::KnownEntity {
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

    let _ = crate::simulation::autonomy::evaluate_goals(
        &mut mind,
        10.0,
        100.0,
        25 * crate::simulation::time::TICKS_PER_YEAR,
        &personality,
        None,
        DecisionContext {
            tick: 0,
            origin: (0, 0),
            food_in_inventory: 0,
            visible_food_need: 0.0,
            best_remembered_social_score: None,
        },
    );
    let score_with_affinity = mind.utility_scores.socialize;

    assert!(
        score_with_affinity > score_no_affinity,
        "socialize utility should increase with positive affinity: no_affinity={score_no_affinity}, with_affinity={score_with_affinity}"
    );
}

// ── Knowledge-bounded social pursuit tests ───────────────────────────────

#[test]
fn socialize_utility_is_zero_without_candidates() {
    use crate::simulation::entity::Personality;

    let mut mind = crate::simulation::autonomy::Mind::default();
    let personality = Personality {
        curiosity: 0.0,
        sociability: 1.0,
        cooperativeness: 0.5,
        caution: 0.0,
        persistence: 0.0,
    };

    // No visible entities at all — socialize should be 0
    mind.visible_entities.clear();

    let _ = crate::simulation::autonomy::evaluate_goals(
        &mut mind,
        0.0,   // no hunger
        100.0, // full health
        25 * crate::simulation::time::TICKS_PER_YEAR,
        &personality,
        None,
        DecisionContext {
            tick: 0,
            origin: (0, 0),
            food_in_inventory: 0,
            visible_food_need: 0.0,
            best_remembered_social_score: None,
        },
    );

    assert_eq!(
        mind.utility_scores.socialize, 0.0,
        "socialize utility should be 0 with no visible candidates"
    );
}

#[test]
fn socialize_utility_nonzero_with_positive_memory_no_visible() {
    use crate::simulation::entity::Personality;

    let mut mind = crate::simulation::autonomy::Mind::default();
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
        .push(crate::simulation::autonomy::KnownEntity {
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
        .push(crate::simulation::autonomy::KnownEntity {
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

    let _ = crate::simulation::autonomy::evaluate_goals(
        &mut mind,
        0.0,   // no hunger
        100.0, // full health
        25 * crate::simulation::time::TICKS_PER_YEAR,
        &personality,
        None,
        DecisionContext {
            tick: 0,
            origin: (0, 0),
            food_in_inventory: 0,
            visible_food_need: 0.0,
            best_remembered_social_score: None,
        },
    );

    assert!(
        mind.utility_scores.socialize > 0.0,
        "socialize utility should be > 0 with high-affinity memories even without visible candidates: score={}",
        mind.utility_scores.socialize
    );

    // Utility should be reduced compared to having visible candidates
    mind.visible_entities = vec![10, 11];
    let _ = crate::simulation::autonomy::evaluate_goals(
        &mut mind,
        0.0,
        100.0,
        25 * crate::simulation::time::TICKS_PER_YEAR,
        &personality,
        None,
        DecisionContext {
            tick: 0,
            origin: (0, 0),
            food_in_inventory: 0,
            visible_food_need: 0.0,
            best_remembered_social_score: None,
        },
    );
    let visible_score = mind.utility_scores.socialize;

    // Now clear visibility again and check memory-only score
    mind.visible_entities.clear();
    let _ = crate::simulation::autonomy::evaluate_goals(
        &mut mind,
        0.0,
        100.0,
        25 * crate::simulation::time::TICKS_PER_YEAR,
        &personality,
        None,
        DecisionContext {
            tick: 0,
            origin: (0, 0),
            food_in_inventory: 0,
            visible_food_need: 0.0,
            best_remembered_social_score: None,
        },
    );
    let memory_only_score = mind.utility_scores.socialize;

    assert!(
        visible_score > memory_only_score,
        "visible candidates should yield higher utility than memory-only: visible={visible_score}, memory={memory_only_score}"
    );
}

#[test]
fn urgent_hunger_still_prioritizes_eating() {
    let mut world = plain_grid(8, 8);
    let food_index = (4 * world.width + 3) as usize;
    world.resources[food_index] = Some(crate::world::ResourceDeposit {
        kind: crate::world::ResourceKind::Food,
        amount: 500,
    });

    let mut sim = Simulation {
        entities: vec![default_adult(1, 3, 3), default_adult(2, 5, 5)],
        next_entity_id: 3,
        seed: 42,
        ..Simulation::default()
    };
    sim.entities[0].personality.sociability = 1.0;
    sim.entities[0].personality.curiosity = 0.0;
    sim.entities[0].hunger = 90.0;

    sim.step(&mut world);

    assert_eq!(
        sim.entities()[0].mind.current_goal,
        Some(Goal::AcquireResource),
        "urgent hunger must keep the survival priority unchanged"
    );
}
