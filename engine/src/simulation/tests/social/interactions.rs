use super::*;

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
fn process_social_interactions_mutates_once_and_reports_directed_crossings() {
    let mut a = default_adult(1, 2, 2);
    let mut b = default_adult(2, 2, 3);
    a.mind.visible_entities = vec![2];
    b.mind.visible_entities = vec![1];
    a.mind
        .memory
        .known_entities
        .push(crate::simulation::autonomy::KnownEntity {
            id: 2,
            first_seen_tick: 0,
            last_seen_tick: 0,
            last_seen_x: 2,
            last_seen_y: 3,
            observed_ticks: 1,
            affinity: 99,
            last_interaction_tick: 0,
            interaction_count: 0,
            seek_retry_after_tick: None,
        });
    b.mind
        .memory
        .known_entities
        .push(crate::simulation::autonomy::KnownEntity {
            id: 1,
            first_seen_tick: 0,
            last_seen_tick: 0,
            last_seen_x: 2,
            last_seen_y: 2,
            observed_ticks: 1,
            affinity: -201,
            last_interaction_tick: 0,
            interaction_count: 0,
            seek_retry_after_tick: None,
        });
    a.personality.cooperativeness = 0.0;

    let mut entities = vec![a, b];
    let population = vec![
        crate::simulation::spatial::EntitySnapshot {
            id: 1,
            x: 2,
            y: 2,
            hunger: 0.0,
            caregiver_id: None,
            household_id: None,
            partner_id: None,
            mother_id: None,
            father_id: None,
            is_adult: true,
            is_child: false,
            is_infant: false,
        },
        crate::simulation::spatial::EntitySnapshot {
            id: 2,
            x: 2,
            y: 3,
            hunger: 0.0,
            caregiver_id: None,
            household_id: None,
            partner_id: None,
            mother_id: None,
            father_id: None,
            is_adult: true,
            is_child: false,
            is_infant: false,
        },
    ];
    let interactions = crate::simulation::autonomy::process_social_interactions(
        &mut entities,
        &population,
        100,
        None,
    );

    assert_eq!(interactions.len(), 1);
    assert_eq!(entities[0].mind.memory.affinity_to(2), Some(103));
    assert_eq!(entities[1].mind.memory.affinity_to(1), Some(-201));
    let actor_change = interactions[0]
        .actor_affinity_change
        .expect("A -> B should enter bonded");
    assert_eq!(actor_change.target_id, 2);
    assert_eq!(actor_change.previous_affinity, 99);
    assert_eq!(actor_change.new_affinity, 103);
    assert_eq!(actor_change.delta, 4);
    assert_eq!(interactions[0].target_affinity_change, None);
}

#[test]
fn social_funnel_counts_each_stage() {
    fn snapshot(id: u32, x: u32, y: u32) -> crate::simulation::spatial::EntitySnapshot {
        crate::simulation::spatial::EntitySnapshot {
            id,
            x,
            y,
            hunger: 0.0,
            caregiver_id: None,
            household_id: None,
            partner_id: None,
            mother_id: None,
            father_id: None,
            is_adult: true,
            is_child: false,
            is_infant: false,
        }
    }

    fn remember(mind: &mut crate::simulation::autonomy::Mind, id: u32) {
        mind.memory
            .known_entities
            .push(crate::simulation::autonomy::KnownEntity {
                id,
                first_seen_tick: 0,
                last_seen_tick: 0,
                last_seen_x: 0,
                last_seen_y: 0,
                observed_ticks: 1,
                affinity: 0,
                last_interaction_tick: 0,
                interaction_count: 0,
                seek_retry_after_tick: None,
            });
    }

    let mut near_a = default_adult(1, 10, 10);
    let mut near_b = default_adult(2, 10, 11);
    let mut far = default_adult(3, 40, 40);
    near_a.mind.visible_entities = vec![2, 3];
    near_b.mind.visible_entities = vec![1];
    far.mind.visible_entities = vec![1];
    remember(&mut near_a.mind, 2);
    remember(&mut near_b.mind, 1);

    let mut entities = vec![near_a, near_b, far];
    let population = vec![
        snapshot(1, 10, 10),
        snapshot(2, 10, 11),
        snapshot(3, 40, 40),
    ];

    let mut work = crate::simulation::WorkCounters::default();
    let interactions = crate::simulation::autonomy::process_social_interactions(
        &mut entities,
        &population,
        100,
        Some(&mut work),
    );

    assert_eq!(
        work.social_pairs_scanned, 2,
        "entity 1 sees two candidates and holds the lower id in both pairs"
    );
    assert_eq!(
        work.social_pairs_in_radius, 1,
        "the distant pair must be dropped by the SOCIAL_RADIUS gate"
    );
    assert_eq!(work.social_pairs_mutual, 1);
    assert_eq!(
        work.social_pairs_due, 1,
        "the surviving pair has no interaction history, so it is due"
    );
    assert_eq!(interactions.len(), 1);
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
