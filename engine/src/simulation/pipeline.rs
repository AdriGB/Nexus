//! Tick orchestration for the simulation.
//!
//! This module owns execution order, while [`Simulation`] owns state and the
//! domain modules own their rules. Keeping orchestration here makes additions
//! to the tick lifecycle visible without turning `Simulation` into a scheduler.

use super::{dependents, grief, households, physiology, AutonomyProfile, PhaseProfile, Simulation};
use crate::world::Grid;
use web_time::Instant;

#[derive(Clone, Copy)]
enum TickPhase {
    Physiology,
    PopulationIndex,
    Autonomy,
    Starvation,
    ResourceChanges,
    RemoveDead,
    Pregnancies,
    Conceptions,
}

trait TickInstrumentation {
    const ENABLED: bool;
    fn record(&mut self, phase: TickPhase, elapsed_us: u64);
}

struct NoopInstrumentation;

impl TickInstrumentation for NoopInstrumentation {
    const ENABLED: bool = false;
    fn record(&mut self, _phase: TickPhase, _elapsed_us: u64) {}
}

struct ExecutionInstrumentation<'a, Phases> {
    phases: Phases,
    autonomy: Option<&'a mut AutonomyProfile>,
}

struct PhaseInstrumentation {
    profile: PhaseProfile,
    total_start: Instant,
}

impl PhaseInstrumentation {
    fn new() -> Self {
        Self {
            profile: PhaseProfile::default(),
            total_start: Instant::now(),
        }
    }

    fn finish(mut self) -> PhaseProfile {
        self.profile.total_us = self.total_start.elapsed().as_micros() as u64;
        self.profile
    }
}

impl TickInstrumentation for PhaseInstrumentation {
    const ENABLED: bool = true;

    fn record(&mut self, phase: TickPhase, elapsed_us: u64) {
        match phase {
            TickPhase::Physiology => self.profile.physiology_us = elapsed_us,
            TickPhase::PopulationIndex => self.profile.population_index_us = elapsed_us,
            TickPhase::Autonomy => self.profile.autonomy_us = elapsed_us,
            TickPhase::Starvation => self.profile.starvation_us = elapsed_us,
            TickPhase::ResourceChanges => self.profile.resource_changes_us = elapsed_us,
            TickPhase::RemoveDead => self.profile.remove_dead_us = elapsed_us,
            TickPhase::Pregnancies => self.profile.pregnancies_us = elapsed_us,
            TickPhase::Conceptions => self.profile.conceptions_us = elapsed_us,
        }
    }
}

fn instrument<I, Output>(
    instrumentation: &mut I,
    phase: TickPhase,
    operation: impl FnOnce() -> Output,
) -> Output
where
    I: TickInstrumentation,
{
    if I::ENABLED {
        let start = Instant::now();
        let output = operation();
        instrumentation.record(phase, start.elapsed().as_micros() as u64);
        output
    } else {
        operation()
    }
}

pub(super) fn run_step(simulation: &mut Simulation, world: &mut Grid) {
    execute_step(
        simulation,
        world,
        &mut ExecutionInstrumentation {
            phases: NoopInstrumentation,
            autonomy: None,
        },
    );
}

pub(super) fn run_profiled_step(simulation: &mut Simulation, world: &mut Grid) -> PhaseProfile {
    let mut instrumentation = ExecutionInstrumentation {
        phases: PhaseInstrumentation::new(),
        autonomy: None,
    };
    execute_step(simulation, world, &mut instrumentation);
    instrumentation.phases.finish()
}

fn execute_step<I>(
    simulation: &mut Simulation,
    world: &mut Grid,
    instrumentation: &mut ExecutionInstrumentation<'_, I>,
) where
    I: TickInstrumentation,
{
    simulation.tick = simulation.tick.saturating_add(1);
    simulation.regenerate_renewable_resources(world);

    instrument(&mut instrumentation.phases, TickPhase::Physiology, || {
        physiology::advance(&mut simulation.entities);
    });

    dependents::clear_graduated_caregivers(&mut simulation.entities);
    dependents::snap_infants_to_caregivers(&mut simulation.entities);
    households::plan_daily_household_migrations(
        &simulation.entities,
        &mut simulation.households,
        world,
        simulation.tick,
    );

    instrument(
        &mut instrumentation.phases,
        TickPhase::PopulationIndex,
        || {
            simulation.rebuild_population_index(world);
        },
    );

    let autonomy_profile = instrumentation.autonomy.as_deref_mut();
    let (consumed_this_tick, world_changed, consumer_ids) =
        instrument(&mut instrumentation.phases, TickPhase::Autonomy, || {
            simulation.execute_autonomy(world, autonomy_profile)
        });

    dependents::snap_infants_to_caregivers(&mut simulation.entities);
    for (id, amount) in consumer_ids {
        dependents::feed_infants_of(&mut simulation.entities, id, amount);
    }

    instrument(&mut instrumentation.phases, TickPhase::Starvation, || {
        physiology::resolve_starvation(&mut simulation.entities);
    });

    instrument(
        &mut instrumentation.phases,
        TickPhase::ResourceChanges,
        || {
            simulation.record_resource_changes(consumed_this_tick, world_changed);
        },
    );

    let deaths = instrument(&mut instrumentation.phases, TickPhase::RemoveDead, || {
        simulation.remove_dead_entities()
    });

    instrument(&mut instrumentation.phases, TickPhase::Pregnancies, || {
        grief::process_witnessed_deaths(
            &mut simulation.entities,
            &simulation.genealogy,
            &deaths,
            simulation.tick,
        );
        dependents::reassign_orphaned_dependents(&mut simulation.entities, world);
        simulation.update_pregnancies(world);
        households::synchronize_dependent_memberships(
            &mut simulation.entities,
            &simulation.households,
        );
        households::settle_completed_migrations(
            &simulation.entities,
            &mut simulation.households,
            simulation.tick,
        );
        let dissolutions = households::dissolve_empty_households(
            &simulation.entities,
            &mut simulation.households,
            simulation.tick,
        );
        households::settle_basic_inheritances(
            &mut simulation.entities,
            &mut simulation.households,
            &simulation.genealogy,
            &deaths,
            &dissolutions,
            simulation.tick,
        );
        dependents::snap_infants_to_caregivers(&mut simulation.entities);
    });

    simulation.run_daily_relationship_decay();

    instrument(&mut instrumentation.phases, TickPhase::Conceptions, || {
        simulation.try_daily_conceptions();
    });
}

pub(super) fn run_profiled_autonomy_step(
    simulation: &mut Simulation,
    world: &mut Grid,
) -> AutonomyProfile {
    let mut profile = AutonomyProfile::default();
    execute_step(
        simulation,
        world,
        &mut ExecutionInstrumentation {
            phases: NoopInstrumentation,
            autonomy: Some(&mut profile),
        },
    );

    profile
}
