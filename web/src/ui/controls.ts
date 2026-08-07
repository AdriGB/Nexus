import { state, requestRender } from "../state";
import { fitWorld } from "../renderer/camera";
import { resizeRenderer } from "../renderer/renderer";
import { updateTileInspector } from "./tile-inspector";
import type { RegionStats } from "../types";

function clampInt(
  val: string,
  min: number,
  max: number,
  fallback: number,
): number {
  const n = parseInt(val, 10);
  if (Number.isNaN(n)) return fallback;
  return Math.max(min, Math.min(max, n));
}

export function bindControls(generateFn: () => void): void {
  const seedInput = document.getElementById(
    "seed-input",
  ) as HTMLInputElement;
  const btnGenerate = document.getElementById("btn-generate")!;
  const btnRandom = document.getElementById("btn-random-seed")!;
  const seaSlider = document.getElementById(
    "sea-slider",
  ) as HTMLInputElement;
  const seaVal = document.getElementById("sea-val")!;
  const renderModeButtons =
    document.querySelectorAll<HTMLButtonElement>("[data-render-mode]");

  btnGenerate.addEventListener("click", generateFn);

  btnRandom.addEventListener("click", () => {
    seedInput.value = String(
      Math.floor(Math.random() * 4294967295),
    );
  });

  seaSlider.addEventListener("input", () => {
    seaVal.textContent = parseFloat(seaSlider.value).toFixed(2);
  });

  renderModeButtons.forEach((button) => {
    button.addEventListener("click", () => {
      if (button.disabled) return;
      state.renderMode = button.dataset.renderMode as "terrain" | "resources";
      renderModeButtons.forEach((candidate) => {
        candidate.classList.toggle("active", candidate === button);
      });
      requestRender();
    });
  });

  window.addEventListener("keydown", (e) => {
    if (e.target instanceof HTMLInputElement) return;

    if (e.key === "r" || e.key === "R") {
      seedInput.value = String(
        Math.floor(Math.random() * 4294967295),
      );
      generateFn();
    }
    if (e.key === "f" || e.key === "F") {
      fitWorld();
      requestRender();
    }
    if (e.key === "g" || e.key === "G") {
      state.showGrid = !state.showGrid;
      requestRender();
    }
    if (e.key === "Escape") {
      state.selectedTile = null;
      updateTileInspector();
      requestRender();
    }
  });

  window.addEventListener("resize", () => {
    resizeRenderer();
    requestRender();
  });
}

export function readParams(): {
  seed: number;
  width: number;
  height: number;
  sea: number;
} {
  const seedInput = document.getElementById(
    "seed-input",
  ) as HTMLInputElement;
  const widthInput = document.getElementById(
    "width-input",
  ) as HTMLInputElement;
  const heightInput = document.getElementById(
    "height-input",
  ) as HTMLInputElement;
  const seaSlider = document.getElementById(
    "sea-slider",
  ) as HTMLInputElement;

  return {
    seed: clampInt(seedInput.value, 0, 4294967295, 42),
    width: clampInt(widthInput.value, 64, 1024, 256),
    height: clampInt(heightInput.value, 64, 1024, 256),
    sea: parseFloat(seaSlider.value) || 0.35,
  };
}

export function updateWorldInfo(
  seed: number,
  width: number,
  height: number,
  sea: number,
): void {
  const panel = document.getElementById("world-info")!;
  panel.style.display = "";
  document.getElementById("info-seed")!.textContent =
    String(seed);
  document.getElementById("info-size")!.textContent =
    `${width} \u00d7 ${height}`;
  document.getElementById("info-tiles")!.textContent = (
    width * height
  ).toLocaleString();
  document.getElementById("info-sea")!.textContent =
    sea.toFixed(2);
}

export function updateRegionStats(): void {
  const container = document.getElementById("region-stats");
  if (!container || !state.world) return;

  try {
    const raw: RegionStats = JSON.parse(
      state.world.region_stats(),
    );
    const continents = raw.land_regions - raw.islands;
    container.innerHTML = `
      <span class="info-key">Continents</span>
      <span class="info-val">${continents}</span>
      <span class="info-key">Islands</span>
      <span class="info-val">${raw.islands}</span>
      <span class="info-key">Land coverage</span>
      <span class="info-val">${(raw.land_coverage * 100).toFixed(1)}%</span>
      <span class="info-key">Largest mass</span>
      <span class="info-val">${(raw.largest_landmass_pct * 100).toFixed(1)}%</span>
    `;
  } catch {
    container.innerHTML = "";
  }
}
