use super::autonomy::{Goal, GriefState, GRIEF_MAX_DURATION_TICKS, GRIEF_MIN_DURATION_TICKS};
use super::{relationship_between, DeathContext, Entity, Genealogy, KinshipRelation};

pub(in crate::simulation) const GRIEF_MIN_INTENSITY: u8 = 20;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum GriefRole {
    Partner,
    CaregiverDependent,
    ParentChild,
    Sibling,
    BondedOther,
}

impl GriefRole {
    fn base_intensity(self) -> i16 {
        match self {
            Self::Partner => 65,
            Self::CaregiverDependent | Self::ParentChild => 60,
            Self::Sibling => 45,
            Self::BondedOther => 30,
        }
    }
}

pub(super) fn process_witnessed_deaths(
    survivors: &mut [Entity],
    genealogy: &Genealogy,
    deaths: &[DeathContext],
    tick: u64,
) {
    if deaths.is_empty() {
        return;
    }
    for survivor in survivors {
        let witnessed: Vec<_> = survivor
            .mind
            .visible_entities
            .iter()
            .filter_map(|entity_id| {
                deaths
                    .iter()
                    .find(|death| death.entity_id == *entity_id)
                    .copied()
            })
            .collect();
        for death in witnessed {
            let newly_known = survivor.mind.memory.mark_entity_dead(death.entity_id);
            if !newly_known {
                continue;
            }
            let affinity = survivor
                .mind
                .memory
                .known_entities
                .binary_search_by_key(&death.entity_id, |known| known.id)
                .ok()
                .map_or(0, |index| {
                    survivor.mind.memory.known_entities[index].affinity
                });
            let Some(role) = grief_role(survivor, death, genealogy, affinity) else {
                continue;
            };
            let intensity = grief_intensity(role, affinity);
            if intensity < GRIEF_MIN_INTENSITY {
                continue;
            }
            let duration = grief_duration(intensity);
            let started = survivor.mind.add_grief(GriefState {
                deceased_id: death.entity_id,
                started_tick: tick,
                ends_tick: tick.saturating_add(duration),
                intensity,
            });
            if started
                && matches!(
                    survivor.mind.current_goal,
                    Some(Goal::Explore | Goal::Socialize | Goal::Rest)
                )
            {
                survivor.mind.clear_goal();
                survivor.path.clear();
                survivor.path_index = 0;
                survivor.action_tick = 0;
            }
        }
    }
}

fn grief_role(
    survivor: &Entity,
    death: DeathContext,
    genealogy: &Genealogy,
    affinity: i16,
) -> Option<GriefRole> {
    if death.partner_id == Some(survivor.id) {
        return Some(GriefRole::Partner);
    }
    if survivor.caregiver_id == Some(death.entity_id) || death.caregiver_id == Some(survivor.id) {
        return Some(GriefRole::CaregiverDependent);
    }
    match relationship_between(genealogy, survivor.id, death.entity_id) {
        KinshipRelation::Parent | KinshipRelation::Child => Some(GriefRole::ParentChild),
        KinshipRelation::FullSibling | KinshipRelation::HalfSibling => Some(GriefRole::Sibling),
        _ if affinity >= 100 => Some(GriefRole::BondedOther),
        _ => None,
    }
}

fn grief_intensity(role: GriefRole, affinity: i16) -> u8 {
    (role.base_intensity() + affinity.clamp(-1_000, 1_000) / 25).clamp(0, 100) as u8
}

fn grief_duration(intensity: u8) -> u64 {
    GRIEF_MIN_DURATION_TICKS
        + (GRIEF_MAX_DURATION_TICKS - GRIEF_MIN_DURATION_TICKS) * u64::from(intensity) / 100
}
