use crate::world::ResourceKind;
use std::collections::VecDeque;
use std::fmt;

pub const RECENT_EVENT_CAPACITY: usize = 1_024;

/// Monotonic identifier assigned when an event enters simulation history.
///
/// Keeping this distinct from raw ticks prevents accidental comparisons or
/// assignments between two unrelated `u64` values.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct EventId(u64);

impl EventId {
    pub(super) const FIRST: Self = Self::new(1);

    pub(crate) const fn new(value: u64) -> Self {
        assert!(value != 0, "event IDs start at one");
        Self(value)
    }

    pub(super) fn checked_next(self) -> Option<Self> {
        self.0.checked_add(1).map(Self)
    }
}

impl fmt::Display for EventId {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.0.fmt(formatter)
    }
}

#[cfg(test)]
mod event_id_tests {
    use super::EventId;

    #[test]
    fn event_ids_are_nonzero_and_checked() {
        assert_eq!(EventId::new(7).to_string(), "7");
        assert_eq!(EventId::new(7).checked_next(), Some(EventId::new(8)));
        assert_eq!(EventId::new(u64::MAX).checked_next(), None);
    }

    #[test]
    #[should_panic(expected = "event IDs start at one")]
    fn zero_is_not_a_valid_assigned_event_id() {
        EventId::new(0);
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct EventLocation {
    pub x: u32,
    pub y: u32,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SimulationEventKind {
    Interaction,
    Birth,
    Death,
    Consumption,
    Discovery,
    Encounter,
    AffinityChange,
    FoodShared,
    FoodShareRefused,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SimulationEventCause {
    MutualSocialContact,
    Born,
    Starvation,
    NaturalDeath,
    AteFood,
    ResourceFound,
    FirstEncounter,
    RelationshipDecay,
    FoodShared,
    FoodShareRefused,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SimulationEventDetails {
    Interaction {
        actor_affinity_delta: i16,
        target_affinity_delta: i16,
    },
    Birth {
        child_id: u32,
    },
    Death,
    Consumption {
        amount: u16,
    },
    ResourceDiscovery {
        kind: ResourceKind,
        amount: u16,
    },
    Encounter,
    AffinityChange {
        previous_affinity: i16,
        new_affinity: i16,
        delta: i16,
    },
    FoodShared {
        amount: u16,
    },
    FoodShareRefused,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SimulationEvent {
    pub id: EventId,
    pub caused_by_event_id: Option<EventId>,
    pub tick: u64,
    pub location: EventLocation,
    pub actor_id: u32,
    pub target_id: Option<u32>,
    pub related_entity_ids: Vec<u32>,
    pub kind: SimulationEventKind,
    pub cause: SimulationEventCause,
    pub details: SimulationEventDetails,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub(crate) struct EntityEventSummary {
    pub entity_id: u32,
    pub total_events: u32,
    pub first_event_tick: Option<u64>,
    pub latest_event_tick: Option<u64>,
    pub births: u32,
    pub deaths: u32,
    pub consumptions: u32,
    pub discoveries: u32,
    pub encounters: u32,
    pub interactions: u32,
    pub affinity_changes: u32,
}

pub(super) struct PendingSimulationEvent {
    pub caused_by_event_id: Option<EventId>,
    pub tick: u64,
    pub location: EventLocation,
    pub actor_id: u32,
    pub target_id: Option<u32>,
    pub related_entity_ids: Vec<u32>,
    pub kind: SimulationEventKind,
    pub cause: SimulationEventCause,
    pub details: SimulationEventDetails,
}

impl PendingSimulationEvent {
    pub(super) fn assign(self, id: EventId) -> SimulationEvent {
        SimulationEvent {
            id,
            caused_by_event_id: self.caused_by_event_id,
            tick: self.tick,
            location: self.location,
            actor_id: self.actor_id,
            target_id: self.target_id,
            related_entity_ids: self.related_entity_ids,
            kind: self.kind,
            cause: self.cause,
            details: self.details,
        }
    }
}

#[derive(Clone, Debug)]
pub(super) struct RecentEventHistory {
    events: VecDeque<SimulationEvent>,
    capacity: usize,
    next_id: EventId,
}

impl Default for RecentEventHistory {
    fn default() -> Self {
        Self {
            events: VecDeque::with_capacity(RECENT_EVENT_CAPACITY),
            capacity: RECENT_EVENT_CAPACITY,
            next_id: EventId::FIRST,
        }
    }
}

impl RecentEventHistory {
    #[cfg(test)]
    pub(super) fn with_capacity(capacity: usize) -> Self {
        Self {
            events: VecDeque::with_capacity(capacity),
            capacity,
            next_id: EventId::FIRST,
        }
    }

    pub(super) fn push(&mut self, event: PendingSimulationEvent) -> EventId {
        let assigned_id = self.next_id;
        self.next_id = self
            .next_id
            .checked_next()
            .expect("simulation event id space exhausted");

        if self.capacity == 0 {
            return assigned_id;
        }
        if self.events.len() == self.capacity {
            self.events.pop_front();
        }
        self.events.push_back(event.assign(assigned_id));
        assigned_id
    }

    pub(super) fn iter(&self) -> impl DoubleEndedIterator<Item = &SimulationEvent> {
        self.events.iter()
    }

    pub(super) fn summary_for(&self, entity_id: u32) -> EntityEventSummary {
        let mut summary = EntityEventSummary {
            entity_id,
            ..EntityEventSummary::default()
        };

        for event in self.events.iter().filter(|event| {
            event.actor_id == entity_id
                || event.target_id == Some(entity_id)
                || event.related_entity_ids.contains(&entity_id)
        }) {
            summary.total_events = summary.total_events.saturating_add(1);
            summary.first_event_tick.get_or_insert(event.tick);
            summary.latest_event_tick = Some(event.tick);
            let counter = match event.kind {
                SimulationEventKind::Birth => &mut summary.births,
                SimulationEventKind::Death => &mut summary.deaths,
                SimulationEventKind::Consumption => &mut summary.consumptions,
                SimulationEventKind::Discovery => &mut summary.discoveries,
                SimulationEventKind::Encounter => &mut summary.encounters,
                SimulationEventKind::Interaction => &mut summary.interactions,
                SimulationEventKind::AffinityChange => &mut summary.affinity_changes,
                SimulationEventKind::FoodShared | SimulationEventKind::FoodShareRefused => {
                    &mut summary.interactions
                }
            };
            *counter = counter.saturating_add(1);
        }

        summary
    }

    #[cfg(test)]
    pub(super) fn next_id(&self) -> EventId {
        self.next_id
    }
}

#[cfg(test)]
mod summary_tests {
    use super::*;

    fn event(
        tick: u64,
        actor_id: u32,
        target_id: Option<u32>,
        related_entity_ids: Vec<u32>,
        kind: SimulationEventKind,
    ) -> PendingSimulationEvent {
        PendingSimulationEvent {
            caused_by_event_id: None,
            tick,
            location: EventLocation { x: 1, y: 2 },
            actor_id,
            target_id,
            related_entity_ids,
            kind,
            cause: SimulationEventCause::FirstEncounter,
            details: SimulationEventDetails::Encounter,
        }
    }

    #[test]
    fn entity_summary_counts_only_related_events_and_preserves_tick_range() {
        let mut history = RecentEventHistory::default();
        history.push(event(
            5,
            1,
            Some(2),
            vec![1, 2],
            SimulationEventKind::Interaction,
        ));
        history.push(event(
            8,
            3,
            None,
            vec![3, 1],
            SimulationEventKind::Discovery,
        ));
        history.push(event(
            10,
            2,
            Some(3),
            vec![2, 3],
            SimulationEventKind::Death,
        ));

        let summary = history.summary_for(1);
        assert_eq!(summary.entity_id, 1);
        assert_eq!(summary.total_events, 2);
        assert_eq!(summary.first_event_tick, Some(5));
        assert_eq!(summary.latest_event_tick, Some(8));
        assert_eq!(summary.interactions, 1);
        assert_eq!(summary.discoveries, 1);
        assert_eq!(summary.deaths, 0);
    }

    #[test]
    fn entity_summary_has_explicit_empty_state() {
        let summary = RecentEventHistory::default().summary_for(99);
        assert_eq!(
            summary,
            EntityEventSummary {
                entity_id: 99,
                ..EntityEventSummary::default()
            }
        );
    }
}
