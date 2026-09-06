export interface WorldConfig {
  seed: number;
  width: number;
  height: number;
  seaLevel: number;
}

export interface WorldSaveV1 {
  formatVersion: 1;
  generatorVersion: string;
  name: string;
  createdAt: string;
  config: WorldConfig;
}

export const GENERATOR_VERSION = "0.1.0";
export const SNAPSHOT_FORMAT = "nexus-snapshot/v1";

export interface SnapshotHeaderV1 {
  format: string;
  engine_version: string;
  hash_version: number;
  created_at_tick: number;
  paused: boolean;
  simulation_seed: number;
  world_seed?: number;
  sea_level?: number;
  world_width: number;
  world_height: number;
  state_hash: string;
}

export interface SavedSnapshotRecordMeta {
  id: string;
  name: string;
  createdAt: string;
  tick: number;
  population: number;
  households: number;
  stateHash: string;
  worldWidth: number;
  worldHeight: number;
  worldSeed?: number;
  seaLevel?: number;
}

export interface SavedSnapshotRecord extends SavedSnapshotRecordMeta {
  snapshotJson: string;
}

export function createSave(
  name: string,
  config: WorldConfig,
): WorldSaveV1 {
  return {
    formatVersion: 1,
    generatorVersion: GENERATOR_VERSION,
    name: name.trim() || "Unnamed World",
    createdAt: new Date().toISOString(),
    config,
  };
}

function isIntegerInRange(
  value: unknown,
  min: number,
  max: number,
): value is number {
  return (
    typeof value === "number" &&
    Number.isFinite(value) &&
    Number.isInteger(value) &&
    value >= min &&
    value <= max
  );
}

function isFloatInRange(
  value: unknown,
  min: number,
  max: number,
): value is number {
  return (
    typeof value === "number" &&
    Number.isFinite(value) &&
    value >= min &&
    value <= max
  );
}

export function validateSave(data: unknown): WorldSaveV1 | null {
  if (!data || typeof data !== "object") return null;

  const d = data as Record<string, unknown>;
  if (d.formatVersion !== 1) return null;

  const cfg = d.config as Record<string, unknown> | undefined;
  if (!cfg) return null;

  if (!isIntegerInRange(cfg.seed, 0, 4294967295)) return null;
  if (!isIntegerInRange(cfg.width, 64, 1024)) return null;
  if (!isIntegerInRange(cfg.height, 64, 1024)) return null;
  if (!isFloatInRange(cfg.seaLevel, 0.05, 0.8)) return null;

  return {
    formatVersion: 1,
    generatorVersion:
      typeof d.generatorVersion === "string"
        ? d.generatorVersion
        : "unknown",
    name:
      typeof d.name === "string" ? d.name : "Unnamed World",
    createdAt:
      typeof d.createdAt === "string"
        ? d.createdAt
        : new Date().toISOString(),
    config: {
      seed: cfg.seed,
      width: cfg.width,
      height: cfg.height,
      seaLevel: cfg.seaLevel,
    },
  };
}

export function detectSaveFormat(data: unknown): "snapshot" | "legacy_config" | "invalid" {
  if (!data || typeof data !== "object") return "invalid";
  const obj = data as Record<string, unknown>;

  if (
    obj.header &&
    typeof obj.header === "object" &&
    (obj.header as Record<string, unknown>).format === SNAPSHOT_FORMAT
  ) {
    return "snapshot";
  }

  if (obj.formatVersion === 1 && obj.config && typeof obj.config === "object") {
    return "legacy_config";
  }

  return "invalid";
}

export function parseSnapshotMeta(
  json: string,
  customName?: string,
  customId?: string,
): SavedSnapshotRecordMeta | null {
  try {
    const data = JSON.parse(json);
    if (detectSaveFormat(data) !== "snapshot") return null;

    const header = data.header as SnapshotHeaderV1;
    const sim = (data.simulation || {}) as Record<string, unknown>;
    const scalars = (sim.scalars || {}) as Record<string, unknown>;

    const tick = typeof header.created_at_tick === "number"
      ? header.created_at_tick
      : (typeof scalars.tick === "number" ? scalars.tick : 0);

    const population = Array.isArray(sim.entities) ? sim.entities.length : 0;
    const households = Array.isArray(sim.households) ? sim.households.length : 0;

    const id = customId || `snapshot_${Date.now()}_${Math.random().toString(36).slice(2, 7)}`;
    const name = customName?.trim() || `World (Tick ${tick})`;

    return {
      id,
      name,
      createdAt: new Date().toISOString(),
      tick,
      population,
      households,
      stateHash: header.state_hash || "",
      worldWidth: header.world_width || 0,
      worldHeight: header.world_height || 0,
      worldSeed: header.world_seed,
      seaLevel: header.sea_level,
    };
  } catch {
    return null;
  }
}
