//! Native deterministic benchmark scenarios.
//!
//! This facade deliberately keeps simulation and world internals private. The
//! native runner can list registered scenarios and request one versioned JSON
//! result; construction and execution remain inside the engine crate.

use crate::generation::generate_world;
use crate::pathfinding::{find_path_with_workspace, PathfindingWorkspace};
use crate::regions::detect_regions;
use crate::resources::generate_resources;
use crate::simulation::{
    children_of, descendants_of, relationship_between, AutonomyProfile, EntityPassBreakdown,
    Genealogy, KinshipRelation, PerformanceRun, PerformanceSummary, Simulation, MAX_POPULATION,
};
use crate::world::{Grid, RenewableResource, ResourceDeposit, ResourceKind, Terrain, Tile};
use serde::Serialize;

pub const BENCHMARK_SCHEMA_VERSION: u32 = 3;

type Coordinate = (u32, u32);
type PreparedArena = (Vec<Coordinate>, Coordinate);

#[derive(Clone, Copy, Debug, PartialEq, Serialize)]
pub struct BenchmarkWorld {
    pub width: u32,
    pub height: u32,
    pub sea_level: f64,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
#[serde(tag = "kind", rename_all = "kebab-case")]
pub enum BenchmarkWorkload {
    Baseline,
    DenseSocial {
        compact_population: bool,
        enclosed_walkable_arena: bool,
    },
    Scarcity {
        food_retention_stride: usize,
    },
    Households {
        household_pairs: u32,
        food_per_household: u16,
    },
    PathfindingHeavy {
        compact_population: bool,
        initial_hunger: u16,
        known_food_targets: u8,
    },
    LongRun {
        window_count: u32,
        food_grid_spacing: u32,
        food_capacity: u16,
    },
    /// A seeded multi-generation lineage, placed in a compact arena.
    ///
    /// The other scenarios have no genealogy at all: `GESTATION_TICKS` is
    /// 6,720 and they run 124 ticks, so nobody is ever born and every kinship
    /// query returns nothing (see #212). Seeding the tree directly is the only
    /// way to measure kinship, partnership formation and inheritance against a
    /// population that has relatives.
    ///
    /// The compact placement is not decoration. Lineage alone produces no
    /// encounters, and without encounters none of the relationship or
    /// inheritance paths fire — the scenario would be a kinship microbench
    /// with extra steps, which is what the criterion bench already has.
    SeededLineage {
        generations: u32,
    },
}

#[derive(Clone, Copy, Debug, PartialEq, Serialize)]
pub struct BenchmarkScenario {
    pub name: &'static str,
    pub seed: u32,
    pub population: u32,
    pub warmup_ticks: u32,
    pub measured_ticks: u32,
    pub world: BenchmarkWorld,
    pub workload: BenchmarkWorkload,
}

const STANDARD_WORLD: BenchmarkWorld = BenchmarkWorld {
    width: 256,
    height: 256,
    sea_level: 0.35,
};

const SCENARIOS: [BenchmarkScenario; 9] = [
    BenchmarkScenario {
        name: "baseline-100",
        seed: 42,
        population: 100,
        warmup_ticks: 24,
        measured_ticks: 100,
        world: STANDARD_WORLD,
        workload: BenchmarkWorkload::Baseline,
    },
    BenchmarkScenario {
        name: "baseline-1000",
        seed: 42,
        population: 1_000,
        warmup_ticks: 24,
        measured_ticks: 100,
        world: STANDARD_WORLD,
        workload: BenchmarkWorkload::Baseline,
    },
    BenchmarkScenario {
        name: "baseline-10000",
        seed: 42,
        population: 10_000,
        warmup_ticks: 24,
        measured_ticks: 100,
        world: STANDARD_WORLD,
        workload: BenchmarkWorkload::Baseline,
    },
    BenchmarkScenario {
        name: "dense-social-1000",
        seed: 42,
        population: 1_000,
        warmup_ticks: 24,
        measured_ticks: 100,
        world: STANDARD_WORLD,
        workload: BenchmarkWorkload::DenseSocial {
            compact_population: true,
            enclosed_walkable_arena: true,
        },
    },
    BenchmarkScenario {
        name: "scarcity-1000",
        seed: 42,
        population: 1_000,
        warmup_ticks: 24,
        measured_ticks: 100,
        world: STANDARD_WORLD,
        workload: BenchmarkWorkload::Scarcity {
            food_retention_stride: 8,
        },
    },
    BenchmarkScenario {
        name: "households-1000",
        seed: 42,
        population: 1_000,
        warmup_ticks: 24,
        measured_ticks: 100,
        world: STANDARD_WORLD,
        workload: BenchmarkWorkload::Households {
            household_pairs: 250,
            food_per_household: 100,
        },
    },
    BenchmarkScenario {
        name: "pathfinding-heavy-1000",
        seed: 42,
        population: 1_000,
        warmup_ticks: 24,
        measured_ticks: 100,
        world: STANDARD_WORLD,
        workload: BenchmarkWorkload::PathfindingHeavy {
            compact_population: true,
            initial_hunger: 0,
            known_food_targets: 0,
        },
    },
    BenchmarkScenario {
        name: "lineage-1000",
        seed: 42,
        population: 1_000,
        warmup_ticks: 24,
        measured_ticks: 100,
        world: STANDARD_WORLD,
        workload: BenchmarkWorkload::SeededLineage { generations: 10 },
    },
    BenchmarkScenario {
        name: "long-run-1000",
        seed: 42,
        population: 1_000,
        warmup_ticks: 24,
        measured_ticks: 8_760,
        world: STANDARD_WORLD,
        workload: BenchmarkWorkload::LongRun {
            window_count: 12,
            food_grid_spacing: 6,
            food_capacity: 5_000,
        },
    },
];

/// Ticks re-measured with the sampled autonomy profiler after the phase run.
///
/// Autonomy sub-phases are timed per entity and only for a fixed fraction of
/// the population, so they run in a second pass: mixing them into the phase run
/// would perturb the very timings the regression gate compares.
const AUTONOMY_PROFILE_TICKS: u32 = 32;

/// Mean per-tick microseconds of the work `Simulation::execute_autonomy` runs
/// after the per-entity loop and the social pass (#195).
///
/// These eight calls were the unattributed remainder. They are timed one by one
/// because the volumes they receive differ wildly between them; the matching
/// `*_recorded` / `*_attempts` counters in the breakdown say how much work each
/// one was handed.
#[derive(Clone, Copy, Debug, Default, PartialEq, Serialize)]
struct PostPassBreakdown {
    resource_discoveries_us: f64,
    entity_encounters_us: f64,
    food_consumptions_us: f64,
    social_interactions_us: f64,
    food_share_us: f64,
    household_deposit_us: f64,
    household_withdraw_us: f64,
    household_conflict_us: f64,
    total_us: f64,
}

/// Mean per-tick microseconds spent inside the autonomy phase, by sub-phase.
///
/// **Use the ratios, not the absolute values.** Every number here comes from a
/// second pass run with the per-entity profiler switched on. That pass is
/// measurably slower than the phase run (roughly 1.25x–1.6x depending on the
/// scenario), because profiling also computes the `sampled_known_resources_*`
/// and `visible_resources_seen` gauges, which walk each sampled entity's
/// memory. So `social_pass_us` and `entity_pass_us` overstate what the phase
/// run pays. They are still the right basis for deciding *where* the time goes,
/// because they are all inflated the same way and share a denominator.
///
/// Two populations are timed and they must not be added together:
///
/// * `social_pass_us` and `entity_pass_us` cover the **whole population**. Each
///   is measured with a single timer around one pass, so they need no sampling
///   correction.
/// * Every other sub-phase is timed per entity and only for the sampled
///   entities, so those values cover `sampled_fraction` of the population.
///
/// `sampled_subphases_extrapolated_us` scales the sampled total by
/// `sampled_fraction` to approximate the full cost. Measured against
/// `entity_pass_us`, which covers the same work without sampling, the
/// extrapolation lands at 91%–98%, so the sample is representative. An earlier
/// reading of 71%–147% was an artefact of comparing it against the whole
/// autonomy phase, which also contains the social pass — a different
/// denominator, not a biased sample.
///
/// That 91%–98% is **measured, not asserted by a test**: it is a ratio of two
/// wall-clock timers, and pinning it in CI would flake. What a test does pin is
/// the arithmetic around it — see `bridge::profiles::tests`, which asserts that
/// sampled and full-population timings are distinguishable in the serialized
/// payload. Re-derive the bound by hand before quoting it.
///
/// `resource_perception_us` is intentionally absent from this struct: the
/// engine defines it as `memory_reconciliation_us + visible_scan_us`, so
/// reporting it alongside its own components double counts.
/// Per-entity pass, decomposed. Full population, microseconds per tick (#207).
///
/// `attributed_us` is the sum of the four blocks. `residual_us` is what is left
/// of `entity_pass_us`: the two blocks deliberately left untimed
/// (`execute_current_action` and `prune_expired_grief`, both under 1% at 10k)
/// plus the cost of the timers themselves. Measured under 3% at 1k and 10k
/// entities — if it grows, the four blocks have stopped being the story.
#[derive(Clone, Copy, Debug, Default, PartialEq, Serialize)]
struct EntityPassBreakdownDto {
    perceive_entities_us: f64,
    plan_validation_us: f64,
    planning_us: f64,
    resource_memory_us: f64,
    attributed_us: f64,
    residual_us: f64,
}

impl EntityPassBreakdownDto {
    fn from_totals(totals: &EntityPassBreakdown, ticks: f64, entity_pass_us: f64) -> Self {
        let us = |nanos: u64| nanos as f64 / 1_000.0 / ticks;
        let perceive_entities_us = us(totals.perceive_entities_ns);
        let plan_validation_us = us(totals.plan_validation_ns);
        let planning_us = us(totals.planning_ns);
        let resource_memory_us = us(totals.resource_memory_ns);
        let attributed_us =
            perceive_entities_us + plan_validation_us + planning_us + resource_memory_us;
        Self {
            perceive_entities_us,
            plan_validation_us,
            planning_us,
            resource_memory_us,
            attributed_us,
            residual_us: (entity_pass_us - attributed_us).max(0.0),
        }
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Serialize)]
struct AutonomyBreakdown {
    profiled_ticks: u32,
    entities_per_tick: f64,
    sampled_entities_per_tick: f64,
    sampled_fraction: f64,
    social_pass_us: f64,
    /// Bucle por-entidad completo, población completa, sin muestreo.
    entity_pass_us: f64,
    /// `social_pass_us + entity_pass_us`: coste atribuido con medidas fiables.
    attributed_passes_us: f64,
    /// Muro de cada tick perfilado, medido dentro de la misma pasada.
    ///
    /// Es el denominador correcto para `attributed_passes_us`: los dos pases se
    /// cronometran en la **misma** pasada, así que la comparación es válida
    /// aunque esa pasada sea más lenta que la de fases (los temporizadores por
    /// entidad sólo están activos aquí). No vale comparar contra
    /// `summary.autonomy.mean_us`, que sale de otra pasada distinta.
    profiled_step_total_us: f64,
    entity_perception_us: f64,
    visible_scan_us: f64,
    memory_reconciliation_us: f64,
    plan_validation_us: f64,
    planning_us: f64,
    action_us: f64,
    sampled_subphases_us: f64,
    sampled_subphases_extrapolated_us: f64,
    /// Descomposición del bucle por-entidad, población completa (#207).
    entity_pass: EntityPassBreakdownDto,
    /// `entity_pass.total_us`, como fracción de `entity_pass_us`.
    entity_pass_attributed_fraction: f64,
    /// Funnel del social pass por tick (#192). Cada etapa es subconjunto de la
    /// anterior; el descenso entre ellas dice si el coste viene de generar
    /// demasiados pares o del trabajo hecho por cada par.
    social_pairs_scanned: f64,
    social_pairs_in_radius: f64,
    social_pairs_mutual: f64,
    social_pairs_due: f64,
    social_interactions: f64,
    /// Encuentros por tick entregados al sort+dedup de `record_entity_encounters`.
    encounters_recorded: f64,
    discoveries_recorded: f64,
    /// Volumen entregado a cada llamada posterior al bucle (#195).
    food_consumptions_recorded: f64,
    food_share_attempts: f64,
    household_deposit_attempts: f64,
    household_withdraw_attempts: f64,
    household_conflict_attempts: f64,
    post_pass: PostPassBreakdown,
}

#[derive(Serialize)]
struct BenchmarkResult {
    schema_version: u32,
    scenario: BenchmarkScenario,
    summary: PerformanceSummary,
    autonomy_breakdown: AutonomyBreakdown,
}

#[derive(Serialize)]
struct LongRunWindow {
    index: u32,
    start_tick: u64,
    end_tick: u64,
    summary: PerformanceSummary,
}

#[derive(Serialize)]
struct LongRunResult {
    schema_version: u32,
    scenario: BenchmarkScenario,
    overall: PerformanceSummary,
    windows: Vec<LongRunWindow>,
    /// Autonomy breakdown measured **after** the last window, not averaged over
    /// the run.
    ///
    /// This is deliberate. The long scenario is the only one that runs longer
    /// than `GESTATION_TICKS` (6,720 ticks against 8,784 here), so it is the
    /// only one where kinship has real lineage to walk: `genealogy_links`
    /// reaches 142 where every short scenario reports 0. That data only exists
    /// at the end, so profiling at the start would reproduce the blind spot the
    /// short scenarios already have, where `children_of` always returns
    /// nothing.
    ///
    /// Consequence: `overall` averages the whole run while this describes the
    /// smaller, older population the windows leave behind (1,000 entities at
    /// the start, 740 at the end). They are not directly comparable.
    autonomy_breakdown: AutonomyBreakdown,
}

pub fn scenario_names() -> impl Iterator<Item = &'static str> {
    SCENARIOS.iter().map(|scenario| scenario.name)
}

pub fn run_scenario_json(name: &str) -> Result<String, String> {
    let scenario = find_scenario(name)?;
    if matches!(scenario.workload, BenchmarkWorkload::LongRun { .. }) {
        let result = run_long_scenario(scenario)?;
        serde_json::to_string(&result)
            .map_err(|error| format!("failed to serialize result: {error}"))
    } else {
        let result = run_scenario(scenario)?;
        serde_json::to_string(&result)
            .map_err(|error| format!("failed to serialize result: {error}"))
    }
}

fn find_scenario(name: &str) -> Result<BenchmarkScenario, String> {
    SCENARIOS
        .iter()
        .copied()
        .find(|scenario| scenario.name == name)
        .ok_or_else(|| {
            format!("unknown benchmark scenario '{name}'; use --list to see available scenarios")
        })
}

fn run_scenario(scenario: BenchmarkScenario) -> Result<BenchmarkResult, String> {
    if scenario.population as usize > MAX_POPULATION {
        return Err(format!(
            "scenario '{}' population {} exceeds engine maximum {MAX_POPULATION}",
            scenario.name, scenario.population
        ));
    }
    let (mut world, mut simulation) = prepare_scenario(scenario)?;
    for _ in 0..scenario.warmup_ticks {
        simulation.step(&mut world);
    }
    let summary = simulation.profile_run(&mut world, scenario.measured_ticks);
    let autonomy_breakdown =
        profile_autonomy_breakdown(&mut simulation, &mut world, AUTONOMY_PROFILE_TICKS);
    Ok(BenchmarkResult {
        schema_version: BENCHMARK_SCHEMA_VERSION,
        scenario,
        summary,
        autonomy_breakdown,
    })
}

fn profile_autonomy_breakdown(
    simulation: &mut Simulation,
    world: &mut Grid,
    ticks: u32,
) -> AutonomyBreakdown {
    if ticks == 0 {
        return AutonomyBreakdown::default();
    }
    let mut totals = AutonomyProfile::default();
    for _ in 0..ticks {
        let profile = simulation.profile_autonomy_step(world);
        totals.work.accumulate(&profile.work);
        totals.post_pass.accumulate(&profile.post_pass);
        totals.step_total_us += profile.step_total_us;
        totals.entity_perception_us += profile.entity_perception_us;
        totals.visible_scan_us += profile.visible_scan_us;
        totals.memory_reconciliation_us += profile.memory_reconciliation_us;
        totals.plan_validation_us += profile.plan_validation_us;
        totals.planning_us += profile.planning_us;
        totals.action_us += profile.action_us;
        totals.social_us += profile.social_us;
        totals.entity_pass_us += profile.entity_pass_us;
        totals.entity_pass.accumulate(&profile.entity_pass);
        totals.sampled_entities += profile.sampled_entities;
    }

    let tick_count = f64::from(ticks);
    let entities_per_tick = totals.work.entities_processed as f64 / tick_count;
    let sampled_entities_per_tick = f64::from(totals.sampled_entities) / tick_count;
    let sampled_fraction = if entities_per_tick > 0.0 {
        sampled_entities_per_tick / entities_per_tick
    } else {
        0.0
    };
    let mean = |micros: u64| micros as f64 / tick_count;
    let entity_pass_us = mean(totals.entity_pass_us);
    let entity_pass =
        EntityPassBreakdownDto::from_totals(&totals.entity_pass, tick_count, entity_pass_us);
    let sampled_subphases_us = mean(totals.entity_perception_us)
        + mean(totals.visible_scan_us)
        + mean(totals.memory_reconciliation_us)
        + mean(totals.plan_validation_us)
        + mean(totals.planning_us)
        + mean(totals.action_us);
    AutonomyBreakdown {
        profiled_ticks: ticks,
        entities_per_tick,
        sampled_entities_per_tick,
        sampled_fraction,
        social_pass_us: mean(totals.social_us),
        entity_pass_us,
        entity_pass,
        entity_pass_attributed_fraction: if entity_pass_us > 0.0 {
            entity_pass.attributed_us / entity_pass_us
        } else {
            0.0
        },
        attributed_passes_us: mean(totals.social_us) + entity_pass_us,
        profiled_step_total_us: mean(totals.step_total_us),
        entity_perception_us: mean(totals.entity_perception_us),
        visible_scan_us: mean(totals.visible_scan_us),
        memory_reconciliation_us: mean(totals.memory_reconciliation_us),
        plan_validation_us: mean(totals.plan_validation_us),
        planning_us: mean(totals.planning_us),
        action_us: mean(totals.action_us),
        sampled_subphases_us,
        sampled_subphases_extrapolated_us: if sampled_fraction > 0.0 {
            sampled_subphases_us / sampled_fraction
        } else {
            0.0
        },
        social_pairs_scanned: totals.work.social_pairs_scanned as f64 / tick_count,
        social_pairs_in_radius: totals.work.social_pairs_in_radius as f64 / tick_count,
        social_pairs_mutual: totals.work.social_pairs_mutual as f64 / tick_count,
        social_pairs_due: totals.work.social_pairs_due as f64 / tick_count,
        social_interactions: totals.work.social_interactions as f64 / tick_count,
        encounters_recorded: totals.work.encounters_recorded as f64 / tick_count,
        discoveries_recorded: totals.work.discoveries_recorded as f64 / tick_count,
        food_consumptions_recorded: totals.work.food_consumptions_recorded as f64 / tick_count,
        food_share_attempts: totals.work.food_share_attempts as f64 / tick_count,
        household_deposit_attempts: totals.work.household_deposit_attempts as f64 / tick_count,
        household_withdraw_attempts: totals.work.household_withdraw_attempts as f64 / tick_count,
        household_conflict_attempts: totals.work.household_conflict_attempts as f64 / tick_count,
        post_pass: PostPassBreakdown {
            resource_discoveries_us: mean(totals.post_pass.resource_discoveries_us),
            entity_encounters_us: mean(totals.post_pass.entity_encounters_us),
            food_consumptions_us: mean(totals.post_pass.food_consumptions_us),
            social_interactions_us: mean(totals.post_pass.social_interactions_us),
            food_share_us: mean(totals.post_pass.food_share_us),
            household_deposit_us: mean(totals.post_pass.household_deposit_us),
            household_withdraw_us: mean(totals.post_pass.household_withdraw_us),
            household_conflict_us: mean(totals.post_pass.household_conflict_us),
            total_us: mean(totals.post_pass.total_us()),
        },
    }
}

fn run_long_scenario(scenario: BenchmarkScenario) -> Result<LongRunResult, String> {
    let BenchmarkWorkload::LongRun { window_count, .. } = scenario.workload else {
        return Err("long-run executor requires a long-run workload".to_string());
    };
    if window_count == 0 || !scenario.measured_ticks.is_multiple_of(window_count) {
        return Err(format!(
            "long-run measured ticks {} must divide exactly into {window_count} windows",
            scenario.measured_ticks
        ));
    }
    let (mut world, mut simulation) = prepare_scenario(scenario)?;
    for _ in 0..scenario.warmup_ticks {
        simulation.step(&mut world);
    }
    let window_ticks = scenario.measured_ticks / window_count;
    let mut overall = PerformanceRun::default();
    let mut windows = Vec::with_capacity(window_count as usize);
    for index in 0..window_count {
        let start_tick = simulation.tick();
        let mut window = PerformanceRun::default();
        for _ in 0..window_ticks {
            let profile = simulation.profile_step(&mut world);
            overall.record(profile.clone());
            window.record(profile);
        }
        let end_tick = simulation.tick();
        let summary = window.summarize();
        if summary.state_final.recent_events_len > summary.state_final.recent_events_capacity
            || summary.state_peak.recent_events_len > summary.state_peak.recent_events_capacity
        {
            return Err(format!("event history exceeded capacity in window {index}"));
        }
        windows.push(LongRunWindow {
            index,
            start_tick,
            end_tick,
            summary,
        });
    }
    let overall_summary = overall.summarize();
    // Profiled after the windows, so the breakdown sees the population and the
    // lineage the run actually produced. See the field docs on `LongRunResult`.
    let autonomy_breakdown =
        profile_autonomy_breakdown(&mut simulation, &mut world, AUTONOMY_PROFILE_TICKS);
    Ok(LongRunResult {
        schema_version: BENCHMARK_SCHEMA_VERSION,
        scenario,
        overall: overall_summary,
        windows,
        autonomy_breakdown,
    })
}

fn prepare_scenario(scenario: BenchmarkScenario) -> Result<(Grid, Simulation), String> {
    let mut world = build_generated_world(scenario.seed, scenario.world);
    let mut simulation =
        Simulation::with_population(u64::from(scenario.seed), &world, scenario.population);
    let spawned = simulation.entities().len() as u32;
    if spawned != scenario.population {
        return Err(format!(
            "scenario '{}' requested {} entities but the generated world supports {spawned}",
            scenario.name, scenario.population
        ));
    }

    apply_workload(scenario.workload, &mut world, &mut simulation)?;
    Ok((world, simulation))
}

fn apply_workload(
    workload: BenchmarkWorkload,
    world: &mut Grid,
    simulation: &mut Simulation,
) -> Result<(), String> {
    match workload {
        BenchmarkWorkload::Baseline => Ok(()),
        BenchmarkWorkload::DenseSocial { .. } => {
            let positions = build_dense_social_arena(world, simulation.entities().len())?;
            simulation.benchmark_set_entity_positions(world, &positions)
        }
        BenchmarkWorkload::Scarcity {
            food_retention_stride,
        } => retain_food_stride(world, food_retention_stride),
        BenchmarkWorkload::Households {
            household_pairs,
            food_per_household,
        } => simulation.benchmark_seed_households(
            world,
            household_pairs as usize,
            food_per_household,
        ),
        BenchmarkWorkload::PathfindingHeavy { .. } => {
            let (positions, target) =
                build_pathfinding_heavy_arena(world, simulation.entities().len())?;
            simulation.benchmark_set_entity_positions(world, &positions)?;
            set_single_food_target(world, target, 500);
            Ok(())
        }
        BenchmarkWorkload::LongRun {
            food_grid_spacing,
            food_capacity,
            ..
        } => prepare_long_run_food(world, food_grid_spacing, food_capacity),
        BenchmarkWorkload::SeededLineage { generations } => {
            // Reuses the dense-social arena for placement: it is a compact
            // walkable square, which is exactly what produces encounters here.
            // The name says "social" but nothing about it is social-specific.
            let positions = build_dense_social_arena(world, simulation.entities().len())?;
            simulation.benchmark_set_entity_positions(world, &positions)?;
            let population = u32::try_from(simulation.entities().len()).unwrap_or(u32::MAX);
            let parents = synthetic_lineage_parents(population, generations)?;
            simulation.benchmark_seed_lineage(world, &parents)
        }
    }
}

fn build_dense_social_arena(world: &mut Grid, count: usize) -> Result<Vec<(u32, u32)>, String> {
    let side = (count as f64).sqrt().ceil() as u32;
    if side + 2 > world.width || side + 2 > world.height {
        return Err(format!(
            "world is too small for a dense arena of {count} entities"
        ));
    }
    let start_x = (world.width - side) / 2;
    let start_y = (world.height - side) / 2;
    let ring_min_x = start_x - 1;
    let ring_min_y = start_y - 1;
    let ring_max_x = start_x + side;
    let ring_max_y = start_y + side;
    for y in ring_min_y..=ring_max_y {
        for x in ring_min_x..=ring_max_x {
            let index = (y * world.width + x) as usize;
            let is_ring = x == ring_min_x || x == ring_max_x || y == ring_min_y || y == ring_max_y;
            world.tiles[index].terrain = if is_ring {
                Terrain::SnowPeak
            } else {
                Terrain::Plains
            };
            if is_ring {
                world.resources[index] = None;
            }
        }
    }
    synchronize_renewable_food(world);
    detect_regions(world);
    let positions = (0..side)
        .flat_map(|dy| (0..side).map(move |dx| (start_x + dx, start_y + dy)))
        .take(count)
        .collect();
    Ok(positions)
}

fn build_pathfinding_heavy_arena(world: &mut Grid, count: usize) -> Result<PreparedArena, String> {
    let mut positions = build_dense_social_arena(world, count.saturating_add(1))?;
    let target = positions
        .pop()
        .ok_or_else(|| "pathfinding arena has no remote target".to_string())?;
    Ok((positions, target))
}

fn retain_food_stride(world: &mut Grid, stride: usize) -> Result<(), String> {
    if stride == 0 {
        return Err("food retention stride must be nonzero".to_string());
    }
    let mut ordinal = 0usize;
    for deposit in &mut world.resources {
        if deposit.is_some_and(|deposit| deposit.kind == ResourceKind::Food) {
            if !ordinal.is_multiple_of(stride) {
                *deposit = None;
            }
            ordinal += 1;
        }
    }
    synchronize_renewable_food(world);
    Ok(())
}

fn set_single_food_target(world: &mut Grid, coordinate: (u32, u32), amount: u16) {
    let retained_index = (coordinate.1 * world.width + coordinate.0) as usize;
    for (index, deposit) in world.resources.iter_mut().enumerate() {
        if index != retained_index
            && deposit.is_some_and(|deposit| deposit.kind == ResourceKind::Food)
        {
            *deposit = None;
        }
    }
    synchronize_renewable_food(world);
    world.resources[retained_index] = Some(ResourceDeposit {
        kind: ResourceKind::Food,
        amount,
    });
    world
        .renewable_resources
        .retain(|renewable| renewable.index != retained_index);
    world.renewable_resources.push(RenewableResource {
        index: retained_index,
        kind: ResourceKind::Food,
        capacity: amount,
    });
}

fn synchronize_renewable_food(world: &mut Grid) {
    world.renewable_resources.retain(|renewable| {
        world.resources.get(renewable.index).is_some_and(|deposit| {
            deposit.is_some_and(|deposit| {
                deposit.kind == renewable.kind && renewable.kind == ResourceKind::Food
            })
        })
    });
}

fn prepare_long_run_food(world: &mut Grid, spacing: u32, capacity: u16) -> Result<(), String> {
    if spacing == 0 || capacity == 0 {
        return Err("long-run food spacing and capacity must be nonzero".to_string());
    }
    for y in 0..world.height {
        for x in 0..world.width {
            if x % spacing == spacing / 2
                && y % spacing == spacing / 2
                && world
                    .get(x, y)
                    .is_some_and(|tile| tile.terrain.is_walkable())
            {
                world.resources[(y * world.width + x) as usize] = Some(ResourceDeposit {
                    kind: ResourceKind::Food,
                    amount: capacity,
                });
            }
        }
    }
    world.renewable_resources = world
        .resources
        .iter()
        .enumerate()
        .filter_map(|(index, deposit)| {
            let deposit = deposit.as_ref()?;
            (deposit.kind == ResourceKind::Food).then_some(RenewableResource {
                index,
                kind: ResourceKind::Food,
                capacity: deposit.amount,
            })
        })
        .collect();
    Ok(())
}

fn build_generated_world(seed: u32, parameters: BenchmarkWorld) -> Grid {
    let mut world = generate_world(
        seed,
        parameters.width,
        parameters.height,
        parameters.sea_level,
    );
    generate_resources(seed, &mut world);
    detect_regions(&mut world);
    world
}

/// Opaque fixture that reuses Nexus's production A* workspace.
pub struct BenchmarkPathfindingFixture {
    world: Grid,
    workspace: PathfindingWorkspace,
    start: (u32, u32),
    goal: (u32, u32),
}

impl BenchmarkPathfindingFixture {
    pub fn short() -> Self {
        Self::new(pathfinding_world(false), (8, 8), (20, 20))
    }

    pub fn long() -> Self {
        Self::new(pathfinding_world(false), (1, 1), (126, 126))
    }

    pub fn mixed_terrain() -> Self {
        Self::new(pathfinding_world(true), (1, 64), (126, 64))
    }

    fn new(world: Grid, start: (u32, u32), goal: (u32, u32)) -> Self {
        Self {
            world,
            workspace: PathfindingWorkspace::new(),
            start,
            goal,
        }
    }

    pub fn run(&mut self) -> usize {
        find_path_with_workspace(&mut self.workspace, &self.world, self.start, self.goal)
            .map_or(0, |path| path.len())
    }
}

fn pathfinding_world(mixed: bool) -> Grid {
    let width = 128;
    let height = 128;
    let tiles = (0..height)
        .flat_map(|y| {
            (0..width).map(move |x| Tile {
                terrain: if mixed {
                    match (x / 8 + y / 16) % 4 {
                        0 => Terrain::Plains,
                        1 => Terrain::Grassland,
                        2 => Terrain::Forest,
                        _ => Terrain::Hills,
                    }
                } else {
                    Terrain::Plains
                },
                altitude: 0.25,
                moisture: 0.5,
                temperature: 0.5,
            })
        })
        .collect();
    Grid {
        width,
        height,
        tiles,
        region_ids: Vec::new(),
        regions: Vec::new(),
        resources: vec![None; (width * height) as usize],
        renewable_resources: Vec::new(),
    }
}

/// Opaque prepared population fixture for spatial-index measurements.
pub struct BenchmarkSpatialFixture {
    world: Grid,
    simulation: Simulation,
    query_x: u32,
    query_y: u32,
}

impl BenchmarkSpatialFixture {
    pub fn new(population: u32) -> Result<Self, String> {
        let world = build_generated_world(42, STANDARD_WORLD);
        let simulation = Simulation::with_population(42, &world, population);
        if simulation.entities().len() as u32 != population {
            return Err(format!("could not prepare spatial population {population}"));
        }
        let (query_x, query_y) = simulation
            .entities()
            .first()
            .map(|entity| (entity.x, entity.y))
            .ok_or_else(|| format!("spatial population {population} is empty"))?;
        let mut fixture = Self {
            world,
            simulation,
            query_x,
            query_y,
        };
        fixture.rebuild();
        Ok(fixture)
    }

    pub fn rebuild(&mut self) -> usize {
        self.simulation
            .benchmark_rebuild_population_index(&self.world)
    }

    pub fn query_local_population(&self) -> usize {
        self.simulation
            .benchmark_spatial_query(self.query_x, self.query_y, 12)
    }
}

/// Synthetic multi-generation lineage, for measuring kinship queries that
/// actually return children.
///
/// No scenario can do this. The short ones report `genealogy_links = 0`
/// (`GESTATION_TICKS` is 6,720 against 124 ticks), and `long-run-1000` produces
/// only a single generation, and only after its social graph has decayed tenfold
/// — see #199. This fixture builds the tree directly and sidesteps
/// `GESTATION_TICKS` and `POSTPARTUM_TICKS` entirely.
///
/// Shape: `KINSHIP_FIXTURE_GENERATIONS` generations of equal size, entity ids
/// running `1..=population` in generation order. Generation 0 is founders with
/// no parents; every other entity has two **distinct** parents drawn from the
/// previous generation by [`fixture_hash`], so parents average two children but
/// are not uniform — some have none, some have five, as in a real population.
///
/// Mating must be pseudo-random rather than regular. An earlier revision paired
/// child `i` with mother `i` and father `(i + n/2) % n`, which is an involution:
/// ancestor sets collapsed to `{i, i + n/2}` at every depth, producing `n/2`
/// parallel lineages that never intermarried. Descendant counts stayed at two
/// per generation and no two entities were ever cousins, so the fixture
/// measured nothing. The current rule has no such fixed points.
pub struct BenchmarkKinshipFixture {
    genealogy: Genealogy,
    parent_ids: Vec<u32>,
    focal_ids: Vec<u32>,
    pair_ids: Vec<(u32, u32)>,
}

const KINSHIP_FIXTURE_GENERATIONS: u32 = 10;
const KINSHIP_FIXTURE_PARENT_SAMPLES: usize = 64;
const KINSHIP_FIXTURE_FOCAL_SAMPLES: usize = 16;
const KINSHIP_FIXTURE_PAIR_SAMPLES: usize = 16;

/// Mother and father for every entity of a synthetic multi-generation tree, as
/// **indices** into the population.
///
/// Indices and not entity ids, because a `Simulation` assigns the ids and the
/// caller cannot assume what they are; the criterion fixture below, which owns
/// its id space, shifts them by one.
///
/// Generation 0 are founders with no parents. Every other entity has two
/// parents drawn from the previous generation by [`fixture_hash`], so parents
/// average two children but are not uniform — some have none, some have five.
/// A uniform tree would make every kinship query return the same size and
/// measure nothing.
///
/// Shared with the seeded-lineage benchmark scenario (#212) so that criterion
/// and the gate exercise the same shape of tree. They used to be able to drift:
/// the fixture's rule lived only here.
/// Mother and father of one entity, as indices into the population.
///
/// An alias rather than a bare tuple because `Result<Vec<(Option<u32>,
/// Option<u32>)>, String>` trips `clippy::type_complexity`, and because the
/// name says which slot is which — a tuple does not.
type LineageParents = (Option<u32>, Option<u32>);

fn synthetic_lineage_parents(
    population: u32,
    generations: u32,
) -> Result<Vec<LineageParents>, String> {
    if generations == 0 {
        return Err("lineage needs at least one generation".to_string());
    }
    if population == 0 || !population.is_multiple_of(generations) {
        return Err(format!(
            "lineage population {population} must be a nonzero multiple of {generations}"
        ));
    }
    let generation_size = population / generations;
    let mut parents = Vec::with_capacity(population as usize);
    for generation in 0..generations {
        let previous_start = generation.saturating_sub(1) * generation_size;
        for index in 0..generation_size {
            if generation == 0 {
                parents.push((None, None));
                continue;
            }
            let mother = previous_start + fixture_hash(index ^ generation) % generation_size;
            // Nudged once when the draw collides, so a child always has two
            // distinct parents and the link count is exact. With
            // `generation_size == 1` there is no other parent to pick, and
            // `register` collapses the pair to one link.
            let mut father_offset =
                fixture_hash(index ^ generation ^ 0x8000_0000) % generation_size;
            if previous_start + father_offset == mother && generation_size > 1 {
                father_offset = (father_offset + 1) % generation_size;
            }
            parents.push((Some(mother), Some(previous_start + father_offset)));
        }
    }
    Ok(parents)
}

impl BenchmarkKinshipFixture {
    pub fn new(population: u32) -> Result<Self, String> {
        let parents = synthetic_lineage_parents(population, KINSHIP_FIXTURE_GENERATIONS)?;
        let generation_size = population / KINSHIP_FIXTURE_GENERATIONS;
        let mut genealogy = Genealogy::default();
        for (&(mother, father), id) in parents.iter().zip(1..=population) {
            // The fixture's ids are 1-based, so an index maps to `index + 1`.
            let shift = |parent: Option<u32>| parent.map(|index| index + 1);
            genealogy.register(id, shift(mother), shift(father));
        }

        // Sampled from parents that are known to have children. Sampling raw ids
        // would include the childless, and every empty lookup is work the
        // benchmark is not trying to measure.
        let mut parents_with_children: Vec<u32> = genealogy
            .records()
            .iter()
            .flat_map(|record| [record.mother_id, record.father_id])
            .flatten()
            .collect();
        parents_with_children.sort_unstable();
        parents_with_children.dedup();
        let parent_ids = spread_sample(
            0,
            parents_with_children.len() as u32,
            KINSHIP_FIXTURE_PARENT_SAMPLES,
        )
        .into_iter()
        .map(|index| parents_with_children[index as usize])
        .collect();

        // Founders: with random mating their descendant sets grow geometrically
        // until they saturate the generation, so this walks most of the tree.
        let focal_ids = spread_sample(1, 1 + generation_size, KINSHIP_FIXTURE_FOCAL_SAMPLES);

        // Pairs are taken *within* the last generation, on purpose.
        //
        // `relationship_between` tests descendants first and ancestors second,
        // so any cross-generation pair short-circuits into the ancestor fast
        // path and never reaches the `AuntUncle` / `NieceNephew` / `Cousin`
        // branches — which are the only ones that walk ancestors on both sides,
        // and the only ones with a nested `ancestors x ancestors` scan.
        // Same-generation pairs cannot be ancestor/descendant of each other, and
        // the last generation has the deepest ancestor chains. Two consecutive
        // ids are full siblings only if both draws collide, which is negligible.
        let last_generation_start = 1 + (KINSHIP_FIXTURE_GENERATIONS - 1) * generation_size;
        let pair_ids = spread_sample(
            last_generation_start,
            population,
            KINSHIP_FIXTURE_PAIR_SAMPLES,
        )
        .into_iter()
        .map(|first_id| (first_id, first_id + 1))
        .collect();

        Ok(Self {
            genealogy,
            parent_ids,
            focal_ids,
            pair_ids,
        })
    }

    /// `children_of` through the `Genealogy` index added in #198.
    ///
    /// Until #202 this allocated and copied a `Vec` on every call, which at 10k
    /// was ~65% of the cost. It now borrows the index, so this group and
    /// [`Self::children_of_index_without_copy`] should agree: if they diverge,
    /// somebody put an allocation back.
    pub fn children_of_index(&self) -> usize {
        self.parent_ids
            .iter()
            .map(|&parent_id| {
                children_of(&self.genealogy, parent_id)
                    .iter()
                    .map(|&child_id| child_id as usize)
                    .sum::<usize>()
            })
            .sum()
    }

    /// The same lookups as [`Self::children_of_index`], reading the index
    /// directly instead of going through `kinship::children_of`.
    ///
    /// These two used to differ by exactly one `Vec` allocation, and comparing
    /// them is how #198's index was priced against the copy it still paid for.
    /// #202 removed the copy, so this is now the floor that
    /// [`Self::children_of_index`] is expected to match rather than a cheaper
    /// alternative to it. Returns the same number as `children_of_index`.
    pub fn children_of_index_without_copy(&self) -> usize {
        self.parent_ids
            .iter()
            .map(|&parent_id| {
                self.genealogy
                    .children_of(parent_id)
                    .iter()
                    .map(|&child_id| child_id as usize)
                    .sum::<usize>()
            })
            .sum()
    }

    /// The same query through the linear filter that #198 replaced. Kept only
    /// to quantify what the index is worth with real children; the short
    /// scenarios cannot, because they have no children at all. Nothing outside
    /// benchmarks calls this.
    pub fn children_of_scan(&self) -> usize {
        self.parent_ids
            .iter()
            .map(|&parent_id| {
                self.genealogy
                    .records()
                    .iter()
                    .filter(|record| {
                        record.mother_id == Some(parent_id) || record.father_id == Some(parent_id)
                    })
                    .map(|record| record.entity_id as usize)
                    .sum::<usize>()
            })
            .sum()
    }

    /// Breadth-first descent from founders, which is the deep traversal the
    /// short scenarios never get to run.
    pub fn descendants(&self) -> usize {
        self.focal_ids
            .iter()
            .map(|&focal_id| descendants_of(&self.genealogy, focal_id).len())
            .sum()
    }

    /// Classification for same-generation pairs, which is the only shape that
    /// reaches the `AuntUncle` / `NieceNephew` / `Cousin` branches that every
    /// scenario leaves dead.
    pub fn relationships(&self) -> usize {
        self.pair_ids
            .iter()
            .filter(|&&(first_id, second_id)| {
                relationship_between(&self.genealogy, first_id, second_id)
                    != KinshipRelation::Unrelated
            })
            .count()
    }
}

/// Deterministic pseudo-random draw for fixture construction: a splitmix32
/// finalizer. Chosen over a `rand` call because fixtures must build the exact
/// same genealogy on every machine and every run, forever — a benchmark whose
/// input moves between runs cannot detect a regression.
fn fixture_hash(value: u32) -> u32 {
    let mut state = value.wrapping_add(0x9E37_79B9);
    state = (state ^ (state >> 16)).wrapping_mul(0x7FEB_352D);
    state = (state ^ (state >> 15)).wrapping_mul(0x846C_A68B);
    state ^ (state >> 16)
}

fn spread_sample(start: u32, end_exclusive: u32, count: usize) -> Vec<u32> {
    let span = usize::try_from(end_exclusive.saturating_sub(start))
        .unwrap_or(0)
        .max(1);
    let take = count.min(span).max(1);
    (0..take)
        .map(|index| start + u32::try_from(index * span / take).unwrap_or(0))
        .collect()
}

/// Cloneable deterministic base state for canonical autonomy measurements.
pub struct BenchmarkAutonomyFixture {
    world: Grid,
    simulation: Simulation,
}

pub struct BenchmarkAutonomyRun {
    world: Grid,
    simulation: Simulation,
}

impl BenchmarkAutonomyFixture {
    pub fn new(population: u32) -> Result<Self, String> {
        let mut world = build_generated_world(42, STANDARD_WORLD);
        let mut simulation = Simulation::with_population(42, &world, population);
        if simulation.entities().len() as u32 != population {
            return Err(format!(
                "could not prepare autonomy population {population}"
            ));
        }
        simulation.step(&mut world);
        simulation.benchmark_rebuild_population_index(&world);
        Ok(Self { world, simulation })
    }

    pub fn prepare(&self) -> BenchmarkAutonomyRun {
        BenchmarkAutonomyRun {
            world: self.world.clone(),
            simulation: self.simulation.clone(),
        }
    }
}

impl BenchmarkAutonomyRun {
    pub fn run_once(&mut self) -> u64 {
        self.simulation.benchmark_autonomy_pass(&mut self.world)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashSet;

    const TEST_SCENARIO: BenchmarkScenario = BenchmarkScenario {
        name: "test-small",
        seed: 7,
        population: 10,
        warmup_ticks: 2,
        measured_ticks: 3,
        world: BenchmarkWorld {
            width: 64,
            height: 64,
            sea_level: 0.0,
        },
        workload: BenchmarkWorkload::Baseline,
    };

    fn small_specialized(workload: BenchmarkWorkload) -> BenchmarkScenario {
        BenchmarkScenario {
            name: "test-specialized",
            seed: 42,
            population: 20,
            warmup_ticks: 1,
            measured_ticks: 2,
            world: BenchmarkWorld {
                width: 96,
                height: 96,
                sea_level: 0.2,
            },
            workload,
        }
    }

    /// #212: the seeded lineage is two data structures that must agree — the
    /// `Genealogy`, which `kinship` reads, and the `mother_id` / `father_id`
    /// fields on each entity, which the state gauges and the population
    /// snapshot read. Nothing else would catch them drifting apart: a
    /// disagreement changes which relative a query returns without failing
    /// anything.
    #[test]
    fn seeded_lineage_agrees_with_the_entity_fields() {
        let scenario = small_specialized(BenchmarkWorkload::SeededLineage { generations: 5 });
        let (_world, simulation) = prepare_scenario(scenario).expect("seeded lineage scenario");

        for entity in simulation.entities() {
            let record = simulation
                .genealogy()
                .get(entity.id)
                .expect("every seeded entity has a lineage record");
            assert_eq!(record.mother_id, entity.mother_id, "entity {}", entity.id);
            assert_eq!(record.father_id, entity.father_id, "entity {}", entity.id);
        }

        // The tree has to reach the genealogy, not just the entities: 20
        // entities over 5 generations is 4 founders and 16 children, each with
        // two distinct parents.
        let links = simulation
            .entities()
            .iter()
            .map(|entity| {
                usize::from(entity.mother_id.is_some()) + usize::from(entity.father_id.is_some())
            })
            .sum::<usize>();
        assert_eq!(links, 32, "expected 16 children x 2 distinct parents");
    }

    /// #201 left a warning about this and it is worth pinning: the first
    /// revision of the mating rule was an involution, so ancestor sets
    /// collapsed to two entities at every depth and the fixture measured
    /// nothing while still looking plausible. A uniform tree is the same
    /// failure — every kinship query returns the same size.
    #[test]
    fn seeded_lineage_does_not_give_every_parent_the_same_number_of_children() {
        let parents = synthetic_lineage_parents(1_000, 10).expect("valid lineage population");

        let mut by_parent: Vec<u32> = parents
            .iter()
            .flat_map(|(mother, father)| [(*mother), (*father)])
            .flatten()
            .collect();
        by_parent.sort_unstable();
        let mut unique = by_parent.clone();
        unique.dedup();

        let counts: Vec<usize> = unique
            .iter()
            .map(|parent| by_parent.iter().filter(|other| *other == parent).count())
            .collect();
        let fewest = counts.iter().min().expect("seeded parents");
        let most = counts.iter().max().expect("seeded parents");
        assert!(
            most > fewest,
            "degenerate lineage: every parent has exactly {fewest} children, so every \
             kinship query returns the same size and measures nothing"
        );
    }

    /// #207: the residual is a number that gets quoted, so its arithmetic is
    /// pinned here. What is *not* pinned is how big it is — that is a ratio of
    /// wall-clock timers and would flake (#204). What is pinned is that
    /// `attributed_us` is exactly the sum of the four blocks and that the
    /// residual is the difference against `entity_pass_us`, never negative.
    /// Without this, adding a fifth block and forgetting `attributed_us` would
    /// silently make the residual meaningless.
    #[test]
    fn entity_pass_residual_is_the_difference_against_the_whole_pass() {
        let totals = EntityPassBreakdown {
            perceive_entities_ns: 400_000,
            plan_validation_ns: 300_000,
            planning_ns: 200_000,
            resource_memory_ns: 100_000,
        };
        let dto = EntityPassBreakdownDto::from_totals(&totals, 2.0, 500.0);

        assert_eq!(dto.perceive_entities_us, 200.0);
        assert_eq!(dto.plan_validation_us, 150.0);
        assert_eq!(dto.planning_us, 100.0);
        assert_eq!(dto.resource_memory_us, 50.0);
        assert_eq!(dto.attributed_us, 500.0);
        assert_eq!(dto.residual_us, 0.0);
    }

    /// The residual must stay non-negative even when the timers overshoot the
    /// pass they are inside — they can, because `entity_pass_us` is taken in
    /// microseconds and the four blocks in nanoseconds, over different spans.
    #[test]
    fn entity_pass_residual_clamps_at_zero() {
        let totals = EntityPassBreakdown {
            perceive_entities_ns: 900_000,
            plan_validation_ns: 900_000,
            planning_ns: 900_000,
            resource_memory_ns: 900_000,
        };
        let dto = EntityPassBreakdownDto::from_totals(&totals, 1.0, 100.0);

        assert_eq!(dto.attributed_us, 3_600.0);
        assert_eq!(dto.residual_us, 0.0);
    }

    /// Every block that gets timed has to survive into the DTO, with its own
    /// value. A distinct nanosecond figure per block is what makes this a
    /// tripwire: with four equal values, dropping one or wiring two to the same
    /// field would pass unnoticed.
    #[test]
    fn entity_pass_breakdown_emits_every_block_it_times() {
        let totals = EntityPassBreakdown {
            perceive_entities_ns: 1_000,
            plan_validation_ns: 2_000,
            planning_ns: 3_000,
            resource_memory_ns: 4_000,
        };
        let dto = EntityPassBreakdownDto::from_totals(&totals, 1.0, 1_000.0);

        assert_eq!(dto.perceive_entities_us, 1.0);
        assert_eq!(dto.plan_validation_us, 2.0);
        assert_eq!(dto.planning_us, 3.0);
        assert_eq!(dto.resource_memory_us, 4.0);
        assert_eq!(dto.attributed_us, 10.0);
        assert_eq!(dto.residual_us, 990.0);
    }

    fn food_count(world: &Grid) -> usize {
        world
            .resources
            .iter()
            .filter(|deposit| deposit.is_some_and(|deposit| deposit.kind == ResourceKind::Food))
            .count()
    }

    #[test]
    fn registered_names_are_unique_and_stable() {
        let names: Vec<_> = scenario_names().collect();
        assert_eq!(
            names,
            vec![
                "baseline-100",
                "baseline-1000",
                "baseline-10000",
                "dense-social-1000",
                "scarcity-1000",
                "households-1000",
                "pathfinding-heavy-1000",
                "lineage-1000",
                "long-run-1000",
            ]
        );
        assert_eq!(
            names.iter().copied().collect::<HashSet<_>>().len(),
            names.len()
        );
        assert!(SCENARIOS
            .iter()
            .all(|scenario| scenario.population as usize <= MAX_POPULATION));
    }

    #[test]
    fn unknown_scenario_has_actionable_error() {
        let error = find_scenario("missing").unwrap_err();
        assert!(error.contains("unknown benchmark scenario 'missing'"));
        assert!(error.contains("--list"));
    }

    #[test]
    fn scenario_json_is_versioned_and_excludes_warmup_samples() {
        let result = run_scenario(TEST_SCENARIO).unwrap();
        let json = serde_json::to_value(&result).unwrap();
        assert_eq!(
            result.summary.samples,
            u64::from(TEST_SCENARIO.measured_ticks)
        );
        assert_eq!(json["schema_version"], BENCHMARK_SCHEMA_VERSION);
        assert_eq!(json["scenario"]["name"], TEST_SCENARIO.name);
        assert_eq!(json["scenario"]["workload"]["kind"], "baseline");
        assert_eq!(json["summary"]["samples"], TEST_SCENARIO.measured_ticks);
    }

    #[test]
    fn same_scenario_repeats_deterministic_metadata_counters_and_gauges() {
        let first = run_scenario(TEST_SCENARIO).unwrap();
        let second = run_scenario(TEST_SCENARIO).unwrap();

        assert_eq!(first.scenario, second.scenario);
        assert_eq!(first.summary.work_total, second.summary.work_total);
        assert_eq!(first.summary.state_final, second.summary.state_final);
        assert_eq!(first.summary.state_peak, second.summary.state_peak);
    }

    #[test]
    fn specialized_setups_preserve_structural_invariants() {
        let baseline =
            build_generated_world(42, small_specialized(BenchmarkWorkload::Baseline).world);
        let baseline_food = food_count(&baseline);

        let dense = small_specialized(BenchmarkWorkload::DenseSocial {
            compact_population: true,
            enclosed_walkable_arena: true,
        });
        let (dense_world, dense_simulation) = prepare_scenario(dense).unwrap();
        let positions: HashSet<_> = dense_simulation
            .entities()
            .iter()
            .map(|entity| (entity.x, entity.y))
            .collect();
        assert_eq!(positions.len(), dense.population as usize);
        assert!(positions.iter().all(|&(x, y)| dense_world
            .get(x, y)
            .is_some_and(|tile| tile.terrain.is_walkable())));
        let min_x = positions.iter().map(|position| position.0).min().unwrap();
        let max_x = positions.iter().map(|position| position.0).max().unwrap();
        let min_y = positions.iter().map(|position| position.1).min().unwrap();
        let max_y = positions.iter().map(|position| position.1).max().unwrap();
        assert!((max_x - min_x) * (max_y - min_y) < dense_world.width * dense_world.height / 8);

        let scarcity = small_specialized(BenchmarkWorkload::Scarcity {
            food_retention_stride: 8,
        });
        let (scarce_world, _) = prepare_scenario(scarcity).unwrap();
        assert!(food_count(&scarce_world) < baseline_food);
        assert!(scarce_world.renewable_resources.iter().all(|renewable| {
            scarce_world
                .resources
                .get(renewable.index)
                .is_some_and(|deposit| {
                    deposit.is_some_and(|deposit| {
                        deposit.kind == ResourceKind::Food && deposit.kind == renewable.kind
                    })
                })
        }));

        let households = small_specialized(BenchmarkWorkload::Households {
            household_pairs: 5,
            food_per_household: 100,
        });
        let (household_world, household_simulation) = prepare_scenario(households).unwrap();
        assert_eq!(household_simulation.households().len(), 5);
        household_simulation
            .benchmark_household_invariants(&household_world)
            .unwrap();

        let heavy = small_specialized(BenchmarkWorkload::PathfindingHeavy {
            compact_population: true,
            initial_hunger: 0,
            known_food_targets: 0,
        });
        let (heavy_world, heavy_simulation) = prepare_scenario(heavy).unwrap();
        assert_eq!(food_count(&heavy_world), 1);
        assert!(heavy_simulation.entities().iter().all(|entity| {
            entity.hunger == 0.0
                && entity.mind.memory.known_resources.is_empty()
                && heavy_world
                    .get(entity.x, entity.y)
                    .is_some_and(|tile| tile.terrain.is_walkable())
        }));
    }

    #[test]
    fn specialized_workloads_repeat_deterministic_counters_and_gauges() {
        let workloads = [
            BenchmarkWorkload::DenseSocial {
                compact_population: true,
                enclosed_walkable_arena: true,
            },
            BenchmarkWorkload::Scarcity {
                food_retention_stride: 8,
            },
            BenchmarkWorkload::Households {
                household_pairs: 5,
                food_per_household: 100,
            },
            BenchmarkWorkload::PathfindingHeavy {
                compact_population: true,
                initial_hunger: 0,
                known_food_targets: 0,
            },
        ];
        for workload in workloads {
            let scenario = small_specialized(workload);
            let first = run_scenario(scenario).unwrap();
            let second = run_scenario(scenario).unwrap();
            assert_eq!(first.scenario, second.scenario);
            assert_eq!(first.summary.work_total, second.summary.work_total);
            assert_eq!(first.summary.state_final, second.summary.state_final);
            assert_eq!(first.summary.state_peak, second.summary.state_peak);
        }
    }

    #[test]
    fn long_run_windows_are_contiguous_complete_and_deterministic() {
        let mut scenario = small_specialized(BenchmarkWorkload::LongRun {
            window_count: 3,
            food_grid_spacing: 12,
            food_capacity: 5_000,
        });
        scenario.warmup_ticks = 2;
        scenario.measured_ticks = 12;
        let first = run_long_scenario(scenario).unwrap();
        let second = run_long_scenario(scenario).unwrap();

        assert_eq!(first.scenario, second.scenario);
        assert_eq!(first.windows.len(), 3);
        assert_eq!(first.windows[0].start_tick, 2);
        assert_eq!(first.windows.last().unwrap().end_tick, 14);
        assert!(first
            .windows
            .windows(2)
            .all(|pair| pair[0].end_tick == pair[1].start_tick));
        assert_eq!(
            first
                .windows
                .iter()
                .map(|window| window.summary.samples)
                .sum::<u64>(),
            first.overall.samples
        );

        let mut summed_work = crate::simulation::WorkCounters::default();
        for (left, right) in first.windows.iter().zip(&second.windows) {
            assert_eq!(left.index, right.index);
            assert_eq!(left.start_tick, right.start_tick);
            assert_eq!(left.end_tick, right.end_tick);
            assert_eq!(left.summary.work_total, right.summary.work_total);
            assert_eq!(left.summary.state_final, right.summary.state_final);
            assert_eq!(left.summary.state_peak, right.summary.state_peak);
            assert!(
                left.summary.state_final.recent_events_len
                    <= left.summary.state_final.recent_events_capacity
            );
            assert!(
                left.summary.state_peak.recent_events_len
                    <= left.summary.state_peak.recent_events_capacity
            );
            assert!(first.overall.state_peak.dominates(&left.summary.state_peak));
            summed_work.accumulate(&left.summary.work_total);
        }
        assert_eq!(summed_work, first.overall.work_total);
        assert_eq!(first.overall.work_total, second.overall.work_total);
        assert_eq!(first.overall.state_final, second.overall.state_final);
        assert_eq!(first.overall.state_peak, second.overall.state_peak);
        assert_eq!(
            first.overall.state_final,
            first.windows.last().unwrap().summary.state_final
        );
    }

    #[test]
    fn long_run_autonomy_breakdown_is_profiled_and_deterministic() {
        let mut scenario = small_specialized(BenchmarkWorkload::LongRun {
            window_count: 3,
            food_grid_spacing: 12,
            food_capacity: 5_000,
        });
        scenario.warmup_ticks = 2;
        scenario.measured_ticks = 12;
        let first = run_long_scenario(scenario).unwrap();
        let second = run_long_scenario(scenario).unwrap();

        assert_eq!(
            first.autonomy_breakdown.profiled_ticks,
            AUTONOMY_PROFILE_TICKS
        );
        assert!(first.autonomy_breakdown.entities_per_tick > 0.0);

        // Only the counters are compared. The `*_us` timings are wall-clock and
        // are not reproducible across runs, so asserting on them would be a
        // flake waiting to happen.
        assert_eq!(
            first.autonomy_breakdown.entities_per_tick,
            second.autonomy_breakdown.entities_per_tick
        );
        assert_eq!(
            first.autonomy_breakdown.sampled_entities_per_tick,
            second.autonomy_breakdown.sampled_entities_per_tick
        );
        assert_eq!(
            first.autonomy_breakdown.social_pairs_scanned,
            second.autonomy_breakdown.social_pairs_scanned
        );
        assert_eq!(
            first.autonomy_breakdown.social_interactions,
            second.autonomy_breakdown.social_interactions
        );
    }

    #[test]
    fn microbenchmark_fixtures_execute_real_deterministic_workloads() {
        assert!(BenchmarkPathfindingFixture::short().run() > 0);
        assert!(BenchmarkPathfindingFixture::long().run() > 0);
        assert!(BenchmarkPathfindingFixture::mixed_terrain().run() > 0);

        let mut spatial = BenchmarkSpatialFixture::new(100).unwrap();
        assert_eq!(spatial.rebuild(), 100);
        let _ = spatial.query_local_population();

        let autonomy = BenchmarkAutonomyFixture::new(100).unwrap();
        assert_eq!(autonomy.prepare().run_once(), autonomy.prepare().run_once());
    }

    #[test]
    fn kinship_fixture_rejects_populations_that_cannot_fill_ten_generations() {
        assert!(BenchmarkKinshipFixture::new(0).is_err());
        // 95 does not divide into ten equal generations.
        assert!(BenchmarkKinshipFixture::new(95).is_err());
        assert!(BenchmarkKinshipFixture::new(10).is_ok());
        assert!(BenchmarkKinshipFixture::new(1_000).is_ok());
    }

    /// The invariant that makes the fixture usable at all, and the one no
    /// scenario satisfies: unlike every scenario, parents here have children.
    #[test]
    fn kinship_fixture_links_every_non_founder_to_two_distinct_parents() {
        let population = 1_000u32;
        let generation_size = population / KINSHIP_FIXTURE_GENERATIONS;
        let fixture = BenchmarkKinshipFixture::new(population).unwrap();
        let records = fixture.genealogy.records();

        assert_eq!(records.len(), population as usize);

        let mut link_count = 0usize;
        for (position, record) in records.iter().enumerate() {
            let generation = position as u32 / generation_size;
            if generation == 0 {
                assert_eq!(
                    (record.mother_id, record.father_id),
                    (None, None),
                    "founder {} must have no parents",
                    record.entity_id
                );
                continue;
            }
            let (Some(mother_id), Some(father_id)) = (record.mother_id, record.father_id) else {
                panic!("entity {} must have two parents", record.entity_id);
            };
            // Two *distinct* parents, so the link count is exact and `register`
            // never has to collapse a pair.
            assert_ne!(
                mother_id, father_id,
                "entity {} has the same parent twice",
                record.entity_id
            );
            let previous_start = 1 + (generation - 1) * generation_size;
            let previous_end = previous_start + generation_size;
            assert!(
                (previous_start..previous_end).contains(&mother_id)
                    && (previous_start..previous_end).contains(&father_id),
                "entity {} has parents outside the previous generation",
                record.entity_id
            );
            link_count += 2;
        }

        assert_eq!(
            link_count,
            2 * (population - generation_size) as usize,
            "every non-founder contributes exactly two links"
        );
        // The last generation cannot be anyone's parent.
        for leaf_id in (1 + (KINSHIP_FIXTURE_GENERATIONS - 1) * generation_size)..=population {
            assert!(fixture.genealogy.children_of(leaf_id).is_empty());
        }
    }

    /// Guards against the collapse that made an earlier revision useless: if
    /// ancestry stops mixing, descendant counts stay tiny and no two entities
    /// are ever cousins, and the benchmark quietly measures nothing.
    #[test]
    fn kinship_fixture_lineages_actually_intermarry() {
        let population = 10_000u32;
        let generation_size = population / KINSHIP_FIXTURE_GENERATIONS;
        let fixture = BenchmarkKinshipFixture::new(population).unwrap();

        // A founder's descendants must grow geometrically, not stay flat at two
        // per generation the way the involutive parent rule did.
        let deepest = descendants_of(&fixture.genealogy, 1).len();
        assert!(
            deepest > 2 * KINSHIP_FIXTURE_GENERATIONS as usize,
            "founder 1 has only {deepest} descendants; lineages are not mixing"
        );

        // Most of the last generation must descend from *some* founder in a
        // shared pool, i.e. ancestry converges rather than staying in `n/2`
        // disjoint clans.
        let founders_with_lineage = (1..=generation_size)
            .filter(|&founder_id| descendants_of(&fixture.genealogy, founder_id).len() > 2)
            .count();
        assert!(
            founders_with_lineage > generation_size as usize / 2,
            "only {founders_with_lineage} of {generation_size} founders have grandchildren"
        );
    }

    /// The oracle that #198 could only run against empty results. Here the scan
    /// and the index both return real children, so a disagreement would be a
    /// real bug rather than two ways of computing zero.
    #[test]
    fn kinship_fixture_index_matches_scan_with_real_children() {
        let fixture = BenchmarkKinshipFixture::new(10_000).unwrap();
        let indexed = fixture.children_of_index();
        let scanned = fixture.children_of_scan();

        assert!(indexed > 0, "the fixture must produce real children");
        assert_eq!(
            indexed, scanned,
            "the #198 index disagrees with a record scan"
        );
        // The copy-free path exists only to price the `Vec` in `children_of`, so
        // it has to return the same thing or the comparison is meaningless.
        assert_eq!(indexed, fixture.children_of_index_without_copy());
    }

    /// `descendants` and `relationships` must do real work; if either returned
    /// zero the bench would be measuring nothing, exactly like the short
    /// scenarios do today.
    #[test]
    fn kinship_fixture_walks_real_descendants_and_relationships() {
        let fixture = BenchmarkKinshipFixture::new(10_000).unwrap();

        let descendants = fixture.descendants();
        assert!(
            descendants > 0,
            "a founder must have descendants in a seeded lineage"
        );

        let relationships = fixture.relationships();
        assert!(
            relationships > 0,
            "same-generation pairs must resolve to a real relation, not Unrelated"
        );

        let second = BenchmarkKinshipFixture::new(10_000).unwrap();
        assert_eq!(fixture.descendants(), descendants);
        assert_eq!(second.descendants(), descendants);
        assert_eq!(second.relationships(), relationships);
    }
}
