use super::super::autonomy::{
    evaluate_goals, Action, DecisionContext, Goal, GriefState, KnownEntity,
    GRIEF_MAX_DURATION_TICKS, GRIEF_MIN_DURATION_TICKS, RELATIONSHIP_DECAY_START_TICKS,
};
use super::super::genealogy::Genealogy;
use super::super::grief::{process_witnessed_deaths, GRIEF_MIN_INTENSITY};
use super::super::households::{Household, HouseholdMigration};
use super::super::time::TICKS_PER_YEAR;
use super::super::{DeathContext, Inventory, ItemKind, Simulation, SimulationEventKind};
use super::support::{entity, plain_grid};

const SURVIVOR_ID: u32 = 1;
const DECEASED_ID: u32 = 2;

fn known(id: u32, affinity: i16) -> KnownEntity {
    KnownEntity {
        id,
        first_seen_tick: 1,
        last_seen_tick: 7,
        last_seen_x: 2,
        last_seen_y: 1,
        observed_ticks: 4,
        affinity,
        last_interaction_tick: 7,
        interaction_count: 3,
        seek_retry_after_tick: None,
    }
}

fn adult(id: u32) -> super::super::Entity {
    let mut adult = entity(id, id, 1, 0.0);
    adult.age_ticks = 30 * TICKS_PER_YEAR;
    adult
}

fn death() -> DeathContext {
    DeathContext {
        entity_id: DECEASED_ID,
        household_id: None,
        partner_id: None,
        caregiver_id: None,
    }
}

fn witnessed_survivor(affinity: i16) -> super::super::Entity {
    let mut survivor = adult(SURVIVOR_ID);
    survivor.mind.visible_entities = vec![DECEASED_ID];
    survivor.mind.memory.known_entities = vec![known(DECEASED_ID, affinity)];
    survivor
}

fn process(survivor: &mut super::super::Entity, death: DeathContext, genealogy: &Genealogy) {
    process_witnessed_deaths(std::slice::from_mut(survivor), genealogy, &[death], 10);
}

#[test]
fn witness_marks_entity_as_known_dead() {
    let mut survivor = witnessed_survivor(0);
    process(&mut survivor, death(), &Genealogy::default());
    assert!(survivor.mind.memory.knows_entity_dead(DECEASED_ID));
    assert_eq!(survivor.mind.memory.known_dead_entities, vec![DECEASED_ID]);
}

#[test]
fn unrelated_witness_knows_death_without_grieving() {
    let mut survivor = witnessed_survivor(0);
    process(&mut survivor, death(), &Genealogy::default());
    assert!(survivor.mind.memory.knows_entity_dead(DECEASED_ID));
    assert!(survivor.mind.grief.is_empty());
}

#[test]
fn invisible_partner_death_is_not_known() {
    let mut survivor = witnessed_survivor(500);
    survivor.mind.visible_entities.clear();
    process(
        &mut survivor,
        DeathContext {
            partner_id: Some(SURVIVOR_ID),
            ..death()
        },
        &Genealogy::default(),
    );
    assert!(!survivor.mind.memory.knows_entity_dead(DECEASED_ID));
    assert!(survivor.mind.grief.is_empty());
}

#[test]
fn same_household_invisible_death_is_not_omniscient() {
    let mut survivor = witnessed_survivor(500);
    survivor.household_id = Some(4);
    survivor.mind.visible_entities.clear();
    process(
        &mut survivor,
        DeathContext {
            household_id: Some(4),
            ..death()
        },
        &Genealogy::default(),
    );
    assert!(!survivor.mind.memory.knows_entity_dead(DECEASED_ID));
}

#[test]
fn witnessed_partner_death_starts_grief() {
    let mut survivor = witnessed_survivor(0);
    process(
        &mut survivor,
        DeathContext {
            partner_id: Some(SURVIVOR_ID),
            ..death()
        },
        &Genealogy::default(),
    );
    assert_eq!(survivor.mind.grief[0].deceased_id, DECEASED_ID);
    assert_eq!(survivor.mind.grief[0].intensity, 65);
}

fn genealogy(records: &[(u32, Option<u32>, Option<u32>)]) -> Genealogy {
    let mut genealogy = Genealogy::default();
    for &(id, mother, father) in records {
        genealogy.register(id, mother, father);
    }
    genealogy
}

#[test]
fn witnessed_parent_and_child_deaths_start_grief() {
    let mut parent = witnessed_survivor(0);
    process(
        &mut parent,
        death(),
        &genealogy(&[(1, None, None), (2, Some(1), None)]),
    );
    assert_eq!(parent.mind.grief[0].intensity, 60);

    let mut child = witnessed_survivor(0);
    process(
        &mut child,
        death(),
        &genealogy(&[(1, Some(2), None), (2, None, None)]),
    );
    assert_eq!(child.mind.grief[0].intensity, 60);
}

#[test]
fn witnessed_full_and_half_sibling_deaths_start_grief() {
    let mut full = witnessed_survivor(0);
    process(
        &mut full,
        death(),
        &genealogy(&[(1, Some(8), Some(9)), (2, Some(8), Some(9))]),
    );
    assert_eq!(full.mind.grief[0].intensity, 45);

    let mut half = witnessed_survivor(0);
    process(
        &mut half,
        death(),
        &genealogy(&[(1, Some(8), None), (2, Some(8), Some(9))]),
    );
    assert_eq!(half.mind.grief[0].intensity, 45);
}

#[test]
fn witnessed_caregiver_and_dependent_deaths_start_grief() {
    let mut dependent = witnessed_survivor(0);
    dependent.caregiver_id = Some(DECEASED_ID);
    process(&mut dependent, death(), &Genealogy::default());
    assert_eq!(dependent.mind.grief[0].intensity, 60);

    let mut caregiver = witnessed_survivor(0);
    process(
        &mut caregiver,
        DeathContext {
            caregiver_id: Some(SURVIVOR_ID),
            ..death()
        },
        &Genealogy::default(),
    );
    assert_eq!(caregiver.mind.grief[0].intensity, 60);
}

#[test]
fn witnessed_bonded_friend_death_starts_grief() {
    let mut survivor = witnessed_survivor(300);
    process(&mut survivor, death(), &Genealogy::default());
    assert_eq!(survivor.mind.grief[0].intensity, 42);
}

#[test]
fn affinity_changes_grief_strength_and_duration() {
    let mut lower = witnessed_survivor(0);
    let mut higher = witnessed_survivor(500);
    let partner_death = DeathContext {
        partner_id: Some(SURVIVOR_ID),
        ..death()
    };
    process(&mut lower, partner_death, &Genealogy::default());
    process(&mut higher, partner_death, &Genealogy::default());
    assert!(higher.mind.grief[0].intensity > lower.mind.grief[0].intensity);
    assert!(higher.mind.grief[0].ends_tick > lower.mind.grief[0].ends_tick);
}

#[test]
fn hostile_sibling_can_fall_below_grief_threshold() {
    let mut survivor = witnessed_survivor(-1_000);
    process(
        &mut survivor,
        death(),
        &genealogy(&[(1, Some(8), None), (2, Some(8), None)]),
    );
    assert!(survivor.mind.memory.knows_entity_dead(DECEASED_ID));
    assert!(survivor.mind.grief.is_empty());
}

#[test]
fn hostile_partner_can_still_grieve_above_threshold() {
    let mut survivor = witnessed_survivor(-1_000);
    process(
        &mut survivor,
        DeathContext {
            partner_id: Some(SURVIVOR_ID),
            ..death()
        },
        &Genealogy::default(),
    );
    assert_eq!(survivor.mind.grief[0].intensity, 25);
    assert!(survivor.mind.grief[0].intensity >= GRIEF_MIN_INTENSITY);
}

#[test]
fn multiple_deaths_are_recorded_without_duplicates() {
    let mut survivor = witnessed_survivor(300);
    survivor.mind.visible_entities = vec![2, 3];
    survivor.mind.memory.known_entities.push(known(3, 300));
    process_witnessed_deaths(
        std::slice::from_mut(&mut survivor),
        &Genealogy::default(),
        &[
            death(),
            DeathContext {
                entity_id: 3,
                ..death()
            },
            death(),
        ],
        10,
    );
    assert_eq!(survivor.mind.memory.known_dead_entities, vec![2, 3]);
    assert_eq!(survivor.mind.grief.len(), 2);
}

#[test]
fn grief_pressure_uses_strongest_active_grief_not_sum() {
    let mut mind = super::super::autonomy::Mind::default();
    mind.grief = vec![
        GriefState {
            deceased_id: 2,
            started_tick: 0,
            ends_tick: 100,
            intensity: 60,
        },
        GriefState {
            deceased_id: 3,
            started_tick: 0,
            ends_tick: 100,
            intensity: 70,
        },
    ];
    assert!((mind.grief_pressure(0) - 0.7).abs() < f32::EPSILON);
}

#[test]
fn expired_grief_is_pruned_during_autonomy() {
    let mut simulation = Simulation {
        entities: vec![adult(1)],
        next_entity_id: 2,
        ..Simulation::default()
    };
    simulation.entities[0].mind.grief.push(GriefState {
        deceased_id: 2,
        started_tick: 0,
        ends_tick: 1,
        intensity: 100,
    });
    simulation.step(&mut plain_grid(4, 4));
    assert!(simulation.entities[0].mind.grief.is_empty());
}

fn decision_context() -> DecisionContext {
    DecisionContext {
        tick: 1,
        origin: (1, 1),
        food_in_inventory: 0,
        best_visible_food_share_score: None,
        best_remembered_social_score: None,
    }
}

#[test]
fn strong_grief_can_choose_grieve_and_wait() {
    let mut simulation = Simulation {
        entities: vec![adult(1)],
        next_entity_id: 2,
        ..Simulation::default()
    };
    simulation.entities[0].mind.grief.push(GriefState {
        deceased_id: 2,
        started_tick: 0,
        ends_tick: GRIEF_MAX_DURATION_TICKS,
        intensity: 100,
    });
    simulation.step(&mut plain_grid(4, 4));
    assert_eq!(simulation.entities[0].mind.current_goal, Some(Goal::Grieve));
    assert_eq!(simulation.entities[0].mind.current_plan, vec![Action::Wait]);
}

#[test]
fn weak_grief_can_lose_to_higher_optional_utility() {
    let mut mind = super::super::autonomy::Mind::default();
    mind.grief.push(GriefState {
        deceased_id: 2,
        started_tick: 0,
        ends_tick: GRIEF_MIN_DURATION_TICKS,
        intensity: 20,
    });
    let mut personality = adult(1).personality;
    personality.curiosity = 1.0;
    assert_eq!(
        evaluate_goals(
            &mut mind,
            0.0,
            100.0,
            30 * TICKS_PER_YEAR,
            &personality,
            None,
            decision_context()
        ),
        Goal::Explore
    );
}

#[test]
fn urgent_hunger_outranks_grief() {
    let mut mind = super::super::autonomy::Mind::default();
    mind.grief.push(GriefState {
        deceased_id: 2,
        started_tick: 0,
        ends_tick: 100,
        intensity: 100,
    });
    let mut context = decision_context();
    context.food_in_inventory = 10;
    assert_eq!(
        evaluate_goals(
            &mut mind,
            100.0,
            100.0,
            30 * TICKS_PER_YEAR,
            &adult(1).personality,
            None,
            context
        ),
        Goal::Eat
    );
}

fn strong_grief() -> GriefState {
    GriefState {
        deceased_id: 99,
        started_tick: 0,
        ends_tick: 1_000,
        intensity: 100,
    }
}

#[test]
fn dependent_provisioning_outranks_grief() {
    let mut caregiver = adult(1);
    caregiver.mind.grief.push(strong_grief());
    caregiver.inventory.add(ItemKind::Food, 10);
    let mut child = entity(2, 2, 1, 80.0);
    child.age_ticks = 8 * TICKS_PER_YEAR;
    child.caregiver_id = Some(1);
    let mut simulation = Simulation {
        entities: vec![caregiver, child],
        next_entity_id: 3,
        ..Simulation::default()
    };
    simulation.step(&mut plain_grid(16, 4));
    assert_eq!(
        simulation.entities[0].mind.current_goal,
        Some(Goal::ShareFood)
    );
}

#[test]
fn dependent_protection_outranks_grief() {
    let mut caregiver = adult(1);
    caregiver.mind.grief.push(strong_grief());
    let mut child = entity(2, 6, 1, 0.0);
    child.age_ticks = 8 * TICKS_PER_YEAR;
    child.caregiver_id = Some(1);
    let mut simulation = Simulation {
        entities: vec![caregiver, child],
        next_entity_id: 3,
        ..Simulation::default()
    };
    simulation.step(&mut plain_grid(16, 4));
    assert_eq!(
        simulation.entities[0].mind.current_goal,
        Some(Goal::ProtectDependent)
    );
}

#[test]
fn household_migration_outranks_grief() {
    let mut migrant = adult(1);
    migrant.household_id = Some(1);
    migrant.mind.grief.push(strong_grief());
    let household = Household {
        id: 1,
        formed_tick: 0,
        dissolved_tick: None,
        inheritance: None,
        migration: Some(HouseholdMigration {
            started_tick: 0,
            proposer_id: 1,
            target_x: 12,
            target_y: 1,
            completed_tick: None,
        }),
        residence_x: 1,
        residence_y: 1,
        storage: Inventory::new(200),
    };
    let mut simulation = Simulation {
        entities: vec![migrant],
        next_entity_id: 2,
        households: vec![household],
        next_household_id: 2,
        ..Simulation::default()
    };
    simulation.step(&mut plain_grid(16, 4));
    assert_eq!(
        simulation.entities[0].mind.current_goal,
        Some(Goal::MigrateHousehold)
    );
}

#[test]
fn child_records_grief_but_follow_remains_priority() {
    let mut child = witnessed_survivor(500);
    child.age_ticks = 8 * TICKS_PER_YEAR;
    child.caregiver_id = Some(3);
    process(
        &mut child,
        DeathContext {
            partner_id: Some(SURVIVOR_ID),
            ..death()
        },
        &Genealogy::default(),
    );
    assert!(!child.mind.grief.is_empty());
    assert_eq!(
        evaluate_goals(
            &mut child.mind,
            0.0,
            100.0,
            child.age_ticks,
            &child.personality,
            None,
            decision_context()
        ),
        Goal::Follow
    );
}

#[test]
fn known_dead_relationship_affinity_does_not_decay_but_unaware_still_does() {
    let mut known_dead = witnessed_survivor(300);
    known_dead.mind.memory.mark_entity_dead(DECEASED_ID);
    known_dead.mind.memory.known_entities[0].last_interaction_tick = 0;
    let mut unaware = witnessed_survivor(300);
    unaware.mind.memory.known_entities[0].last_interaction_tick = 0;
    let tick = RELATIONSHIP_DECAY_START_TICKS;
    known_dead.mind.memory.decay_relationships(tick);
    unaware.mind.memory.decay_relationships(tick);
    assert_eq!(known_dead.mind.memory.known_entities[0].affinity, 300);
    assert_eq!(unaware.mind.memory.known_entities[0].affinity, 299);
}

#[test]
fn witnessed_death_preserves_relationship_memory() {
    let mut survivor = witnessed_survivor(300);
    let before = survivor.mind.memory.known_entities[0];
    process(&mut survivor, death(), &Genealogy::default());
    assert_eq!(survivor.mind.memory.known_entities, vec![before]);
    assert!(survivor.mind.memory.knows_entity_dead(DECEASED_ID));
}

#[test]
fn known_dead_person_is_not_a_remembered_social_target() {
    let mut seeker = adult(1);
    seeker.personality.sociability = 1.0;
    seeker.personality.curiosity = 0.0;
    seeker.mind.memory.known_entities = vec![known(2, 1_000)];
    seeker.mind.memory.known_entities[0].last_seen_x = 12;
    seeker.mind.memory.mark_entity_dead(2);
    let mut simulation = Simulation {
        entities: vec![seeker],
        next_entity_id: 2,
        ..Simulation::default()
    };
    simulation.step(&mut plain_grid(16, 4));
    assert_ne!(
        simulation.entities[0].mind.current_goal,
        Some(Goal::Socialize)
    );
    assert!(!simulation.entities[0]
        .mind
        .current_plan
        .iter()
        .any(|action| action.target_entity_id() == Some(2)));
}

#[test]
fn active_social_seek_is_cleared_when_target_death_is_known() {
    let mut seeker = adult(1);
    seeker.mind.memory.known_entities = vec![known(2, 500)];
    seeker.mind.memory.mark_entity_dead(2);
    seeker.mind.set_plan(
        Goal::Socialize,
        vec![Action::ApproachEntity(2), Action::Interact(2)],
        0,
    );
    seeker.path = vec![(2, 1), (3, 1)];
    let mut simulation = Simulation {
        entities: vec![seeker],
        next_entity_id: 2,
        ..Simulation::default()
    };
    simulation.step(&mut plain_grid(16, 4));
    assert!(!simulation.entities[0]
        .mind
        .current_plan
        .iter()
        .any(|action| action.target_entity_id() == Some(2)));
}

fn natural_death_simulation(witnessed: bool) -> Simulation {
    let mut survivor = adult(1);
    let mut deceased = adult(2);
    survivor.x = 1;
    deceased.x = if witnessed { 2 } else { 20 };
    survivor.partner_id = Some(2);
    deceased.partner_id = Some(1);
    survivor.mind.memory.known_entities = vec![known(2, 1_000)];
    deceased.age_ticks = deceased.lifespan_ticks - 1;
    Simulation {
        entities: vec![survivor, deceased],
        next_entity_id: 3,
        ..Simulation::default()
    }
}

#[test]
fn natural_death_end_to_end_records_death_knowledge_grief_and_clears_partner() {
    let mut simulation = natural_death_simulation(true);
    let mut world = plain_grid(24, 4);
    simulation.step(&mut world);
    assert_eq!(simulation.entities.len(), 1);
    assert_eq!(simulation.entities[0].partner_id, None);
    assert!(simulation.entities[0].mind.memory.knows_entity_dead(2));
    assert_eq!(simulation.entities[0].mind.grief.len(), 1);
    assert_eq!(
        simulation
            .recent_events()
            .filter(|event| event.kind == SimulationEventKind::Death)
            .count(),
        1
    );
    simulation.step(&mut world);
    assert_eq!(simulation.entities[0].mind.current_goal, Some(Goal::Grieve));
}

#[test]
fn starvation_death_uses_same_grief_pipeline() {
    let mut simulation = natural_death_simulation(true);
    simulation.entities[1].age_ticks = 30 * TICKS_PER_YEAR;
    simulation.entities[1].lifespan_ticks = 800_000;
    simulation.entities[1].hunger = 100.0;
    simulation.entities[1].health = 0.1;
    simulation.step(&mut plain_grid(24, 4));
    assert!(simulation.entities[0].mind.memory.knows_entity_dead(2));
    assert_eq!(simulation.entities[0].mind.grief.len(), 1);
}

#[test]
fn unaware_agent_can_still_seek_person_who_died_elsewhere() {
    let mut simulation = natural_death_simulation(false);
    simulation.entities[0].mind.memory.known_entities[0].last_seen_x = 12;
    let mut world = plain_grid(24, 4);
    simulation.step(&mut world);
    assert!(!simulation.entities[0].mind.memory.knows_entity_dead(2));
    simulation.entities[0].mind.clear_goal();
    simulation.entities[0].personality.sociability = 1.0;
    simulation.entities[0].personality.curiosity = 0.0;
    simulation.entities[0].personality.caution = 1.0;
    simulation.step(&mut world);
    assert_eq!(
        simulation.entities[0].mind.current_goal,
        Some(Goal::Socialize)
    );
    assert!(simulation.entities[0]
        .mind
        .current_plan
        .iter()
        .any(|action| action.target_entity_id() == Some(2)));
}

#[test]
fn grief_matches_normal_and_profiled_paths() {
    let mut normal = natural_death_simulation(true);
    let mut profiled = normal.clone();
    let mut autonomy_profiled = normal.clone();
    normal.step(&mut plain_grid(24, 4));
    profiled.profile_step(&mut plain_grid(24, 4));
    autonomy_profiled.profile_autonomy_step(&mut plain_grid(24, 4));
    let state = |simulation: &Simulation| {
        simulation
            .entities
            .iter()
            .map(|entity| {
                (
                    entity.id,
                    entity.partner_id,
                    entity.caregiver_id,
                    entity.household_id,
                    entity.mind.memory.known_dead_entities.clone(),
                    entity.mind.grief.clone(),
                    entity.mind.current_goal,
                    entity.mind.current_action(),
                )
            })
            .collect::<Vec<_>>()
    };
    assert_eq!(state(&normal), state(&profiled));
    assert_eq!(state(&normal), state(&autonomy_profiled));
}

#[test]
fn grief_pressure_fades_and_expires_without_manual_mutation() {
    let mut mind = super::super::autonomy::Mind::default();
    mind.grief.push(GriefState {
        deceased_id: 2,
        started_tick: 0,
        ends_tick: 100,
        intensity: 100,
    });
    assert!(mind.grief_pressure(1) > mind.grief_pressure(50));
    assert_eq!(mind.grief_pressure(100), 0.0);
    mind.prune_expired_grief(100);
    assert!(mind.grief.is_empty());
}

#[test]
fn grief_fades_behaviorally_until_optional_behavior_wins() {
    let mut mind = super::super::autonomy::Mind::default();
    mind.grief.push(GriefState {
        deceased_id: 2,
        started_tick: 0,
        ends_tick: 100,
        intensity: 100,
    });
    let mut personality = adult(1).personality;
    personality.curiosity = 1.0;
    let mut early = decision_context();
    early.tick = 1;
    assert_eq!(
        evaluate_goals(
            &mut mind,
            0.0,
            100.0,
            30 * TICKS_PER_YEAR,
            &personality,
            None,
            early,
        ),
        Goal::Grieve
    );
    mind.clear_goal();
    let mut late = decision_context();
    late.tick = 90;
    assert_eq!(
        evaluate_goals(
            &mut mind,
            0.0,
            100.0,
            30 * TICKS_PER_YEAR,
            &personality,
            None,
            late,
        ),
        Goal::Explore
    );
}
