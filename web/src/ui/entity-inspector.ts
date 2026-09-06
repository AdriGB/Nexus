import { state, requestRender } from "../state";
import type { EntityHousehold, EntityInfo, EntityKinship, KnownRelationshipInfo } from "../types";
import { switchTab } from "./tabs";
import { centerOnTile } from "../renderer/camera";
import { updateTileInspector, syncInspectorEmptyState } from "./tile-inspector";

function infoRow(key: string, value: string, valueClass?: string): string {
  const cls = valueClass ? `info-val ${valueClass}` : "info-val";
  return `<span class="info-key">${key}</span><span class="${cls}">${value}</span>`;
}

function infoSectionTitle(title: string): string {
  return `<span class="info-key info-section-title">${title}</span>`;
}

function entityLink(id: number | null): string {
  if (id === null) return "—";
  return `<button class="entity-link-btn" type="button" data-entity-id="${id}">#${id}</button>`;
}

function entityLinks(ids: number[]): string {
  if (ids.length === 0) return "—";
  return ids
    .map((id) => `<button class="entity-link-btn" type="button" data-entity-id="${id}">#${id}</button>`)
    .join(", ");
}

const RELATIONSHIP_LIMIT = 20;

function formatTicksAgo(ticks: number): string {
  if (ticks <= 0) return "just now";
  if (ticks < 24) return `${ticks}h ago`;
  const days = Math.floor(ticks / 24);
  if (days < 365) return `${days}d ago`;
  return `${Math.floor(days / 365)}y ago`;
}

function formatTickDuration(ticks: number): string {
  if (ticks < 24) return `${ticks}h`;
  const days = Math.floor(ticks / 24);
  if (days < 365) return `${days}d`;
  return `${Math.floor(days / 365)}y`;
}

function relationshipLabel(affinity: number): string {
  if (affinity >= 200) return "Friendly";
  if (affinity > 0) return "Positive";
  if (affinity === 0) return "Neutral";
  if (affinity < -200) return "Hostile";
  return "Negative";
}

function relationshipColor(affinity: number): string {
  if (affinity > 0) return "#65c983";
  if (affinity < 0) return "#ef4444";
  return "#8e8da2";
}

function decisionExplanation(entity: EntityInfo): string {
  const explanation = entity.decision_explanation;
  if (!explanation) return "No decision evaluated yet";

  switch (explanation.reason) {
    case "goal_persistence":
      return `${explanation.chosen_goal} was retained: ${explanation.highest_utility_goal} scored ${explanation.highest_score.toFixed(2)}, but did not exceed the current score ${explanation.chosen_score.toFixed(2)} by the ${explanation.switch_margin.toFixed(2)} persistence margin.`;
    case "dependent_needs_food":
      return "Eat was selected because this dependent is hungry and remembers available food.";
    case "dependent_follows_caregiver":
      return "Follow was selected because this child depends on its caregiver and does not need food now.";
    case "dependent_provisioning":
      return `${explanation.chosen_goal} was selected to provision food for a hungry dependent.`;
    case "highest_utility":
      return `${explanation.chosen_goal} was selected with the highest utility (${explanation.chosen_score.toFixed(2)}).`;
  }
}

export function renderInventorySection(
  inventory: EntityInfo["inventory"],
): string {
  const contents = inventory.items.length
    ? inventory.items
        .map((item) => `${item.kind}: ${item.amount}`)
        .join(" · ")
    : "Empty";
  return (
    infoSectionTitle("Inventory") +
    infoRow(
      "Carrying capacity",
      `${inventory.used_capacity} / ${inventory.capacity}`,
    ) +
    infoRow("Contents", contents)
  );
}

export function renderHouseholdSection(household: EntityHousehold): string {
  const storageContents = household.storage?.items.length
    ? household.storage.items.map((item) => `${item.kind}: ${item.amount}`).join(" · ")
    : "Empty";
  return (
    infoSectionTitle("Household") +
    infoRow("Household", household.household_id === null ? "—" : `#${household.household_id}`) +
    infoRow(
      "Members",
      household.member_ids.length === 0
        ? "—"
        : household.member_ids.map((id) => `#${id}`).join(", "),
    ) +
    infoRow(
      "Storage",
      household.storage === null
        ? "—"
        : `${household.storage.used_capacity} / ${household.storage.capacity}`,
    ) +
    infoRow("Contents", household.storage === null ? "—" : storageContents) +
    infoRow(
      "Residence",
      household.residence_x === null || household.residence_y === null
        ? "—"
        : `(${household.residence_x}, ${household.residence_y})`,
    ) +
    infoRow(
      "Formed",
      household.formed_tick === null
        ? "—"
        : `${household.formed_tick.toLocaleString()} ticks`,
    )
  );
}

function relationshipsSection(
  relationships: KnownRelationshipInfo[],
  tick: number,
): string {
  if (relationships.length === 0) return "";

  const shown = relationships.slice(0, RELATIONSHIP_LIMIT);
  const truncated =
    relationships.length > shown.length
      ? `<span class="info-key"></span><span class="info-val">Showing ${shown.length} of ${relationships.length} relationships</span>`
      : "";

  const blocks = shown.map((relationship) => {
    const label = relationshipLabel(relationship.affinity);
    const color = relationshipColor(relationship.affinity);
    const affinityText = `${relationship.affinity > 0 ? "+" : ""}${relationship.affinity}`;
    const cooldown =
      relationship.seek_retry_after_tick != null &&
      relationship.seek_retry_after_tick > tick
        ? ` · seek cooldown ${formatTickDuration(relationship.seek_retry_after_tick - tick)} remaining`
        : "";
    const lastInteraction =
      relationship.last_interaction_tick === 0
        ? "Never interacted"
        : `Last interaction ${formatTicksAgo(tick - relationship.last_interaction_tick)}`;
    const lastSeen = `Last seen (${relationship.last_seen_x}, ${relationship.last_seen_y}) ${formatTicksAgo(tick - relationship.last_seen_tick)}`;

    return `
      <span class="info-key" style="margin-top:6px;"><button class="entity-link-btn" type="button" data-entity-id="${relationship.id}">#${relationship.id}</button></span>
      <span class="info-val" style="color:${color};">${affinityText} ${label}${cooldown}</span>
      <span class="info-key"></span>
      <span class="info-val">${relationship.interaction_count} interactions · observed ${relationship.observed_ticks} ticks</span>
      <span class="info-key"></span>
      <span class="info-val">${lastInteraction}</span>
      <span class="info-key"></span>
      <span class="info-val">${lastSeen}</span>`;
  });

  return (
    infoSectionTitle(`Relationships (${relationships.length})`) +
    truncated +
    blocks.join("")
  );
}

export function selectEntity(id: number | null): void {
  state.selectedEntityId = id;
  if (id === null) {
    state.cameraFollowEntityId = null;
  } else if (state.world) {
    try {
      const raw = state.world.entity_info(id);
      if (raw && raw !== "{}") {
        const ent = JSON.parse(raw);
        if (ent.id !== undefined) {
          state.selectedTile = { x: ent.x, y: ent.y };
        }
      }
    } catch (_) {
      /* ignore */
    }
  }

  switchTab("inspector");
  syncEntityInspector();
  updateTileInspector();
  requestRender();
}

export function bindEntityInspector(): void {
  const followBtn = document.getElementById("btn-entity-follow");
  const centerBtn = document.getElementById("btn-entity-center");
  const chipsContainer = document.getElementById("entity-chips-container");
  const infoGrid = document.getElementById("entity-info-grid");

  followBtn?.addEventListener("click", () => {
    if (state.selectedEntityId === null) return;
    if (state.cameraFollowEntityId === state.selectedEntityId) {
      state.cameraFollowEntityId = null;
    } else {
      state.cameraFollowEntityId = state.selectedEntityId;
    }
    syncEntityInspector();
    requestRender();
  });

  centerBtn?.addEventListener("click", () => {
    if (state.selectedEntityId === null || !state.world) return;
    try {
      const raw = state.world.entity_info(state.selectedEntityId);
      if (raw && raw !== "{}") {
        const ent = JSON.parse(raw);
        if (ent.id !== undefined) {
          centerOnTile(ent.x, ent.y);
        }
      }
    } catch (_) {
      /* ignore */
    }
  });

  const handleEntityClick = (e: MouseEvent) => {
    const target = (e.target as HTMLElement).closest<HTMLElement>("[data-entity-id]");
    if (!target || !target.dataset.entityId) return;
    const id = Number(target.dataset.entityId);
    if (!Number.isNaN(id)) {
      selectEntity(id);
    }
  };

  chipsContainer?.addEventListener("click", handleEntityClick);
  infoGrid?.addEventListener("click", handleEntityClick);
}

export function syncEntityInspector(): void {
  const panel = document.getElementById("entity-inspector");
  const grid = document.getElementById("entity-info-grid");
  const chipsContainer = document.getElementById("entity-chips-container");
  const titleEl = document.getElementById("entity-inspector-title");
  const followBtn = document.getElementById("btn-entity-follow");

  if (!panel || !grid) return;

  if (!state.world || state.world.entity_count() === 0) {
    panel.hidden = true;
    grid.innerHTML = "";
    if (chipsContainer) {
      chipsContainer.hidden = true;
      chipsContainer.innerHTML = "";
    }
    state.selectedEntityId = null;
    state.cameraFollowEntityId = null;
    syncInspectorEmptyState();
    return;
  }

  // If no entity is selected, inspect any entity at selectedTile
  if (state.selectedEntityId === null && state.selectedTile) {
    const atTile = state.world.entities_at(state.selectedTile.x, state.selectedTile.y);
    if (atTile.length > 0) {
      state.selectedEntityId = atTile[0];
    }
  }

  if (state.selectedEntityId === null) {
    panel.hidden = true;
    grid.innerHTML = "";
    if (chipsContainer) {
      chipsContainer.hidden = true;
      chipsContainer.innerHTML = "";
    }
    syncInspectorEmptyState();
    return;
  }

  let entity: EntityInfo;
  try {
    const raw = state.world.entity_info(state.selectedEntityId);
    if (!raw || raw === "{}") {
      panel.hidden = true;
      grid.innerHTML = "";
      if (chipsContainer) {
        chipsContainer.hidden = true;
        chipsContainer.innerHTML = "";
      }
      state.selectedEntityId = null;
      if (state.cameraFollowEntityId === state.selectedEntityId) {
        state.cameraFollowEntityId = null;
      }
      syncInspectorEmptyState();
      return;
    }
    entity = JSON.parse(raw);
    if (entity.id === undefined) {
      panel.hidden = true;
      grid.innerHTML = "";
      if (chipsContainer) {
        chipsContainer.hidden = true;
        chipsContainer.innerHTML = "";
      }
      state.selectedEntityId = null;
      syncInspectorEmptyState();
      return;
    }
  } catch (_) {
    panel.hidden = true;
    grid.innerHTML = "";
    syncInspectorEmptyState();
    return;
  }

  panel.hidden = false;
  syncInspectorEmptyState();

  if (titleEl) {
    titleEl.textContent = `Entity #${entity.id}`;
  }

  if (followBtn) {
    const isFollowing = state.cameraFollowEntityId === entity.id;
    followBtn.classList.toggle("active", isFollowing);
    followBtn.textContent = isFollowing ? "Following" : "Follow";
  }

  if (chipsContainer) {
    const coLocated = state.world.entities_at(entity.x, entity.y);
    if (coLocated.length > 1) {
      chipsContainer.hidden = false;
      chipsContainer.innerHTML = Array.from(coLocated)
        .map(
          (id) =>
            `<button class="entity-chip ${id === entity.id ? "active" : ""}" data-entity-id="${id}" type="button">#${id}</button>`,
        )
        .join("");
    } else {
      chipsContainer.hidden = true;
      chipsContainer.innerHTML = "";
    }
  }

  const relationships = JSON.parse(
    state.world.entity_relationships(entity.id),
  ) as KnownRelationshipInfo[];
  const kinship = JSON.parse(state.world.entity_kinship(entity.id)) as EntityKinship;
  const household = JSON.parse(state.world.entity_household(entity.id)) as EntityHousehold;
  const simulationTick = Number(state.world.simulation_tick());
  const relationshipsHtml = relationshipsSection(relationships, simulationTick);
  const dueInHours =
    entity.pregnancy_due_tick === null
      ? null
      : Math.max(
          0,
          entity.pregnancy_due_tick - simulationTick,
        );

  grid.innerHTML = [
    infoRow("ID", `#${entity.id}`),
    infoRow("Sex", entity.sex),
    infoRow("Position", `(${entity.x}, ${entity.y})`),
    infoRow("Movement Credit", entity.movement_credit.toFixed(2)),
    infoRow("Life Stage", entity.life_stage),
    infoRow(
      "Stage Speed",
      `${(entity.stage_movement_factor * 100).toFixed(0)}%`,
    ),
    infoSectionTitle("Kinship"),
    infoRow("Mother", entityLink(kinship.mother_id)),
    infoRow("Father", entityLink(kinship.father_id)),
    infoRow("Children", entityLinks(kinship.children_ids)),
    infoRow("Siblings", entityLinks(kinship.sibling_ids)),
    `<span class="info-key"></span><button class="btn-secondary family-tree-open" type="button" data-family-tree-id="${entity.id}">View Family Tree</button>`,
    infoRow("Caregiver", entityLink(entity.caregiver_id)),
    infoRow("Partner", entityLink(entity.partner_id)),
    renderHouseholdSection(household),
    infoSectionTitle("Personality"),
    infoRow(
      "Curiosity",
      `${(entity.personality.curiosity * 100).toFixed(0)}%`,
    ),
    infoRow(
      "Sociability",
      `${(entity.personality.sociability * 100).toFixed(0)}%`,
    ),
    infoRow(
      "Cooperativeness",
      `${(entity.personality.cooperativeness * 100).toFixed(0)}%`,
    ),
    infoRow("Caution", `${(entity.personality.caution * 100).toFixed(0)}%`),
    infoRow(
      "Persistence",
      `${(entity.personality.persistence * 100).toFixed(0)}%`,
    ),
    infoRow("Hunger", `${entity.hunger.toFixed(0)} / 100`),
    infoRow("Health", `${entity.health.toFixed(0)} / 100`),
    infoRow("Age", `${entity.age_years.toFixed(1)} years`),
    infoRow("Age ticks", entity.age_ticks.toLocaleString()),
    infoRow("Lifespan", `${entity.lifespan_ticks.toLocaleString()} ticks`),
    infoRow("Pregnant", entity.pregnant ? "Yes" : "No"),
    ...(dueInHours === null
      ? []
      : [infoRow("Due", `${dueInHours.toLocaleString()} hours`)]),
    infoRow("Activity", entity.activity, "entity-activity"),
    infoRow("Goal", entity.goal, "entity-goal"),
    infoRow("Decision", decisionExplanation(entity)),
    infoRow("Action", entity.action),
    ...(entity.action_duration_ticks === null
      ? []
      : [
          infoRow(
            "Action progress",
            `${entity.action_progress_ticks} / ${entity.action_duration_ticks} ticks`,
          ),
        ]),
    infoRow("Goal retained", `${entity.goal_age_ticks.toLocaleString()} ticks`),
    infoRow("Path remaining", entity.remaining_path.toString()),
    infoRow("Known resources", entity.known_resources.toString()),
    infoRow("Known individuals", entity.known_entities.toString()),
    infoRow("Known chunks", entity.known_chunks.toString()),
    infoRow("Visible entities", entity.visible_entities.toString()),
    infoRow("Utility: eat", entity.utilities.eat.toFixed(2)),
    infoRow("Utility: acquire resource", entity.utilities.acquire_resource.toFixed(2)),
    infoRow("Utility: explore", entity.utilities.explore.toFixed(2)),
    infoRow("Utility: rest", entity.utilities.rest.toFixed(2)),
    infoRow("Utility: socialize", entity.utilities.socialize.toFixed(2)),
    infoRow("Utility: share food", entity.utilities.share_food.toFixed(2)),
  ].join("") + renderInventorySection(entity.inventory) + relationshipsHtml;
}
