import "./styles/main.css";

import { state, setRenderCallback, requestRender } from "./state";
import { loadWasm, createWorld } from "./wasm";
import {
  initializeRenderer,
  renderWorld,
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

/* ── World generation ─────────────────────── */

function generateWorld(): void {
  const { seed, width, height, sea } = readParams();

  state.selectedTile = null;
  state.hoverTile = null;
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
  uploadWorldToRenderer();

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

  inputLayer.addEventListener("click", () => {
    if (inputLayer.dataset.wasDrag === "true") return;
    if (state.hoverTile) {
      state.selectedTile = { ...state.hoverTile };
      updateTileInspector();
      requestRender();
    }
  });

  restoreLastWorld();
  generateWorld();
}

boot();
