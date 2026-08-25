import { handleSimulationChange } from "./simulation";
import { state } from "./state";

const PERF_DEBUG_MODE = "perf";
const MAX_BENCHMARK_TICKS = 100_000;

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
  world_maintenance_us: number;
  physiology_us: number;
  dependent_care_us: number;
  households_us: number;
  spatial_index_us: number;
  autonomy_us: number;
  survival_us: number;
  mortality_us: number;
  lifecycle_us: number;
  relationships_us: number;
  reproduction_us: number;
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
  memory_reconciliation_us: number;
  visible_scan_us: number;
  known_resources_total: number;
  known_resources_max: number;
  visible_resources_seen: number;
  social_us: number;
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

declare global {
  interface Window {
    nexusBenchmark?: (ticks?: number) => SimulationBenchmark | null;
    nexusProfile?: () => SimulationPhaseProfile | null;
    nexusProfileAutonomy?: () => SimulationAutonomyProfile | null;
  }
}

export function installPerformanceDebug(): void {
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
      profile.world_maintenance_us +
      profile.physiology_us +
      profile.dependent_care_us +
      profile.households_us +
      profile.spatial_index_us +
      profile.autonomy_us +
      profile.survival_us +
      profile.mortality_us +
      profile.lifecycle_us +
      profile.relationships_us +
      profile.reproduction_us;
    const unaccountedUs = Math.max(0, profile.total_us - measuredUs);

    const rows = Object.entries(profile).map(([phase, value]) => ({
      phase,
      ms: (value / 1000).toFixed(3),
      percent:
        phase === "total_us"
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
      profile.action_us +
      profile.social_us;
    const totalSafe = Math.max(totalUs, 1);

    const rows = [
      {
        phase: "memory_reconciliation",
        ms: (profile.memory_reconciliation_us / 1000).toFixed(3),
        percent: `${((profile.memory_reconciliation_us / totalSafe) * 100).toFixed(1)}%`,
      },
      {
        phase: "visible_scan",
        ms: (profile.visible_scan_us / 1000).toFixed(3),
        percent: `${((profile.visible_scan_us / totalSafe) * 100).toFixed(1)}%`,
      },
      {
        phase: "resource_perception (sum)",
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
      {
        phase: "social",
        ms: (profile.social_us / 1000).toFixed(3),
        percent: `${((profile.social_us / totalSafe) * 100).toFixed(1)}%`,
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
      {
        phase: "avg_known_resources",
        ms: (
          profile.known_resources_total /
          Math.max(profile.sampled_entities, 1)
        ).toFixed(1),
        percent: "",
      },
      {
        phase: "max_known_resources",
        ms: String(profile.known_resources_max),
        percent: "",
      },
      {
        phase: "visible_resources_seen",
        ms: String(profile.visible_resources_seen),
        percent: "",
      },
    ];

    console.table(rows);
    return profile;
  };

  console.info("Performance benchmark enabled. Use: nexusBenchmark(ticks)");
}
