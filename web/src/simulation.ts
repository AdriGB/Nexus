import { uploadSimulationToRenderer } from "./renderer/renderer";
import { centerOnTile } from "./renderer/camera";
import { requestRender, state } from "./state";
import { syncEntityInspector } from "./ui/entity-inspector";
import {
  resetInteractionHistory,
  syncInteractionHistory,
} from "./ui/interaction-history";
import { syncPopulationStats } from "./ui/population-stats";
import { syncHouseholdStats } from "./ui/household-stats";
import { updateTileInspector } from "./ui/tile-inspector";

const BASE_TICKS_PER_SECOND = 4;
const MAX_FRAME_DELTA_SECONDS = 0.25;

let speed = 1;
let accumulator = 0;
let previousTimestamp: number | null = null;
let lastWorldRevision = 0n;

export function playSimulation(): void {
  if (!state.world) return;
  state.world.simulation_resume();
  syncSimulationUi();
}

export function pauseSimulation(): void {
  if (!state.world) return;
  state.world.simulation_pause();
  syncSimulationUi();
}

export function togglePlaySimulation(): void {
  if (!state.world) return;
  if (state.world.simulation_is_paused()) {
    playSimulation();
  } else {
    pauseSimulation();
  }
}

export function stepSimulation(): void {
  if (!state.world) return;
  state.world.simulation_step();
  handleSimulationChange();
}

export function setSimulationSpeed(newSpeed: number): void {
  speed = newSpeed;
  accumulator = 0;
  const speedSelect = document.getElementById("simulation-speed") as HTMLSelectElement | null;
  if (speedSelect) {
    speedSelect.value = String(newSpeed);
  }
  document.querySelectorAll(".hud-speed-btn").forEach((btn) => {
    const s = Number((btn as HTMLElement).dataset.speed);
    btn.classList.toggle("active", s === newSpeed);
  });
}

export function bindSimulationControls(): void {
  const playButton = document.getElementById("btn-sim-play");
  const pauseButton = document.getElementById("btn-sim-pause");
  const stepButton = document.getElementById("btn-sim-step");
  const hudPlayButton = document.getElementById("hud-btn-play");
  const hudPauseButton = document.getElementById("hud-btn-pause");
  const hudStepButton = document.getElementById("hud-btn-step");
  const speedSelect = document.getElementById(
    "simulation-speed",
  ) as HTMLSelectElement | null;

  document.getElementById("btn-spawn-10")?.addEventListener("click", () => {
    spawnEntities(10);
  });
  document.getElementById("btn-spawn-100")?.addEventListener("click", () => {
    spawnEntities(100);
  });

  playButton?.addEventListener("click", playSimulation);
  pauseButton?.addEventListener("click", pauseSimulation);
  stepButton?.addEventListener("click", stepSimulation);

  hudPlayButton?.addEventListener("click", playSimulation);
  hudPauseButton?.addEventListener("click", pauseSimulation);
  hudStepButton?.addEventListener("click", stepSimulation);

  document.querySelectorAll(".hud-speed-btn").forEach((btn) => {
    btn.addEventListener("click", () => {
      const s = Number((btn as HTMLElement).dataset.speed);
      if (s) setSimulationSpeed(s);
    });
  });

  speedSelect?.addEventListener("change", () => {
    const s = Number(speedSelect.value) || 1;
    setSimulationSpeed(s);
  });

  document.addEventListener("visibilitychange", () => {
    previousTimestamp = null;
    accumulator = 0;
  });

  const bindCopyHash = (id: string) => {
    const el = document.getElementById(id);
    el?.addEventListener("click", () => {
      const fullHash = el.dataset.fullHash;
      if (!fullHash) return;
      navigator.clipboard?.writeText(fullHash).then(() => {
        const original = el.textContent;
        el.textContent = "Copied!";
        setTimeout(() => {
          el.textContent = original;
        }, 1200);
      }).catch(() => {});
    });
  };
  bindCopyHash("simulation-hash");
  bindCopyHash("hud-hash-val");

  requestAnimationFrame(runSimulationFrame);
  syncSimulationUi();
}

export function syncSimulationUi(): void {
  const tickElement = document.getElementById("simulation-tick");
  const stateElement = document.getElementById("simulation-state");
  const hashElement = document.getElementById("simulation-hash");
  const hudTick = document.getElementById("hud-tick");
  const hudPop = document.getElementById("hud-pop");
  const hudHash = document.getElementById("hud-hash-val");
  const hudPlay = document.getElementById("hud-btn-play");
  const hudPause = document.getElementById("hud-btn-pause");
  const paused = state.world?.simulation_is_paused() ?? true;

  const tickStr = state.world
    ? state.world.simulation_tick().toLocaleString()
    : "0";
  const popStr = state.world
    ? state.world.entity_count().toLocaleString()
    : "0";

  if (tickElement) tickElement.textContent = tickStr;
  if (hudTick) hudTick.textContent = tickStr;
  if (hudPop) hudPop.textContent = popStr;

  if (stateElement) {
    stateElement.textContent = paused ? "Paused" : "Running";
    stateElement.classList.toggle("running", !paused);
  }

  document.getElementById("btn-sim-play")?.classList.toggle("active", !paused);
  document.getElementById("btn-sim-pause")?.classList.toggle("active", paused);
  hudPlay?.classList.toggle("active", !paused);
  hudPause?.classList.toggle("active", paused);

  const hash = state.world ? state.world.state_hash() : "";
  const shortHash = hash.length > 12 ? `${hash.slice(0, 10)}…` : hash;

  if (hashElement) {
    if (state.world) {
      hashElement.textContent = shortHash;
      hashElement.dataset.fullHash = hash;
      hashElement.title = `State Hash: ${hash}\nClick to copy`;
    } else {
      hashElement.textContent = "—";
      hashElement.title = "";
      delete hashElement.dataset.fullHash;
    }
  }

  if (hudHash) {
    if (state.world) {
      hudHash.textContent = shortHash;
      hudHash.dataset.fullHash = hash;
      hudHash.title = `State Hash: ${hash}\nClick to copy`;
    } else {
      hudHash.textContent = "—";
      hudHash.title = "";
      delete hudHash.dataset.fullHash;
    }
  }

  syncPopulationStats();
  syncHouseholdStats();
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

  if (state.cameraFollowEntityId !== null && state.world) {
    try {
      const raw = state.world.entity_info(state.cameraFollowEntityId);
      if (raw && raw !== "{}") {
        const ent = JSON.parse(raw);
        if (ent.id !== undefined) {
          centerOnTile(ent.x, ent.y);
        } else {
          state.cameraFollowEntityId = null;
          syncEntityInspector();
        }
      } else {
        state.cameraFollowEntityId = null;
        syncEntityInspector();
      }
    } catch (_) {
      state.cameraFollowEntityId = null;
    }
  }

  requestAnimationFrame(runSimulationFrame);
}

function spawnEntities(count: number): void {
  if (!state.world) return;
  state.world.spawn_entities(count);
  handleSimulationChange();
}
