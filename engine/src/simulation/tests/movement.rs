use super::super::autonomy::{effective_movement_speed, Action, Goal};
use super::super::config::BASE_MOVEMENT_SPEED;
use super::super::entity::{EntityActivity, Pregnancy};
use super::super::time::{GESTATION_TICKS, TICKS_PER_WEEK, TICKS_PER_YEAR};
use super::super::Simulation;
use super::support::*;

#[test]
fn entity_stores_and_follows_unsmoothed_path() {
    let mut world = grid_from_rows(&["PPPPP", "P###F", "PPPPP"]);
    let mut simulation = simulation_with_entity(0, 1, 59.0);
    simulation.entities[0].age_ticks = 25 * TICKS_PER_YEAR;
    simulation.step(&mut world);
    let original_path = simulation.entities()[0].path.clone();
    assert!(original_path.len() > 2);
    assert_eq!(simulation.entities()[0].path_index, 1);
    simulation.step(&mut world);
    assert_eq!(simulation.entities()[0].path, original_path);
    assert_eq!(simulation.entities()[0].path_index, 2);
}

#[test]
fn mountain_movement_requires_four_ticks() {
    let mut world = grid_from_rows(&["PMF"]);
    let mut simulation = simulation_with_entity(0, 0, 90.0);
    simulation.entities[0].age_ticks = 25 * TICKS_PER_YEAR;

    for _ in 0..3 {
        simulation.step(&mut world);
        assert_eq!(simulation.entities()[0].x, 0, "should not move yet");
    }

    simulation.step(&mut world);
    assert_eq!(
        simulation.entities()[0].x,
        1,
        "should cross Mountain on tick 4"
    );

    simulation.step(&mut world);
    assert_eq!(
        simulation.entities()[0].x,
        2,
        "should cross Plains on tick 5"
    );
}

#[test]
fn resting_clears_movement_credit() {
    let mut world = grid_from_rows(&["P"]);
    let mut simulation = simulation_with_entity(0, 0, 0.0);
    simulation.entities[0].age_ticks = 25 * TICKS_PER_YEAR;
    simulation.entities[0].movement_credit = 0.75;

    simulation.step(&mut world);

    assert_eq!(simulation.entities()[0].movement_credit, 0.0);
}

#[test]
fn diagonal_movement_requires_sqrt2_credit() {
    let mut world = plain_grid(2, 2);
    let mut mover = entity(1, 0, 0, 0.0);
    mover.age_ticks = 25 * TICKS_PER_YEAR;
    mover.path = vec![(1, 1)];
    mover
        .mind
        .set_plan(Goal::Explore, vec![Action::ExploreArea(1, 1)], 0);
    mover.activity = EntityActivity::Exploring;

    let mut simulation = Simulation {
        entities: vec![mover],
        next_entity_id: 2,
        ..Simulation::default()
    };

    simulation.step(&mut world);
    assert_eq!(
        (simulation.entities()[0].x, simulation.entities()[0].y),
        (0, 0)
    );

    simulation.step(&mut world);
    assert_eq!(
        (simulation.entities()[0].x, simulation.entities()[0].y),
        (1, 1)
    );
}

#[test]
fn pregnant_entity_moves_slower() {
    let mut world = plain_grid(10, 1);
    let mut simulation = simulation_with_entity(0, 0, 90.0);
    simulation.entities[0].age_ticks = 25 * TICKS_PER_YEAR;

    simulation.entities[0].path = vec![(1, 0), (2, 0)];
    simulation.entities[0]
        .mind
        .set_plan(Goal::Explore, vec![Action::ExploreArea(2, 0)], 0);
    simulation.entities[0].activity = EntityActivity::Exploring;

    simulation.step(&mut world);
    assert_eq!(
        simulation.entities()[0].x,
        1,
        "non-pregnant moves on tick 1"
    );

    simulation.entities[0].x = 0;
    simulation.entities[0].path = vec![(1, 0)];
    simulation.entities[0].path_index = 0;
    simulation.entities[0].movement_credit = 0.0;
    simulation.entities[0].pregnancy = Some(Pregnancy {
        father_id: 2,
        conceived_tick: 0,
        due_tick: GESTATION_TICKS,
    });
    simulation.entities[0]
        .mind
        .set_plan(Goal::Explore, vec![Action::ExploreArea(1, 0)], 0);

    simulation.tick = 36 * TICKS_PER_WEEK;
    simulation.step(&mut world);
    assert_eq!(
        simulation.entities()[0].x,
        0,
        "pregnant at week 36 should not move on first tick"
    );

    simulation.step(&mut world);
    assert_eq!(
        simulation.entities()[0].x,
        1,
        "pregnant at week 36 should move on second tick"
    );
}

#[test]
fn pregnancy_speed_transitions_at_phase_boundaries() {
    let base = BASE_MOVEMENT_SPEED;
    let speed_at_week = |week: u64| -> f32 {
        let mut entity = entity(0, 0, 0, 0.0);
        entity.age_ticks = 25 * TICKS_PER_YEAR;
        entity.pregnancy = Some(Pregnancy {
            father_id: 0,
            conceived_tick: 0,
            due_tick: GESTATION_TICKS,
        });
        effective_movement_speed(&entity, week * TICKS_PER_WEEK)
    };

    assert_eq!(speed_at_week(0), base * 1.0);
    assert_eq!(speed_at_week(13), base * 1.0);
    assert_eq!(speed_at_week(14), base * 0.9);
    assert_eq!(speed_at_week(27), base * 0.9);
    assert_eq!(speed_at_week(28), base * 0.75);
    assert_eq!(speed_at_week(35), base * 0.75);
    assert_eq!(speed_at_week(36), base * 0.6);
    assert_eq!(speed_at_week(40), base * 0.6);
}
