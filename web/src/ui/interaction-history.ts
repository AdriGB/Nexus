import { state } from "../state";
import type { EntityEventSummary, SimulationEvent } from "../types";

const DISPLAY_BATCH_SIZE = 100;

let selectedEntityId: number | null = null;
let visibleEventLimit = DISPLAY_BATCH_SIZE;

export interface EventHistoryExport {
  schema: "nexus-event-history/v1";
  simulation_tick: string;
  entity_filter: number | null;
  event_count: number;
  events: SimulationEvent[];
}

export function createEventHistoryExport(
  events: SimulationEvent[],
  simulationTick: bigint,
  entityId: number | null,
): EventHistoryExport {
  const filtered = filterInteractionEvents(events, entityId);
  return {
    schema: "nexus-event-history/v1",
    simulation_tick: simulationTick.toString(),
    entity_filter: entityId,
    event_count: filtered.length,
    events: filtered,
  };
}

function downloadEventHistory(history: EventHistoryExport): void {
  const blob = new Blob([JSON.stringify(history, null, 2)], {
    type: "application/json",
  });
  const url = URL.createObjectURL(blob);
  const link = document.createElement("a");
  const scope = history.entity_filter === null ? "all" : `entity-${history.entity_filter}`;
  link.href = url;
  link.download = `nexus-event-history-${scope}-tick-${history.simulation_tick}.json`;
  link.click();
  URL.revokeObjectURL(url);
}

function escapeHtml(value: string): string {
  return value.replace(
    /[&<>"']/g,
    (character) =>
      ({
        "&": "&amp;",
        "<": "&lt;",
        ">": "&gt;",
        '"': "&quot;",
        "'": "&#039;",
      })[character]!,
  );
}

export function parseEntityFilter(value: string): number | null {
  if (value.trim() === "") return null;
  const id = Number(value);
  return Number.isSafeInteger(id) && id > 0 ? id : null;
}

export function filterInteractionEvents(
  events: SimulationEvent[],
  entityId: number | null,
): SimulationEvent[] {
  if (entityId === null) return events;
  return events.filter(
    (event) =>
      event.actor_id === entityId ||
      event.target_id === entityId ||
      event.related_entity_ids.includes(entityId),
  );
}

function affinityDelta(delta: number): string {
  const tone = delta > 0 ? "positive" : delta < 0 ? "negative" : "neutral";
  const symbol = delta > 0 ? "+" : delta < 0 ? "−" : "±";
  const value = Math.abs(delta);
  const label = delta > 0 ? "positive" : delta < 0 ? "negative" : "neutral";
  return `<span class="affinity-delta ${tone}" aria-label="${label} affinity change">${symbol}${value} ${label}</span>`;
}

function entityButton(id: number, label: string): string {
  return `<button class="interaction-entity-link" type="button" data-entity-id="${id}">${label} #${id}</button>`;
}

function eventDetails(event: SimulationEvent): string {
  if (event.kind === "food_shared") {
    return `shared ${event.amount ?? 0} food with #${event.target_id}`;
  }
  if (event.kind === "food_share_refused") {
    return `refused to share food with #${event.target_id}`;
  }
  if (event.kind === "interaction") {
    return `<div class="interaction-event-row">${entityButton(event.actor_id, "Actor")}${affinityDelta(event.actor_affinity_delta)}</div>
      <div class="interaction-event-row">${entityButton(event.target_id!, "Target")}${affinityDelta(event.target_affinity_delta)}</div>
      <div class="interaction-event-row"><span>Mutual social contact</span><span>(${event.location.x}, ${event.location.y})</span></div>`;
  }
  if (event.kind === "partnership_formed") {
    const compatibility = (event.compatibility_per_mille / 10).toFixed(1);
    const causalLink = event.caused_by_event_id
      ? `<div class="interaction-event-row"><span>Caused by</span><button class="interaction-entity-link" type="button" data-event-target-id="${event.caused_by_event_id}">Event #${event.caused_by_event_id}</button></div>`
      : "";
    return `<div class="interaction-event-row">${entityButton(event.actor_id, "Partner")}<span>formed a partnership with</span>${entityButton(event.target_id, "Partner")}</div>
      <div class="interaction-event-row"><span>Mutual affinity: ${event.partnership_actor_affinity} / ${event.partnership_target_affinity}</span><span>Compatibility ${compatibility}%</span></div>${causalLink}`;
  }
  if (event.kind === "birth") {
    return `<div class="interaction-event-row">${entityButton(event.actor_id, "Mother")}<span>gave birth</span></div>
      <div class="interaction-event-row">${entityButton(event.child_id, "Newborn")}<span>born at (${event.location.x}, ${event.location.y})</span></div>`;
  }
  if (event.kind === "death") {
    const cause = event.cause === "starvation" ? "Starvation" : "Natural death";
    return `<div class="interaction-event-row">${entityButton(event.actor_id, "Entity")}<span>${cause}</span></div>
      <div class="interaction-event-row"><span>Died at (${event.location.x}, ${event.location.y})</span></div>`;
  }
  if (event.kind === "consumption") {
    return `<div class="interaction-event-row">${entityButton(event.actor_id, "Entity")}<span>Ate ${event.amount} food</span></div>
      <div class="interaction-event-row"><span>Consumed at (${event.location.x}, ${event.location.y})</span></div>`;
  }
  if (event.kind === "discovery") {
    return `<div class="interaction-event-row">${entityButton(event.actor_id, "Entity")}<span>Discovered ${event.resource_kind}</span></div>
      <div class="interaction-event-row"><span>Observed ${event.amount} at (${event.location.x}, ${event.location.y})</span></div>`;
  }
  if (event.kind === "encounter") {
    return `<div class="interaction-event-row">${entityButton(event.actor_id, "Entity")}<span>first encountered</span></div>
      <div class="interaction-event-row">${entityButton(event.target_id, "Entity")}<span>at (${event.location.x}, ${event.location.y})</span></div>`;
  }
  if (event.kind === "affinity_change") {
    const causeLabel =
      event.cause === "mutual_social_contact"
        ? "Social interaction"
        : "Relationship decay";
    const signedDelta = event.delta >= 0 ? `+${event.delta}` : `${event.delta}`;
    const causalLink = event.caused_by_event_id
      ? `<div class="interaction-event-row"><span>Caused by</span><button class="interaction-entity-link" type="button" data-event-target-id="${event.caused_by_event_id}">Event #${event.caused_by_event_id}</button></div>`
      : "";
    return `<div class="interaction-event-row">${entityButton(event.actor_id, "Entity")}<span>changed attitude toward</span>${entityButton(event.target_id, "Entity")}</div>
      <div class="interaction-event-row"><span>Affinity: ${event.previous_affinity} → ${event.new_affinity} (${signedDelta})</span></div>
      <div class="interaction-event-row"><span>${causeLabel}</span><span>(${event.location.x}, ${event.location.y})</span></div>${causalLink}`;
  }
  const exhaustive: never = event;
  return exhaustive;
}

export function renderInteractionHistory(
  events: SimulationEvent[],
  entityId: number | null = null,
  visibleLimit = DISPLAY_BATCH_SIZE,
): string {
  const filtered = filterInteractionEvents(events, entityId);
  if (filtered.length === 0) {
    return `<div class="interaction-history-empty">No recent events${
      entityId === null ? "" : ` for entity #${entityId}`
    }.</div>`;
  }

  const shown = filtered.slice(0, visibleLimit);
  const cards = shown.map(
    (event) => `<article id="event-${event.id}" class="interaction-event" data-event-id="${event.id}">
      <div class="interaction-event-header">
        <strong>Event #${event.id}</strong>
        <span>Tick ${event.tick} · ${escapeHtml(event.relative_time)}</span>
      </div>
      ${eventDetails(event)}
    </article>`,
  );

  if (filtered.length > shown.length) {
    const remaining = filtered.length - shown.length;
    cards.push(
      `<button class="btn-secondary interaction-history-more" type="button" data-history-more>Show ${Math.min(DISPLAY_BATCH_SIZE, remaining)} more · ${remaining} remaining</button>`,
    );
  }
  return cards.join("");
}

export function renderEntityEventSummary(summary: EntityEventSummary): string {
  if (summary.total_events === 0) {
    return `<div class="interaction-history-empty">No recent history summary for entity #${summary.entity_id}.</div>`;
  }

  const stats: Array<[string, number]> = [
    ["Encounters", summary.encounters],
    ["Interactions", summary.interactions],
    ["Affinity changes", summary.affinity_changes],
    ["Partnerships formed", summary.partnerships_formed],
    ["Discoveries", summary.discoveries],
    ["Meals", summary.consumptions],
    ["Birth events", summary.births],
    ["Death events", summary.deaths],
  ];
  const visibleStats = stats
    .filter(([, count]) => count > 0)
    .map(
      ([label, count]) =>
        `<div class="interaction-history-summary-stat"><span>${label}</span><strong>${count}</strong></div>`,
    )
    .join("");
  const tickRange =
    summary.first_event_tick === summary.latest_event_tick
      ? `Tick ${summary.first_event_tick}`
      : `Ticks ${summary.first_event_tick}–${summary.latest_event_tick}`;

  return `<div class="interaction-history-summary-header"><strong>Entity #${summary.entity_id}</strong><span>${summary.total_events} recent events</span></div>
    <div class="interaction-history-summary-range">${tickRange} · bounded simulation history</div>
    <div class="interaction-history-summary-grid">${visibleStats}</div>`;
}

export function bindInteractionHistory(): void {
  const input = document.getElementById("interaction-entity-id") as HTMLInputElement;
  const allButton = document.getElementById("btn-history-all")!;
  const exportButton = document.getElementById("btn-history-export")!;

  input.addEventListener("input", () => {
    selectedEntityId = parseEntityFilter(input.value);
    visibleEventLimit = DISPLAY_BATCH_SIZE;
    syncInteractionHistory();
  });
  allButton.addEventListener("click", () => {
    selectedEntityId = null;
    visibleEventLimit = DISPLAY_BATCH_SIZE;
    input.value = "";
    syncInteractionHistory();
  });
  exportButton.addEventListener("click", () => {
    if (!state.world) return;
    const events = JSON.parse(
      state.world.recent_events(selectedEntityId ?? undefined),
    ) as SimulationEvent[];
    downloadEventHistory(
      createEventHistoryExport(
        events,
        state.world.simulation_tick(),
        selectedEntityId,
      ),
    );
  });
  document.getElementById("interaction-history-list")!.addEventListener("click", (event) => {
    const target = event.target as HTMLElement;
    if (target.closest("[data-history-more]")) {
      visibleEventLimit += DISPLAY_BATCH_SIZE;
      syncInteractionHistory();
      return;
    }
    const causalButton = target.closest<HTMLButtonElement>("[data-event-target-id]");
    if (causalButton) {
      const causalEvent = document.getElementById(
        `event-${causalButton.dataset.eventTargetId}`,
      );
      causalEvent?.scrollIntoView({ behavior: "smooth", block: "nearest" });
      return;
    }
    const button = target.closest<HTMLButtonElement>("[data-entity-id]");
    if (!button) return;
    const id = parseEntityFilter(button.dataset.entityId ?? "");
    if (id === null) return;
    selectedEntityId = id;
    visibleEventLimit = DISPLAY_BATCH_SIZE;
    input.value = id.toString();
    syncInteractionHistory();
  });
}

export function resetInteractionHistory(): void {
  selectedEntityId = null;
  visibleEventLimit = DISPLAY_BATCH_SIZE;
  const input = document.getElementById("interaction-entity-id") as HTMLInputElement | null;
  if (input) input.value = "";
}

export function syncInteractionHistory(): void {
  const list = document.getElementById("interaction-history-list");
  const scope = document.getElementById("interaction-history-scope");
  const summaryElement = document.getElementById("interaction-history-summary");
  const allButton = document.getElementById("btn-history-all");
  if (!list || !scope || !summaryElement || !allButton) return;

  allButton.classList.toggle("active", selectedEntityId === null);
  scope.textContent =
    selectedEntityId === null
      ? "All recent events"
      : `Events involving entity #${selectedEntityId}`;
  summaryElement.hidden = selectedEntityId === null;
  summaryElement.innerHTML = "";

  if (!state.world) {
    list.innerHTML = renderInteractionHistory([], selectedEntityId);
    return;
  }

  if (selectedEntityId !== null) {
    const summary = JSON.parse(
      state.world.entity_event_summary(selectedEntityId),
    ) as EntityEventSummary;
    summaryElement.innerHTML = renderEntityEventSummary(summary);
  }

  const payload = state.world.recent_events(
    selectedEntityId ?? undefined,
  );
  const events = JSON.parse(payload) as SimulationEvent[];
  list.innerHTML = renderInteractionHistory(
    events,
    selectedEntityId,
    visibleEventLimit,
  );
}
