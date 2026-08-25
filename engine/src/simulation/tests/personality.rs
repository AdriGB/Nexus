use super::super::autonomy::{evaluate_goals, DecisionContext, Goal, Mind};
use super::super::config::MAX_HEALTH;
use super::super::entity::Personality;
use super::super::lifecycle::personality_for;
use super::super::time::TICKS_PER_YEAR;
use super::support::*;
use crate::world::{ResourceDeposit, ResourceKind};

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

    evaluate_goals(
        &mut mind_base,
        hunger,
        health,
        age,
        &base,
        None,
        DecisionContext {
            tick: 0,
            origin: (0, 0),
            food_in_inventory: 10,
            best_visible_food_share_score: None,
            best_remembered_social_score: None,
        },
    );
    evaluate_goals(
        &mut mind_curious,
        hunger,
        health,
        age,
        &curious,
        None,
        DecisionContext {
            tick: 0,
            origin: (0, 0),
            food_in_inventory: 10,
            best_visible_food_share_score: None,
            best_remembered_social_score: None,
        },
    );

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

    evaluate_goals(
        &mut mind_base,
        hunger,
        health,
        age,
        &base,
        None,
        DecisionContext {
            tick: 0,
            origin: (0, 0),
            food_in_inventory: 10,
            best_visible_food_share_score: None,
            best_remembered_social_score: None,
        },
    );
    evaluate_goals(
        &mut mind_cautious,
        hunger,
        health,
        age,
        &cautious,
        None,
        DecisionContext {
            tick: 0,
            origin: (0, 0),
            food_in_inventory: 10,
            best_visible_food_share_score: None,
            best_remembered_social_score: None,
        },
    );

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

    evaluate_goals(
        &mut mind,
        hunger,
        health,
        age,
        &neutral,
        None,
        DecisionContext {
            tick: 0,
            origin: (0, 0),
            food_in_inventory: 10,
            best_visible_food_share_score: None,
            best_remembered_social_score: None,
        },
    );

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

    evaluate_goals(
        &mut mind_extreme,
        hunger,
        health,
        age,
        &extreme,
        None,
        DecisionContext {
            tick: 0,
            origin: (0, 0),
            food_in_inventory: 0,
            best_visible_food_share_score: None,
            best_remembered_social_score: None,
        },
    );
    evaluate_goals(
        &mut mind_neutral,
        hunger,
        health,
        age,
        &neutral,
        None,
        DecisionContext {
            tick: 0,
            origin: (0, 0),
            food_in_inventory: 0,
            best_visible_food_share_score: None,
            best_remembered_social_score: None,
        },
    );

    assert_eq!(
        mind_extreme.utility_scores.eat,
        mind_neutral.utility_scores.eat
    );
}

#[test]
fn persistence_changes_goal_switch_after_plan_completion() {
    let mut world_low = plain_grid(64, 64);
    let mut world_high = plain_grid(64, 64);

    let food_index = (32 * 64 + 34) as usize;

    world_low.resources[food_index] = Some(ResourceDeposit {
        kind: ResourceKind::Food,
        amount: 20,
    });
    world_high.resources[food_index] = Some(ResourceDeposit {
        kind: ResourceKind::Food,
        amount: 20,
    });

    let mut sim_low = simulation_with_entity(32, 32, 33.0);
    let mut sim_high = simulation_with_entity(32, 32, 33.0);

    for simulation in [&mut sim_low, &mut sim_high] {
        simulation.entities[0].age_ticks = 25 * TICKS_PER_YEAR;
        simulation.entities[0].personality.curiosity = 0.0;
        simulation.entities[0].personality.caution = 0.0;

        // Completed Explore plan, but goal remains active.
        simulation.entities[0]
            .mind
            .set_plan(Goal::Explore, vec![], 0);
    }

    sim_low.entities[0].personality.persistence = 0.0;
    sim_high.entities[0].personality.persistence = 1.0;

    sim_low.step(&mut world_low);
    sim_high.step(&mut world_high);

    assert_eq!(
        sim_low.entities()[0].mind.current_goal,
        Some(Goal::AcquireResource)
    );
    assert_eq!(
        sim_high.entities()[0].mind.current_goal,
        Some(Goal::Explore)
    );
}
