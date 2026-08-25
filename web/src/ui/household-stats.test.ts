import { describe, expect, it } from "vitest";

import { householdStatsValues } from "./household-stats";
import type { HouseholdStats } from "../types";

function payload(overrides: Partial<HouseholdStats> = {}): string {
  return JSON.stringify({
    total_households: 0,
    active_households: 0,
    dissolved_households: 0,
    housed_entities: 0,
    unhoused_entities: 0,
    average_active_household_size: 0,
    largest_active_household_size: 0,
    single_member_households: 0,
    households_with_dependents: 0,
    active_storage_capacity: 0,
    active_storage_used: 0,
    active_storage_utilization: 0,
    active_food_stored: 0,
    active_timber_stored: 0,
    active_stone_stored: 0,
    active_iron_stored: 0,
    settled_inheritances: 0,
    inheritances_without_heir: 0,
    average_active_household_age_ticks: 0,
    average_dissolved_household_lifetime_ticks: 0,
    ...overrides,
  });
}

describe("household statistics readout", () => {
  it("reads the bridge payload and formats visible values", () => {
    const values = householdStatsValues(payload({
      total_households: 8,
      active_households: 5,
      average_active_household_size: 3.36,
      active_storage_used: 370,
      active_storage_capacity: 1000,
      active_storage_utilization: 0.37,
    }));
    expect(values["household-total"]).toBe("8");
    expect(values["household-active"]).toBe("5");
    expect(values["household-average-size"]).toBe("3.4");
    expect(values["household-shared-storage"]).toBe(
      `${(370).toLocaleString()} / ${(1000).toLocaleString()} (37.0%)`,
    );
  });

  it("renders zero statistics without NaN or Infinity", () => {
    const values = Object.values(householdStatsValues(payload()));
    expect(values.join(" ")).not.toMatch(/NaN|Infinity/);
    expect(values).toContain("0 / 0 (0.0%)");
  });
});
