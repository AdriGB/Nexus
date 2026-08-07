import "./styles/main.css";

import { state, setRenderCallback, requestRender } from "./state";
import { loadWasm, createWorld } from "./wasm";
import {
  initializeRenderer,
  renderWorld,
  uploadRouteToRenderer,
  uploadWorldToRenderer,
} from "./renderer/renderer";
import {
  fitWorld,
  screenToTile,
  bindCamera,
} from "./renderer/camera";
import {
  renderMinimap,
  drawMinimapViewport,
} from "./renderer/minimap";
import {
  bindControls,
  readParams,
  updateWorldInfo,
  updateRegionStats,
} from "./ui/controls";
import { updateHover, hideTooltip } from "./ui/tooltip";
import { buildLegend } from "./ui/legend";
import {
  updateTileInspector,
  clearTileInspector,
} from "./ui/tile-inspector";
import {
  bindSaveControls,
  autoSave,
  restoreLastWorld,
} from "./ui/save-controls";
import {
  bindSimulationControls,
  syncSimulationUi,
} from "./simulation";

/* ── World generation ─────────────────────── */

function generateWorld(): void {
  const { seed, width, height, sea } = readParams();

  state.selectedTile = null;
  state.hoverTile = null;
  state.routeStart = null;
  state.routeEnd = null;
  state.route = [];
  hideTooltip();
  clearTileInspector();

  if (state.world) {
    try {
      state.world.free();
    } catch (_) {
      /* ignore */
    }
  }

  state.world = createWorld(seed, width, height, sea);
  state.worldW = state.world.width();
  state.worldH = state.world.height();
  syncSimulationUi();
  uploadWorldToRenderer();
  uploadRouteToRenderer();
  updateRouteStatus();

  updateWorldInfo(seed, width, height, sea);
  updateRegionStats();
  fitWorld();
  renderMinimap();
  requestRender();
  autoSave();
}

/* ── Render callback ──────────────────────── */

function fullRender(): void {
  renderWorld();
  drawMinimapViewport();
}

/* ── Boot ─────────────────────────────────── */

async function boot(): Promise<void> {
  try {
    await loadWasm();
  } catch (err) {
    const textEl = document.getElementById("loading-text")!;
    textEl.outerHTML = `<div class="error">
      <strong>Failed to load WASM engine</strong><br><br>
      ${(err as Error).message || err}<br><br>
      Build the engine first, then serve via HTTP:<br>
      <code>cd engine && wasm-pack build --target web --out-dir ../web/src/wasm<br>
      cd ../web && npm run dev</code>
    </div>`;
    return;
  }

  document.getElementById("loading")!.classList.add("done");
  setTimeout(
    () => document.getElementById("loading")!.remove(),
    600,
  );

  setRenderCallback(fullRender);

  buildLegend();
  await initializeRenderer();

  const inputLayer = document.getElementById("world-input-layer")!;
  bindCamera(inputLayer);
  bindControls(generateWorld);
  bindSaveControls(generateWorld);
  bindSimulationControls();

  inputLayer.addEventListener("mousemove", (e) => {
    const rect = inputLayer.getBoundingClientRect();
    const mx = e.clientX - rect.left;
    const my = e.clientY - rect.top;
    if (
      mx >= 0 &&
      my >= 0 &&
      mx < state.cssW &&
      my < state.cssH
    ) {
      state.hoverTile = screenToTile(mx, my);
      updateHover(state.hoverTile, e.clientX, e.clientY);
      requestRender();
    }
  });

  inputLayer.addEventListener("mouseleave", () => {
    state.hoverTile = null;
    hideTooltip();
    requestRender();
  });

  inputLayer.addEventListener("click", (event) => {
    if (inputLayer.dataset.wasDrag === "true") return;
    if (!state.hoverTile || !isTileInWorld(state.hoverTile)) return;

    const tile = { ...state.hoverTile };
    state.selectedTile = tile;
    updateTileInspector();

    if (event.shiftKey && state.routeStart && state.world) {
      state.routeEnd = tile;
      const coordinates = state.world.find_path(
        state.routeStart.x,
        state.routeStart.y,
        tile.x,
        tile.y,
      );
      state.route = unpackRoute(coordinates);
    } else {
      state.routeStart = tile;
      state.routeEnd = null;
      state.route = [];
    }

    uploadRouteToRenderer();
    updateRouteStatus();
    requestRender();
  });

  restoreLastWorld();
  generateWorld();
}

boot();

function unpackRoute(coordinates: Uint32Array): Array<{ x: number; y: number }> {
  const route = [];
  for (let index = 0; index + 1 < coordinates.length; index += 2) {
    route.push({ x: coordinates[index], y: coordinates[index + 1] });
  }
  return route;
}

function isTileInWorld(tile: { x: number; y: number }): boolean {
  return (
    tile.x >= 0 &&
    tile.y >= 0 &&
    tile.x < state.worldW &&
    tile.y < state.worldH
  );
}

function updateRouteStatus(): void {
  const status = document.getElementById("st-route");
  if (!status) return;

  if (!state.routeStart) {
    status.textContent = "Select origin";
  } else if (!state.routeEnd) {
    status.textContent = `${state.routeStart.x},${state.routeStart.y} → Shift+Click`;
  } else if (state.route.length === 0) {
    status.textContent = "No path";
  } else {
    status.textContent = `${state.route.length} tiles`;
  }
}
