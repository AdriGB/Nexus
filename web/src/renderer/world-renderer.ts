import { state } from "../state";
import { TERRAIN, BASE_TILE } from "../constants";

const canvas = document.getElementById("world-canvas") as HTMLCanvasElement;
const ctx = canvas.getContext("2d")!;

/* ── Deterministic per-tile jitter for texture ── */

function jitter(x: number, y: number): number {
  let h = (x * 374761393 + y * 668265263) | 0;
  h = ((h ^ (h >> 13)) * 1274126177) | 0;
  return ((h ^ (h >> 16)) & 0xff) / 255;
}

function tileColor(
  terrainId: number,
  altByte: number,
  _moistByte: number,
  _tempByte: number,
  tx: number,
  ty: number
): string {
  const t = TERRAIN[terrainId] ?? TERRAIN[0];
  const altNorm = altByte / 255;
  const altFactor = 0.72 + altNorm * 0.5;
  let l = t.l * altFactor;
  l += (jitter(tx, ty) - 0.5) * (terrainId <= 1 ? 3 : 5);
  l = Math.max(0, Math.min(100, l));
  return `hsl(${t.h},${t.s}%,${l}%)`;
}

/* ── Resize canvas respecting devicePixelRatio ── */

export function resizeCanvas(): void {
  const rect = canvas.parentElement!.getBoundingClientRect();
  state.cssW = rect.width;
  state.cssH = rect.height;
  const dpr = window.devicePixelRatio || 1;
  canvas.width = Math.round(state.cssW * dpr);
  canvas.height = Math.round(state.cssH * dpr);
  canvas.style.width = state.cssW + "px";
  canvas.style.height = state.cssH + "px";
}

/* ── Main render ── */

export function render(): void {
  if (!state.world) return;

  const dpr = window.devicePixelRatio || 1;
  ctx.setTransform(dpr, 0, 0, dpr, 0, 0);
  ctx.fillStyle = "#07080c";
  ctx.fillRect(0, 0, state.cssW, state.cssH);

  const tileSize = BASE_TILE * state.zoom;

  const startTX = Math.floor(state.panX / tileSize);
  const startTY = Math.floor(state.panY / tileSize);
  const endTX = Math.ceil((state.panX + state.cssW) / tileSize);
  const endTY = Math.ceil((state.panY + state.cssH) / tileSize);
  const cols = endTX - startTX;
  const rows = endTY - startTY;

  if (cols <= 0 || rows <= 0 || cols > 2000 || rows > 2000) return;

  const data = state.world.get_tile_data(startTX, startTY, cols, rows);

  for (let i = 0; i < cols * rows; i++) {
    const terrain = data[i * 4];
    const alt = data[i * 4 + 1];
    const moist = data[i * 4 + 2];
    const temp = data[i * 4 + 3];

    const tx = startTX + (i % cols);
    const ty = startTY + Math.floor(i / cols);
    const px = tx * tileSize - state.panX;
    const py = ty * tileSize - state.panY;

    ctx.fillStyle = tileColor(terrain, alt, moist, temp, tx, ty);
    ctx.fillRect(px, py, Math.ceil(tileSize) + 1, Math.ceil(tileSize) + 1);
  }

  // Grid overlay
  if (state.showGrid && state.zoom > 2.5) {
    ctx.strokeStyle = "rgba(255,255,255,0.06)";
    ctx.lineWidth = 0.5;
    const offX = -state.panX % tileSize;
    const offY = -state.panY % tileSize;
    for (let x = offX; x < state.cssW; x += tileSize) {
      ctx.beginPath();
      ctx.moveTo(x, 0);
      ctx.lineTo(x, state.cssH);
      ctx.stroke();
    }
    for (let y = offY; y < state.cssH; y += tileSize) {
      ctx.beginPath();
      ctx.moveTo(0, y);
      ctx.lineTo(state.cssW, y);
      ctx.stroke();
    }
  }

  // Hover highlight
  if (state.hoverTile) {
    const hx = state.hoverTile.x * tileSize - state.panX;
    const hy = state.hoverTile.y * tileSize - state.panY;
    ctx.strokeStyle = "rgba(201,168,76,0.6)";
    ctx.lineWidth = 2;
    ctx.strokeRect(hx, hy, tileSize, tileSize);
  }

  // Selected highlight
  if (state.selectedTile) {
    const sx = state.selectedTile.x * tileSize - state.panX;
    const sy = state.selectedTile.y * tileSize - state.panY;
    ctx.strokeStyle = "rgba(201,168,76,0.9)";
    ctx.lineWidth = 2;
    ctx.strokeRect(sx, sy, tileSize, tileSize);
  }

  // Zoom display
  const zoomEl = document.getElementById("st-zoom");
  if (zoomEl) zoomEl.textContent = Math.round((state.zoom / BASE_TILE) * BASE_TILE * 100) + "%";
}
