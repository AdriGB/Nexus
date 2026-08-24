import { describe, expect, it } from "vitest";

import { renderHouseholdSection, renderInventorySection } from "./entity-inspector";

describe("inventory inspector", () => {
  it("renders an empty bounded inventory", () => {
    const html = renderInventorySection({
      capacity: 50,
      used_capacity: 0,
      remaining_capacity: 50,
      items: [],
    });

    expect(html).toContain("Inventory");
    expect(html).toContain("0 / 50");
    expect(html).toContain("Empty");
  });

  it("renders typed item quantities in deterministic order", () => {
    const html = renderInventorySection({
      capacity: 50,
      used_capacity: 12,
      remaining_capacity: 38,
      items: [
        { kind: "Food", amount: 7 },
        { kind: "Stone", amount: 5 },
      ],
    });

    expect(html).toContain("12 / 50");
    expect(html).toContain("Food: 7 · Stone: 5");
  });
});

describe("household inspector", () => {
  it("renders persistent identity and deterministically ordered members", () => {
    const html = renderHouseholdSection({
      household_id: 4,
      member_ids: [12, 19],
      formed_tick: 5821,
    });

    expect(html).toContain("Household");
    expect(html).toContain("#4");
    expect(html).toContain("#12, #19");
  });

  it("renders an unassigned entity compactly", () => {
    const html = renderHouseholdSection({
      household_id: null,
      member_ids: [],
      formed_tick: null,
    });

    expect(html).toContain("—");
  });
});
