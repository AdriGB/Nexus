import { state } from "../state";
import type { SimulationEvent } from "../types";

const DISPLAY_BATCH_SIZE = 100;

let selectedEntityId: number | null = null;
let visibleEventLimit = DISPLAY_BATCH_SIZE;

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
  if (event.kind === "interaction") {
    return `<div class="interaction-event-row">${entityButton(event.actor_id, "Actor")}${affinityDelta(event.actor_affinity_delta)}</div>
      <div class="interaction-event-row">${entityButton(event.target_id!, "Target")}${affinityDelta(event.target_affinity_delta)}</div>
      <div class="interaction-event-row"><span>Mutual social contact</span><span>(${event.location.x}, ${event.location.y})</span></div>`;
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
    (event) => `<article class="interaction-event" data-event-id="${event.id}">
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

export function bindInteractionHistory(): void {
  const input = document.getElementById("interaction-entity-id") as HTMLInputElement;
  const allButton = document.getElementById("btn-history-all")!;

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
  document.getElementById("interaction-history-list")!.addEventListener("click", (event) => {
    const target = event.target as HTMLElement;
    if (target.closest("[data-history-more]")) {
      visibleEventLimit += DISPLAY_BATCH_SIZE;
      syncInteractionHistory();
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
  const allButton = document.getElementById("btn-history-all");
  if (!list || !scope || !allButton) return;

  allButton.classList.toggle("active", selectedEntityId === null);
  scope.textContent =
    selectedEntityId === null
      ? "All recent events"
      : `Events involving entity #${selectedEntityId}`;

  if (!state.world) {
    list.innerHTML = renderInteractionHistory([], selectedEntityId);
    return;
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
