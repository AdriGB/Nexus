import { describe, expect, it } from "vitest";

import { renderInventorySection } from "./entity-inspector";

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
