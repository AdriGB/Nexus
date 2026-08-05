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

export function createSave(name: string, config: WorldConfig): WorldSaveV1 {
  return {
    formatVersion: 1,
    generatorVersion: GENERATOR_VERSION,
    name: name.trim() || "Unnamed World",
    createdAt: new Date().toISOString(),
    config,
  };
}

export function validateSave(data: unknown): WorldSaveV1 | null {
  if (!data || typeof data !== "object") return null;

  const d = data as Record<string, unknown>;
  if (d.formatVersion !== 1) return null;

  const cfg = d.config as Record<string, unknown> | undefined;
  if (!cfg) return null;

  if (typeof cfg.seed !== "number" || cfg.seed < 0 || cfg.seed > 4294967295) return null;
  if (typeof cfg.width !== "number" || cfg.width < 64 || cfg.width > 1024) return null;
  if (typeof cfg.height !== "number" || cfg.height < 64 || cfg.height > 1024) return null;
  if (typeof cfg.seaLevel !== "number" || cfg.seaLevel < 0.05 || cfg.seaLevel > 0.8) return null;

  return {
    formatVersion: 1,
    generatorVersion: typeof d.generatorVersion === "string" ? d.generatorVersion : "unknown",
    name: typeof d.name === "string" ? d.name : "Unnamed World",
    createdAt: typeof d.createdAt === "string" ? d.createdAt : new Date().toISOString(),
    config: {
      seed: cfg.seed,
      width: cfg.width,
      height: cfg.height,
      seaLevel: cfg.seaLevel,
    },
  };
}
