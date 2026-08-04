import type { IWorldBridge, WorldBridgeConstructor } from "./types";

let Ctor: WorldBridgeConstructor | null = null;

export async function loadWasm(): Promise<void> {
  const mod = (await import(
    /* @vite-ignore */ "/wasm/nexus_engine.js"
  )) as { default: () => Promise<void>; WorldBridge: WorldBridgeConstructor };
  await mod.default();
  Ctor = mod.WorldBridge;
}

export function createWorld(
  seed: number,
  width: number,
  height: number,
  seaLevel: number
): IWorldBridge {
  if (!Ctor) throw new Error("WASM engine not loaded");
  return new Ctor(seed, width, height, seaLevel);
}
