import { describe, expect, it } from "vitest";

import type {
  BirthEvent,
  ConsumptionEvent,
  DeathEvent,
  InteractionEvent,
} from "../types";
import {
  filterInteractionEvents,
  parseEntityFilter,
  renderInteractionHistory,
} from "./interaction-history";

function event(
  id: number,
  actorId: number,
  targetId: number,
  actorDelta: number,
  targetDelta: number,
): InteractionEvent {
  return {
    id: id.toString(),
    tick: "48",
    relative_time: "just now",
    location: { x: 4, y: 7 },
    actor_id: actorId,
    target_id: targetId,
    related_entity_ids: [actorId, targetId],
    kind: "interaction",
    cause: "mutual_social_contact",
    actor_affinity_delta: actorDelta,
    target_affinity_delta: targetDelta,
    child_id: null,
    amount: null,
  };
}

function birth(): BirthEvent {
  return {
    id: "6",
    tick: "49",
    relative_time: "just now",
    location: { x: 5, y: 7 },
    actor_id: 1,
    target_id: null,
    related_entity_ids: [1, 7],
    kind: "birth",
    cause: "born",
    actor_affinity_delta: null,
    target_affinity_delta: null,
    child_id: 7,
    amount: null,
  };
}

function death(cause: DeathEvent["cause"]): DeathEvent {
  return {
    id: "7",
    tick: "50",
    relative_time: "just now",
    location: { x: 8, y: 9 },
    actor_id: 4,
    target_id: null,
    related_entity_ids: [4],
    kind: "death",
    cause,
    actor_affinity_delta: null,
    target_affinity_delta: null,
    child_id: null,
    amount: null,
  };
}

function consumption(): ConsumptionEvent {
  return {
    id: "9",
    tick: "51",
    relative_time: "just now",
    location: { x: 2, y: 4 },
    actor_id: 3,
    target_id: null,
    related_entity_ids: [3],
    kind: "consumption",
    cause: "ate_food",
    actor_affinity_delta: null,
    target_affinity_delta: null,
    child_id: null,
    amount: 6,
  };
}

describe("interaction history", () => {
  it("renders a clear empty state for all events and selected entities", () => {
    expect(renderInteractionHistory([])).toContain("No recent events.");
    expect(renderInteractionHistory([], 42)).toContain(
      "No recent events for entity #42.",
    );
  });

  it("renders births and both death causes without optional participants", () => {
    const html = renderInteractionHistory([
      birth(),
      death("starvation"),
      { ...death("natural_death"), id: "8", actor_id: 5, related_entity_ids: [5] },
    ]);

    expect(html).toContain("Mother #1");
    expect(html).toContain("Newborn #7");
    expect(html).toContain("Starvation");
    expect(html).toContain("Natural death");
    expect(html).not.toContain("Target #null");
  });

  it("filters lifecycle events through actors and related newborns", () => {
    const events = [birth(), death("starvation")];
    expect(filterInteractionEvents(events, 7).map(({ id }) => id)).toEqual(["6"]);
    expect(filterInteractionEvents(events, 4).map(({ id }) => id)).toEqual(["7"]);
  });

  it("renders and filters food consumption events", () => {
    const html = renderInteractionHistory([consumption()]);
    expect(html).toContain("Entity #3");
    expect(html).toContain("Ate 6 food");
    expect(html).toContain("Consumed at (2, 4)");
    expect(filterInteractionEvents([consumption()], 3)).toHaveLength(1);
    expect(filterInteractionEvents([consumption()], 4)).toHaveLength(0);
  });

  it("renders positive, neutral, and negative deltas with text and symbols", () => {
    const html = renderInteractionHistory([
      event(3, 1, 2, 4, 0),
      event(2, 3, 4, -2, 1),
    ]);

    expect(html).toContain("+4 positive");
    expect(html).toContain("±0 neutral");
    expect(html).toContain("−2 negative");
    expect(html).toContain("Mutual social contact");
    expect(html).toContain('data-entity-id="1"');
  });

  it("filters by actor, target, or related entity without reordering", () => {
    const relatedOnly = event(4, 5, 6, 1, 1);
    relatedOnly.related_entity_ids.push(99);
    const events = [event(5, 1, 2, 1, 1), relatedOnly];

    expect(filterInteractionEvents(events, 2).map(({ id }) => id)).toEqual(["5"]);
    expect(filterInteractionEvents(events, 5).map(({ id }) => id)).toEqual(["4"]);
    expect(filterInteractionEvents(events, 99).map(({ id }) => id)).toEqual(["4"]);
    expect(filterInteractionEvents(events, null)).toBe(events);
  });

  it("offers incremental access to every event beyond the first batch", () => {
    const events = Array.from({ length: 101 }, (_, index) =>
      event(101 - index, 1, 2, 1, 1),
    );

    const firstBatch = renderInteractionHistory(events);
    expect(firstBatch).toContain("Show 1 more · 1 remaining");
    expect(firstBatch).not.toContain('data-event-id="1"');

    const expanded = renderInteractionHistory(events, null, 200);
    expect(expanded).toContain('data-event-id="1"');
    expect(expanded).not.toContain("data-history-more");
  });

  it("accepts only positive integer entity filters", () => {
    expect(parseEntityFilter("")).toBeNull();
    expect(parseEntityFilter("0")).toBeNull();
    expect(parseEntityFilter("1.5")).toBeNull();
    expect(parseEntityFilter("12")).toBe(12);
  });
});
