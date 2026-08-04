import { state } from "../state";
import type { TileInfo } from "../types";

const panel = document.getElementById("tile-inspector")!;
const grid = document.getElementById("tile-info-grid")!;

export function updateTileInspector(): void {
  if (
    !state.selectedTile || !state.world ||
    state.selectedTile.x < 0 || state.selectedTile.y < 0 ||
    state.selectedTile.x >= state.worldW || state.selectedTile.y >= state.worldH
  ) {
    panel.style.display = "none";
    return;
  }

  const info: TileInfo = JSON.parse(
    state.world.tile_info(state.selectedTile.x, state.selectedTile.y)
  );
  if (!info.terrain) { panel.style.display = "none"; return; }

  panel.style.display = "";
  const altPct = ((info.altitude + 1) / 2 * 100).toFixed(1);
  grid.innerHTML = [
    `<span class="info-key">Position</span><span class="info-val">(${info.x}, ${info.y})</span>`,
    `<span class="info-key">Terrain</span><span class="info-val" style="color:var(--accent)">${info.terrain}</span>`,
    `<span class="info-key">Altitude</span><span class="info-val">${altPct}%</span>`,
    `<span class="info-key">Moisture</span><span class="info-val">${(info.moisture * 100).toFixed(1)}%</span>`,
    `<span class="info-key">Temperature</span><span class="info-val">${(info.temperature * 100).toFixed(1)}%</span>`,
  ].join("");
}
