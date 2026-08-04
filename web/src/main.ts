import "./styles/main.css";

import { state, setRenderCallback, requestRender } from "./state";
import { loadWasm, createWorld } from "./wasm";
import { resizeCanvas, render } from "./renderer/world-renderer";
import { fitWorld, screenToTile, bindCamera } from "./renderer/camera";
import { renderMinimap, drawMinimapViewport } from "./renderer/minimap";
import { bindControls, readParams, updateWorldInfo } from "./ui/controls";
import { updateHover, hideTooltip } from "./ui/tooltip";
import { buildLegend } from "./ui/legend";
import {
  updateTileInspector,
  clearTileInspector,
} from "./ui/tile-inspector";

/* ── World generation ─────────────────────── */

function generateWorld(): void {
  const { seed, width, height, sea } = readParams();

  // FIX #5: Clear selection state before replacing world
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
}

/* ── Full render pass (FIX #7: no resizeCanvas here) ── */

function fullRender(): void {
  render();
  drawMinimapViewport();
}

/* ── Boot ─────────────────────────────────── */

async function boot(): Promise<void> {
  try {
    await loadWasm();
  } catch (err) {
    const loadingEl = document.getElementById("loading")!;
    const textEl = document.getElementById("loading-text")!;
    textEl.outerHTML = `<div class="error">
      <strong>Failed to load WASM engine</strong><br><br>
      ${(err as Error).message || err}<br><br>
      Build the engine first, then serve via HTTP:<br>
      <code>cd engine && wasm-pack build --target web --out-dir ../web/public/wasm<br>
      cd ../web && npm run dev</code>
    </div>`;
    return;
  }

  document.getElementById("loading")!.classList.add("done");
  setTimeout(() => document.getElementById("loading")!.remove(), 600);

  // Set render callback
  setRenderCallback(fullRender);

  // Build static UI
  buildLegend();

  // FIX #7: Resize once at startup, then only on window resize
  resizeCanvas();

  // Bind input
  bindCamera(document.getElementById("world-canvas") as HTMLCanvasElement);
  bindControls(generateWorld);

  // Hover + click
  const canvas = document.getElementById("world-canvas") as HTMLCanvasElement;

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

  canvas.addEventListener("click", () => {
    if (canvas.dataset.wasDrag === "true") return;
    if (state.hoverTile) {
      state.selectedTile = { ...state.hoverTile };
      updateTileInspector();
      requestRender();
    }
  });

  // Generate first world
  generateWorld();
}

boot();
