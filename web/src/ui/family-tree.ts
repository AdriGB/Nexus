import { state } from "../state";
import type { FamilyTree, FamilyTreeNode } from "../types";

const ANCESTOR_DEPTH = 2;
const DESCENDANT_DEPTH = 2;

function generationLabel(generation: number): string {
  if (generation === 0) return "Selected";
  if (generation === -1) return "Parents";
  if (generation === -2) return "Grandparents";
  if (generation === 1) return "Children";
  if (generation === 2) return "Grandchildren";
  return generation < 0
    ? `Ancestors · ${Math.abs(generation)} generations`
    : `Descendants · ${generation} generations`;
}

function renderNode(node: FamilyTreeNode, focalId: number): string {
  const classes = ["family-tree-node"];
  if (!node.alive) classes.push("deceased");
  if (node.entity_id === focalId) classes.push("focal");
  const label = `#${node.entity_id}${node.alive ? "" : " †"}`;
  if (!node.alive) {
    return `<span class="${classes.join(" ")}" title="Deceased historical entity">${label}</span>`;
  }
  return `<button class="${classes.join(" ")}" type="button" data-family-tree-node="${node.entity_id}" aria-label="View family tree for entity ${node.entity_id}">${label}</button>`;
}

export function renderFamilyTree(tree: FamilyTree): string {
  if (tree.nodes.length === 0) {
    return `<p class="family-tree-empty">No persistent genealogy is known for entity #${tree.focal_id}.</p>`;
  }

  const uniqueNodes = [...new Map(tree.nodes.map((node) => [node.entity_id, node])).values()];
  const groups = new Map<number, FamilyTreeNode[]>();
  for (const node of uniqueNodes) {
    const group = groups.get(node.generation) ?? [];
    group.push(node);
    groups.set(node.generation, group);
  }

  return [...groups.entries()]
    .sort(([first], [second]) => first - second)
    .map(([generation, nodes]) => `
      <section class="family-tree-generation" data-generation="${generation}">
        <h3>${generationLabel(generation)}</h3>
        <div class="family-tree-nodes">
          ${nodes.sort((a, b) => a.entity_id - b.entity_id).map((node) => renderNode(node, tree.focal_id)).join("")}
        </div>
      </section>`)
    .join("");
}

function showFamilyTree(entityId: number): void {
  if (!state.world) return;
  const tree = JSON.parse(
    state.world.entity_family_tree(entityId, ANCESTOR_DEPTH, DESCENDANT_DEPTH),
  ) as FamilyTree;
  const overlay = document.getElementById("family-tree-overlay")!;
  document.getElementById("family-tree-content")!.innerHTML = renderFamilyTree(tree);
  overlay.hidden = false;
}

function closeFamilyTree(): void {
  document.getElementById("family-tree-overlay")!.hidden = true;
}

export function bindFamilyTree(): void {
  const overlay = document.getElementById("family-tree-overlay")!;
  document.getElementById("entity-inspector")!.addEventListener("click", (event) => {
    const button = (event.target as HTMLElement).closest<HTMLButtonElement>("[data-family-tree-id]");
    if (button) showFamilyTree(Number(button.dataset.familyTreeId));
  });
  document.getElementById("family-tree-content")!.addEventListener("click", (event) => {
    const button = (event.target as HTMLElement).closest<HTMLButtonElement>("[data-family-tree-node]");
    if (button) showFamilyTree(Number(button.dataset.familyTreeNode));
  });
  document.getElementById("btn-family-tree-close")!.addEventListener("click", closeFamilyTree);
  overlay.addEventListener("click", (event) => {
    if (event.target === overlay) closeFamilyTree();
  });
  document.addEventListener("keydown", (event) => {
    if (event.key === "Escape" && !overlay.hidden) closeFamilyTree();
  });
}
