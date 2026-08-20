import { describe, expect, it } from "vitest";

import type {
  AffinityChangeEvent,
  BirthEvent,
  ConsumptionEvent,
  DeathEvent,
  EncounterEvent,
  EntityEventSummary,
  InteractionEvent,
  ResourceDiscoveryEvent,
} from "../types";
import {
  createEventHistoryExport,
  filterInteractionEvents,
  parseEntityFilter,
  renderEntityEventSummary,
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
    caused_by_event_id: null,
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
    resource_kind: null,
    previous_affinity: null,
    new_affinity: null,
    delta: null,
  };
}

function birth(): BirthEvent {
  return {
    id: "6",
    caused_by_event_id: null,
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
    resource_kind: null,
    previous_affinity: null,
    new_affinity: null,
    delta: null,
  };
}

describe("event history export", () => {
  it("exports a stable versioned payload with string ticks", () => {
    const events = [event(1, 2, 3, 4, 5), event(2, 4, 5, -2, 1)];

    expect(createEventHistoryExport(events, 9_007_199_254_740_993n, null)).toEqual({
      schema: "nexus-event-history/v1",
      simulation_tick: "9007199254740993",
      entity_filter: null,
      event_count: 2,
      events,
    });
  });

  it("exports only events involving the selected entity", () => {
    const included = event(1, 2, 3, 4, 5);
    const history = createEventHistoryExport(
      [included, event(2, 4, 5, -2, 1)],
      48n,
      3,
    );

    expect(history.entity_filter).toBe(3);
    expect(history.event_count).toBe(1);
    expect(history.events).toEqual([included]);
  });
});

function death(cause: DeathEvent["cause"]): DeathEvent {
  return {
    id: "7",
    caused_by_event_id: null,
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
    resource_kind: null,
    previous_affinity: null,
    new_affinity: null,
    delta: null,
  };
}

function consumption(): ConsumptionEvent {
  return {
    id: "9",
    caused_by_event_id: null,
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
    resource_kind: null,
    previous_affinity: null,
    new_affinity: null,
    delta: null,
  };
}

function discovery(): ResourceDiscoveryEvent {
  return {
    id: "10",
    caused_by_event_id: null,
    tick: "52",
    relative_time: "just now",
    location: { x: 6, y: 3 },
    actor_id: 8,
    target_id: null,
    related_entity_ids: [8],
    kind: "discovery",
    cause: "resource_found",
    actor_affinity_delta: null,
    target_affinity_delta: null,
    child_id: null,
    amount: 14,
    resource_kind: "timber",
    previous_affinity: null,
    new_affinity: null,
    delta: null,
  };
}

function encounter(): EncounterEvent {
  return {
    id: "11",
    caused_by_event_id: null,
    tick: "53",
    relative_time: "just now",
    location: { x: 3, y: 5 },
    actor_id: 2,
    target_id: 7,
    related_entity_ids: [2, 7],
    kind: "encounter",
    cause: "first_encounter",
    actor_affinity_delta: null,
    target_affinity_delta: null,
    child_id: null,
    amount: null,
    resource_kind: null,
    previous_affinity: null,
    new_affinity: null,
    delta: null,
  };
}

function affinityChange(
  cause: AffinityChangeEvent["cause"] = "mutual_social_contact",
): AffinityChangeEvent {
  return {
    id: "12",
    caused_by_event_id: cause === "mutual_social_contact" ? "1" : null,
    tick: "54",
    relative_time: "just now",
    location: { x: 3, y: 5 },
    actor_id: 2,
    target_id: 7,
    related_entity_ids: [2, 7],
    kind: "affinity_change",
    cause,
    actor_affinity_delta: null,
    target_affinity_delta: null,
    child_id: null,
    amount: null,
    resource_kind: null,
    previous_affinity: 99,
    new_affinity: 103,
    delta: 4,
  };
}

function summary(overrides: Partial<EntityEventSummary> = {}): EntityEventSummary {
  return {
    entity_id: 2,
    total_events: 5,
    first_event_tick: "10",
    latest_event_tick: "54",
    births: 0,
    deaths: 0,
    consumptions: 1,
    discoveries: 0,
    encounters: 1,
    interactions: 2,
    affinity_changes: 1,
    ...overrides,
  };
}

describe("interaction history", () => {
  it("renders a compact entity summary from the domain aggregation", () => {
    const html = renderEntityEventSummary(summary());

    expect(html).toContain("Entity #2");
    expect(html).toContain("5 recent events");
    expect(html).toContain("Ticks 10–54");
    expect(html).toContain("Interactions");
    expect(html).toContain("Meals");
    expect(html).not.toContain("Birth events");
  });

  it("renders explicit empty and single-tick summary states", () => {
    expect(
      renderEntityEventSummary(
        summary({
          total_events: 0,
          first_event_tick: null,
          latest_event_tick: null,
          consumptions: 0,
          encounters: 0,
          interactions: 0,
          affinity_changes: 0,
        }),
      ),
    ).toContain("No recent history summary for entity #2");

    expect(
      renderEntityEventSummary(
        summary({ first_event_tick: "54", latest_event_tick: "54" }),
      ),
    ).toContain("Tick 54");
  });

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

  it("renders and filters resource discoveries", () => {
    const html = renderInteractionHistory([discovery()]);
    expect(html).toContain("Entity #8");
    expect(html).toContain("Discovered timber");
    expect(html).toContain("Observed 14 at (6, 3)");
    expect(filterInteractionEvents([discovery()], 8)).toHaveLength(1);
  });

  it("renders and filters first encounters", () => {
    const html = renderInteractionHistory([encounter()]);
    expect(html).toContain("Entity #2");
    expect(html).toContain("first encountered");
    expect(html).toContain("Entity #7");
    expect(html).toContain("at (3, 5)");
    expect(filterInteractionEvents([encounter()], 7)).toHaveLength(1);
  });

  it("renders and filters directed affinity changes", () => {
    const social = affinityChange();
    const decay = { ...affinityChange("relationship_decay"), id: "13" };
    const html = renderInteractionHistory([social, decay]);

    expect(html).toContain("Entity #2");
    expect(html).toContain("Entity #7");
    expect(html).toContain("Affinity: 99 → 103 (+4)");
    expect(html).toContain("Social interaction");
    expect(html).toContain("Relationship decay");
    expect(html).toContain('data-event-target-id="1"');
    expect(html).toContain("Caused by");
    expect(html).toContain("Event #1");
    expect(filterInteractionEvents([social], 2)).toHaveLength(1);
    expect(filterInteractionEvents([social], 7)).toHaveLength(1);
    expect(filterInteractionEvents([social], 8)).toHaveLength(0);
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
