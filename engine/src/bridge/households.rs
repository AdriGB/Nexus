use serde::Serialize;

use super::to_json;
use crate::simulation::{self, HouseholdStats, ItemKind, Simulation};

#[derive(Serialize)]
struct HouseholdStatsDto {
    total_households: u32,
    active_households: u32,
    dissolved_households: u32,
    housed_entities: u32,
    unhoused_entities: u32,
    average_active_household_size: f32,
    largest_active_household_size: u32,
    single_member_households: u32,
    households_with_dependents: u32,
    active_storage_capacity: u64,
    active_storage_used: u64,
    active_storage_utilization: f32,
    active_food_stored: u64,
    active_timber_stored: u64,
    active_stone_stored: u64,
    active_iron_stored: u64,
    settled_inheritances: u32,
    inheritances_without_heir: u32,
    average_active_household_age_ticks: f64,
    average_dissolved_household_lifetime_ticks: f64,
}

pub(crate) fn household_stats_json(stats: HouseholdStats) -> String {
    to_json(&HouseholdStatsDto {
        total_households: stats.total_households,
        active_households: stats.active_households,
        dissolved_households: stats.dissolved_households,
        housed_entities: stats.housed_entities,
        unhoused_entities: stats.unhoused_entities,
        average_active_household_size: stats.average_active_household_size,
        largest_active_household_size: stats.largest_active_household_size,
        single_member_households: stats.single_member_households,
        households_with_dependents: stats.households_with_dependents,
        active_storage_capacity: stats.active_storage_capacity,
        active_storage_used: stats.active_storage_used,
        active_storage_utilization: stats.active_storage_utilization,
        active_food_stored: stats.active_food_stored,
        active_timber_stored: stats.active_timber_stored,
        active_stone_stored: stats.active_stone_stored,
        active_iron_stored: stats.active_iron_stored,
        settled_inheritances: stats.settled_inheritances,
        inheritances_without_heir: stats.inheritances_without_heir,
        average_active_household_age_ticks: stats.average_active_household_age_ticks,
        average_dissolved_household_lifetime_ticks: stats
            .average_dissolved_household_lifetime_ticks,
    })
}

#[derive(Serialize)]
struct InventoryItemDto {
    kind: &'static str,
    amount: u16,
}

#[derive(Serialize)]
struct InventoryDto {
    capacity: u16,
    used_capacity: u16,
    remaining_capacity: u16,
    items: Vec<InventoryItemDto>,
}

#[derive(Serialize)]
struct EntityHouseholdDto {
    household_id: Option<u32>,
    member_ids: Vec<u32>,
    formed_tick: Option<u64>,
    residence_x: Option<u32>,
    residence_y: Option<u32>,
    storage: Option<InventoryDto>,
}

pub(crate) fn entity_household_json(simulation: &Simulation, entity_id: u32) -> String {
    let household_id = simulation
        .entities()
        .binary_search_by_key(&entity_id, |entity| entity.id)
        .ok()
        .and_then(|index| simulation.entities()[index].household_id);
    let household = household_id.and_then(|id| {
        simulation
            .households()
            .binary_search_by_key(&id, |household| household.id)
            .ok()
            .filter(|index| simulation.households()[*index].is_active())
            .map(|index| &simulation.households()[index])
    });

    to_json(&EntityHouseholdDto {
        household_id,
        member_ids: household_id.map_or_else(Vec::new, |id| {
            simulation::members_of(simulation.entities(), id)
        }),
        formed_tick: household.map(|household| household.formed_tick),
        residence_x: household.map(|household| household.residence_x),
        residence_y: household.map(|household| household.residence_y),
        storage: household.map(|household| InventoryDto {
            capacity: household.storage.capacity(),
            used_capacity: household.storage.used_capacity(),
            remaining_capacity: household.storage.remaining_capacity(),
            items: ItemKind::ALL
                .into_iter()
                .filter_map(|kind| {
                    let amount = household.storage.amount(kind);
                    (amount > 0).then_some(InventoryItemDto {
                        kind: kind.label(),
                        amount,
                    })
                })
                .collect(),
        }),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn household_stats_json_is_valid_with_stable_empty_shape() {
        let json: serde_json::Value =
            serde_json::from_str(&household_stats_json(HouseholdStats::default())).unwrap();
        assert_eq!(json["total_households"], 0);
        assert_eq!(json["active_storage_utilization"], 0.0);
        assert_eq!(json["average_active_household_age_ticks"], 0.0);
        assert_eq!(json.as_object().unwrap().len(), 20);
    }

    #[test]
    fn household_stats_json_serializes_counts_storage_and_fractional_averages() {
        let json: serde_json::Value = serde_json::from_str(&household_stats_json(HouseholdStats {
            total_households: 3,
            active_households: 2,
            dissolved_households: 1,
            housed_entities: 5,
            active_storage_capacity: 400,
            active_storage_used: 123,
            active_storage_utilization: 0.3075,
            active_food_stored: 80,
            average_active_household_size: 2.5,
            average_dissolved_household_lifetime_ticks: 12.5,
            ..HouseholdStats::default()
        }))
        .unwrap();
        assert_eq!(json["total_households"], 3);
        assert_eq!(json["housed_entities"], 5);
        assert_eq!(json["active_storage_used"], 123);
        assert_eq!(json["active_food_stored"], 80);
        assert_eq!(json["average_active_household_size"], 2.5);
        assert_eq!(json["average_dissolved_household_lifetime_ticks"], 12.5);
    }
}
