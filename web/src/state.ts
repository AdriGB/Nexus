import type { IWorldBridge, TileCoord } from "./types";

export interface AppState {
  world: IWorldBridge | null;
  worldW: number;
  worldH: number;
  panX: number;
  panY: number;
  zoom: number;
  cssW: number;
  cssH: number;
  showGrid: boolean;
  hoverTile: TileCoord | null;
  selectedTile: TileCoord | null;
  minimapImageData: ImageData | null;
}

export const state: AppState = {
  world: null,
  worldW: 0,
  worldH: 0,
  panX: 0,
  panY: 0,
  zoom: 1,
  cssW: 0,
  cssH: 0,
  showGrid: false,
  hoverTile: null,
  selectedTile: null,
  minimapImageData: null,
};

/* ── Render-on-demand queue ───────────────── */

let renderPending = false;
let renderCallback: (() => void) | null = null;

export function setRenderCallback(cb: () => void): void {
  renderCallback = cb;
}

export function requestRender(): void {
  if (renderPending) return;
  renderPending = true;
  requestAnimationFrame(() => {
    renderPending = false;
    renderCallback?.();
  });
}
