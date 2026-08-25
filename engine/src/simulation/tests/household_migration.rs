use super::super::autonomy::{Action, Goal, KnownResource};
use super::super::households::{
    plan_daily_household_migrations, settle_completed_migrations, Household, HouseholdMigration,
    HOUSEHOLD_MIGRATION_COOLDOWN_TICKS,
};
use super::super::time::{TICKS_PER_DAY, TICKS_PER_YEAR};
use super::super::{Inventory, ItemKind, Simulation};
use super::support::{entity, plain_grid};
use crate::world::ResourceKind;

const HOUSEHOLD_ID: u32 = 1;
const TARGET: (u32, u32) = (12, 1);

fn household(residence: (u32, u32)) -> Household {
    Household {
        id: HOUSEHOLD_ID,
        formed_tick: 0,
        dissolved_tick: None,
        inheritance: None,
        migration: None,
        residence_x: residence.0,
        residence_y: residence.1,
        storage: Inventory::new(200),
    }
}

fn migration(started_tick: u64, target: (u32, u32)) -> HouseholdMigration {
    HouseholdMigration {
        started_tick,
        proposer_id: 1,
        target_x: target.0,
        target_y: target.1,
        completed_tick: None,
    }
}

fn member(id: u32, age_years: u64, position: (u32, u32)) -> super::super::Entity {
    let mut member = entity(id, position.0, position.1, 0.0);
    member.age_ticks = age_years * TICKS_PER_YEAR;
    member.household_id = Some(HOUSEHOLD_ID);
    member
}

fn remember_food(member: &mut super::super::Entity, target: (u32, u32), amount: u16, seen: u64) {
    member.mind.memory.known_resources.push(KnownResource {
        x: target.0,
        y: target.1,
        kind: ResourceKind::Food,
        last_seen_tick: seen,
        estimated_amount: amount,
        failed_attempts: 0,
        avoid_until_tick: 0,
    });
}

fn plan(entities: &[super::super::Entity], households: &mut [Household], tick: u64) {
    plan_daily_household_migrations(entities, households, &plain_grid(24, 4), tick);
}

fn planned_target(household: &Household) -> Option<(u32, u32)> {
    household.active_migration_target()
}

#[test]
fn scarce_household_with_distant_known_food_starts_migration() {
    let mut adult = member(1, 30, (1, 1));
    remember_food(&mut adult, TARGET, 20, 7);
    let mut households = vec![household((1, 1))];

    plan(&[adult], &mut households, TICKS_PER_DAY);

    assert_eq!(planned_target(&households[0]), Some(TARGET));
    assert_eq!(households[0].migration.unwrap().proposer_id, 1);
}

#[test]
fn household_with_enough_food_does_not_migrate() {
    let mut adult = member(1, 30, (1, 1));
    adult.inventory.add(ItemKind::Food, 10);
    remember_food(&mut adult, TARGET, 20, 7);
    let mut households = vec![household((1, 1))];
    plan(&[adult], &mut households, TICKS_PER_DAY);
    assert_eq!(planned_target(&households[0]), None);
}

#[test]
fn scarce_household_with_local_known_food_does_not_migrate() {
    let mut adult = member(1, 30, (1, 1));
    remember_food(&mut adult, (8, 1), 20, 7);
    remember_food(&mut adult, TARGET, 40, 8);
    let mut households = vec![household((1, 1))];
    plan(&[adult], &mut households, TICKS_PER_DAY);
    assert_eq!(planned_target(&households[0]), None);
}

#[test]
fn scarce_household_without_known_food_does_not_migrate() {
    let mut households = vec![household((1, 1))];
    plan(&[member(1, 30, (1, 1))], &mut households, TICKS_PER_DAY);
    assert_eq!(planned_target(&households[0]), None);
}

#[test]
fn non_member_food_knowledge_does_not_trigger_migration() {
    let resident = member(1, 30, (1, 1));
    let mut outsider = member(2, 30, (1, 1));
    outsider.household_id = None;
    remember_food(&mut outsider, TARGET, 20, 7);
    let mut households = vec![household((1, 1))];
    plan(&[resident, outsider], &mut households, TICKS_PER_DAY);
    assert_eq!(planned_target(&households[0]), None);
}

macro_rules! knowledge_stage_test {
    ($name:ident, $age:expr, $expected:expr) => {
        #[test]
        fn $name() {
            let mut proposer = member(1, $age, (1, 1));
            remember_food(&mut proposer, TARGET, 20, 7);
            let mut households = vec![household((1, 1))];
            plan(&[proposer], &mut households, TICKS_PER_DAY);
            assert_eq!(planned_target(&households[0]), $expected);
        }
    };
}

knowledge_stage_test!(child_food_knowledge_does_not_propose_migration, 8, None);
knowledge_stage_test!(
    adolescent_food_knowledge_does_not_propose_migration,
    15,
    None
);
knowledge_stage_test!(adult_food_knowledge_can_propose_migration, 30, Some(TARGET));
knowledge_stage_test!(elder_food_knowledge_can_propose_migration, 70, Some(TARGET));

#[test]
fn migration_target_is_farther_than_local_food_radius() {
    let mut adult = member(1, 30, (1, 1));
    remember_food(&mut adult, (9, 1), 20, 7);
    let mut households = vec![household((1, 1))];
    plan(&[adult], &mut households, TICKS_PER_DAY);
    assert_eq!(planned_target(&households[0]), None);
}

fn rank(candidates: &[(u32, (u32, u32), u16, u64)]) -> HouseholdMigration {
    let entities: Vec<_> = candidates
        .iter()
        .map(|&(id, target, amount, seen)| {
            let mut adult = member(id, 30, (1, 1));
            remember_food(&mut adult, target, amount, seen);
            adult
        })
        .collect();
    let mut households = vec![household((1, 1))];
    plan(&entities, &mut households, TICKS_PER_DAY);
    households[0].migration.unwrap()
}

#[test]
fn nearest_viable_destination_wins() {
    assert_eq!(
        rank(&[(1, (15, 1), 50, 5), (2, (11, 1), 10, 1)]).target_x,
        11
    );
}

#[test]
fn higher_food_amount_breaks_equal_distance() {
    let migration = rank(&[(1, (11, 1), 10, 5), (2, (10, 2), 20, 1)]);
    assert_eq!((migration.target_x, migration.target_y), (10, 2));
}

#[test]
fn newer_memory_breaks_equal_distance_and_amount() {
    let migration = rank(&[(1, (11, 1), 20, 5), (2, (10, 2), 20, 6)]);
    assert_eq!((migration.target_x, migration.target_y), (10, 2));
}

#[test]
fn lower_proposer_id_breaks_remaining_tie() {
    assert_eq!(
        rank(&[(2, TARGET, 20, 5), (1, TARGET, 20, 5)]).proposer_id,
        1
    );
}

#[test]
fn coordinates_break_final_tie_deterministically() {
    let migration = rank(&[(1, (11, 1), 20, 5), (1, (10, 2), 20, 5)]);
    assert_eq!((migration.target_x, migration.target_y), (10, 2));
}

#[test]
fn active_migration_does_not_retarget() {
    let mut adult = member(1, 30, (1, 1));
    remember_food(&mut adult, (20, 1), 100, 20);
    let mut home = household((1, 1));
    home.migration = Some(migration(0, TARGET));
    plan(&[adult], std::slice::from_mut(&mut home), TICKS_PER_DAY);
    assert_eq!(planned_target(&home), Some(TARGET));
}

#[test]
fn recently_completed_migration_respects_cooldown() {
    let mut adult = member(1, 30, (1, 1));
    remember_food(&mut adult, TARGET, 20, 7);
    let mut home = household((1, 1));
    home.migration = Some(HouseholdMigration {
        completed_tick: Some(TICKS_PER_DAY),
        ..migration(0, (2, 1))
    });
    plan(&[adult], std::slice::from_mut(&mut home), 2 * TICKS_PER_DAY);
    assert_eq!(home.active_migration_target(), None);
    assert_eq!(home.migration.unwrap().target_x, 2);
}

#[test]
fn migration_can_start_after_cooldown() {
    let mut adult = member(1, 30, (1, 1));
    remember_food(&mut adult, TARGET, 20, 7);
    let mut home = household((1, 1));
    home.migration = Some(HouseholdMigration {
        completed_tick: Some(TICKS_PER_DAY),
        ..migration(0, (2, 1))
    });
    plan(
        &[adult],
        std::slice::from_mut(&mut home),
        TICKS_PER_DAY + HOUSEHOLD_MIGRATION_COOLDOWN_TICKS,
    );
    assert_eq!(home.active_migration_target(), Some(TARGET));
}

#[test]
fn migration_start_does_not_change_residence_or_storage() {
    let mut adult = member(1, 30, (1, 1));
    remember_food(&mut adult, TARGET, 20, 7);
    let mut home = household((1, 1));
    home.storage.add(ItemKind::Timber, 17);
    plan(&[adult], std::slice::from_mut(&mut home), TICKS_PER_DAY);
    assert_eq!((home.residence_x, home.residence_y), (1, 1));
    assert_eq!(home.storage.amount(ItemKind::Timber), 17);
}

#[test]
fn migration_target_cannot_access_storage_before_completion() {
    let mut simulation = migrating_simulation(30, TARGET);
    simulation.households[0].storage.add(ItemKind::Food, 10);
    assert_eq!(simulation.withdraw_from_household(1, ItemKind::Food, 10), 0);
    assert_eq!(simulation.households[0].storage.amount(ItemKind::Food), 10);
}

#[test]
fn completed_migration_moves_storage_access_to_new_residence() {
    let mut simulation = migrating_simulation(30, TARGET);
    simulation.households[0].storage.add(ItemKind::Food, 10);
    settle_completed_migrations(
        &simulation.entities,
        &mut simulation.households,
        TICKS_PER_DAY,
    );
    assert_eq!(
        simulation.withdraw_from_household(1, ItemKind::Food, 10),
        10
    );
}

#[test]
fn old_residence_loses_storage_access_after_completion() {
    let mut simulation = migrating_simulation(30, TARGET);
    simulation.households[0].storage.add(ItemKind::Food, 10);
    settle_completed_migrations(
        &simulation.entities,
        &mut simulation.households,
        TICKS_PER_DAY,
    );
    simulation.entities[0].x = 1;
    simulation.entities[0].y = 1;
    assert_eq!(simulation.withdraw_from_household(1, ItemKind::Food, 10), 0);
}

fn migrating_simulation(age: u64, position: (u32, u32)) -> Simulation {
    let mut home = household((1, 1));
    home.migration = Some(migration(0, TARGET));
    Simulation {
        entities: vec![member(1, age, position)],
        next_entity_id: 2,
        households: vec![home],
        next_household_id: 2,
        ..Simulation::default()
    }
}

macro_rules! direct_migrant_test {
    ($name:ident, $age:expr) => {
        #[test]
        fn $name() {
            let mut simulation = migrating_simulation($age, (1, 1));
            simulation.step(&mut plain_grid(24, 4));
            let migrant = &simulation.entities[0];
            assert_eq!(migrant.mind.current_goal, Some(Goal::MigrateHousehold));
            assert_eq!(
                migrant.mind.current_plan,
                vec![Action::MoveTo(TARGET.0, TARGET.1)]
            );
            assert_eq!(migrant.path.last(), Some(&TARGET));
        }
    };
}

direct_migrant_test!(adult_member_chooses_migrate_household, 30);
direct_migrant_test!(elder_member_chooses_migrate_household, 70);
direct_migrant_test!(adolescent_member_chooses_migrate_household, 15);

#[test]
fn all_direct_members_receive_same_target_without_teleporting() {
    let mut simulation = migrating_simulation(30, (1, 1));
    simulation.entities.push(member(2, 70, (2, 1)));
    simulation.step(&mut plain_grid(24, 4));
    for migrant in &simulation.entities {
        assert_eq!(migrant.mind.current_goal, Some(Goal::MigrateHousehold));
        assert_eq!(migrant.path.last(), Some(&TARGET));
        assert_ne!((migrant.x, migrant.y), TARGET);
    }
}

#[test]
fn child_keeps_follow_and_infant_stays_with_caregiver_during_migration() {
    let mut simulation = migrating_simulation(30, (1, 1));
    let mut child = member(2, 8, (1, 1));
    child.caregiver_id = Some(1);
    let mut infant = member(3, 1, (0, 1));
    infant.caregiver_id = Some(1);
    simulation.entities.extend([child, infant]);
    simulation.step(&mut plain_grid(24, 4));
    let caregiver = &simulation.entities[0];
    assert_eq!(simulation.entities[1].mind.current_goal, Some(Goal::Follow));
    assert_eq!(
        (simulation.entities[2].x, simulation.entities[2].y),
        (caregiver.x, caregiver.y)
    );
    assert_ne!(
        simulation.entities[2].mind.current_goal,
        Some(Goal::MigrateHousehold)
    );
}

#[test]
fn arrived_member_waits_for_household() {
    let mut simulation = migrating_simulation(30, TARGET);
    simulation.entities.push(member(2, 30, (1, 1)));
    simulation.step(&mut plain_grid(24, 4));
    let arrived = simulation
        .entities
        .iter()
        .find(|entity| entity.id == 1)
        .unwrap();
    assert_eq!(arrived.mind.current_goal, Some(Goal::MigrateHousehold));
    assert_eq!(arrived.mind.current_plan, vec![Action::Wait]);
    assert_eq!(
        (
            simulation.households[0].residence_x,
            simulation.households[0].residence_y
        ),
        (1, 1)
    );
}

#[test]
fn urgent_hunger_interrupts_household_migration_and_then_resumes() {
    let mut simulation = migrating_simulation(30, (1, 1));
    simulation.entities[0].hunger = super::super::autonomy::URGENT_HUNGER_THRESHOLD;
    simulation.entities[0].inventory.add(ItemKind::Food, 20);
    let mut world = plain_grid(24, 4);

    simulation.step(&mut world);
    assert_eq!(simulation.entities[0].mind.current_goal, Some(Goal::Eat));
    simulation.step(&mut world);
    assert_eq!(
        simulation.entities[0].mind.current_goal,
        Some(Goal::MigrateHousehold)
    );
}

#[test]
fn dependent_provisioning_outranks_migration() {
    let mut simulation = migrating_simulation(30, (1, 1));
    simulation.entities[0].inventory.add(ItemKind::Food, 10);
    let mut child = member(2, 8, (1, 1));
    child.caregiver_id = Some(1);
    child.hunger = 80.0;
    simulation.entities.push(child);

    simulation.step(&mut plain_grid(24, 4));

    assert_eq!(
        simulation.entities[0].mind.current_goal,
        Some(Goal::ShareFood)
    );
}

#[test]
fn active_dependent_protection_outranks_migration() {
    let mut simulation = migrating_simulation(30, (1, 1));
    let mut child = member(2, 8, (6, 1));
    child.caregiver_id = Some(1);
    simulation.entities.push(child);

    simulation.step(&mut plain_grid(24, 4));

    assert_eq!(
        simulation.entities[0].mind.current_goal,
        Some(Goal::ProtectDependent)
    );
}

#[test]
fn laggard_prevents_settlement() {
    let entities = vec![member(1, 30, TARGET), member(2, 30, (1, 1))];
    let mut home = household((1, 1));
    home.migration = Some(migration(0, TARGET));
    settle_completed_migrations(&entities, std::slice::from_mut(&mut home), 20);
    assert_eq!((home.residence_x, home.residence_y), (1, 1));
    assert_eq!(home.migration.unwrap().completed_tick, None);
}

#[test]
fn collective_arrival_commits_residence_and_preserves_storage() {
    let entities = vec![
        member(1, 30, TARGET),
        member(2, 30, (11, 1)),
        member(3, 8, (12, 2)),
    ];
    let mut home = household((1, 1));
    home.storage.add(ItemKind::Food, 3);
    home.storage.add(ItemKind::Timber, 4);
    home.storage.add(ItemKind::Stone, 5);
    home.storage.add(ItemKind::Iron, 6);
    home.migration = Some(migration(0, TARGET));
    settle_completed_migrations(&entities, std::slice::from_mut(&mut home), 20);
    assert_eq!((home.residence_x, home.residence_y), TARGET);
    assert_eq!(home.migration.unwrap().completed_tick, Some(20));
    assert_eq!(home.storage.amount(ItemKind::Food), 3);
    assert_eq!(home.storage.amount(ItemKind::Timber), 4);
    assert_eq!(home.storage.amount(ItemKind::Stone), 5);
    assert_eq!(home.storage.amount(ItemKind::Iron), 6);
}

#[test]
fn empty_household_does_not_vacuously_complete_migration() {
    let mut home = household((1, 1));
    home.migration = Some(migration(0, TARGET));
    settle_completed_migrations(&[], std::slice::from_mut(&mut home), 20);
    assert_eq!((home.residence_x, home.residence_y), (1, 1));
    assert_eq!(home.migration.unwrap().completed_tick, None);
}

#[test]
fn household_migration_matches_normal_and_profiled_paths() {
    let mut normal = migrating_simulation(30, (1, 1));
    normal.entities.push(member(2, 15, (2, 1)));
    let mut profiled = normal.clone();
    let mut autonomy_profiled = normal.clone();
    normal.step(&mut plain_grid(24, 4));
    profiled.profile_step(&mut plain_grid(24, 4));
    autonomy_profiled.profile_autonomy_step(&mut plain_grid(24, 4));

    let state = |simulation: &Simulation| {
        (
            simulation.households.clone(),
            simulation
                .entities
                .iter()
                .map(|entity| {
                    (
                        entity.id,
                        entity.x,
                        entity.y,
                        entity.household_id,
                        entity.caregiver_id,
                        entity.mind.current_goal,
                        entity.mind.current_plan.clone(),
                        entity.path.clone(),
                    )
                })
                .collect::<Vec<_>>(),
        )
    };
    assert_eq!(state(&normal), state(&profiled));
    assert_eq!(state(&normal), state(&autonomy_profiled));
}
