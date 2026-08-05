import { state } from "../state";
import type { TileInfo } from "../types";

const panel = document.getElementById("tile-inspector")!;
const grid = document.getElementById("tile-info-grid")!;

export function updateTileInspector(): void {
  if (
    !state.selectedTile ||
    !state.world ||
    state.selectedTile.x < 0 ||
    state.selectedTile.y < 0 ||
    state.selectedTile.x >= state.worldW ||
    state.selectedTile.y >= state.worldH
  ) {
    panel.style.display = "none";
    return;
  }

  const info: TileInfo = JSON.parse(
    state.world.tile_info(
      state.selectedTile.x,
      state.selectedTile.y,
    ),
  );
  if (!info.terrain) {
    panel.style.display = "none";
    return;
  }

  panel.style.display = "";
  const altPct = (((info.altitude + 1) / 2) * 100).toFixed(1);

  const regionLabel =
    info.region_id === 4294967295
      ? "\u2014"
      : String(info.region_id);
  const regionArea =
    info.region_area > 0
      ? info.region_area.toLocaleString() + " tiles"
      : "\u2014";
  const coastalLabel =
    info.region_type === "Water"
      ? "\u2014"
      : info.coastal
        ? "Yes"
        : "No";

  grid.innerHTML = [
    `<span class="info-key">Position</span><span class="info-val">(${info.x}, ${info.y})</span>`,
    `<span class="info-key">Terrain</span><span class="info-val" style="color:var(--accent)">${info.terrain}</span>`,
    `<span class="info-key">Altitude</span><span class="info-val">${altPct}%</span>`,
    `<span class="info-key">Moisture</span><span class="info-val">${(info.moisture * 100).toFixed(1)}%</span>`,
    `<span class="info-key">Temperature</span><span class="info-val">${(info.temperature * 100).toFixed(1)}%</span>`,
    `<span class="info-key" style="margin-top:4px;">Region</span><span class="info-val" style="margin-top:4px;">${regionLabel}</span>`,
    `<span class="info-key">Type</span><span class="info-val">${info.region_type}</span>`,
    `<span class="info-key">Area</span><span class="info-val">${regionArea}</span>`,
    `<span class="info-key">Coastal</span><span class="info-val">${coastalLabel}</span>`,
  ].join("");
}

export function clearTileInspector(): void {
  panel.style.display = "none";
  grid.innerHTML = "";
}
