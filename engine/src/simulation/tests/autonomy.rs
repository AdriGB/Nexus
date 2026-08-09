use super::super::autonomy::{
    exploration_target, perceive, Action, Goal, KnownResource, Mind, URGENT_HUNGER_THRESHOLD,
};
use super::super::spatial::{EntitySnapshot, SpatialGrid};
use super::super::time::TICKS_PER_YEAR;
use super::super::Simulation;
use super::support::*;
use crate::world::ResourceKind;

#[test]
fn distant_food_is_not_known_without_perception() {
    let mut world = grid_from_rows(&["PPPPPPPPPPPPPPPPPPPF"]);
    let mut simulation = simulation_with_entity(0, 0, 90.0);
    simulation.entities[0].age_ticks = 25 * TICKS_PER_YEAR;
    simulation.step(&mut world);

    let entity = &simulation.entities()[0];
    assert!(entity.mind.memory.known_resources.is_empty());
    assert_eq!(entity.mind.current_goal, Some(Goal::Explore));
}

#[test]
fn entity_remembers_seen_food_and_interrupts_exploration_when_hungry() {
    let mut world = grid_from_rows(&["PPPPPFPPPPPPPPPP"]);
    let mut simulation = simulation_with_entity(0, 0, 0.0);
    simulation.entities[0].age_ticks = 25 * TICKS_PER_YEAR;
    simulation.step(&mut world);
    assert_eq!(
        simulation.entities()[0].mind.memory.known_resources.len(),
        1
    );
    assert_eq!(
        simulation.entities()[0].mind.current_goal,
        Some(Goal::Explore)
    );

    simulation.entities[0].hunger = URGENT_HUNGER_THRESHOLD;
    simulation.step(&mut world);
    assert_eq!(simulation.entities()[0].mind.current_goal, Some(Goal::Eat));
    assert!(simulation.entities()[0]
        .mind
        .current_plan
        .iter()
        .any(|action| matches!(action, Action::Consume(ResourceKind::Food))));
}

#[test]
fn exploration_goal_is_retained_while_its_plan_is_viable() {
    let mut world = plain_grid(32, 8);
    let mut simulation = simulation_with_entity(0, 0, 0.0);
    simulation.entities[0].age_ticks = 25 * TICKS_PER_YEAR;
    simulation.step(&mut world);
    let goal_since = simulation.entities()[0].mind.goal_since_tick;
    assert_eq!(
        simulation.entities()[0].mind.current_goal,
        Some(Goal::Explore)
    );

    simulation.step(&mut world);
    assert_eq!(
        simulation.entities()[0].mind.current_goal,
        Some(Goal::Explore)
    );
    assert_eq!(simulation.entities()[0].mind.goal_since_tick, goal_since);
}

#[test]
fn stale_resource_memory_is_forgotten() {
    let mut world = grid_from_rows(&["FPPPPPPPPPPPPPPPPPPP"]);
    let mut observer = entity(1, 0, 0, 0.0);
    perceive(&mut observer.mind, &world, (0, 0), 0);
    assert_eq!(observer.mind.memory.known_resources.len(), 1);

    perceive(&mut observer.mind, &world, (19, 0), 3_000);
    assert!(observer.mind.memory.known_resources.is_empty());
    world.resources[0] = None;
}

#[test]
fn unreachable_food_is_temporarily_avoided() {
    let mut world = grid_from_rows(&["P#F"]);
    let mut simulation = simulation_with_entity(0, 0, 90.0);
    simulation.entities[0].age_ticks = 25 * TICKS_PER_YEAR;
    simulation.step(&mut world);

    let remembered = &simulation.entities()[0].mind.memory.known_resources[0];
    assert_eq!(remembered.failed_attempts, 1);
    assert!(remembered.avoid_until_tick > simulation.tick());
    assert_ne!(simulation.entities()[0].mind.current_goal, Some(Goal::Eat));
}

#[test]
fn false_food_memory_is_corrected_when_the_target_becomes_visible() {
    let mut world = plain_grid(6, 1);
    let mut observer = entity(1, 0, 0, 90.0);
    observer.age_ticks = 25 * TICKS_PER_YEAR;
    observer.mind.perception_radius = 1;
    observer.mind.memory.known_resources.push(KnownResource {
        x: 4,
        y: 0,
        kind: ResourceKind::Food,
        last_seen_tick: 0,
        estimated_amount: 20,
        failed_attempts: 0,
        avoid_until_tick: 0,
    });
    let mut simulation = Simulation {
        entities: vec![observer],
        next_entity_id: 2,
        ..Simulation::default()
    };

    for _ in 0..4 {
        simulation.step(&mut world);
    }
    assert!(simulation.entities()[0]
        .mind
        .memory
        .known_resources
        .is_empty());
    assert_ne!(simulation.entities()[0].mind.current_goal, Some(Goal::Eat));
    assert_eq!(simulation.food_consumed, 0);
}

#[test]
fn local_perception_reports_only_nearby_entities() {
    let mut world = plain_grid(20, 1);
    let mut simulation = Simulation {
        entities: vec![
            entity(1, 0, 0, 0.0),
            entity(2, 3, 0, 0.0),
            entity(3, 15, 0, 0.0),
        ],
        next_entity_id: 4,
        ..Simulation::default()
    };
    for entity in &mut simulation.entities {
        entity.age_ticks = 25 * TICKS_PER_YEAR;
    }
    simulation.step(&mut world);
    assert_eq!(simulation.entities()[0].mind.visible_entities, vec![2]);
}

#[test]
fn spatial_perception_matches_linear_brute_force() {
    let mut population = Vec::new();

    for i in 0..100u32 {
        let x = (i * 7) % 64;
        let y = (i * 11) % 64;
        population.push(EntitySnapshot { id: i, x, y });
    }

    let mut spatial = SpatialGrid::default();
    spatial.prepare(64, 64);

    for (index, snapshot) in population.iter().enumerate() {
        spatial.insert(index, snapshot.x, snapshot.y);
    }

    let radius = 6;

    for snapshot in &population {
        let linear =
            linear_visible_entities(snapshot.id, (snapshot.x, snapshot.y), radius, &population);

        let mut spatial_result = Vec::new();
        spatial.visit_candidates(snapshot.x, snapshot.y, radius, |index| {
            let other = population[index];
            if other.id != snapshot.id
                && snapshot.x.abs_diff(other.x) + snapshot.y.abs_diff(other.y) <= radius
            {
                spatial_result.push(other.id);
            }
        });
        spatial_result.sort_unstable();

        assert_eq!(
            linear, spatial_result,
            "mismatch for entity {} at ({}, {})",
            snapshot.id, snapshot.x, snapshot.y,
        );
    }
}

#[test]
fn exploration_never_targets_a_different_land_region() {
    let rows = [
        "PPPPPPPP########PPPPPPPP",
        "PPPPPPPP########PPPPPPPP",
        "PPPPPPPP########PPPPPPPP",
        "PPPPPPPP########PPPPPPPP",
        "PPPPPPPP########PPPPPPPP",
        "PPPPPPPP########PPPPPPPP",
        "PPPPPPPP########PPPPPPPP",
        "PPPPPPPP########PPPPPPPP",
    ];
    let mut world = grid_from_rows(&rows);
    crate::regions::detect_regions(&mut world);
    let origin = (3, 3);
    let origin_region = world.region_id_at(origin.0, origin.1);
    let mut mind = Mind::default();
    perceive(&mut mind, &world, origin, 0);

    let target = exploration_target(&mind, &world, origin, 1, 0);

    assert!(target.is_none_or(|(x, y)| world.region_id_at(x, y) == origin_region));
}
