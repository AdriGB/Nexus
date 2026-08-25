//! Deterministic aggregation for canonical per-tick performance profiles.
//!
//! Percentiles use nearest-rank: `ceil(percentile * sample_count) - 1` in
//! zero-based indexing. For an even sample count, the integer median is the
//! floor of the arithmetic mean of the two middle samples.

use super::{PhaseProfile, StateGauges, WorkCounters};
use serde::Serialize;

#[derive(Clone, Debug, Default, PartialEq, Serialize)]
pub(crate) struct TimingStats {
    pub mean_us: f64,
    pub median_us: u64,
    pub p95_us: u64,
    pub p99_us: u64,
    pub max_us: u64,
}

impl TimingStats {
    fn from_samples(mut samples: Vec<u64>) -> Self {
        if samples.is_empty() {
            return Self::default();
        }
        samples.sort_unstable();
        let len = samples.len();
        let total: u128 = samples.iter().map(|&value| u128::from(value)).sum();
        let median_us = if len % 2 == 1 {
            samples[len / 2]
        } else {
            let lower = samples[len / 2 - 1];
            let upper = samples[len / 2];
            lower / 2 + upper / 2 + (lower % 2 + upper % 2) / 2
        };
        Self {
            mean_us: total as f64 / len as f64,
            median_us,
            p95_us: nearest_rank(&samples, 95),
            p99_us: nearest_rank(&samples, 99),
            max_us: samples[len - 1],
        }
    }
}

fn nearest_rank(sorted: &[u64], percentile: usize) -> u64 {
    let rank = percentile.saturating_mul(sorted.len()).div_ceil(100);
    sorted[rank.saturating_sub(1).min(sorted.len() - 1)]
}

#[derive(Clone, Debug, Default, Serialize)]
pub(crate) struct PerformanceSummary {
    pub samples: u64,
    pub total: TimingStats,
    pub world_maintenance: TimingStats,
    pub physiology: TimingStats,
    pub dependent_care: TimingStats,
    pub households: TimingStats,
    pub spatial_index: TimingStats,
    pub autonomy: TimingStats,
    pub survival: TimingStats,
    pub mortality: TimingStats,
    pub lifecycle: TimingStats,
    pub relationships: TimingStats,
    pub reproduction: TimingStats,
    pub work_total: WorkCounters,
    pub state_final: StateGauges,
    pub state_peak: StateGauges,
}

#[derive(Default)]
pub(crate) struct PerformanceRun {
    profiles: Vec<PhaseProfile>,
}

impl PerformanceRun {
    pub(crate) fn record(&mut self, profile: PhaseProfile) {
        self.profiles.push(profile);
    }

    pub(crate) fn summarize(self) -> PerformanceSummary {
        let Some(final_profile) = self.profiles.last() else {
            return PerformanceSummary::default();
        };
        let mut work_total = WorkCounters::default();
        let mut state_peak = StateGauges::default();
        for profile in &self.profiles {
            work_total.accumulate(&profile.work);
            state_peak.retain_maximums(&profile.state);
        }
        PerformanceSummary {
            samples: self.profiles.len() as u64,
            total: stats(&self.profiles, |profile| profile.total_us),
            world_maintenance: stats(&self.profiles, |profile| profile.world_maintenance_us),
            physiology: stats(&self.profiles, |profile| profile.physiology_us),
            dependent_care: stats(&self.profiles, |profile| profile.dependent_care_us),
            households: stats(&self.profiles, |profile| profile.households_us),
            spatial_index: stats(&self.profiles, |profile| profile.spatial_index_us),
            autonomy: stats(&self.profiles, |profile| profile.autonomy_us),
            survival: stats(&self.profiles, |profile| profile.survival_us),
            mortality: stats(&self.profiles, |profile| profile.mortality_us),
            lifecycle: stats(&self.profiles, |profile| profile.lifecycle_us),
            relationships: stats(&self.profiles, |profile| profile.relationships_us),
            reproduction: stats(&self.profiles, |profile| profile.reproduction_us),
            work_total,
            state_final: final_profile.state.clone(),
            state_peak,
        }
    }
}

fn stats(profiles: &[PhaseProfile], select: impl Fn(&PhaseProfile) -> u64) -> TimingStats {
    TimingStats::from_samples(profiles.iter().map(select).collect())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn profile(total_us: u64, actions: u64, entities: u64) -> PhaseProfile {
        let mut profile = PhaseProfile {
            total_us,
            world_maintenance_us: total_us,
            physiology_us: total_us,
            dependent_care_us: total_us,
            households_us: total_us,
            spatial_index_us: total_us,
            autonomy_us: total_us,
            survival_us: total_us,
            mortality_us: total_us,
            lifecycle_us: total_us,
            relationships_us: total_us,
            reproduction_us: total_us,
            ..PhaseProfile::default()
        };
        profile.work.actions_executed = actions;
        profile.state.entities_alive = entities;
        profile.state.known_entities_total = entities * 2;
        profile
    }

    #[test]
    fn empty_run_has_explicit_zero_summary() {
        let summary = PerformanceRun::default().summarize();
        assert_eq!(summary.samples, 0);
        assert_eq!(summary.total, TimingStats::default());
        assert_eq!(summary.work_total.actions_executed, 0);
        assert_eq!(summary.state_final.entities_alive, 0);
    }

    #[test]
    fn single_sample_populates_every_stat_and_both_gauge_snapshots() {
        let mut run = PerformanceRun::default();
        run.record(profile(7, 3, 2));
        let summary = run.summarize();
        assert_eq!(summary.samples, 1);
        assert_eq!(
            summary.total,
            TimingStats {
                mean_us: 7.0,
                median_us: 7,
                p95_us: 7,
                p99_us: 7,
                max_us: 7,
            }
        );
        assert_eq!(summary.work_total.actions_executed, 3);
        assert_eq!(summary.state_final.entities_alive, 2);
        assert_eq!(summary.state_peak.entities_alive, 2);
    }

    #[test]
    fn aggregation_uses_integer_median_and_nearest_rank_percentiles() {
        let mut run = PerformanceRun::default();
        for value in 1..=100 {
            run.record(profile(value, value, 101 - value));
        }
        let summary = run.summarize();
        assert_eq!(summary.total.mean_us, 50.5);
        assert_eq!(summary.total.median_us, 50);
        assert_eq!(summary.total.p95_us, 95);
        assert_eq!(summary.total.p99_us, 99);
        assert_eq!(summary.total.max_us, 100);
        assert_eq!(summary.world_maintenance.p95_us, 95);
        assert_eq!(summary.physiology.p99_us, 99);
        assert_eq!(summary.dependent_care.max_us, 100);
        assert_eq!(summary.households.median_us, 50);
        assert_eq!(summary.spatial_index.mean_us, 50.5);
        assert_eq!(summary.autonomy.p95_us, 95);
        assert_eq!(summary.survival.p99_us, 99);
        assert_eq!(summary.mortality.max_us, 100);
        assert_eq!(summary.lifecycle.median_us, 50);
        assert_eq!(summary.relationships.mean_us, 50.5);
        assert_eq!(summary.reproduction.p99_us, 99);
        assert_eq!(summary.work_total.actions_executed, 5_050);
        assert_eq!(summary.state_final.entities_alive, 1);
        assert_eq!(summary.state_peak.entities_alive, 100);
        assert_eq!(summary.state_peak.known_entities_total, 200);
    }
}
