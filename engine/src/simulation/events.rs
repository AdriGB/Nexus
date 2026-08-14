use std::collections::VecDeque;

pub const RECENT_EVENT_CAPACITY: usize = 1_024;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct EventLocation {
    pub x: u32,
    pub y: u32,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SimulationEventKind {
    Interaction,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SimulationEventCause {
    MutualSocialContact,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SimulationEventDetails {
    Interaction {
        actor_affinity_delta: i16,
        target_affinity_delta: i16,
    },
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SimulationEvent {
    pub id: u64,
    pub tick: u64,
    pub location: EventLocation,
    pub actor_id: u32,
    pub target_id: Option<u32>,
    pub related_entity_ids: Vec<u32>,
    pub kind: SimulationEventKind,
    pub cause: SimulationEventCause,
    pub details: SimulationEventDetails,
}

#[derive(Clone, Debug)]
pub(super) struct RecentEventHistory {
    events: VecDeque<SimulationEvent>,
    capacity: usize,
}

impl Default for RecentEventHistory {
    fn default() -> Self {
        Self {
            events: VecDeque::with_capacity(RECENT_EVENT_CAPACITY),
            capacity: RECENT_EVENT_CAPACITY,
        }
    }
}

impl RecentEventHistory {
    #[cfg(test)]
    pub(super) fn with_capacity(capacity: usize) -> Self {
        Self {
            events: VecDeque::with_capacity(capacity),
            capacity,
        }
    }

    pub(super) fn push(&mut self, event: SimulationEvent) {
        if self.capacity == 0 {
            return;
        }
        if self.events.len() == self.capacity {
            self.events.pop_front();
        }
        self.events.push_back(event);
    }

    #[cfg(test)]
    pub(super) fn iter(&self) -> impl DoubleEndedIterator<Item = &SimulationEvent> {
        self.events.iter()
    }
}
