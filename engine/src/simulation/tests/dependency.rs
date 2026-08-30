use super::super::autonomy::{Action, Goal};
use super::super::config::{FOOD_CONSUMED_PER_MEAL, HUNGER_PER_TICK, HUNGER_REDUCTION_PER_MEAL};
use super::super::entity::{LifeStage, Pregnancy, Sex};
use super::super::time::{CHILD_AGE_END, GESTATION_TICKS, TICKS_PER_YEAR};
use super::super::Simulation;
use super::support::*;

#[test]
fn infant_is_carried_by_caregiver() {
    let mut world = plain_grid(10, 10);
    let mut simulation = Simulation::with_population(42, &world, 1);
    let caregiver_id = simulation.entities()[0].id;

    simulation.push_entity((5, 5), 0);
    let infant_id = simulation.entities().last().unwrap().id;
    simulation.entities.last_mut().unwrap().caregiver_id = Some(caregiver_id);
    simulation.step(&mut world);

    let caregiver = simulation
        .entities()
        .iter()
        .find(|entity| entity.id == caregiver_id)
        .unwrap();
    let infant = simulation
        .entities()
        .iter()
        .find(|entity| entity.id == infant_id)
        .unwrap();
    assert_eq!((infant.x, infant.y), (caregiver.x, caregiver.y));
}

#[test]
fn child_follows_caregiver() {
    let mut world = plain_grid(10, 10);
    let mut simulation = simulation_with_entity(0, 0, 0.0);
    simulation.entities[0].age_ticks = 25 * TICKS_PER_YEAR;
    simulation.entities[0].health = 1.0;
    simulation.entities[0].personality.curiosity = 0.0;
    simulation.entities[0].personality.caution = 1.0;
    let caregiver_id = simulation.entities[0].id;

    simulation.push_entity((9, 9), 5 * TICKS_PER_YEAR);
    let child_id = simulation.entities().last().unwrap().id;
    simulation.entities.last_mut().unwrap().caregiver_id = Some(caregiver_id);
    simulation.resume();
    simulation.advance(40, &mut world);

    let child = simulation
        .entities()
        .iter()
        .find(|entity| entity.id == child_id)
        .unwrap();
    let caregiver = simulation
        .entities()
        .iter()
        .find(|entity| entity.id == caregiver_id)
        .unwrap();
    let distance = child.x.abs_diff(caregiver.x) + child.y.abs_diff(caregiver.y);
    assert!(distance <= 2, "child distance from caregiver is {distance}");
}

#[test]
fn child_never_explores() {
    let mut world = plain_grid(32, 32);
    let mut simulation = Simulation::with_population(42, &world, 1);
    let caregiver_id = simulation.entities()[0].id;

    simulation.push_entity((0, 0), 5 * TICKS_PER_YEAR);
    let child_id = simulation.entities().last().unwrap().id;
    simulation.entities.last_mut().unwrap().caregiver_id = Some(caregiver_id);
    simulation.step(&mut world);

    let child = simulation
        .entities()
        .iter()
        .find(|entity| entity.id == child_id)
        .unwrap();
    assert_ne!(child.mind.current_goal, Some(Goal::Explore));
}

#[test]
fn hungry_child_with_unreachable_food_never_explores() {
    let mut world = grid_from_rows(&["P#F"]);
    let mut simulation = simulation_with_entity(0, 0, 0.0);
    simulation.entities[0].age_ticks = 25 * TICKS_PER_YEAR;
    let caregiver_id = simulation.entities[0].id;

    simulation.push_entity((0, 0), 5 * TICKS_PER_YEAR);
    let child_id = simulation.entities().last().unwrap().id;
    let child = simulation.entities.last_mut().unwrap();
    child.caregiver_id = Some(caregiver_id);
    child.hunger = 90.0;

    simulation.step(&mut world);
    simulation.step(&mut world);

    let child = simulation
        .entities()
        .iter()
        .find(|entity| entity.id == child_id)
        .unwrap();
    assert_ne!(child.mind.current_goal, Some(Goal::Explore));
}

#[test]
fn caregiver_feeds_infant() {
    let mut world = grid_from_rows(&["F"]);
    let mut simulation = simulation_with_entity(0, 0, 90.0);
    simulation.entities[0].age_ticks = 25 * TICKS_PER_YEAR;
    let caregiver_id = simulation.entities()[0].id;

    simulation.push_entity((0, 0), 0);
    let infant_id = simulation.entities().last().unwrap().id;
    let infant = simulation.entities.last_mut().unwrap();
    infant.caregiver_id = Some(caregiver_id);
    infant.hunger = 80.0;
    for _ in 0..11 {
        simulation.step(&mut world);
    }

    let infant = simulation
        .entities()
        .iter()
        .find(|entity| entity.id == infant_id)
        .unwrap();
    assert!(infant.hunger < 80.0);
}

#[test]
fn caregiver_feeds_infant_proportionally() {
    let mut world = grid_from_rows(&["F"]);
    world.resources[0].as_mut().unwrap().amount = 3;
    let mut simulation = simulation_with_entity(0, 0, 90.0);
    simulation.entities[0].age_ticks = 25 * TICKS_PER_YEAR;
    let caregiver_id = simulation.entities()[0].id;

    simulation.push_entity((0, 0), 0);
    let infant_id = simulation.entities().last().unwrap().id;
    let infant = simulation.entities.last_mut().unwrap();
    infant.caregiver_id = Some(caregiver_id);
    infant.hunger = 80.0;
    for _ in 0..11 {
        simulation.step(&mut world);
    }

    let infant = simulation
        .entities()
        .iter()
        .find(|entity| entity.id == infant_id)
        .unwrap();
    let expected = 80.0 + HUNGER_PER_TICK * 11.0
        - HUNGER_REDUCTION_PER_MEAL * (3.0 / f32::from(FOOD_CONSUMED_PER_MEAL));
    assert!((infant.hunger - expected).abs() < 0.001);
}

#[test]
fn orphaned_dependent_gets_new_caregiver() {
    let mut world = plain_grid(10, 10);
    let mut simulation = Simulation::with_population(42, &world, 5);
    let previous_caregiver = simulation.entities()[0].id;

    simulation.push_entity((0, 0), 5 * TICKS_PER_YEAR);
    let child_id = simulation.entities().last().unwrap().id;
    let child = simulation.entities.last_mut().unwrap();
    child.caregiver_id = Some(previous_caregiver);
    child
        .mind
        .set_plan(Goal::Follow, vec![Action::MoveTo(9, 9)], 0);
    child.path = vec![(1, 1), (2, 2), (9, 9)];
    child.path_index = 1;
    child.movement_credit = 0.75;
    simulation.entities[0].health = 0.0;
    simulation.step(&mut world);

    let child = simulation
        .entities()
        .iter()
        .find(|entity| entity.id == child_id)
        .unwrap();
    assert!(child.caregiver_id.is_some());
    assert_ne!(child.caregiver_id, Some(previous_caregiver));
    assert_ne!(child.mind.current_goal, Some(Goal::Follow));
    assert!(child.path.is_empty());
    assert_eq!(child.path_index, 0);
    assert_eq!(child.movement_credit, 0.0);
}

#[test]
fn dependent_without_caregiver_gets_assigned_one() {
    let mut world = plain_grid(10, 10);
    let mut simulation = Simulation::with_population(42, &world, 1);
    simulation.push_entity((0, 0), 5 * TICKS_PER_YEAR);
    let child_id = simulation.entities().last().unwrap().id;
    simulation.step(&mut world);

    let child = simulation
        .entities()
        .iter()
        .find(|entity| entity.id == child_id)
        .unwrap();
    assert!(child.caregiver_id.is_some());
}

#[test]
fn newborn_gets_mother_as_caregiver() {
    let mut world = plain_grid(4, 4);
    let mut mother = fertile_entity(1, Sex::Female, 1, 1);
    let father = fertile_entity(2, Sex::Male, 2, 1);
    mother.pregnancy = Some(Pregnancy {
        father_id: 2,
        conceived_tick: 0,
        due_tick: GESTATION_TICKS,
    });
    let mut simulation = Simulation {
        tick: GESTATION_TICKS - 1,
        entities: vec![mother, father],
        next_entity_id: 3,
        seed: 42,
        ..Simulation::default()
    };

    simulation.step(&mut world);
    assert_eq!(simulation.entities().len(), 3);
    assert_eq!(simulation.entities()[2].caregiver_id, Some(1));
}

/// The caregiver index replaced a filter over `population`. This pins that it
/// yields exactly the same dependents, **in the same order**: the order matters
/// because `dependent_protection_target` breaks ties with `max_by`, which keeps
/// the *last* maximum, so reordering would change which dependent gets
/// protected without anything else giving it away.
#[test]
fn caregiver_index_matches_the_population_scan_it_replaced() {
    let world = plain_grid(10, 10);
    let mut simulation = Simulation::with_population(42, &world, 6);
    let ids: Vec<u32> = simulation
        .entities()
        .iter()
        .map(|entity| entity.id)
        .collect();

    // Two caregivers with several dependents each, at NON-contiguous
    // positions. That is what makes a reordering visible: if the index
    // reversed or reshuffled its vector, the two lists would stop matching.
    simulation.entities[0].caregiver_id = Some(ids[4]);
    simulation.entities[3].caregiver_id = Some(ids[4]);
    simulation.entities[1].caregiver_id = Some(ids[5]);
    simulation.entities[2].caregiver_id = Some(ids[5]);
    simulation.entities[5].caregiver_id = Some(ids[5]);

    simulation.rebuild_population_index(&world);

    assert!(
        simulation
            .caregiver_index
            .values()
            .any(|dependents| dependents.len() > 1),
        "degenerate test: no caregiver has more than one dependent"
    );

    let population = &simulation.population_cache;
    for entity in &simulation.entities {
        let scanned: Vec<u32> = population
            .iter()
            .enumerate()
            .filter(|(_, snapshot)| snapshot.caregiver_id == Some(entity.id))
            .map(|(index, _)| index as u32)
            .collect();
        let indexed: Vec<u32> = simulation
            .caregiver_index
            .get(&entity.id)
            .cloned()
            .unwrap_or_default();
        assert_eq!(
            indexed, scanned,
            "index for {} does not match the scan it replaced",
            entity.id
        );
    }
}

#[test]
fn adolescent_releases_caregiver() {
    let mut world = plain_grid(10, 10);
    let mut simulation = Simulation::with_population(42, &world, 1);
    let caregiver_id = simulation.entities()[0].id;

    simulation.push_entity((0, 0), CHILD_AGE_END - 1);
    let child_id = simulation.entities().last().unwrap().id;
    let child = simulation.entities.last_mut().unwrap();
    child.caregiver_id = Some(caregiver_id);
    child
        .mind
        .set_plan(Goal::Follow, vec![Action::MoveTo(9, 9)], 0);
    child.path = vec![(1, 1), (9, 9)];
    child.path_index = 1;
    child.movement_credit = 0.75;
    simulation.step(&mut world);

    let child = simulation
        .entities()
        .iter()
        .find(|entity| entity.id == child_id)
        .unwrap();
    assert_eq!(
        LifeStage::from_age_ticks(child.age_ticks),
        LifeStage::Adolescent
    );
    assert_eq!(child.caregiver_id, None);
    assert_ne!(child.mind.current_goal, Some(Goal::Follow));
    assert_ne!(child.path, vec![(1, 1), (9, 9)]);
}
