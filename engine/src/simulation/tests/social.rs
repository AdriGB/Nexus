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
