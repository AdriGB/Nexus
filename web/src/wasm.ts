import type { IWorldBridge, WorldBridgeConstructor } from "./types";

let Ctor: WorldBridgeConstructor | null = null;

export async function loadWasm(): Promise<void> {
  // FIX #1: Variable avoids TypeScript module resolution error
  // FIX #10: BASE_URL makes it work on GitHub Pages subdirectory
  const wasmUrl = `${import.meta.env.BASE_URL}wasm/nexus_engine.js`;

  const mod = (await import(
    /* @vite-ignore */ wasmUrl
  )) as {
    default: () => Promise<void>;
    WorldBridge: WorldBridgeConstructor;
  };

  await mod.default();
  Ctor = mod.WorldBridge;
}

export function createWorld(
  seed: number,
  width: number,
  height: number,
  seaLevel: number,
): IWorldBridge {
  if (!Ctor) throw new Error("WASM engine not loaded");
  return new Ctor(seed, width, height, seaLevel);
}
