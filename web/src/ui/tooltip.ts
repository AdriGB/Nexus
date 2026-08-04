import { state } from "../state";
import { TERRAIN } from "../constants";
import type { TileCoord, TileInfo } from "../types";

const tooltip = document.getElementById("tooltip")!;
const ttTerrain = document.getElementById("tt-terrain")!;
const ttAlt = document.getElementById("tt-alt")!;
const ttMoist = document.getElementById("tt-moist")!;
const ttTemp = document.getElementById("tt-temp")!;
const ttPos = document.getElementById("tt-pos")!;

const stX = document.getElementById("st-x")!;
const stY = document.getElementById("st-y")!;
const stTerrain = document.getElementById("st-terrain")!;
const stAlt = document.getElementById("st-alt")!;
const stMoist = document.getElementById("st-moist")!;
const stTemp = document.getElementById("st-temp")!;

let throttle = 0;

function parseAlt(alt: number): string {
  return ((alt + 1) / 2 * 100).toFixed(1) + "%";
}

export function updateHover(tile: TileCoord, mx: number, my: number): void {
  if (!state.world || tile.x < 0 || tile.y < 0 || tile.x >= state.worldW || tile.y >= state.worldH) {
    hideTooltip();
    return;
  }

  const now = performance.now();
  if (now - throttle < 50) return;
  throttle = now;

  const info: TileInfo = JSON.parse(state.world.tile_info(tile.x, tile.y));
  if (!info.terrain) { hideTooltip(); return; }

  // Status bar
  stX.textContent = String(tile.x);
  stY.textContent = String(tile.y);
  stTerrain.textContent = info.terrain;
  stAlt.textContent = parseAlt(info.altitude);
  stMoist.textContent = (info.moisture * 100).toFixed(0) + "%";
  stTemp.textContent = (info.temperature * 100).toFixed(0) + "%";

  // Tooltip
  const def = TERRAIN.find((t) => t.name === info.terrain) ?? TERRAIN[0];
  ttTerrain.textContent = info.terrain;
  ttTerrain.style.color = `hsl(${def.h},${def.s}%,60%)`;
  ttAlt.textContent = parseAlt(info.altitude);
  ttMoist.textContent = (info.moisture * 100).toFixed(1) + "%";
  ttTemp.textContent = (info.temperature * 100).toFixed(1) + "%";
  ttPos.textContent = `(${info.x}, ${info.y})`;

  const tx = Math.min(mx + 16, window.innerWidth - 260);
  const ty = Math.max(10, Math.min(my - 10, window.innerHeight - 160));
  tooltip.style.left = tx + "px";
  tooltip.style.top = ty + "px";
  tooltip.classList.add("visible");
}

export function hideTooltip(): void {
  tooltip.classList.remove("visible");
  stTerrain.textContent = "\u2014";
  stX.textContent = "\u2014";
  stY.textContent = "\u2014";
  stAlt.textContent = "\u2014";
  stMoist.textContent = "\u2014";
  stTemp.textContent = "\u2014";
}
