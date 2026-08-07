import { state } from "./state";

const BASE_TICKS_PER_SECOND = 4;
const MAX_FRAME_DELTA_SECONDS = 0.25;

let speed = 1;
let accumulator = 0;
let previousTimestamp: number | null = null;

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
    state.world?.simulation_step();
    syncSimulationUi();
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
      syncSimulationUi();
    }
  }

  requestAnimationFrame(runSimulationFrame);
}
