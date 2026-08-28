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
    AutonomyProfile, PerformanceRun, PerformanceSummary, Simulation, MAX_POPULATION,
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

const SCENARIOS: [BenchmarkScenario; 8] = [
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
/// `resource_perception_us` is intentionally absent from this struct: the
/// engine defines it as `memory_reconciliation_us + visible_scan_us`, so
/// reporting it alongside its own components double counts.
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
        totals.step_total_us += profile.step_total_us;
        totals.entity_perception_us += profile.entity_perception_us;
        totals.visible_scan_us += profile.visible_scan_us;
        totals.memory_reconciliation_us += profile.memory_reconciliation_us;
        totals.plan_validation_us += profile.plan_validation_us;
        totals.planning_us += profile.planning_us;
        totals.action_us += profile.action_us;
        totals.social_us += profile.social_us;
        totals.entity_pass_us += profile.entity_pass_us;
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
        entity_pass_us: mean(totals.entity_pass_us),
        attributed_passes_us: mean(totals.social_us) + mean(totals.entity_pass_us),
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
    Ok(LongRunResult {
        schema_version: BENCHMARK_SCHEMA_VERSION,
        scenario,
        overall: overall.summarize(),
        windows,
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
}
