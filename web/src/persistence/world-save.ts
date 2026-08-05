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
