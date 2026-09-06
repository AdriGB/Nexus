import "./styles/main.css";

import { state, setRenderCallback, requestRender } from "./state";
import type { IWorldBridge } from "./types";
import { loadWasm, createWorld, loadSnapshotWorld } from "./wasm";
import {
  initializeRenderer,
  renderWorld,
  uploadRouteToRenderer,
  uploadWorldToRenderer,
  uploadSimulationToRenderer,
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
  resetSimulationView,
  togglePlaySimulation,
  stepSimulation,
  setSimulationSpeed,
} from "./simulation";
import { installPerformanceDebug } from "./simulation-debug";
import { bindInteractionHistory } from "./ui/interaction-history";
import { bindFamilyTree } from "./ui/family-tree";
import { bindTabs, switchTab } from "./ui/tabs";
import {
  bindEntityInspector,
  selectEntity,
  syncEntityInspector,
} from "./ui/entity-inspector";
import type { SavedSnapshotRecordMeta } from "./persistence/world-save";

/* ── World activation & generation ─────────── */

function activateWorld(
  newWorld: IWorldBridge,
  params?: { seed?: number; width?: number; height?: number; sea?: number },
): void {
  state.selectedTile = null;
  state.selectedEntityId = null;
  state.cameraFollowEntityId = null;
  state.hoverTile = null;
  state.routeStart = null;
  state.routeEnd = null;
  state.route = [];
  hideTooltip();
  clearTileInspector();
  syncEntityInspector();

  if (state.world) {
    try {
      state.world.free();
    } catch (_) {
      /* ignore */
    }
  }

  state.world = newWorld;
  state.worldW = state.world.width();
  state.worldH = state.world.height();

  const width = params?.width ?? state.worldW;
  const height = params?.height ?? state.worldH;
  const seed = params?.seed ?? 42;
  const sea = params?.sea ?? 0.35;

  resetSimulationView();
  uploadWorldToRenderer();
  uploadSimulationToRenderer(true);
  uploadRouteToRenderer();
  updateRouteStatus();

  updateWorldInfo(seed, width, height, sea);
  updateRegionStats();

  // The minimap is an empty frame until a world exists, so it is hidden in the
  // markup and only revealed here.
  const minimapWrap = document.getElementById("minimap-wrap");
  if (minimapWrap) minimapWrap.hidden = false;

  fitWorld();
  renderMinimap();
  requestRender();
}

function generateWorld(): void {
  const { seed, width, height, sea } = readParams();
  const world = createWorld(seed, width, height, sea);
  activateWorld(world, { seed, width, height, sea });
  autoSave();
}

function restoreSnapshotWorld(
  json: string,
  meta?: SavedSnapshotRecordMeta | null,
): void {
  const world = loadSnapshotWorld(json);
  const seed = meta?.worldSeed ?? 42;
  const sea = meta?.seaLevel ?? 0.35;
  const width = world.width();
  const height = world.height();

  activateWorld(world, { seed, width, height, sea });
}

/* ── Render callback ──────────────────────── */

function fullRender(): void {
  renderWorld();
  drawMinimapViewport();
}

/* ── Boot ─────────────────────────────────── */

async function boot(): Promise<void> {
  // The minimap is an empty frame until a world exists; activateWorld() reveals
  // it. Done here rather than in the markup so the change stays out of
  // index.html, which another agent is editing concurrently.
  document.getElementById("minimap-wrap")?.setAttribute("hidden", "");

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

  // The engine is up but no world exists yet. Keep the overlay until the first
  // world is on screen: with no world the minimap, the HUD telemetry and the
  // status bar are all empty shells, and revealing them early just shows an
  // empty viewport with a stray spinner floating in it.
  const loadingText = document.getElementById("loading-text");
  if (loadingText) loadingText.textContent = "Generating world\u2026";

  setRenderCallback(fullRender);

  buildLegend();
  await initializeRenderer();

  const inputLayer = document.getElementById("world-input-layer")!;
  bindTabs();
  bindCamera(inputLayer);
  bindControls(generateWorld);
  bindSaveControls(generateWorld, restoreSnapshotWorld);
  bindSimulationControls();
  bindEntityInspector();
  bindInteractionHistory();
  bindFamilyTree();
  installPerformanceDebug();

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
    const rect = inputLayer.getBoundingClientRect();
    const mx = event.clientX - rect.left;
    const my = event.clientY - rect.top;
    const tile = screenToTile(mx, my);
    if (!isTileInWorld(tile)) return;

    state.selectedTile = tile;

    if (state.world) {
      const entities = state.world.entities_at(tile.x, tile.y);
      if (entities.length > 0) {
        selectEntity(entities[0]);
      } else {
        state.selectedEntityId = null;
        state.cameraFollowEntityId = null;
        switchTab("inspector");
        syncEntityInspector();
        updateTileInspector();
      }
    } else {
      switchTab("inspector");
      updateTileInspector();
    }

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

  window.addEventListener("keydown", (e: KeyboardEvent) => {
    const target = e.target as HTMLElement | null;
    const tag = target?.tagName?.toLowerCase();
    if (tag === "input" || tag === "textarea" || tag === "select") return;

    switch (e.key) {
      case " ":
        e.preventDefault();
        togglePlaySimulation();
        break;
      case ".":
        stepSimulation();
        break;
      case "1":
        setSimulationSpeed(1);
        break;
      case "2":
        setSimulationSpeed(2);
        break;
      case "3":
        setSimulationSpeed(4);
        break;
      case "f":
      case "F":
        fitWorld();
        requestRender();
        break;
      case "g":
      case "G": {
        state.showGrid = !state.showGrid;
        const gridBtn = document.getElementById("btn-toggle-grid");
        gridBtn?.classList.toggle("active", state.showGrid);
        requestRender();
        break;
      }
      case "Escape":
        state.selectedEntityId = null;
        state.cameraFollowEntityId = null;
        state.selectedTile = null;
        state.routeStart = null;
        state.routeEnd = null;
        state.route = [];
        uploadRouteToRenderer();
        updateRouteStatus();
        clearTileInspector();
        syncEntityInspector();
        requestRender();
        break;
    }
  });

  try {
    restoreLastWorld();
    generateWorld();
  } finally {
    // `finally` so a failure inside generation cannot leave the user staring at
    // "Generating world…" forever.
    const loading = document.getElementById("loading");
    if (loading) {
      loading.classList.add("done");
      setTimeout(() => loading.remove(), 600);
    }
  }
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
