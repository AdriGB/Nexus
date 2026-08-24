import { describe, expect, it } from "vitest";

import type { FamilyTree } from "../types";
import { renderFamilyTree } from "./family-tree";

function tree(nodes: FamilyTree["nodes"]): FamilyTree {
  return { focal_id: 12, nodes, edges: [] };
}

describe("family tree view", () => {
  it("renders generation groups and marks deceased nodes", () => {
    const html = renderFamilyTree(tree([
      { entity_id: 1, generation: -2, alive: false },
      { entity_id: 5, generation: -1, alive: false },
      { entity_id: 12, generation: 0, alive: true },
      { entity_id: 20, generation: 1, alive: true },
    ]));

    expect(html).toContain("Grandparents");
    expect(html).toContain("Parents");
    expect(html).toContain("Selected");
    expect(html).toContain("Children");
    expect(html).toContain("#1 †");
    expect(html).toContain('title="Deceased historical entity"');
  });

  it("makes living nodes navigable but not deceased nodes", () => {
    const html = renderFamilyTree(tree([
      { entity_id: 5, generation: -1, alive: false },
      { entity_id: 12, generation: 0, alive: true },
      { entity_id: 20, generation: 1, alive: true },
    ]));

    expect(html).toContain('data-family-tree-node="12"');
    expect(html).toContain('data-family-tree-node="20"');
    expect(html).not.toContain('data-family-tree-node="5"');
  });

  it("renders an unknown tree safely", () => {
    expect(renderFamilyTree(tree([]))).toContain("No persistent genealogy");
  });

  it("does not duplicate converging nodes", () => {
    const html = renderFamilyTree(tree([
      { entity_id: 1, generation: -2, alive: false },
      { entity_id: 1, generation: -2, alive: false },
      { entity_id: 12, generation: 0, alive: true },
    ]));

    expect(html.match(/#1 †/g)).toHaveLength(1);
  });
});
