import { requestRender, state } from "./state";
import { uploadSimulationToRenderer } from "./renderer/renderer";
import type { EntityInfo, PopulationStats } from "./types";
import { updateTileInspector } from "./ui/tile-inspector";

const BASE_TICKS_PER_SECOND = 4;
const PERF_DEBUG_MODE = "perf";
const MAX_BENCHMARK_TICKS = 100_000;
const MAX_FRAME_DELTA_SECONDS = 0.25;

export interface SimulationBenchmark {
  ticks: number;
  startTick: number;
  endTick: number;
  population: number;
  totalMs: number;
  msPerTick: number;
  ticksPerSecond: number;
}

interface SimulationPhaseProfile {
  physiology_us: number;
  population_index_us: number;
  autonomy_us: number;
  starvation_us: number;
  resource_changes_us: number;
  remove_dead_us: number;
  pregnancies_us: number;
  conceptions_us: number;
  total_us: number;
}

interface SimulationAutonomyProfile {
  resource_perception_us: number;
  entity_perception_us: number;
  plan_validation_us: number;
  planning_us: number;
  action_us: number;
  sampled_entities: number;
  planned_entities: number;
  urgent_interrupts: number;
}

let speed = 1;
let accumulator = 0;
let previousTimestamp: number | null = null;
let lastWorldRevision = 0n;

export function bindSimulationControls(): void {
  const playButton = document.getElementById("btn-sim-play")!;
  const pauseButton = document.getElementById("btn-sim-pause")!;
  const stepButton = document.getElementById("btn-sim-step")!;
  const speedSelect = document.getElementById(
    "simulation-speed",
  ) as HTMLSelectElement;

  document.getElementById("btn-spawn-10")!.addEventListener("click", () => {
    spawnEntities(10);
  });
  document.getElementById("btn-spawn-100")!.addEventListener("click", () => {
    spawnEntities(100);
  });

  playButton.addEventListener("click", () => {
    state.world?.simulation_resume();
    syncSimulationUi();
  });

  pauseButton.addEventListener("click", () => {
    state.world?.simulation_pause();
    syncSimulationUi();
  });

  stepButton.addEventListener("click", () => {
    if (!state.world) return;
    state.world.simulation_step();
    handleSimulationChange();
  });

  speedSelect.addEventListener("change", () => {
    speed = Number(speedSelect.value) || 1;
    accumulator = 0;
  });

  document.addEventListener("visibilitychange", () => {
    previousTimestamp = null;
    accumulator = 0;
  });

  requestAnimationFrame(runSimulationFrame);
  syncSimulationUi();
  installPerformanceDebug();
}

export function benchmarkSimulation(
  ticks = 1000,
): SimulationBenchmark | null {
  const world = state.world;

  if (
    !world ||
    !Number.isInteger(ticks) ||
    ticks <= 0 ||
    ticks > MAX_BENCHMARK_TICKS
  ) {
    console.warn(
      "benchmarkSimulation: world unavailable or invalid tick count",
    );
    return null;
  }

  const wasPaused = world.simulation_is_paused();
  const startTick = Number(world.simulation_tick());
  const population = world.entity_count();

  if (wasPaused) {
    world.simulation_resume();
  }

  let totalMs: number;

  try {
    const startedAt = performance.now();
    world.simulation_advance(ticks);
    totalMs = performance.now() - startedAt;
  } finally {
    if (wasPaused) {
      world.simulation_pause();
    }
  }

  const endTick = Number(world.simulation_tick());

  const result: SimulationBenchmark = {
    ticks,
    startTick,
    endTick,
    population,
    totalMs,
    msPerTick: totalMs / ticks,
    ticksPerSecond: ticks / (totalMs / 1000),
  };

  handleSimulationChange();

  return result;
}

export function syncSimulationUi(): void {
  const tickElement = document.getElementById("simulation-tick")!;
  const stateElement = document.getElementById("simulation-state")!;
  const paused = state.world?.simulation_is_paused() ?? true;

  tickElement.textContent = state.world
    ? state.world.simulation_tick().toLocaleString()
    : "0";
  stateElement.textContent = paused ? "Paused" : "Running";
  stateElement.classList.toggle("running", !paused);
  document.getElementById("btn-sim-play")?.classList.toggle("active", !paused);
  document.getElementById("btn-sim-pause")?.classList.toggle("active", paused);
  syncPopulationStats();
  syncEntityInspector();
}

export function resetSimulationView(): void {
  lastWorldRevision = state.world?.simulation_world_revision() ?? 0n;
  syncSimulationUi();
}

function runSimulationFrame(timestamp: number): void {
  if (previousTimestamp === null) {
    previousTimestamp = timestamp;
  }

  const deltaSeconds = Math.min(
    (timestamp - previousTimestamp) / 1_000,
    MAX_FRAME_DELTA_SECONDS,
  );
  previousTimestamp = timestamp;

  if (state.world && !state.world.simulation_is_paused()) {
    accumulator += deltaSeconds * BASE_TICKS_PER_SECOND * speed;
    const ticks = Math.floor(accumulator);
    if (ticks > 0) {
      accumulator -= ticks;
      state.world.simulation_advance(ticks);
      handleSimulationChange();
    }
  }

  requestAnimationFrame(runSimulationFrame);
}

function handleSimulationChange(): void {
  if (!state.world) return;
  const revision = state.world.simulation_world_revision();
  const resourcesChanged = revision !== lastWorldRevision;
  lastWorldRevision = revision;
  uploadSimulationToRenderer(resourcesChanged);
  if (resourcesChanged) {
    updateTileInspector();
  }
  syncSimulationUi();
  requestRender();
}

function spawnEntities(count: number): void {
  if (!state.world) return;
  state.world.spawn_entities(count);
  handleSimulationChange();
}

function syncPopulationStats(): void {
  if (!state.world) return;
  const stats: PopulationStats = JSON.parse(state.world.population_stats());
  const values: Record<string, string> = {
    "population-count": stats.population.toLocaleString(),
    "population-females": stats.females.toLocaleString(),
    "population-males": stats.males.toLocaleString(),
    "population-pregnant": stats.pregnant.toLocaleString(),
    "population-births": stats.births.toLocaleString(),
    "population-deaths": stats.deaths.toLocaleString(),
    "population-hungry": stats.hungry.toLocaleString(),
    "population-seeking": stats.seeking_food.toLocaleString(),
    "population-average-hunger": `${stats.average_hunger.toFixed(1)}%`,
    "population-food-consumed": stats.food_consumed.toLocaleString(),
  };
  for (const [id, value] of Object.entries(values)) {
    document.getElementById(id)!.textContent = value;
  }
}

function syncEntityInspector(): void {
  const panel = document.getElementById("entity-inspector")!;
  const grid = document.getElementById("entity-info-grid")!;
  if (!state.world || state.world.entity_count() === 0) {
    panel.hidden = true;
    grid.innerHTML = "";
    return;
  }

  const entity: EntityInfo = JSON.parse(state.world.first_entity_info());
  const dueInHours = entity.pregnancy_due_tick === null
    ? null
    : Math.max(0, entity.pregnancy_due_tick - Number(state.world.simulation_tick()));
  panel.hidden = false;
  grid.innerHTML = [
    `<span class="info-key">ID</span><span class="info-val">#${entity.id}</span>`,
    `<span class="info-key">Sex</span><span class="info-val">${entity.sex}</span>`,
    `<span class="info-key">Position</span><span class="info-val">(${entity.x}, ${entity.y})</span>`,
    `<span class="info-key">Hunger</span><span class="info-val">${entity.hunger.toFixed(0)} / 100</span>`,
    `<span class="info-key">Health</span><span class="info-val">${entity.health.toFixed(0)} / 100</span>`,
    `<span class="info-key">Age</span><span class="info-val">${entity.age_years.toFixed(1)} years</span>`,
    `<span class="info-key">Age ticks</span><span class="info-val">${entity.age_ticks.toLocaleString()}</span>`,
    `<span class="info-key">Lifespan</span><span class="info-val">${entity.lifespan_ticks.toLocaleString()} ticks</span>`,
    `<span class="info-key">Pregnant</span><span class="info-val">${entity.pregnant ? "Yes" : "No"}</span>`,
    ...(dueInHours === null
      ? []
      : [`<span class="info-key">Due</span><span class="info-val">${dueInHours.toLocaleString()} hours</span>`]),
    `<span class="info-key">Activity</span><span class="info-val entity-activity">${entity.activity}</span>`,
    `<span class="info-key">Goal</span><span class="info-val entity-goal">${entity.goal}</span>`,
    `<span class="info-key">Action</span><span class="info-val">${entity.action}</span>`,
    `<span class="info-key">Goal retained</span><span class="info-val">${entity.goal_age_ticks.toLocaleString()} ticks</span>`,
    `<span class="info-key">Path remaining</span><span class="info-val">${entity.remaining_path}</span>`,
    `<span class="info-key">Known resources</span><span class="info-val">${entity.known_resources}</span>`,
    `<span class="info-key">Known chunks</span><span class="info-val">${entity.known_chunks}</span>`,
    `<span class="info-key">Visible entities</span><span class="info-val">${entity.visible_entities}</span>`,
    `<span class="info-key">Utility: eat</span><span class="info-val">${entity.utilities.eat.toFixed(2)}</span>`,
    `<span class="info-key">Utility: explore</span><span class="info-val">${entity.utilities.explore.toFixed(2)}</span>`,
    `<span class="info-key">Utility: rest</span><span class="info-val">${entity.utilities.rest.toFixed(2)}</span>`,
  ].join("");
}

declare global {
  interface Window {
    nexusBenchmark?: (ticks?: number) => SimulationBenchmark | null;
    nexusProfile?: () => SimulationPhaseProfile | null;
    nexusProfileAutonomy?: () => SimulationAutonomyProfile | null;
  }
}

function installPerformanceDebug(): void {
  const debugMode = new URLSearchParams(window.location.search).get("debug");

  if (debugMode !== PERF_DEBUG_MODE) {
    return;
  }

  window.nexusBenchmark = (ticks = 1000) => {
    const result = benchmarkSimulation(ticks);

    if (result) {
      console.table(result);
    }

    return result;
  };

  window.nexusProfile = () => {
    const world = state.world;

    if (!world) {
      console.warn("No world loaded");
      return null;
    }

    const profiledWorld = world as typeof world & {
      simulation_profile_step(): string;
    };
    const profile = JSON.parse(
      profiledWorld.simulation_profile_step(),
    ) as SimulationPhaseProfile;

    handleSimulationChange();

    const totalUs = Math.max(profile.total_us, 1);
    const measuredUs =
      profile.physiology_us +
      profile.population_index_us +
      profile.autonomy_us +
      profile.starvation_us +
      profile.resource_changes_us +
      profile.remove_dead_us +
      profile.pregnancies_us +
      profile.conceptions_us;
    const unaccountedUs = Math.max(0, profile.total_us - measuredUs);

    const rows = Object.entries(profile).map(([phase, value]) => ({
      phase,
      ms: (value / 1000).toFixed(3),
      percent: phase === "total_us"
        ? "100.0%"
        : `${((value / totalUs) * 100).toFixed(1)}%`,
    }));

    rows.splice(rows.length - 1, 0, {
      phase: "unaccounted_us",
      ms: (unaccountedUs / 1000).toFixed(3),
      percent: `${((unaccountedUs / totalUs) * 100).toFixed(1)}%`,
    });

    console.table(rows);
    return profile;
  };

  window.nexusProfileAutonomy = () => {
    const world = state.world;

    if (!world) {
      console.warn("No world loaded");
      return null;
    }

    const profiledWorld = world as typeof world & {
      simulation_profile_autonomy_step(): string;
    };
    const profile = JSON.parse(
      profiledWorld.simulation_profile_autonomy_step(),
    ) as SimulationAutonomyProfile;

    handleSimulationChange();

    const totalUs =
      profile.resource_perception_us +
      profile.entity_perception_us +
      profile.plan_validation_us +
      profile.planning_us +
      profile.action_us;
    const totalSafe = Math.max(totalUs, 1);

    const rows = [
      {
        phase: "resource_perception",
        ms: (profile.resource_perception_us / 1000).toFixed(3),
        percent: `${((profile.resource_perception_us / totalSafe) * 100).toFixed(1)}%`,
      },
      {
        phase: "entity_perception",
        ms: (profile.entity_perception_us / 1000).toFixed(3),
        percent: `${((profile.entity_perception_us / totalSafe) * 100).toFixed(1)}%`,
      },
      {
        phase: "plan_validation",
        ms: (profile.plan_validation_us / 1000).toFixed(3),
        percent: `${((profile.plan_validation_us / totalSafe) * 100).toFixed(1)}%`,
      },
      {
        phase: "planning",
        ms: (profile.planning_us / 1000).toFixed(3),
        percent: `${((profile.planning_us / totalSafe) * 100).toFixed(1)}%`,
      },
      {
        phase: "action",
        ms: (profile.action_us / 1000).toFixed(3),
        percent: `${((profile.action_us / totalSafe) * 100).toFixed(1)}%`,
      },
      { phase: "---", ms: "---", percent: "---" },
      {
        phase: "sampled_entities",
        ms: String(profile.sampled_entities),
        percent: "",
      },
      {
        phase: "planned_entities",
        ms: String(profile.planned_entities),
        percent: "",
      },
      {
        phase: "urgent_interrupts",
        ms: String(profile.urgent_interrupts),
        percent: "",
      },
    ];

    console.table(rows);
    return profile;
  };

  console.info("Performance benchmark enabled. Use: nexusBenchmark(ticks)");
}
