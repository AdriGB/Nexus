//! Native deterministic benchmark scenarios.
//!
//! This facade deliberately keeps simulation and world internals private. The
//! native runner can list registered scenarios and request one versioned JSON
//! result; construction and execution remain inside the engine crate.

use crate::generation::generate_world;
use crate::pathfinding::{find_path_with_workspace, PathfindingWorkspace};
use crate::regions::detect_regions;
use crate::resources::generate_resources;
use crate::simulation::{PerformanceSummary, Simulation, MAX_POPULATION};
use crate::world::{Grid, Terrain, Tile};
use serde::Serialize;

pub const BENCHMARK_SCHEMA_VERSION: u32 = 1;

#[derive(Clone, Copy, Debug, PartialEq, Serialize)]
pub struct BenchmarkWorld {
    pub width: u32,
    pub height: u32,
    pub sea_level: f64,
}

#[derive(Clone, Copy, Debug, PartialEq, Serialize)]
pub struct BenchmarkScenario {
    pub name: &'static str,
    pub seed: u32,
    pub population: u32,
    pub warmup_ticks: u32,
    pub measured_ticks: u32,
    pub world: BenchmarkWorld,
}

const STANDARD_WORLD: BenchmarkWorld = BenchmarkWorld {
    width: 256,
    height: 256,
    sea_level: 0.35,
};

const SCENARIOS: [BenchmarkScenario; 3] = [
    BenchmarkScenario {
        name: "baseline-100",
        seed: 42,
        population: 100,
        warmup_ticks: 24,
        measured_ticks: 100,
        world: STANDARD_WORLD,
    },
    BenchmarkScenario {
        name: "baseline-1000",
        seed: 42,
        population: 1_000,
        warmup_ticks: 24,
        measured_ticks: 100,
        world: STANDARD_WORLD,
    },
    BenchmarkScenario {
        name: "baseline-10000",
        seed: 42,
        population: 10_000,
        warmup_ticks: 24,
        measured_ticks: 100,
        world: STANDARD_WORLD,
    },
];

#[derive(Serialize)]
struct BenchmarkResult {
    schema_version: u32,
    scenario: BenchmarkScenario,
    summary: PerformanceSummary,
}

pub fn scenario_names() -> impl Iterator<Item = &'static str> {
    SCENARIOS.iter().map(|scenario| scenario.name)
}

pub fn run_scenario_json(name: &str) -> Result<String, String> {
    let scenario = find_scenario(name)?;
    let result = run_scenario(scenario)?;
    serde_json::to_string(&result).map_err(|error| format!("failed to serialize result: {error}"))
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

    for _ in 0..scenario.warmup_ticks {
        simulation.step(&mut world);
    }
    let summary = simulation.profile_run(&mut world, scenario.measured_ticks);
    Ok(BenchmarkResult {
        schema_version: BENCHMARK_SCHEMA_VERSION,
        scenario,
        summary,
    })
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
    };

    #[test]
    fn registered_names_are_unique_and_stable() {
        let names: Vec<_> = scenario_names().collect();
        assert_eq!(
            names,
            vec!["baseline-100", "baseline-1000", "baseline-10000"]
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
