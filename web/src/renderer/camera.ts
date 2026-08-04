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

export function bindCamera(canvas: HTMLCanvasElement): void {
  let dragging = false;
  let moved = false;
  let lastMX = 0;
  let lastMY = 0;

  canvas.addEventListener("mousedown", (e) => {
    dragging = true;
    moved = false;
    lastMX = e.clientX;
    lastMY = e.clientY;
    canvas.classList.add("dragging");
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
      canvas.dataset.wasDrag = "true";
      setTimeout(() => {
        canvas.dataset.wasDrag = "";
      }, 0);
    }
    dragging = false;
    canvas.classList.remove("dragging");
  });

  canvas.addEventListener(
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

      const rect = canvas.getBoundingClientRect();
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
  miniCanvas.addEventListener("click", (e) => {
    const rect = miniCanvas.getBoundingClientRect();
    const mx = (e.clientX - rect.left) / miniCanvas.width;
    const my = (e.clientY - rect.top) / miniCanvas.height;
    state.panX =
      mx * state.worldW * BASE_TILE * state.zoom - state.cssW / 2;
    state.panY =
      my * state.worldH * BASE_TILE * state.zoom - state.cssH / 2;
    requestRender();
  });
}
