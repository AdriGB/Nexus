import { state } from "../state";
import type { IGpuRendererBridge } from "../types";
import { createGpuRenderer } from "../wasm";
import {
  render as renderCanvas,
  resizeCanvas,
} from "./world-renderer";

export type RendererBackend = "canvas" | "wgpu";

const canvas2d = document.getElementById(
  "world-canvas",
) as HTMLCanvasElement;
const gpuCanvas = document.getElementById(
  "world-gpu-canvas",
) as HTMLCanvasElement;
const rendererStatus = document.getElementById("st-renderer")!;
const rendererDebug = document.getElementById("renderer-debug")!;
const rendererDebugBackend = document.getElementById(
  "debug-renderer-backend",
)!;
const rendererDebugFrame = document.getElementById("debug-renderer-frame")!;
const rendererDebugWorld = document.getElementById("debug-renderer-world")!;
const rendererDebugZoom = document.getElementById("debug-renderer-zoom")!;

const searchParams = new URLSearchParams(window.location.search);
const debugEnabled = searchParams.get("debug") === "renderer";

let backend: RendererBackend = "canvas";
let gpuRenderer: IGpuRendererBridge | null = null;

export async function initializeRenderer(): Promise<void> {
  rendererDebug.hidden = !debugEnabled;
  const requested = searchParams.get("renderer");

  if (requested === "canvas") {
    fallbackToCanvas("Canvas 2D forced by ?renderer=canvas");
    return;
  }

  if ("gpu" in navigator) {
    try {
      sizeGpuCanvas();
      gpuRenderer = await createGpuRenderer(gpuCanvas.id);
      backend = "wgpu";
      activateBackend();
      resizeRenderer();
      console.info("✓ wgpu renderer initialized successfully");
      return;
    } catch (error) {
      console.warn(
        "WebGPU initialization failed; falling back to Canvas 2D.",
        error,
      );
    }
  } else {
    console.info("WebGPU is not supported; falling back to Canvas 2D.");
  }

  fallbackToCanvas();
}

export function resizeRenderer(): void {
  resizeCanvas();
  if (backend === "wgpu" && gpuRenderer) {
    const { width, height, dpr } = sizeGpuCanvas();
    gpuRenderer.resize(width, height, dpr);
  }
}

export function uploadWorldToRenderer(): void {
  if (backend === "wgpu" && gpuRenderer && state.world) {
    gpuRenderer.upload_world(state.world);
  }
}

export function renderWorld(): void {
  const frameStartedAt = debugEnabled ? performance.now() : 0;

  if (backend === "wgpu" && gpuRenderer) {
    try {
      gpuRenderer.render(
        state.panX,
        state.panY,
        state.zoom,
        state.hoverTile?.x ?? -1,
        state.hoverTile?.y ?? -1,
        state.selectedTile?.x ?? -1,
        state.selectedTile?.y ?? -1,
        state.showGrid,
      );
    } catch (error) {
      console.warn("WebGPU rendering failed; using Canvas 2D.", error);
      fallbackToCanvas();
      renderCanvas();
    }
  } else {
    renderCanvas();
  }

  const zoomEl = document.getElementById("st-zoom");
  if (zoomEl) {
    zoomEl.textContent = Math.round(state.zoom * 100) + "%";
  }

  if (debugEnabled) {
    updateDebugTelemetry(performance.now() - frameStartedAt);
  }
}

export function getRendererBackend(): RendererBackend {
  return backend;
}

function fallbackToCanvas(reason?: string): void {
  try {
    gpuRenderer?.free();
  } catch (_) {
    // The WebGPU device may already be lost.
  }
  gpuRenderer = null;
  backend = "canvas";
  activateBackend();
  resizeCanvas();
  console.info(reason ?? "✓ Canvas 2D renderer active");
}

function activateBackend(): void {
  canvas2d.hidden = backend !== "canvas";
  gpuCanvas.hidden = backend !== "wgpu";
  rendererStatus.textContent = backend === "wgpu" ? "wgpu" : "Canvas 2D";
}

function updateDebugTelemetry(frameMs: number): void {
  rendererDebugBackend.textContent = backend;
  rendererDebugFrame.textContent = `${frameMs.toFixed(2)} ms`;
  rendererDebugWorld.textContent = `${state.worldW}×${state.worldH}`;
  rendererDebugZoom.textContent = `${Math.round(state.zoom * 100)}%`;
}

function sizeGpuCanvas(): {
  width: number;
  height: number;
  dpr: number;
} {
  const rect = gpuCanvas.parentElement!.getBoundingClientRect();
  const dpr = window.devicePixelRatio || 1;
  state.cssW = rect.width;
  state.cssH = rect.height;
  const width = Math.max(1, Math.round(rect.width * dpr));
  const height = Math.max(1, Math.round(rect.height * dpr));
  gpuCanvas.width = width;
  gpuCanvas.height = height;
  gpuCanvas.style.width = rect.width + "px";
  gpuCanvas.style.height = rect.height + "px";
  return { width, height, dpr };
}
