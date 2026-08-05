import init, { WorldBridge } from "./wasm/nexus_engine.js";
import type { IWorldBridge } from "./types";

let ready = false;

export async function loadWasm(): Promise<void> {
  await init();
  ready = true;
}

export function createWorld(
  seed: number,
  width: number,
  height: number,
  seaLevel: number,
): IWorldBridge {
  if (!ready) {
    throw new Error("WASM engine not loaded");
  }
  return new WorldBridge(seed, width, height, seaLevel);
}
