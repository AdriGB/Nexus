import { state } from "../state";
import type { HouseholdStats } from "../types";

export function syncHouseholdStats(): void {
  if (!state.world) return;
  const values = householdStatsValues(state.world.household_stats());
  for (const [id, value] of Object.entries(values)) {
    document.getElementById(id)!.textContent = value;
  }
}

export function householdStatsValues(payload: string): Record<string, string> {
  const stats: HouseholdStats = JSON.parse(payload);
  return {
    "household-total": stats.total_households.toLocaleString(),
    "household-active": stats.active_households.toLocaleString(),
    "household-dissolved": stats.dissolved_households.toLocaleString(),
    "household-housed": stats.housed_entities.toLocaleString(),
    "household-unhoused": stats.unhoused_entities.toLocaleString(),
    "household-average-size": stats.average_active_household_size.toFixed(1),
    "household-largest": stats.largest_active_household_size.toLocaleString(),
    "household-with-dependents": stats.households_with_dependents.toLocaleString(),
    "household-shared-storage": `${stats.active_storage_used.toLocaleString()} / ${stats.active_storage_capacity.toLocaleString()} (${(stats.active_storage_utilization * 100).toFixed(1)}%)`,
    "household-food": stats.active_food_stored.toLocaleString(),
    "household-inheritances": stats.settled_inheritances.toLocaleString(),
    "household-no-heir": stats.inheritances_without_heir.toLocaleString(),
  };
}
