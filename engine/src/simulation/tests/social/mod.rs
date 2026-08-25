use super::super::autonomy::{Action, DecisionContext};
use super::super::entity::{Entity, Personality};
use super::super::time::{TICKS_PER_DAY, TICKS_PER_YEAR};
use super::super::Simulation;
use super::support::*;

fn default_adult(id: u32, x: u32, y: u32) -> Entity {
    let mut entity = entity(id, x, y, 0.0);

    entity.age_ticks = 25 * TICKS_PER_YEAR;
    entity.personality = Personality {
        curiosity: 0.5,
        sociability: 0.5,
        cooperativeness: 0.5,
        caution: 0.5,
        persistence: 0.5,
    };

    entity
}

fn interaction_count(simulation: &Simulation, observer: usize, target_id: u32) -> u32 {
    simulation.entities()[observer]
        .mind
        .memory
        .known_entities
        .iter()
        .find(|known| known.id == target_id)
        .map_or(0, |known| known.interaction_count)
}

fn affinity(simulation: &Simulation, observer: usize, target_id: u32) -> i16 {
    simulation.entities()[observer]
        .mind
        .memory
        .known_entities
        .iter()
        .find(|known| known.id == target_id)
        .expect("relationship should exist")
        .affinity
}

// ── Socialization goal tests ──────────────────────────────────────────────

use super::super::autonomy::Goal;

fn has_socialize_goal(sim: &Simulation, entity_index: usize) -> bool {
    sim.entities()[entity_index].mind.current_goal == Some(Goal::Socialize)
}

mod goals;
mod interactions;
mod pursuit;
mod relationships;
mod seeking;
