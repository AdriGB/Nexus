use super::super::entity::{Entity, Personality};
use super::super::time::TICKS_PER_YEAR;
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
    sim.entities()[entity_index]
        .mind
        .current_goal
        == Some(Goal::Socialize)
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
    assert!(socialized, "highly sociable entity should choose Socialize goal");
}

#[test]
fn unsociable_entity_rarely_socializes() {
    let mut world = plain_grid(20, 20);
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
    assert!(moved_closer, "entity should move toward socialization target");
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
    sim.entities[0].mind.memory.known_entities.push(
        super::super::autonomy::KnownEntity {
            id: 2,
            first_seen_tick: 0,
            last_seen_tick: 0,
            last_seen_x: 7,
            last_seen_y: 5,
            observed_ticks: 1,
            affinity: -500,
            last_interaction_tick: 0,
            interaction_count: 0,
        },
    );

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
    let goal_no_affinity = super::super::autonomy::evaluate_goals(
        &mut mind,
        10.0,
        100.0,
        25 * super::super::time::TICKS_PER_YEAR,
        &personality,
        None,
    );
    let score_no_affinity = mind.utility_scores.socialize;

    // Add positive affinity entities
    mind.memory.known_entities.push(
        super::super::autonomy::KnownEntity {
            id: 10,
            first_seen_tick: 0,
            last_seen_tick: 0,
            last_seen_x: 0,
            last_seen_y: 0,
            observed_ticks: 5,
            affinity: 500,
            last_interaction_tick: 0,
            interaction_count: 3,
        },
    );
    mind.memory.known_entities.push(
        super::super::autonomy::KnownEntity {
            id: 11,
            first_seen_tick: 0,
            last_seen_tick: 0,
            last_seen_x: 0,
            last_seen_y: 0,
            observed_ticks: 5,
            affinity: 300,
            last_interaction_tick: 0,
            interaction_count: 2,
        },
    );

    let _ = super::super::autonomy::evaluate_goals(
        &mut mind,
        10.0,
        100.0,
        25 * super::super::time::TICKS_PER_YEAR,
        &personality,
        None,
    );
    let score_with_affinity = mind.utility_scores.socialize;

    assert!(
        score_with_affinity > score_no_affinity,
        "socialize utility should increase with positive affinity: no_affinity={score_no_affinity}, with_affinity={score_with_affinity}"
    );
}
