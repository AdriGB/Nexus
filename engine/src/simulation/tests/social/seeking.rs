use super::*;

fn relationship_seeking_simulation() -> Simulation {
    use crate::simulation::autonomy::KnownEntity;

    let mut actor = default_adult(1, 5, 5);
    actor.personality.sociability = 1.0;
    actor.personality.curiosity = 0.0;
    let mut family = default_adult(2, 15, 5);
    family.mother_id = Some(1);
    let unrelated = default_adult(3, 15, 6);
    actor.mind.memory.known_entities = vec![
        KnownEntity {
            id: 2,
            first_seen_tick: 0,
            last_seen_tick: 0,
            last_seen_x: 15,
            last_seen_y: 5,
            observed_ticks: 1,
            affinity: 500,
            last_interaction_tick: 0,
            interaction_count: 0,
            seek_retry_after_tick: None,
        },
        KnownEntity {
            id: 3,
            first_seen_tick: 0,
            last_seen_tick: 0,
            last_seen_x: 15,
            last_seen_y: 6,
            observed_ticks: 1,
            affinity: 500,
            last_interaction_tick: 0,
            interaction_count: 0,
            seek_retry_after_tick: None,
        },
    ];
    Simulation {
        entities: vec![actor, family, unrelated],
        next_entity_id: 4,
        ..Simulation::default()
    }
}

#[test]
fn family_seeking_reuses_socialize_memory_and_normal_pathfinding() {
    let mut simulation = relationship_seeking_simulation();
    simulation.step(&mut plain_grid(24, 12));
    assert_eq!(
        simulation.entities()[0].mind.current_goal,
        Some(Goal::Socialize)
    );
    assert_eq!(
        simulation.entities()[0].mind.current_action(),
        Some(Action::ApproachEntity(2))
    );
    assert_eq!(simulation.entities()[0].path.last().copied(), Some((15, 5)));
}

#[test]
fn visible_unpartnered_adult_is_preferred_as_potential_partner() {
    let mut actor = default_adult(1, 5, 5);
    actor.personality.sociability = 1.0;
    actor.personality.curiosity = 0.0;
    let candidate = default_adult(2, 10, 5);
    let mut unavailable = default_adult(3, 10, 6);
    unavailable.partner_id = Some(99);
    let mut simulation = Simulation {
        entities: vec![actor, candidate, unavailable],
        next_entity_id: 4,
        ..Simulation::default()
    };
    simulation.step(&mut plain_grid(20, 12));
    assert_eq!(
        simulation.entities()[0].mind.current_goal,
        Some(Goal::Socialize)
    );
    assert_eq!(
        simulation.entities()[0].mind.current_action(),
        Some(Action::ApproachEntity(2))
    );
    assert!(simulation
        .entities()
        .iter()
        .all(|entity| entity.partner_id != Some(2)));
}

#[test]
fn relationship_seeking_matches_normal_and_profiled_paths() {
    let mut normal = relationship_seeking_simulation();
    let mut profiled = normal.clone();
    let mut autonomy_profiled = normal.clone();
    normal.step(&mut plain_grid(24, 12));
    profiled.profile_step(&mut plain_grid(24, 12));
    autonomy_profiled.profile_autonomy_step(&mut plain_grid(24, 12));
    let state = |simulation: &Simulation| {
        simulation
            .entities()
            .iter()
            .map(|entity| {
                (
                    entity.id,
                    entity.mind.current_goal,
                    entity.mind.current_action(),
                    entity.path.clone(),
                    entity.partner_id,
                    entity.household_id,
                )
            })
            .collect::<Vec<_>>()
    };
    assert_eq!(state(&normal), state(&profiled));
    assert_eq!(state(&normal), state(&autonomy_profiled));
}
