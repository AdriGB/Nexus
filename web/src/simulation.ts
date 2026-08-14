import { uploadSimulationToRenderer } from "./renderer/renderer";
import { requestRender, state } from "./state";
import { syncEntityInspector } from "./ui/entity-inspector";
import {
  resetInteractionHistory,
  syncInteractionHistory,
} from "./ui/interaction-history";
import { syncPopulationStats } from "./ui/population-stats";
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

  document.getElementById("btn-spawn-10")!.addEventListener("click", () => {
    spawnEntities(10);
  });
  document.getElementById("btn-spawn-100")!.addEventListener("click", () => {
    spawnEntities(100);
  });

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
  syncPopulationStats();
  syncEntityInspector();
  syncInteractionHistory();
}

export function resetSimulationView(): void {
  lastWorldRevision = state.world?.simulation_world_revision() ?? 0n;
  resetInteractionHistory();
  syncSimulationUi();
}

export function handleSimulationChange(): void {
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

function spawnEntities(count: number): void {
  if (!state.world) return;
  state.world.spawn_entities(count);
  handleSimulationChange();
}
