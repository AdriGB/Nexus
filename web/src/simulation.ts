import { requestRender, state } from "./state";
import { uploadSimulationToRenderer } from "./renderer/renderer";
import type { EntityInfo, PopulationStats } from "./types";
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

function spawnEntities(count: number): void {
  if (!state.world) return;
  state.world.spawn_entities(count);
  handleSimulationChange();
}

function syncPopulationStats(): void {
  if (!state.world) return;
  const stats: PopulationStats = JSON.parse(state.world.population_stats());
  const values: Record<string, string> = {
    "population-count": stats.population.toLocaleString(),
    "population-births": stats.births.toLocaleString(),
    "population-deaths": stats.deaths.toLocaleString(),
    "population-hungry": stats.hungry.toLocaleString(),
    "population-seeking": stats.seeking_food.toLocaleString(),
    "population-average-hunger": `${stats.average_hunger.toFixed(1)}%`,
    "population-food-consumed": stats.food_consumed.toLocaleString(),
  };
  for (const [id, value] of Object.entries(values)) {
    document.getElementById(id)!.textContent = value;
  }
}

function syncEntityInspector(): void {
  const panel = document.getElementById("entity-inspector")!;
  const grid = document.getElementById("entity-info-grid")!;
  if (!state.world || state.world.entity_count() === 0) {
    panel.hidden = true;
    grid.innerHTML = "";
    return;
  }

  const entity: EntityInfo = JSON.parse(state.world.first_entity_info());
  panel.hidden = false;
  grid.innerHTML = [
    `<span class="info-key">ID</span><span class="info-val">#${entity.id}</span>`,
    `<span class="info-key">Position</span><span class="info-val">(${entity.x}, ${entity.y})</span>`,
    `<span class="info-key">Hunger</span><span class="info-val">${entity.hunger.toFixed(0)} / 100</span>`,
    `<span class="info-key">Health</span><span class="info-val">${entity.health.toFixed(0)} / 100</span>`,
    `<span class="info-key">Age</span><span class="info-val">${entity.age_ticks.toLocaleString()} ticks</span>`,
    `<span class="info-key">Activity</span><span class="info-val entity-activity">${entity.activity}</span>`,
    `<span class="info-key">Goal</span><span class="info-val entity-goal">${entity.goal}</span>`,
    `<span class="info-key">Action</span><span class="info-val">${entity.action}</span>`,
    `<span class="info-key">Goal retained</span><span class="info-val">${entity.goal_age_ticks.toLocaleString()} ticks</span>`,
    `<span class="info-key">Path remaining</span><span class="info-val">${entity.remaining_path}</span>`,
    `<span class="info-key">Known resources</span><span class="info-val">${entity.known_resources}</span>`,
    `<span class="info-key">Known chunks</span><span class="info-val">${entity.known_chunks}</span>`,
    `<span class="info-key">Visible entities</span><span class="info-val">${entity.visible_entities}</span>`,
    `<span class="info-key">Utility: eat</span><span class="info-val">${entity.utilities.eat.toFixed(2)}</span>`,
    `<span class="info-key">Utility: explore</span><span class="info-val">${entity.utilities.explore.toFixed(2)}</span>`,
    `<span class="info-key">Utility: rest</span><span class="info-val">${entity.utilities.rest.toFixed(2)}</span>`,
  ].join("");
}
