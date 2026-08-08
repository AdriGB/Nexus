use super::autonomy::URGENT_HUNGER_THRESHOLD;
use super::entity::Pregnancy;
use super::lifecycle::{
    conception_roll, female_is_fertile, male_is_fertile, DAILY_CONCEPTION_SCALE,
};
use super::time::{
    FEMALE_REPRODUCTIVE_AGE_END, FOUNDER_AGE_MAX, FOUNDER_AGE_MIN, GESTATION_TICKS,
    MALE_REPRODUCTIVE_AGE_END, POSTPARTUM_TICKS, REPRODUCTIVE_AGE_START, TICKS_PER_DAY,
    TICKS_PER_HOUR, TICKS_PER_YEAR,
};
use super::*;
use crate::world::{ResourceDeposit, ResourceKind, Terrain, Tile};

fn grid_from_rows(rows: &[&str]) -> Grid {
    let height = rows.len() as u32;
    let width = rows.first().map_or(0, |row| row.len()) as u32;
    let tiles = rows
        .iter()
        .flat_map(|row| row.chars())
        .map(|symbol| Tile {
            terrain: match symbol {
                'P' | 'F' => Terrain::Plains,
                'M' => Terrain::Mountain,
                '#' => Terrain::DeepWater,
                _ => panic!("unknown terrain symbol: {symbol}"),
            },
            altitude: 0.0,
            moisture: 0.5,
            temperature: 0.5,
        })
        .collect();
    let resources = rows
        .iter()
        .flat_map(|row| row.chars())
        .map(|symbol| {
            (symbol == 'F').then_some(ResourceDeposit {
                kind: ResourceKind::Food,
                amount: 20,
            })
        })
        .collect();
    Grid {
        width,
        height,
        tiles,
        region_ids: Vec::new(),
        regions: Vec::new(),
        resources,
    }
}

fn plain_grid(width: u32, height: u32) -> Grid {
    let row = "P".repeat(width as usize);
    let rows: Vec<_> = (0..height).map(|_| row.as_str()).collect();
    grid_from_rows(&rows)
}

fn entity(id: u32, x: u32, y: u32, hunger: f32) -> Entity {
    Entity {
        id,
        x,
        y,
        sex: Sex::Female,
        lifespan_ticks: 800_000,
        hunger,
        health: MAX_HEALTH,
        age_ticks: 0,
        path: Vec::new(),
        path_index: 0,
        activity: EntityActivity::Idle,
        mind: Mind::default(),
        pregnancy: None,
        postpartum_until_tick: 0,
    }
}

fn simulation_with_entity(x: u32, y: u32, hunger: f32) -> Simulation {
    Simulation {
        entities: vec![entity(1, x, y, hunger)],
        next_entity_id: 2,
        ..Simulation::default()
    }
}

fn fertile_entity(id: u32, sex: Sex, x: u32, y: u32) -> Entity {
    let mut entity = entity(id, x, y, 0.0);
    entity.sex = sex;
    entity.age_ticks = 25 * TICKS_PER_YEAR;
    entity
}

#[test]
fn simulation_starts_paused_at_tick_zero() {
    let simulation = Simulation::default();
    assert_eq!(simulation.tick(), 0);
    assert!(simulation.is_paused());
}

#[test]
fn spawns_multiple_entities_with_unique_ids_and_positions() {
    let world = plain_grid(10, 10);
    let simulation = Simulation::with_population(42, &world, 10);
    let ids: HashSet<_> = simulation
        .entities()
        .iter()
        .map(|entity| entity.id)
        .collect();
    let positions: HashSet<_> = simulation
        .entities()
        .iter()
        .map(|entity| (entity.x, entity.y))
        .collect();
    assert_eq!(ids.len(), 10);
    assert_eq!(positions.len(), 10);
}

#[test]
fn paused_simulation_does_not_change_entities() {
    let mut world = grid_from_rows(&["PF"]);
    let mut simulation = simulation_with_entity(0, 0, 59.0);
    simulation.advance(10, &mut world);
    assert_eq!(simulation.tick(), 0);
    assert_eq!(simulation.entities()[0].hunger, 59.0);
}

#[test]
fn entity_stores_and_follows_unsmoothed_path() {
    let mut world = grid_from_rows(&["PPPPP", "P###F", "PPPPP"]);
    let mut simulation = simulation_with_entity(0, 1, 59.0);
    simulation.step(&mut world);
    let original_path = simulation.entities()[0].path.clone();
    assert!(original_path.len() > 2);
    assert_eq!(simulation.entities()[0].path_index, 1);
    simulation.step(&mut world);
    assert_eq!(simulation.entities()[0].path, original_path);
    assert_eq!(simulation.entities()[0].path_index, 2);
}

#[test]
fn competing_entities_consume_a_finite_deposit_once() {
    let mut world = grid_from_rows(&["F"]);
    world.resources[0].as_mut().unwrap().amount = 10;
    let mut simulation = Simulation {
        entities: vec![entity(1, 0, 0, 60.0), entity(2, 0, 0, 60.0)],
        next_entity_id: 3,
        ..Simulation::default()
    };
    simulation.step(&mut world);

    assert!(world.resources[0].is_none());
    assert_eq!(simulation.food_consumed, 10);
    assert!(simulation.entities()[0].hunger < simulation.entities()[1].hunger);
    assert_eq!(simulation.world_revision(), 1);
}

#[test]
fn starving_entity_loses_health_and_dies() {
    let mut world = grid_from_rows(&["P"]);
    let mut starving = entity(1, 0, 0, MAX_HUNGER);
    starving.health = STARVATION_DAMAGE_PER_TICK;
    let mut simulation = Simulation {
        entities: vec![starving],
        next_entity_id: 2,
        ..Simulation::default()
    };
    simulation.step(&mut world);
    assert!(simulation.entities().is_empty());
    assert_eq!(simulation.population_stats().deaths, 1);
}

#[test]
fn entity_ids_are_never_reused_after_death() {
    let mut world = plain_grid(3, 1);
    let mut simulation = Simulation::with_population(42, &world, 2);
    simulation.entities[0].health = 0.0;
    simulation.step(&mut world);
    assert_eq!(simulation.spawn_entities(&world, 1), 1);
    let ids: Vec<_> = simulation
        .entities()
        .iter()
        .map(|entity| entity.id)
        .collect();
    assert_eq!(ids, vec![2, 3]);
}

#[test]
fn age_increases_once_per_tick() {
    let mut world = grid_from_rows(&["P"]);
    let mut simulation = simulation_with_entity(0, 0, 0.0);
    simulation.step(&mut world);
    assert_eq!(simulation.entities()[0].age_ticks, 1);
}

#[test]
fn one_tick_represents_one_hour() {
    assert_eq!(TICKS_PER_HOUR, 1);
    assert_eq!(TICKS_PER_DAY, 24);
    assert_eq!(TICKS_PER_YEAR, 8_760);
}

#[test]
fn same_seed_produces_same_founder_biology() {
    let world = plain_grid(10, 10);
    let left = Simulation::with_population(91, &world, 10);
    let right = Simulation::with_population(91, &world, 10);
    let biology = |simulation: &Simulation| {
        simulation
            .entities()
            .iter()
            .map(|entity| (entity.sex, entity.age_ticks, entity.lifespan_ticks))
            .collect::<Vec<_>>()
    };
    assert_eq!(biology(&left), biology(&right));
}

#[test]
fn founders_have_deterministic_adult_demographics() {
    let world = plain_grid(10, 10);
    let simulation = Simulation::with_population(42, &world, 10);
    assert!(simulation.entities().iter().all(|entity| {
        entity.age_ticks >= FOUNDER_AGE_MIN && entity.age_ticks <= FOUNDER_AGE_MAX
    }));
    let lifespans: HashSet<_> = simulation
        .entities()
        .iter()
        .map(|entity| entity.lifespan_ticks)
        .collect();
    assert!(lifespans.len() > 1);
    let sexes: HashSet<_> = simulation
        .entities()
        .iter()
        .map(|entity| entity.sex)
        .collect();
    assert_eq!(sexes.len(), 2);
}

#[test]
fn newborn_starts_at_age_zero() {
    let mut simulation = Simulation {
        seed: 42,
        ..Simulation::default()
    };
    simulation.push_newborn((0, 0)).unwrap();
    assert_eq!(simulation.entities()[0].age_ticks, 0);
    assert!(simulation.entities()[0].lifespan_ticks > FOUNDER_AGE_MAX);
}

#[test]
fn conception_requires_female_and_male() {
    for (left_sex, right_sex) in [(Sex::Female, Sex::Female), (Sex::Male, Sex::Male)] {
        let mut entities = vec![
            fertile_entity(1, left_sex, 0, 0),
            fertile_entity(2, right_sex, 1, 0),
        ];
        assert_eq!(
            lifecycle::try_conceptions(
                &mut entities,
                TICKS_PER_DAY,
                42,
                MAX_HEALTH,
                DAILY_CONCEPTION_SCALE,
            ),
            0
        );
        assert!(entities.iter().all(|entity| entity.pregnancy.is_none()));
    }
}

#[test]
fn underage_parent_cannot_conceive() {
    let mut underage_female = fertile_entity(1, Sex::Female, 0, 0);
    underage_female.age_ticks = REPRODUCTIVE_AGE_START - 1;
    let male = fertile_entity(2, Sex::Male, 1, 0);
    let mut entities = vec![underage_female, male];
    assert_eq!(
        lifecycle::try_conceptions(
            &mut entities,
            TICKS_PER_DAY,
            42,
            MAX_HEALTH,
            DAILY_CONCEPTION_SCALE,
        ),
        0
    );

    entities[0] = fertile_entity(1, Sex::Female, 0, 0);
    entities[1].age_ticks = REPRODUCTIVE_AGE_START - 1;
    assert_eq!(
        lifecycle::try_conceptions(
            &mut entities,
            TICKS_PER_DAY,
            42,
            MAX_HEALTH,
            DAILY_CONCEPTION_SCALE,
        ),
        0
    );
}

#[test]
fn reproductive_age_windows_are_exclusive_at_the_end() {
    let mut female = fertile_entity(1, Sex::Female, 0, 0);
    female.age_ticks = FEMALE_REPRODUCTIVE_AGE_END - 1;
    assert!(female_is_fertile(&female, 0, MAX_HEALTH));
    female.age_ticks = FEMALE_REPRODUCTIVE_AGE_END;
    assert!(!female_is_fertile(&female, 0, MAX_HEALTH));

    let mut male = fertile_entity(2, Sex::Male, 0, 0);
    male.age_ticks = MALE_REPRODUCTIVE_AGE_END - 1;
    assert!(male_is_fertile(&male, MAX_HEALTH));
    male.age_ticks = MALE_REPRODUCTIVE_AGE_END;
    assert!(!male_is_fertile(&male, MAX_HEALTH));
}

#[test]
fn conception_creates_pregnancy_not_child() {
    let mut entities = vec![
        fertile_entity(1, Sex::Female, 0, 0),
        fertile_entity(2, Sex::Male, 1, 0),
    ];
    let tick = TICKS_PER_DAY;
    assert_eq!(
        lifecycle::try_conceptions(&mut entities, tick, 42, MAX_HEALTH, DAILY_CONCEPTION_SCALE,),
        1
    );
    assert_eq!(entities.len(), 2);
    let pregnancy = entities[0].pregnancy.unwrap();
    assert_eq!(pregnancy.father_id, 2);
    assert_eq!(pregnancy.conceived_tick, tick);
    assert_eq!(pregnancy.due_tick, tick + GESTATION_TICKS);
}

#[test]
fn conception_roll_is_deterministic() {
    let first = conception_roll(42, 7, 11, 240);
    let second = conception_roll(42, 7, 11, 240);
    assert_eq!(first, second);
    assert!(first < DAILY_CONCEPTION_SCALE);
}

#[test]
fn simulation_does_not_check_conception_every_hour() {
    let mut simulation = Simulation {
        tick: 1,
        entities: vec![
            fertile_entity(1, Sex::Female, 0, 0),
            fertile_entity(2, Sex::Male, 1, 0),
        ],
        next_entity_id: 3,
        seed: 42,
        ..Simulation::default()
    };
    simulation.try_daily_conceptions();
    assert!(simulation.entities()[0].pregnancy.is_none());
}

#[test]
fn birth_occurs_exactly_at_due_tick_and_sets_postpartum() {
    let mut world = plain_grid(4, 4);
    let mut mother = fertile_entity(1, Sex::Female, 1, 1);
    let father = fertile_entity(2, Sex::Male, 2, 1);
    mother.pregnancy = Some(Pregnancy {
        father_id: 2,
        conceived_tick: 0,
        due_tick: GESTATION_TICKS,
    });
    let mut simulation = Simulation {
        tick: GESTATION_TICKS - 2,
        entities: vec![mother, father],
        next_entity_id: 3,
        seed: 42,
        ..Simulation::default()
    };

    simulation.step(&mut world);
    assert_eq!(simulation.entities().len(), 2);
    assert!(simulation.entities()[0].pregnancy.is_some());
    simulation.step(&mut world);
    assert_eq!(simulation.entities().len(), 3);
    assert_eq!(simulation.entities()[2].age_ticks, 0);
    assert!(simulation.entities()[0].pregnancy.is_none());
    assert_eq!(
        simulation.entities()[0].postpartum_until_tick,
        GESTATION_TICKS + POSTPARTUM_TICKS
    );
    assert_eq!(simulation.population_stats().births, 1);
}

#[test]
fn pregnancy_and_postpartum_prevent_conception() {
    let mut female = fertile_entity(1, Sex::Female, 0, 0);
    let male = fertile_entity(2, Sex::Male, 1, 0);
    female.pregnancy = Some(Pregnancy {
        father_id: 2,
        conceived_tick: 0,
        due_tick: GESTATION_TICKS,
    });
    let mut entities = vec![female, male];
    assert_eq!(
        lifecycle::try_conceptions(
            &mut entities,
            TICKS_PER_DAY,
            42,
            MAX_HEALTH,
            DAILY_CONCEPTION_SCALE,
        ),
        0
    );

    entities[0].pregnancy = None;
    entities[0].postpartum_until_tick = POSTPARTUM_TICKS;
    assert_eq!(
        lifecycle::try_conceptions(
            &mut entities,
            POSTPARTUM_TICKS - 1,
            42,
            MAX_HEALTH,
            DAILY_CONCEPTION_SCALE,
        ),
        0
    );
    assert!(female_is_fertile(
        &entities[0],
        POSTPARTUM_TICKS,
        MAX_HEALTH
    ));
}

#[test]
fn population_stats_include_biology() {
    let mut female = fertile_entity(1, Sex::Female, 0, 0);
    female.pregnancy = Some(Pregnancy {
        father_id: 2,
        conceived_tick: 0,
        due_tick: GESTATION_TICKS,
    });
    let simulation = Simulation {
        entities: vec![female, fertile_entity(2, Sex::Male, 1, 0)],
        next_entity_id: 3,
        ..Simulation::default()
    };
    let stats = simulation.population_stats();
    assert_eq!(stats.females, 1);
    assert_eq!(stats.males, 1);
    assert_eq!(stats.pregnant, 1);
}

#[test]
fn entity_dies_when_reaching_individual_lifespan() {
    let mut world = plain_grid(1, 1);
    let mut old = entity(1, 0, 0, 0.0);
    old.age_ticks = old.lifespan_ticks - 1;
    let mut simulation = Simulation {
        entities: vec![old],
        next_entity_id: 2,
        ..Simulation::default()
    };
    simulation.step(&mut world);
    assert!(simulation.entities().is_empty());
    assert_eq!(simulation.population_stats().deaths, 1);
}

#[test]
fn population_stats_report_pressure_and_consumption() {
    let mut world = grid_from_rows(&["F"]);
    let mut simulation = simulation_with_entity(0, 0, 60.0);
    simulation.step(&mut world);
    let stats = simulation.population_stats();
    assert_eq!(stats.population, 1);
    assert_eq!(stats.food_consumed, 10);
    assert!(stats.average_hunger < FOOD_SEARCH_THRESHOLD);
    assert_eq!(
        simulation.entities()[0].mind.memory.known_resources[0].estimated_amount,
        10
    );
}

#[test]
fn distant_food_is_not_known_without_perception() {
    let mut world = grid_from_rows(&["PPPPPPPPPPPPPPPPPPPF"]);
    let mut simulation = simulation_with_entity(0, 0, 90.0);
    simulation.step(&mut world);

    let entity = &simulation.entities()[0];
    assert!(entity.mind.memory.known_resources.is_empty());
    assert_eq!(entity.mind.current_goal, Some(Goal::Explore));
}

#[test]
fn entity_remembers_seen_food_and_interrupts_exploration_when_hungry() {
    let mut world = grid_from_rows(&["PPPPPFPPPPPPPPPP"]);
    let mut simulation = simulation_with_entity(0, 0, 0.0);
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
    autonomy::perceive(&mut observer.mind, &world, (0, 0), 0);
    assert_eq!(observer.mind.memory.known_resources.len(), 1);

    autonomy::perceive(&mut observer.mind, &world, (19, 0), 3_000);
    assert!(observer.mind.memory.known_resources.is_empty());
    world.resources[0] = None;
}

#[test]
fn unreachable_food_is_temporarily_avoided() {
    let mut world = grid_from_rows(&["P#F"]);
    let mut simulation = simulation_with_entity(0, 0, 90.0);
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
    observer.mind.perception_radius = 1;
    observer
        .mind
        .memory
        .known_resources
        .push(autonomy::KnownResource {
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
    simulation.step(&mut world);
    assert_eq!(simulation.entities()[0].mind.visible_entities, vec![2]);
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
    autonomy::perceive(&mut mind, &world, origin, 0);

    let target = autonomy::exploration_target(&mind, &world, origin, 1, 0);

    assert!(target.is_none_or(|(x, y)| world.region_id_at(x, y) == origin_region));
}

#[test]
fn handles_10_100_and_1000_entity_populations() {
    for count in [10, 100, 1_000] {
        let mut world = plain_grid(40, 25);
        let mut simulation = Simulation::with_population(42, &world, count);
        assert_eq!(simulation.entities().len(), count as usize);
        simulation.resume();
        simulation.advance(10, &mut world);
        assert_eq!(simulation.entities().len(), count as usize);
        assert_eq!(simulation.tick(), 10);
    }
}

#[test]
fn same_seed_and_steps_are_deterministic() {
    let rows = ["PPPFPPPPPP", "PPPPPPFPPP", "PFPPPPPPPP", "PPPPFPPPPP"];
    let mut world_a = grid_from_rows(&rows);
    let mut world_b = grid_from_rows(&rows);
    let mut simulation_a = Simulation::with_population(42, &world_a, 10);
    let mut simulation_b = Simulation::with_population(42, &world_b, 10);

    for _ in 0..100 {
        simulation_a.step(&mut world_a);
        simulation_b.step(&mut world_b);
    }

    assert_eq!(simulation_a.tick(), simulation_b.tick());
    assert_eq!(simulation_a.entities().len(), simulation_b.entities().len());

    for (entity_a, entity_b) in simulation_a.entities().iter().zip(simulation_b.entities()) {
        assert_eq!(entity_a.id, entity_b.id);
        assert_eq!((entity_a.x, entity_a.y), (entity_b.x, entity_b.y));
        assert_eq!(entity_a.sex, entity_b.sex);
        assert_eq!(entity_a.age_ticks, entity_b.age_ticks);
        assert_eq!(entity_a.hunger, entity_b.hunger);
        assert_eq!(entity_a.health, entity_b.health);
        assert_eq!(entity_a.pregnancy, entity_b.pregnancy);
        assert_eq!(entity_a.mind.current_goal, entity_b.mind.current_goal);
    }

    assert_eq!(
        simulation_a.population_stats().births,
        simulation_b.population_stats().births
    );
}
