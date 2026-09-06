import { state, requestRender } from "../state";
import { BASE_TILE, MIN_ZOOM, MAX_ZOOM } from "../constants";
import type { TileCoord } from "../types";

// FIX #6: Prevent rendering more than 2000 tiles in any dimension
function getEffectiveMinZoom(): number {
  const minForWidth = state.cssW / (2000 * BASE_TILE);
  const minForHeight = state.cssH / (2000 * BASE_TILE);
  return Math.max(MIN_ZOOM, minForWidth, minForHeight);
}

export function fitWorld(): void {
  if (!state.world) return;
  const pad = 40;
  const scaleX = (state.cssW - pad * 2) / (state.worldW * BASE_TILE);
  const scaleY = (state.cssH - pad * 2) / (state.worldH * BASE_TILE);
  const effectiveMin = getEffectiveMinZoom();
  state.zoom = Math.max(effectiveMin, Math.min(scaleX, scaleY, 3));
  state.panX = (state.worldW * BASE_TILE * state.zoom) / 2 - state.cssW / 2;
  state.panY = (state.worldH * BASE_TILE * state.zoom) / 2 - state.cssH / 2;
}

export function screenToTile(sx: number, sy: number): TileCoord {
  const tileSize = BASE_TILE * state.zoom;
  return {
    x: Math.floor((sx + state.panX) / tileSize),
    y: Math.floor((sy + state.panY) / tileSize),
  };
}

export function bindCamera(inputLayer: HTMLElement): void {
  let dragging = false;
  let moved = false;
  let lastMX = 0;
  let lastMY = 0;

  inputLayer.addEventListener("mousedown", (e) => {
    dragging = true;
    moved = false;
    lastMX = e.clientX;
    lastMY = e.clientY;
    inputLayer.classList.add("dragging");
  });

  window.addEventListener("mousemove", (e) => {
    if (dragging) {
      state.panX -= e.clientX - lastMX;
      state.panY -= e.clientY - lastMY;
      lastMX = e.clientX;
      lastMY = e.clientY;
      moved = true;
      requestRender();
    }
  });

  window.addEventListener("mouseup", () => {
    if (dragging && moved) {
      inputLayer.dataset.wasDrag = "true";
      setTimeout(() => {
        inputLayer.dataset.wasDrag = "";
      }, 0);
    }
    dragging = false;
    inputLayer.classList.remove("dragging");
  });

  inputLayer.addEventListener(
    "wheel",
    (e) => {
      e.preventDefault();
      const oldZoom = state.zoom;
      const factor = e.deltaY > 0 ? 0.9 : 1.1;
      const effectiveMin = getEffectiveMinZoom();
      state.zoom = Math.max(
        effectiveMin,
        Math.min(MAX_ZOOM, state.zoom * factor),
      );

      const rect = inputLayer.getBoundingClientRect();
      const cx = e.clientX - rect.left;
      const cy = e.clientY - rect.top;
      const ratio = state.zoom / oldZoom;
      state.panX = (state.panX + cx) * ratio - cx;
      state.panY = (state.panY + cy) * ratio - cy;

      requestRender();
    },
    { passive: false },
  );

  // Minimap click-to-navigate
  const miniCanvas = document.getElementById(
    "minimap-canvas",
  ) as HTMLCanvasElement;
  let miniDragging = false;
  const navigateMinimap = (e: MouseEvent) => {
    const rect = miniCanvas.getBoundingClientRect();
    const mx = Math.max(0, Math.min(1, (e.clientX - rect.left) / miniCanvas.width));
    const my = Math.max(0, Math.min(1, (e.clientY - rect.top) / miniCanvas.height));
    state.panX = mx * state.worldW * BASE_TILE * state.zoom - state.cssW / 2;
    state.panY = my * state.worldH * BASE_TILE * state.zoom - state.cssH / 2;
    requestRender();
  };

  miniCanvas.addEventListener("mousedown", (e) => {
    miniDragging = true;
    navigateMinimap(e);
  });
  window.addEventListener("mousemove", (e) => {
    if (miniDragging) {
      navigateMinimap(e);
    }
  });
  window.addEventListener("mouseup", () => {
    miniDragging = false;
  });

  // Camera on-screen buttons
  document.getElementById("btn-zoom-in")?.addEventListener("click", () => zoomIn());
  document.getElementById("btn-zoom-out")?.addEventListener("click", () => zoomOut());
  document.getElementById("btn-zoom-fit")?.addEventListener("click", () => {
    fitWorld();
    requestRender();
  });
  document.getElementById("btn-toggle-grid")?.addEventListener("click", () => {
    state.showGrid = !state.showGrid;
    document.getElementById("btn-toggle-grid")?.classList.toggle("active", state.showGrid);
    requestRender();
  });
}

export function centerOnTile(x: number, y: number): void {
  const tileSize = BASE_TILE * state.zoom;
  state.panX = (x + 0.5) * tileSize - state.cssW / 2;
  state.panY = (y + 0.5) * tileSize - state.cssH / 2;
  requestRender();
}

export function zoomIn(): void {
  zoomByFactor(1.25);
}

export function zoomOut(): void {
  zoomByFactor(0.8);
}

function zoomByFactor(factor: number): void {
  const oldZoom = state.zoom;
  const effectiveMin = getEffectiveMinZoom();
  const newZoom = Math.max(
    effectiveMin,
    Math.min(MAX_ZOOM, state.zoom * factor),
  );
  if (newZoom === oldZoom) return;

  const cx = state.cssW / 2;
  const cy = state.cssH / 2;
  const ratio = newZoom / oldZoom;
  state.panX = (state.panX + cx) * ratio - cx;
  state.panY = (state.panY + cy) * ratio - cy;
  state.zoom = newZoom;
  requestRender();
}
