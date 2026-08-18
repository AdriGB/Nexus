//! Hourly physiology rules for living entities.

use super::config::{HUNGER_PER_TICK, MAX_HUNGER, STARVATION_DAMAGE_PER_TICK};
use super::{Entity, EntityActivity};

pub(super) fn advance(entities: &mut [Entity]) {
    for entity in entities.iter_mut().filter(|entity| entity.health > 0.0) {
        entity.age_ticks = entity.age_ticks.saturating_add(1);
        entity.hunger = (entity.hunger + HUNGER_PER_TICK).min(MAX_HUNGER);
        if entity.age_ticks >= entity.lifespan_ticks {
            entity.health = 0.0;
        }
    }
}

pub(super) fn resolve_starvation(entities: &mut [Entity]) {
    for entity in entities.iter_mut().filter(|entity| entity.health > 0.0) {
        if entity.hunger >= MAX_HUNGER {
            entity.health = (entity.health - STARVATION_DAMAGE_PER_TICK).max(0.0);
            entity.activity = EntityActivity::Starving;
        }
    }
}
