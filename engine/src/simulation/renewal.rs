//! Deterministic renewal of natural resource deposits.

use crate::world::{Grid, ResourceDeposit};

const REGENERATION_DIVISOR: u16 = 100;

pub(super) fn regenerate(world: &mut Grid) -> bool {
    let mut changed = false;

    for renewable in &world.renewable_resources {
        let Some(slot) = world.resources.get_mut(renewable.index) else {
            continue;
        };
        let current = match slot {
            Some(deposit) if deposit.kind == renewable.kind => deposit.amount,
            Some(_) => continue,
            None => 0,
        };
        if current >= renewable.capacity {
            continue;
        }

        let daily_growth = renewable.capacity.div_ceil(REGENERATION_DIVISOR);
        let amount = current.saturating_add(daily_growth).min(renewable.capacity);
        *slot = Some(ResourceDeposit {
            kind: renewable.kind,
            amount,
        });
        changed = true;
    }

    changed
}
