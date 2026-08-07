import { requestRender, state } from "./state";
import { uploadSimulationToRenderer } from "./renderer/renderer";
import type { EntityInfo } from "./types";
import { updateTileInspector } from "./ui/tile-inspector";

const BASE_TICKS_PER_SECOND = 4;
const MAX_FRAME_DELTA_SECONDS = 0.25;

let speed = 1;
let accumulator = 0;
let previousTimestamp: number | null = null;
let lastWorldRevision = 0n;

export function bindSimulationControls(): void {
  const playButton = document.getElementById("btn-sim-play")!;
  const pauseButton = document.getElementById("btn-sim-pause")!;
  const stepButton = document.getElementById("btn-sim-step")!;
  const speedSelect = document.getElementById(
    "simulation-speed",
  ) as HTMLSelectElement;

  playButton.addEventListener("click", () => {
    state.world?.simulation_resume();
    syncSimulationUi();
  });

  pauseButton.addEventListener("click", () => {
    state.world?.simulation_pause();
    syncSimulationUi();
  });

  stepButton.addEventListener("click", () => {
    if (!state.world) return;
    state.world.simulation_step();
    handleSimulationChange();
  });

  speedSelect.addEventListener("change", () => {
    speed = Number(speedSelect.value) || 1;
    accumulator = 0;
  });

  document.addEventListener("visibilitychange", () => {
    previousTimestamp = null;
    accumulator = 0;
  });

  requestAnimationFrame(runSimulationFrame);
  syncSimulationUi();
}

export function syncSimulationUi(): void {
  const tickElement = document.getElementById("simulation-tick")!;
  const stateElement = document.getElementById("simulation-state")!;
  const paused = state.world?.simulation_is_paused() ?? true;

  tickElement.textContent = state.world
    ? state.world.simulation_tick().toLocaleString()
    : "0";
  stateElement.textContent = paused ? "Paused" : "Running";
  stateElement.classList.toggle("running", !paused);
  document.getElementById("btn-sim-play")?.classList.toggle("active", !paused);
  document.getElementById("btn-sim-pause")?.classList.toggle("active", paused);
  syncEntityInspector();
}

export function resetSimulationView(): void {
  lastWorldRevision = state.world?.simulation_world_revision() ?? 0n;
  syncSimulationUi();
}

function runSimulationFrame(timestamp: number): void {
  if (previousTimestamp === null) {
    previousTimestamp = timestamp;
  }

  const deltaSeconds = Math.min(
    (timestamp - previousTimestamp) / 1_000,
    MAX_FRAME_DELTA_SECONDS,
  );
  previousTimestamp = timestamp;

  if (state.world && !state.world.simulation_is_paused()) {
    accumulator += deltaSeconds * BASE_TICKS_PER_SECOND * speed;
    const ticks = Math.floor(accumulator);
    if (ticks > 0) {
      accumulator -= ticks;
      state.world.simulation_advance(ticks);
      handleSimulationChange();
    }
  }

  requestAnimationFrame(runSimulationFrame);
}

function handleSimulationChange(): void {
  if (!state.world) return;
  const revision = state.world.simulation_world_revision();
  const resourcesChanged = revision !== lastWorldRevision;
  lastWorldRevision = revision;
  uploadSimulationToRenderer(resourcesChanged);
  if (resourcesChanged) {
    updateTileInspector();
  }
  syncSimulationUi();
  requestRender();
}

function syncEntityInspector(): void {
  const panel = document.getElementById("entity-inspector")!;
  const grid = document.getElementById("entity-info-grid")!;
  if (!state.world || state.world.entity_count() === 0) {
    panel.hidden = true;
    grid.innerHTML = "";
    return;
  }

  const entity: EntityInfo = JSON.parse(state.world.entity_info(1));
  panel.hidden = false;
  grid.innerHTML = [
    `<span class="info-key">ID</span><span class="info-val">#${entity.id}</span>`,
    `<span class="info-key">Position</span><span class="info-val">(${entity.x}, ${entity.y})</span>`,
    `<span class="info-key">Hunger</span><span class="info-val">${entity.hunger.toFixed(0)} / 100</span>`,
    `<span class="info-key">Activity</span><span class="info-val entity-activity">${entity.activity}</span>`,
    `<span class="info-key">Path remaining</span><span class="info-val">${entity.remaining_path}</span>`,
  ].join("");
}
