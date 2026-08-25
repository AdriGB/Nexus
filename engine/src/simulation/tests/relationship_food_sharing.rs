use super::super::autonomy::{Action, CloseRelationshipRole, Goal, KnownEntity};
use super::super::time::TICKS_PER_YEAR;
use super::super::{ItemKind, Simulation, SimulationEventKind};
use super::support::{entity, plain_grid};

fn adult(id: u32, hunger: f32) -> super::super::entity::Entity {
    let mut entity = entity(id, 1, 1, hunger);
    entity.age_ticks = 25 * TICKS_PER_YEAR;
    entity.personality.curiosity = 0.0;
    entity.personality.sociability = 0.0;
    entity
}

fn sharing_simulation(
    mut giver: super::super::entity::Entity,
    recipients: Vec<super::super::entity::Entity>,
) -> Simulation {
    giver.inventory.add(ItemKind::Food, 30);
    let next_entity_id = recipients.iter().map(|entity| entity.id).max().unwrap_or(1) + 1;
    let mut entities = vec![giver];
    entities.extend(recipients);
    entities.sort_by_key(|entity| entity.id);
    Simulation {
        entities,
        next_entity_id,
        ..Simulation::default()
    }
}

fn remember(giver: &mut super::super::entity::Entity, target_id: u32, affinity: i16) {
    giver.mind.memory.known_entities.push(KnownEntity {
        id: target_id,
        first_seen_tick: 0,
        last_seen_tick: 0,
        last_seen_x: 1,
        last_seen_y: 1,
        observed_ticks: 1,
        affinity,
        last_interaction_tick: 0,
        interaction_count: 0,
        seek_retry_after_tick: None,
    });
    giver
        .mind
        .memory
        .known_entities
        .sort_by_key(|known| known.id);
}

fn run_and_shared_target(simulation: &mut Simulation) -> Option<u32> {
    simulation.step(&mut plain_grid(16, 3));
    simulation
        .recent_events()
        .find(|event| event.kind == SimulationEventKind::FoodShared)
        .and_then(|event| event.target_id)
}

fn neutral_giver() -> super::super::entity::Entity {
    let mut giver = adult(1, 0.0);
    giver.personality.cooperativeness = 1.0;
    giver
}

#[test]
fn partner_is_preferred_over_equivalent_stranger() {
    let mut giver = neutral_giver();
    giver.partner_id = Some(3);
    let mut simulation = sharing_simulation(giver, vec![adult(2, 80.0), adult(3, 80.0)]);
    assert_eq!(run_and_shared_target(&mut simulation), Some(3));
}

#[test]
fn parent_is_preferred_over_equivalent_stranger() {
    let mut giver = neutral_giver();
    giver.mother_id = Some(3);
    let mut simulation = sharing_simulation(giver, vec![adult(2, 80.0), adult(3, 80.0)]);
    assert_eq!(run_and_shared_target(&mut simulation), Some(3));
}

#[test]
fn child_is_preferred_over_equivalent_stranger() {
    let giver = neutral_giver();
    let mut biological_child = adult(3, 80.0);
    biological_child.mother_id = Some(1);
    let mut simulation = sharing_simulation(giver, vec![adult(2, 80.0), biological_child]);
    assert_eq!(run_and_shared_target(&mut simulation), Some(3));
}

#[test]
fn full_sibling_is_preferred_over_equivalent_stranger() {
    let mut giver = neutral_giver();
    giver.mother_id = Some(10);
    giver.father_id = Some(11);
    let mut sibling = adult(3, 80.0);
    sibling.mother_id = Some(10);
    sibling.father_id = Some(11);
    let mut simulation = sharing_simulation(giver, vec![adult(2, 80.0), sibling]);
    assert_eq!(run_and_shared_target(&mut simulation), Some(3));
}

#[test]
fn half_sibling_is_preferred_over_equivalent_stranger() {
    let mut giver = neutral_giver();
    giver.mother_id = Some(10);
    let mut sibling = adult(3, 80.0);
    sibling.mother_id = Some(10);
    sibling.father_id = Some(12);
    let mut simulation = sharing_simulation(giver, vec![adult(2, 80.0), sibling]);
    assert_eq!(run_and_shared_target(&mut simulation), Some(3));
}

#[test]
fn more_hungry_stranger_can_outrank_less_hungry_sibling() {
    let mut giver = neutral_giver();
    giver.mother_id = Some(10);
    let mut sibling = adult(3, 60.0);
    sibling.mother_id = Some(10);
    let mut simulation = sharing_simulation(giver, vec![adult(2, 100.0), sibling]);
    assert_eq!(run_and_shared_target(&mut simulation), Some(2));
}

#[test]
fn strong_affinity_stranger_can_outrank_low_affinity_sibling() {
    let mut giver = neutral_giver();
    giver.mother_id = Some(10);
    remember(&mut giver, 2, 1_000);
    remember(&mut giver, 3, -200);
    let mut sibling = adult(3, 80.0);
    sibling.mother_id = Some(10);
    let mut simulation = sharing_simulation(giver, vec![adult(2, 80.0), sibling]);
    assert_eq!(run_and_shared_target(&mut simulation), Some(2));
}

#[test]
fn hostile_partner_and_sibling_are_not_optional_share_targets() {
    for sibling in [false, true] {
        let mut giver = neutral_giver();
        if sibling {
            giver.mother_id = Some(10);
        } else {
            giver.partner_id = Some(3);
        }
        remember(&mut giver, 3, -201);
        let mut relative = adult(3, 100.0);
        if sibling {
            relative.mother_id = Some(10);
        }
        let mut simulation = sharing_simulation(giver, vec![adult(2, 80.0), relative]);
        assert_eq!(run_and_shared_target(&mut simulation), Some(2));
    }
}

#[test]
fn equal_candidates_use_lower_entity_id() {
    let mut simulation = sharing_simulation(neutral_giver(), vec![adult(2, 80.0), adult(3, 80.0)]);
    assert_eq!(run_and_shared_target(&mut simulation), Some(2));
}

#[test]
fn household_potential_partner_and_cousin_get_no_close_relationship_bonus() {
    let mut giver = neutral_giver();
    giver.household_id = Some(1);
    giver.mother_id = Some(10);
    let ordinary = adult(2, 80.0);
    let mut same_household_potential_partner = adult(3, 80.0);
    same_household_potential_partner.household_id = Some(1);
    let mut cousin = adult(4, 80.0);
    cousin.mother_id = Some(11);
    let mut simulation = sharing_simulation(
        giver,
        vec![ordinary, same_household_potential_partner, cousin],
    );
    assert_eq!(run_and_shared_target(&mut simulation), Some(2));
}

#[test]
fn relationship_bonuses_change_borderline_willingness_but_are_not_unconditional() {
    assert!(!super::super::relationship_food_share_willingness(
        0.3,
        0,
        CloseRelationshipRole::Other
    ));
    assert!(super::super::relationship_food_share_willingness(
        0.3,
        0,
        CloseRelationshipRole::CurrentPartner
    ));
    assert!(super::super::relationship_food_share_willingness(
        0.35,
        0,
        CloseRelationshipRole::ParentChild
    ));
    assert!(super::super::relationship_food_share_willingness(
        0.36,
        0,
        CloseRelationshipRole::Sibling
    ));
    assert!(!super::super::relationship_food_share_willingness(
        0.0,
        -200,
        CloseRelationshipRole::CurrentPartner
    ));
}

#[test]
fn decision_and_planning_use_the_same_candidate_score() {
    let mut giver = neutral_giver();
    giver.partner_id = Some(3);
    let mut simulation = sharing_simulation(giver, vec![adult(2, 80.0), adult(3, 80.0)]);
    assert_eq!(run_and_shared_target(&mut simulation), Some(3));
    let target_hunger = simulation
        .entities
        .iter()
        .find(|entity| entity.id == 3)
        .expect("selected partner")
        .hunger;
    let expected_score = (target_hunger / 100.0) * (0.65 + 0.35 * 0.5) + 0.25;
    let sated_factor = (1.0 - simulation.entities[0].hunger / 100.0) * 0.7 + 0.3;
    assert!(
        (simulation.entities[0].mind.utility_scores.share_food - sated_factor * expected_score)
            .abs()
            < 0.0001
    );
}

#[test]
fn caregiver_feeds_own_dependent_before_a_hungrier_partner() {
    let mut giver = neutral_giver();
    giver.partner_id = Some(3);
    let mut dependent = adult(2, 80.0);
    dependent.age_ticks = 8 * TICKS_PER_YEAR;
    dependent.caregiver_id = Some(1);
    let mut simulation = sharing_simulation(giver, vec![dependent, adult(3, 100.0)]);
    assert_eq!(run_and_shared_target(&mut simulation), Some(2));
}

#[test]
fn invisible_hungry_sibling_does_not_leak_need() {
    let mut giver = neutral_giver();
    giver.mother_id = Some(10);
    let mut hidden_sibling = adult(2, 100.0);
    hidden_sibling.x = 12;
    hidden_sibling.mother_id = Some(10);
    let visible_stranger = adult(3, 80.0);
    let mut simulation = sharing_simulation(giver, vec![hidden_sibling, visible_stranger]);
    assert_eq!(run_and_shared_target(&mut simulation), Some(3));
}

#[test]
fn end_to_end_partner_sharing_conserves_food_and_records_gratitude() {
    let mut giver = adult(1, 0.0);
    giver.partner_id = Some(2);
    giver.personality.cooperativeness = 0.3;
    let partner = adult(2, 80.0);
    let stranger = adult(3, 80.0);
    let mut simulation = sharing_simulation(giver, vec![partner, stranger]);
    assert_eq!(run_and_shared_target(&mut simulation), Some(2));
    assert_eq!(simulation.entities[0].inventory.amount(ItemKind::Food), 20);
    assert_eq!(simulation.entities[1].inventory.amount(ItemKind::Food), 10);
    assert_eq!(simulation.entities[2].inventory.amount(ItemKind::Food), 0);
    assert_eq!(
        simulation
            .entities
            .iter()
            .map(|entity| entity.inventory.amount(ItemKind::Food))
            .sum::<u16>(),
        30
    );
    assert!(
        simulation.entities[1]
            .mind
            .memory
            .affinity_to(1)
            .unwrap_or(0)
            > 0
    );
}

#[test]
fn very_uncooperative_relative_can_refuse_with_existing_event_and_resentment() {
    let mut giver = adult(1, 0.0);
    giver.partner_id = Some(2);
    giver.personality.cooperativeness = 0.0;
    let mut simulation = sharing_simulation(giver, vec![adult(2, 90.0)]);
    simulation.step(&mut plain_grid(3, 3));
    assert_eq!(simulation.entities[0].inventory.amount(ItemKind::Food), 30);
    assert!(simulation
        .recent_events()
        .any(|event| event.kind == SimulationEventKind::FoodShareRefused));
    assert!(
        simulation.entities[1]
            .mind
            .memory
            .affinity_to(1)
            .unwrap_or(0)
            < 0
    );
}

#[test]
fn relationship_food_sharing_matches_normal_and_profiled_paths() {
    let mut giver = neutral_giver();
    giver.partner_id = Some(3);
    let base = sharing_simulation(giver, vec![adult(2, 80.0), adult(3, 80.0)]);
    let mut normal = base.clone();
    let mut profiled = base.clone();
    let mut autonomy_profiled = base;
    normal.step(&mut plain_grid(3, 3));
    profiled.profile_step(&mut plain_grid(3, 3));
    autonomy_profiled.profile_autonomy_step(&mut plain_grid(3, 3));
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
                    entity.inventory.amount(ItemKind::Food),
                    entity.mind.memory.affinity_to(1),
                    entity.household_id,
                    entity.caregiver_id,
                )
            })
            .collect::<Vec<_>>()
    };
    assert_eq!(state(&normal), state(&profiled));
    assert_eq!(state(&normal), state(&autonomy_profiled));
    assert!(matches!(
        normal.entities[0].mind.current_goal,
        Some(Goal::ShareFood)
    ));
    assert!(matches!(
        normal.entities[0].mind.current_action(),
        None | Some(Action::ShareFood(3))
    ));
}
