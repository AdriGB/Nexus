import { TERRAIN } from "../constants";

export function buildLegend(): void {
  const grid = document.getElementById("legend-grid")!;
  for (const t of TERRAIN) {
    const item = document.createElement("div");
    item.className = "legend-item";
    item.innerHTML = `<span class="legend-swatch" style="background:hsl(${t.h},${t.s}%,${t.l}%)"></span>${t.name}`;
    grid.appendChild(item);
  }
}
