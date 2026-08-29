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

// `work` and `state` arrive flattened into the parent object, so the profiles
// extend these rather than nest them. Keeping the fields in one place stops the
// two profiles from drifting apart the way the Rust DTO did in #178 and #194.
interface SimulationStateGauges {
  entities_alive: number;
  known_entities_total: number;
  known_entities_max_per_entity: number;
  known_resources_total: number;
  known_resources_max_per_entity: number;
  known_dead_entities_total: number;
  active_grief_states: number;
  recent_events_len: number;
  recent_events_capacity: number;
  households_active: number;
  genealogy_links: number;
}

interface SimulationWorkCounters {
  entities_processed: number;
  entities_perceived: number;
  goal_evaluations: number;
  goal_changes: number;
  plans_created: number;
  actions_executed: number;
  social_interactions: number;
  social_pairs_scanned: number;
  social_pairs_in_radius: number;
  social_pairs_mutual: number;
  social_pairs_due: number;
  encounters_recorded: number;
  discoveries_recorded: number;
  food_consumptions_recorded: number;
  food_share_attempts: number;
  household_deposit_attempts: number;
  household_withdraw_attempts: number;
  household_conflict_attempts: number;
  spatial_queries: number;
  pathfinding_searches: number;
  pathfinding_nodes_expanded: number;
  events_created: number;
  orphan_reassignment_scans: number;
  household_sync_scans: number;
  household_migration_scans: number;
  conception_scans: number;
}

interface SimulationPostPassProfile {
  resource_discoveries_us: number;
  entity_encounters_us: number;
  food_consumptions_us: number;
  social_interactions_us: number;
  food_share_us: number;
  household_deposit_us: number;
  household_withdraw_us: number;
  household_conflict_us: number;
  total_us: number;
}

/**
 * The per-entity loop, decomposed. Full population (#207).
 *
 * `attributed_us` is the sum of the four blocks; `residual_us` is what is left
 * of `entity_pass_us` after subtracting them. Both come from the engine rather
 * than being recomputed here, so there is one definition of the residual.
 */
interface SimulationEntityPassBreakdown {
  perceive_entities_us: number;
  plan_validation_us: number;
  planning_us: number;
  resource_memory_us: number;
  attributed_us: number;
  residual_us: number;
}

interface SimulationPhaseProfile
  extends SimulationStateGauges,
    SimulationWorkCounters {
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

/**
 * Two populations are timed and the names say which is which.
 *
 * `social_us` / `entity_pass_us` are one timer around one whole pass, so they
 * cover every entity. Everything prefixed `sampled_` is timed per entity over a
 * 1-in-N sample. Adding a sampled value to a full-population one yields a number
 * that means nothing — that is what this panel did before #191. Compare each
 * group against its own denominator and never across groups.
 */
interface SimulationAutonomyProfile
  extends SimulationStateGauges,
    SimulationWorkCounters {
  /** Full population: one timer around the social pass. */
  social_us: number;
  /** Full population: one timer around the per-entity loop. */
  entity_pass_us: number;
  /** The same loop, decomposed. Full population. Denominator is `entity_pass_us`. */
  entity_pass: SimulationEntityPassBreakdown;
  /** `social_us + entity_pass_us`. The only total that mixes no sampled value. */
  attributed_passes_us: number;
  /** Wall clock of the profiled step, the denominator for the fields above. */
  step_total_us: number;
  /** Sampled, per entity. Do not add to the full-population fields. */
  sampled_entity_perception_us: number;
  sampled_plan_validation_us: number;
  sampled_planning_us: number;
  sampled_action_us: number;
  sampled_memory_reconciliation_us: number;
  sampled_visible_scan_us: number;
  sampled_entities: number;
  planned_entities: number;
  urgent_interrupts: number;
  sampled_known_resources_total: number;
  sampled_known_resources_max: number;
  visible_resources_seen: number;
  post_pass: SimulationPostPassProfile;
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

    // Two populations, two denominators. The sampled sub-phases cover part of
    // the entity loop; social_us and entity_pass_us cover all of it. Summing
    // across the two produced meaningless percentages and double counted the
    // resource_perception rollup alongside its own components (#191).
    const sampledUs =
      profile.sampled_entity_perception_us +
      profile.sampled_plan_validation_us +
      profile.sampled_planning_us +
      profile.sampled_action_us +
      profile.sampled_memory_reconciliation_us +
      profile.sampled_visible_scan_us;
    const sampledSafe = Math.max(sampledUs, 1);
    const stepSafe = Math.max(profile.step_total_us, 1);
    // A third denominator: the four blocks below decompose entity_pass_us, so
    // that is what they are percentages of. Not step_total, not the sampled sum.
    const entityPassSafe = Math.max(profile.entity_pass_us, 1);

    const rows = [
      {
        phase: "step_total (denominator)",
        ms: (profile.step_total_us / 1000).toFixed(3),
        percent: "100.0%",
      },
      {
        phase: "social (all entities)",
        ms: (profile.social_us / 1000).toFixed(3),
        percent: `${((profile.social_us / stepSafe) * 100).toFixed(1)}%`,
      },
      {
        phase: "entity_pass (all entities)",
        ms: (profile.entity_pass_us / 1000).toFixed(3),
        percent: `${((profile.entity_pass_us / stepSafe) * 100).toFixed(1)}%`,
      },
      {
        phase: "attributed (social + entity_pass)",
        ms: (profile.attributed_passes_us / 1000).toFixed(3),
        percent: `${((profile.attributed_passes_us / stepSafe) * 100).toFixed(1)}%`,
      },
      {
        phase: "--- entity_pass decomposed, all entities ---",
        ms: "---",
        percent: "---",
      },
      {
        phase: "perceive_entities",
        ms: (profile.entity_pass.perceive_entities_us / 1000).toFixed(3),
        percent: `${(
          (profile.entity_pass.perceive_entities_us / entityPassSafe) *
          100
        ).toFixed(1)}%`,
      },
      {
        phase: "plan_validation",
        ms: (profile.entity_pass.plan_validation_us / 1000).toFixed(3),
        percent: `${(
          (profile.entity_pass.plan_validation_us / entityPassSafe) *
          100
        ).toFixed(1)}%`,
      },
      {
        phase: "planning",
        ms: (profile.entity_pass.planning_us / 1000).toFixed(3),
        percent: `${((profile.entity_pass.planning_us / entityPassSafe) * 100).toFixed(1)}%`,
      },
      {
        phase: "resource_memory",
        ms: (profile.entity_pass.resource_memory_us / 1000).toFixed(3),
        percent: `${(
          (profile.entity_pass.resource_memory_us / entityPassSafe) *
          100
        ).toFixed(1)}%`,
      },
      {
        phase: "residual (untimed blocks + timers)",
        ms: (profile.entity_pass.residual_us / 1000).toFixed(3),
        percent: `${((profile.entity_pass.residual_us / entityPassSafe) * 100).toFixed(1)}%`,
      },
      {
        phase: "--- sampled, 1-in-N entities ---",
        ms: "---",
        percent: "---",
      },
      {
        phase: "entity_perception",
        ms: (profile.sampled_entity_perception_us / 1000).toFixed(3),
        percent: `${((profile.sampled_entity_perception_us / sampledSafe) * 100).toFixed(1)}%`,
      },
      {
        phase: "plan_validation",
        ms: (profile.sampled_plan_validation_us / 1000).toFixed(3),
        percent: `${((profile.sampled_plan_validation_us / sampledSafe) * 100).toFixed(1)}%`,
      },
      {
        phase: "planning",
        ms: (profile.sampled_planning_us / 1000).toFixed(3),
        percent: `${((profile.sampled_planning_us / sampledSafe) * 100).toFixed(1)}%`,
      },
      {
        phase: "action",
        ms: (profile.sampled_action_us / 1000).toFixed(3),
        percent: `${((profile.sampled_action_us / sampledSafe) * 100).toFixed(1)}%`,
      },
      {
        phase: "memory_reconciliation",
        ms: (profile.sampled_memory_reconciliation_us / 1000).toFixed(3),
        percent: `${((profile.sampled_memory_reconciliation_us / sampledSafe) * 100).toFixed(1)}%`,
      },
      {
        phase: "visible_scan",
        ms: (profile.sampled_visible_scan_us / 1000).toFixed(3),
        percent: `${((profile.sampled_visible_scan_us / sampledSafe) * 100).toFixed(1)}%`,
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
          profile.sampled_known_resources_total /
          Math.max(profile.sampled_entities, 1)
        ).toFixed(1),
        percent: "",
      },
      {
        phase: "max_known_resources",
        ms: String(profile.sampled_known_resources_max),
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
