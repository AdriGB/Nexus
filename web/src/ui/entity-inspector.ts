import { state } from "../state";
import type { EntityInfo } from "../types";

function infoRow(key: string, value: string, valueClass?: string): string {
  const cls = valueClass ? `info-val ${valueClass}` : "info-val";
  return `<span class="info-key">${key}</span><span class="${cls}">${value}</span>`;
}

function infoSectionTitle(title: string): string {
  return `<span class="info-key info-section-title">${title}</span>`;
}

export function syncEntityInspector(): void {
  const panel = document.getElementById("entity-inspector")!;
  const grid = document.getElementById("entity-info-grid")!;
  if (!state.world || state.world.entity_count() === 0) {
    panel.hidden = true;
    grid.innerHTML = "";
    return;
  }

  const entity: EntityInfo = JSON.parse(state.world.first_entity_info());
  const dueInHours =
    entity.pregnancy_due_tick === null
      ? null
      : Math.max(
          0,
          entity.pregnancy_due_tick - Number(state.world.simulation_tick()),
        );
  panel.hidden = false;
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
    infoRow("Caregiver", entity.caregiver_id?.toString() ?? "—"),
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
    infoRow("Action", entity.action),
    infoRow("Goal retained", `${entity.goal_age_ticks.toLocaleString()} ticks`),
    infoRow("Path remaining", entity.remaining_path.toString()),
    infoRow("Known resources", entity.known_resources.toString()),
    infoRow("Known individuals", entity.known_entities.toString()),
    infoRow("Known chunks", entity.known_chunks.toString()),
    infoRow("Visible entities", entity.visible_entities.toString()),
    infoRow("Utility: eat", entity.utilities.eat.toFixed(2)),
    infoRow("Utility: explore", entity.utilities.explore.toFixed(2)),
    infoRow("Utility: rest", entity.utilities.rest.toFixed(2)),
  ].join("");
}
