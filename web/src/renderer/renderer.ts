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

let backend: RendererBackend = "canvas";
let gpuRenderer: IGpuRendererBridge | null = null;

export async function initializeRenderer(): Promise<void> {
  const requested = new URLSearchParams(window.location.search).get(
    "renderer",
  );

  if (requested === "wgpu") {
    try {
      sizeGpuCanvas();
      gpuRenderer = await createGpuRenderer(gpuCanvas.id);
      backend = "wgpu";
      activateBackend();
      resizeRenderer();
      return;
    } catch (error) {
      console.warn("WebGPU unavailable; using Canvas 2D.", error);
    }
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
}

export function getRendererBackend(): RendererBackend {
  return backend;
}

function fallbackToCanvas(): void {
  try {
    gpuRenderer?.free();
  } catch (_) {
    // The WebGPU device may already be lost.
  }
  gpuRenderer = null;
  backend = "canvas";
  activateBackend();
  resizeCanvas();
}

function activateBackend(): void {
  canvas2d.hidden = backend !== "canvas";
  gpuCanvas.hidden = backend !== "wgpu";
  rendererStatus.textContent = backend === "wgpu" ? "wgpu" : "Canvas 2D";
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
