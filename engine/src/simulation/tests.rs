use super::autonomy::{evaluate_goals, URGENT_HUNGER_THRESHOLD};
use super::config::{
    BASE_MOVEMENT_SPEED, FOOD_CONSUMED_PER_MEAL, HUNGER_PER_TICK, HUNGER_REDUCTION_PER_MEAL,
};
use super::entity::Pregnancy;
use super::lifecycle::{
    conception_roll, female_is_fertile, male_is_fertile, personality_for, DAILY_CONCEPTION_SCALE,
};
use super::spatial::{EntitySnapshot, SpatialGrid};
use super::time::{
    ADOLESCENT_AGE_END, CHILD_AGE_END, ELDER_AGE_START, FEMALE_REPRODUCTIVE_AGE_END,
    FOUNDER_AGE_MAX, FOUNDER_AGE_MIN, GESTATION_TICKS, INFANT_AGE_END, MALE_REPRODUCTIVE_AGE_END,
    POSTPARTUM_TICKS, REPRODUCTIVE_AGE_START, TICKS_PER_DAY, TICKS_PER_HOUR, TICKS_PER_WEEK,
    TICKS_PER_YEAR,
};
use super::*;
use crate::world::{ResourceDeposit, ResourceKind, Terrain, Tile};

fn linear_visible_entities(
    entity_id: u32,
    position: (u32, u32),
    radius: u32,
    population: &[EntitySnapshot],
) -> Vec<u32> {
    let mut visible: Vec<u32> = population
        .iter()
        .filter(|other| {
            other.id != entity_id
                && position.0.abs_diff(other.x) + position.1.abs_diff(other.y) <= radius
        })
        .map(|other| other.id)
        .collect();
    visible.sort_unstable();
    visible
}

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
        movement_credit: 0.0,
        caregiver_id: None,
        personality: personality_for(0, id),
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
        let entity = Entity {
            id: 0,
            x: 0,
            y: 0,
            sex: Sex::Female,
            lifespan_ticks: 800_000,
            hunger: 0.0,
            health: MAX_HEALTH,
            age_ticks: 25 * TICKS_PER_YEAR,
            path: Vec::new(),
            path_index: 0,
            activity: EntityActivity::Idle,
            mind: Mind::default(),
            pregnancy: Some(Pregnancy {
                father_id: 0,
                conceived_tick: 0,
                due_tick: GESTATION_TICKS,
            }),
            postpartum_until_tick: 0,
            movement_credit: 0.0,
            caregiver_id: None,
            personality: personality_for(0, 0),
        };
        super::autonomy::effective_movement_speed(&entity, week * TICKS_PER_WEEK)
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

#[test]
fn competing_entities_consume_a_finite_deposit_once() {
    let mut world = grid_from_rows(&["F"]);
    world.resources[0].as_mut().unwrap().amount = 10;
    let mut simulation = Simulation {
        entities: vec![entity(1, 0, 0, 60.0), entity(2, 0, 0, 60.0)],
        next_entity_id: 3,
        ..Simulation::default()
    };
    for entity in &mut simulation.entities {
        entity.age_ticks = 25 * TICKS_PER_YEAR;
    }
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
fn life_stage_transitions_at_correct_ages() {
    use super::entity::LifeStage;

    assert_eq!(LifeStage::from_age_ticks(0), LifeStage::Infant);
    assert_eq!(
        LifeStage::from_age_ticks(INFANT_AGE_END - 1),
        LifeStage::Infant
    );
    assert_eq!(LifeStage::from_age_ticks(INFANT_AGE_END), LifeStage::Child);
    assert_eq!(
        LifeStage::from_age_ticks(CHILD_AGE_END - 1),
        LifeStage::Child
    );
    assert_eq!(
        LifeStage::from_age_ticks(CHILD_AGE_END),
        LifeStage::Adolescent
    );
    assert_eq!(
        LifeStage::from_age_ticks(ADOLESCENT_AGE_END - 1),
        LifeStage::Adolescent
    );
    assert_eq!(
        LifeStage::from_age_ticks(ADOLESCENT_AGE_END),
        LifeStage::Adult
    );
    assert_eq!(
        LifeStage::from_age_ticks(ELDER_AGE_START - 1),
        LifeStage::Adult
    );
    assert_eq!(LifeStage::from_age_ticks(ELDER_AGE_START), LifeStage::Elder);
    assert_eq!(LifeStage::from_age_ticks(FOUNDER_AGE_MIN), LifeStage::Adult);
    assert_eq!(LifeStage::from_age_ticks(FOUNDER_AGE_MAX), LifeStage::Adult);
}

#[test]
fn life_stages_gate_reproduction() {
    use super::entity::LifeStage;

    let mut underage = fertile_entity(1, Sex::Female, 0, 0);
    underage.age_ticks = ADOLESCENT_AGE_END - TICKS_PER_YEAR;
    assert_eq!(
        LifeStage::from_age_ticks(underage.age_ticks),
        LifeStage::Adolescent
    );
    assert!(!female_is_fertile(&underage, 0, MAX_HEALTH));

    let mut female_54 = fertile_entity(2, Sex::Female, 0, 0);
    female_54.age_ticks = 54 * TICKS_PER_YEAR;
    assert_eq!(
        LifeStage::from_age_ticks(female_54.age_ticks),
        LifeStage::Adult
    );
    assert!(female_is_fertile(&female_54, 0, MAX_HEALTH));

    let mut female_55 = fertile_entity(3, Sex::Female, 0, 0);
    female_55.age_ticks = 55 * TICKS_PER_YEAR;
    assert!(!female_is_fertile(&female_55, 0, MAX_HEALTH));

    let mut male_66 = fertile_entity(4, Sex::Male, 0, 0);
    male_66.age_ticks = 66 * TICKS_PER_YEAR;
    assert_eq!(
        LifeStage::from_age_ticks(male_66.age_ticks),
        LifeStage::Elder
    );
    assert!(male_is_fertile(&male_66, MAX_HEALTH));

    let mut male_69 = fertile_entity(5, Sex::Male, 0, 0);
    male_69.age_ticks = 69 * TICKS_PER_YEAR;
    assert!(male_is_fertile(&male_69, MAX_HEALTH));

    let mut male_70 = fertile_entity(6, Sex::Male, 0, 0);
    male_70.age_ticks = 70 * TICKS_PER_YEAR;
    assert!(!male_is_fertile(&male_70, MAX_HEALTH));

    let mut elder_female = fertile_entity(7, Sex::Female, 0, 0);
    elder_female.age_ticks = 66 * TICKS_PER_YEAR;
    assert!(!female_is_fertile(&elder_female, 0, MAX_HEALTH));
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
    simulation.entities[0].age_ticks = 25 * TICKS_PER_YEAR;
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
        assert_eq!(entity_a.personality, entity_b.personality);
        assert_eq!(entity_a.mind.current_goal, entity_b.mind.current_goal);
    }

    assert_eq!(
        simulation_a.population_stats().births,
        simulation_b.population_stats().births
    );
}

#[test]
fn same_seed_and_id_produce_same_personality() {
    let seed = 42u64;
    let id = 7u32;
    let a = personality_for(seed, id);
    let b = personality_for(seed, id);
    assert_eq!(a, b);
}

#[test]
fn different_entities_have_personality_variation() {
    let seed = 42u64;
    let p0 = personality_for(seed, 0);
    let p1 = personality_for(seed, 1);
    let p2 = personality_for(seed, 2);

    assert_ne!(p0, p1);
    assert_ne!(p1, p2);
    assert_ne!(p0, p2);
}

#[test]
fn personality_traits_stay_in_unit_interval() {
    let seed = 999u64;
    for id in 0..500u32 {
        let personality = personality_for(seed, id);
        assert!((0.0..=1.0).contains(&personality.curiosity));
        assert!((0.0..=1.0).contains(&personality.sociability));
        assert!((0.0..=1.0).contains(&personality.cooperativeness));
        assert!((0.0..=1.0).contains(&personality.caution));
        assert!((0.0..=1.0).contains(&personality.persistence));
    }
}

#[test]
fn personality_generation_matches_snapshot() {
    let personality = personality_for(12_345, 0);

    assert_eq!(personality.curiosity.to_bits(), 0x3e64_74d9);
    assert_eq!(personality.sociability.to_bits(), 0x3ee5_df26);
    assert_eq!(personality.cooperativeness.to_bits(), 0x3ea6_f421);
    assert_eq!(personality.caution.to_bits(), 0x3ef0_49d3);
    assert_eq!(personality.persistence.to_bits(), 0x3f15_f141);
}

#[test]
fn curious_entity_explores_more() {
    let mut mind_base = Mind::default();
    let mut mind_curious = Mind::default();

    let base = Personality {
        curiosity: 0.5,
        sociability: 0.5,
        cooperativeness: 0.5,
        caution: 0.5,
        persistence: 0.5,
    };

    let curious = Personality {
        curiosity: 1.0,
        ..base
    };

    let hunger = 30.0;
    let health = MAX_HEALTH;
    let age = 25 * TICKS_PER_YEAR;

    evaluate_goals(&mut mind_base, hunger, health, age, &base);
    evaluate_goals(&mut mind_curious, hunger, health, age, &curious);

    assert!(mind_curious.utility_scores.explore > mind_base.utility_scores.explore);
    assert_eq!(
        mind_curious.utility_scores.eat,
        mind_base.utility_scores.eat
    );
    assert_eq!(
        mind_curious.utility_scores.rest,
        mind_base.utility_scores.rest
    );
}

#[test]
fn cautious_entity_rests_more_and_explores_less() {
    let mut mind_base = Mind::default();
    let mut mind_cautious = Mind::default();

    let base = Personality {
        curiosity: 0.5,
        sociability: 0.5,
        cooperativeness: 0.5,
        caution: 0.5,
        persistence: 0.5,
    };

    let cautious = Personality {
        caution: 1.0,
        ..base
    };

    let hunger = 10.0;
    let health = 50.0;
    let age = 25 * TICKS_PER_YEAR;

    evaluate_goals(&mut mind_base, hunger, health, age, &base);
    evaluate_goals(&mut mind_cautious, hunger, health, age, &cautious);

    assert!(mind_cautious.utility_scores.rest > mind_base.utility_scores.rest);
    assert!(mind_cautious.utility_scores.explore < mind_base.utility_scores.explore);
    assert_eq!(
        mind_cautious.utility_scores.eat,
        mind_base.utility_scores.eat
    );
}

#[test]
fn neutral_personality_preserves_base_utilities() {
    let mut mind = Mind::default();
    let neutral = Personality {
        curiosity: 0.5,
        sociability: 0.5,
        cooperativeness: 0.5,
        caution: 0.5,
        persistence: 0.5,
    };

    let hunger = 40.0;
    let health = 70.0;
    let age = 25 * TICKS_PER_YEAR;

    evaluate_goals(&mut mind, hunger, health, age, &neutral);

    let hunger_ratio = 0.4;
    let food_confidence = 0.25;
    let health_deficit = 0.3;
    let expected_eat = hunger_ratio * (0.65 + 0.35 * food_confidence);
    let expected_explore = (1.0 - hunger_ratio) * 0.55 + (1.0 - food_confidence) * 0.2;
    let expected_rest = health_deficit * 0.8 + 0.05;

    assert!((mind.utility_scores.eat - expected_eat).abs() < 0.001);
    assert!((mind.utility_scores.explore - expected_explore).abs() < 0.001);
    assert!((mind.utility_scores.rest - expected_rest).abs() < 0.001);
}

#[test]
fn personality_does_not_affect_eat_utility() {
    let mut mind_extreme = Mind::default();
    let mut mind_neutral = Mind::default();

    let extreme = Personality {
        curiosity: 1.0,
        sociability: 1.0,
        cooperativeness: 1.0,
        caution: 1.0,
        persistence: 1.0,
    };
    let neutral = Personality {
        curiosity: 0.5,
        sociability: 0.5,
        cooperativeness: 0.5,
        caution: 0.5,
        persistence: 0.5,
    };

    let hunger = 60.0;
    let health = 80.0;
    let age = 25 * TICKS_PER_YEAR;

    evaluate_goals(&mut mind_extreme, hunger, health, age, &extreme);
    evaluate_goals(&mut mind_neutral, hunger, health, age, &neutral);

    assert_eq!(
        mind_extreme.utility_scores.eat,
        mind_neutral.utility_scores.eat
    );
}

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
    let caregiver_id = simulation.entities[0].id;

    simulation.push_entity((0, 0), 0);
    let infant_id = simulation.entities().last().unwrap().id;
    let infant = simulation.entities.last_mut().unwrap();
    infant.caregiver_id = Some(caregiver_id);
    infant.hunger = 80.0;
    simulation.step(&mut world);

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
    let caregiver_id = simulation.entities[0].id;

    simulation.push_entity((0, 0), 0);
    let infant_id = simulation.entities().last().unwrap().id;
    let infant = simulation.entities.last_mut().unwrap();
    infant.caregiver_id = Some(caregiver_id);
    infant.hunger = 80.0;
    simulation.step(&mut world);

    let infant = simulation
        .entities()
        .iter()
        .find(|entity| entity.id == infant_id)
        .unwrap();
    let expected = 80.0 + HUNGER_PER_TICK
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
