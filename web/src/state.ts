import type { IWorldBridge, TileCoord } from "./types";

export type SidebarTab = "world" | "simulation" | "inspector" | "events" | "saves";
export type ToolMode = "inspect" | "path";

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
  renderMode: "terrain" | "resources";
  hoverTile: TileCoord | null;
  selectedTile: TileCoord | null;
  selectedEntityId: number | null;
  cameraFollowEntityId: number | null;
  activeSidebarTab: SidebarTab;
  activeTool: ToolMode;
  routeStart: TileCoord | null;
  routeEnd: TileCoord | null;
  route: TileCoord[];
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
  renderMode: "terrain",
  hoverTile: null,
  selectedTile: null,
  selectedEntityId: null,
  cameraFollowEntityId: null,
  activeSidebarTab: "world",
  activeTool: "inspect",
  routeStart: null,
  routeEnd: null,
  route: [],
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
