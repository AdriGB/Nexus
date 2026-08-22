use super::super::autonomy::KnownEntity;
use super::super::config::MAX_HEALTH;
use super::super::entity::{Entity, LifeStage, Pregnancy, Sex};
use super::super::lifecycle::{
    self, conception_roll, female_is_fertile, male_is_fertile, DAILY_CONCEPTION_SCALE,
};
use super::super::time::{
    ADOLESCENT_AGE_END, CHILD_AGE_END, ELDER_AGE_START, FEMALE_REPRODUCTIVE_AGE_END,
    FOUNDER_AGE_MAX, FOUNDER_AGE_MIN, GESTATION_TICKS, INFANT_AGE_END, MALE_REPRODUCTIVE_AGE_END,
    POSTPARTUM_TICKS, REPRODUCTIVE_AGE_START, TICKS_PER_DAY, TICKS_PER_YEAR,
};
use super::super::Simulation;
use super::support::*;
use std::collections::HashSet;

#[test]
fn age_increases_once_per_tick() {
    let mut world = grid_from_rows(&["P"]);
    let mut simulation = simulation_with_entity(0, 0, 0.0);
    simulation.step(&mut world);
    assert_eq!(simulation.entities()[0].age_ticks, 1);
}

#[test]
fn life_stage_transitions_at_correct_ages() {
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
fn life_stages_gate_reproduction() {
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

// ── Relationship-influenced partner selection ───────────────────────────

fn known_entity(id: u32, affinity: i16) -> KnownEntity {
    KnownEntity {
        id,
        first_seen_tick: 0,
        last_seen_tick: 0,
        last_seen_x: 0,
        last_seen_y: 0,
        observed_ticks: 1,
        affinity,
        last_interaction_tick: 0,
        interaction_count: 0,
        seek_retry_after_tick: None,
    }
}

#[test]
fn positive_relationship_beats_closer_unknown_partner() {
    let mut female = fertile_entity(1, Sex::Female, 0, 0);
    let close_unknown = fertile_entity(2, Sex::Male, 1, 0);
    let mut distant_known = fertile_entity(3, Sex::Male, 2, 0);

    female.mind.memory.known_entities.push(known_entity(3, 400));
    distant_known
        .mind
        .memory
        .known_entities
        .push(known_entity(1, 300));

    let entities = vec![female, close_unknown, distant_known];

    assert_eq!(
        lifecycle::select_reproduction_partner(&entities[0], &entities, MAX_HEALTH),
        Some(3),
        "mutual positive affinity should beat a smaller distance"
    );
}

#[test]
fn persistent_partner_is_preferred_over_a_higher_affinity_candidate() {
    let mut female = fertile_entity(1, Sex::Female, 0, 0);
    let mut partner = fertile_entity(2, Sex::Male, 2, 0);
    let mut alternative = fertile_entity(3, Sex::Male, 1, 0);
    female.partner_id = Some(2);
    partner.partner_id = Some(1);
    female.mind.memory.known_entities.push(known_entity(2, 100));
    female.mind.memory.known_entities.push(known_entity(3, 800));
    partner
        .mind
        .memory
        .known_entities
        .push(known_entity(1, 100));
    alternative
        .mind
        .memory
        .known_entities
        .push(known_entity(1, 800));
    let entities = vec![female, partner, alternative];

    assert_eq!(
        lifecycle::select_reproduction_partner(&entities[0], &entities, MAX_HEALTH),
        Some(2)
    );
}

#[test]
fn strong_negative_relationship_prevents_pairing() {
    let mut female = fertile_entity(1, Sex::Female, 0, 0);
    let negative_close = fertile_entity(2, Sex::Male, 1, 0);
    let neutral_far = fertile_entity(3, Sex::Male, 2, 0);

    female
        .mind
        .memory
        .known_entities
        .push(known_entity(2, -500));

    let entities = vec![female, negative_close, neutral_far];
    assert_eq!(
        lifecycle::select_reproduction_partner(&entities[0], &entities, MAX_HEALTH),
        Some(3)
    );

    let female = fertile_entity(1, Sex::Female, 0, 0);
    let mut negative_male = fertile_entity(2, Sex::Male, 1, 0);
    negative_male
        .mind
        .memory
        .known_entities
        .push(known_entity(1, -500));

    let entities = vec![female, negative_male];
    assert_eq!(
        lifecycle::select_reproduction_partner(&entities[0], &entities, MAX_HEALTH),
        None,
        "strong negative affinity from either individual rejects the pairing"
    );
}

#[test]
fn all_rejected_candidates_return_none() {
    let mut female = fertile_entity(1, Sex::Female, 0, 0);
    let negative_close = fertile_entity(2, Sex::Male, 1, 0);
    female
        .mind
        .memory
        .known_entities
        .push(known_entity(2, -700));

    let entities = vec![female, negative_close];
    assert_eq!(
        lifecycle::select_reproduction_partner(&entities[0], &entities, MAX_HEALTH),
        None
    );
}

#[test]
fn unknown_relationships_fall_back_to_distance_then_id() {
    let female = fertile_entity(1, Sex::Female, 0, 0);
    let closer = fertile_entity(2, Sex::Male, 1, 0);
    let same_distance = fertile_entity(4, Sex::Male, 1, 0);
    let farther = fertile_entity(3, Sex::Male, 2, 0);

    let entities = vec![female, closer, same_distance, farther];

    assert_eq!(
        lifecycle::select_reproduction_partner(&entities[0], &entities, MAX_HEALTH),
        Some(2),
        "with no relationships, closest wins; ties resolve by lowest id"
    );
}

#[test]
fn relationship_partner_selection_is_deterministic() {
    fn scenario() -> Vec<Entity> {
        let mut female = fertile_entity(1, Sex::Female, 0, 0);
        let mut male_a = fertile_entity(2, Sex::Male, 1, 0);
        let mut male_b = fertile_entity(3, Sex::Male, 2, 0);
        female.mind.memory.known_entities.push(known_entity(2, 200));
        female.mind.memory.known_entities.push(known_entity(3, 250));
        male_a.mind.memory.known_entities.push(known_entity(1, 100));
        male_b.mind.memory.known_entities.push(known_entity(1, 150));
        vec![female, male_a, male_b]
    }

    let first = scenario();
    let second = scenario();

    assert_eq!(
        lifecycle::select_reproduction_partner(&first[0], &first, MAX_HEALTH),
        lifecycle::select_reproduction_partner(&second[0], &second, MAX_HEALTH),
    );
    assert_eq!(
        lifecycle::select_reproduction_partner(&first[0], &first, MAX_HEALTH),
        Some(3),
        "the higher mutual score (min 150 > min 100) wins over the closer distance"
    );
}

#[test]
fn mutual_positive_relationship_beats_one_sided_affinity() {
    let mut female = fertile_entity(1, Sex::Female, 0, 0);
    let mut one_sided = fertile_entity(2, Sex::Male, 1, 0);
    let mut mutual = fertile_entity(3, Sex::Male, 2, 0);

    female.mind.memory.known_entities.push(known_entity(2, 800));
    female.mind.memory.known_entities.push(known_entity(3, 250));
    one_sided
        .mind
        .memory
        .known_entities
        .push(known_entity(1, -100));
    mutual.mind.memory.known_entities.push(known_entity(1, 250));

    let entities = vec![female, one_sided, mutual];

    assert_eq!(
        lifecycle::select_reproduction_partner(&entities[0], &entities, MAX_HEALTH),
        Some(3),
        "mutual positive affinity should beat one-sided affinity"
    );
}

#[test]
fn reproduction_affinity_boundary_is_inclusive_at_minus_200() {
    let mut female = fertile_entity(1, Sex::Female, 0, 0);
    let allowed = fertile_entity(2, Sex::Male, 1, 0);
    let rejected = fertile_entity(3, Sex::Male, 1, 0);

    female
        .mind
        .memory
        .known_entities
        .push(known_entity(2, -200));
    female
        .mind
        .memory
        .known_entities
        .push(known_entity(3, -201));

    let entities = vec![female, allowed, rejected];

    assert_eq!(
        lifecycle::select_reproduction_partner(&entities[0], &entities, MAX_HEALTH),
        Some(2),
        "affinity -200 is permitted; -201 is rejected"
    );
}

#[test]
fn conception_uses_relationship_preferred_partner() {
    let mut female = fertile_entity(1, Sex::Female, 0, 0);
    let close_unknown = fertile_entity(2, Sex::Male, 1, 0);
    let mut preferred = fertile_entity(3, Sex::Male, 2, 0);

    female.mind.memory.known_entities.push(known_entity(3, 300));
    preferred
        .mind
        .memory
        .known_entities
        .push(known_entity(1, 300));

    let mut entities = vec![female, close_unknown, preferred];

    let conceptions = lifecycle::try_conceptions(
        &mut entities,
        TICKS_PER_DAY,
        42,
        MAX_HEALTH,
        DAILY_CONCEPTION_SCALE,
    );

    assert_eq!(conceptions, 1);

    let pregnancy = entities[0]
        .pregnancy
        .expect("relationship-preferred partner should conceive");

    assert_eq!(pregnancy.father_id, 3);
}
