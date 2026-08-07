import init, {
  GpuRenderer,
  WorldBridge,
} from "./wasm/nexus_engine.js";
import type { IGpuRendererBridge, IWorldBridge } from "./types";

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

export async function createGpuRenderer(
  canvasId: string,
): Promise<IGpuRendererBridge> {
  if (!ready) {
    throw new Error("WASM engine not loaded");
  }
  return GpuRenderer.create(canvasId);
}
