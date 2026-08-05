import "./styles/main.css";

import { state, setRenderCallback, requestRender } from "./state";
import { loadWasm, createWorld } from "./wasm";
import { resizeCanvas, render } from "./renderer/world-renderer";
import { fitWorld, screenToTile, bindCamera } from "./renderer/camera";
import { renderMinimap, drawMinimapViewport } from "./renderer/minimap";
import { bindControls, readParams, updateWorldInfo } from "./ui/controls";
import { updateHover, hideTooltip } from "./ui/tooltip";
import { buildLegend } from "./ui/legend";
import { updateTileInspector, clearTileInspector } from "./ui/tile-inspector";
import { bindSaveControls, autoSave, restoreLastWorld } from "./ui/save-controls";

/* ── World generation ─────────────────────── */

function generateWorld(): void {
  const { seed, width, height, sea } = readParams();

  // Clear selection state before replacing world
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

  updateWorldInfo(seed, width, height, sea);
  fitWorld();
  renderMinimap();
  requestRender();

  // Persist last world config
  autoSave();
}

/* ── Full render pass ─────────────────────── */

function fullRender(): void {
  render();
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
  setTimeout(() => document.getElementById("loading")!.remove(), 600);

  setRenderCallback(fullRender);

  buildLegend();
  resizeCanvas();

  const canvas = document.getElementById("world-canvas") as HTMLCanvasElement;
  bindCamera(canvas);
  bindControls(generateWorld);
  bindSaveControls(generateWorld);

  // Hover
  canvas.addEventListener("mousemove", (e) => {
    const rect = canvas.getBoundingClientRect();
    const mx = e.clientX - rect.left;
    const my = e.clientY - rect.top;
    if (mx >= 0 && my >= 0 && mx < state.cssW && my < state.cssH) {
      state.hoverTile = screenToTile(mx, my);
      updateHover(state.hoverTile, e.clientX, e.clientY);
      requestRender();
    }
  });

  canvas.addEventListener("mouseleave", () => {
    state.hoverTile = null;
    hideTooltip();
    requestRender();
  });

  // Click to select tile
  canvas.addEventListener("click", () => {
    if (canvas.dataset.wasDrag === "true") return;
    if (state.hoverTile) {
      state.selectedTile = { ...state.hoverTile };
      updateTileInspector();
      requestRender();
    }
  });

  // Restore last world if available, otherwise generate with defaults
  restoreLastWorld();
  generateWorld();
}

boot();
