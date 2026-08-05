import { readParams } from "./controls";
import { createSave, type WorldConfig, type WorldSaveV1 } from "../persistence/world-save";
import {
  loadAllSaves,
  addSave,
  deleteSave,
  saveAutoSave,
  loadAutoSave,
} from "../persistence/local-storage";
import { exportSave, parseImportFile } from "../persistence/file-transfer";

const STORAGE_KEY = "nexus_saves";

function currentConfig(): WorldConfig {
  const p = readParams();
  return { seed: p.seed, width: p.width, height: p.height, seaLevel: p.sea };
}

function applyConfig(config: WorldConfig): void {
  (document.getElementById("seed-input") as HTMLInputElement).value = String(config.seed);
  (document.getElementById("width-input") as HTMLInputElement).value = String(config.width);
  (document.getElementById("height-input") as HTMLInputElement).value = String(config.height);
  const seaSlider = document.getElementById("sea-slider") as HTMLInputElement;
  seaSlider.value = String(config.seaLevel);
  document.getElementById("sea-val")!.textContent = config.seaLevel.toFixed(2);
}

/* ── Saved list rendering ────────────────── */

function renderSavedList(onLoad: () => void): void {
  const container = document.getElementById("saved-worlds-list")!;
  const saves = loadAllSaves();

  if (saves.length === 0) {
    container.innerHTML = '<div class="saved-list-empty">No saved worlds yet</div>';
    return;
  }

  container.innerHTML = "";
  saves.forEach((save, index) => {
    const item = document.createElement("div");
    item.className = "save-item";

    const dateStr = save.createdAt
      ? new Date(save.createdAt).toLocaleDateString(undefined, {
          month: "short",
          day: "numeric",
          hour: "2-digit",
          minute: "2-digit",
        })
      : "";

    item.innerHTML = `
      <div class="save-item-info">
        <div class="save-item-name">${escapeHtml(save.name)}</div>
        <div class="save-item-meta">seed ${save.config.seed} \u00b7 ${save.config.width}\u00d7${save.config.height} \u00b7 sea ${save.config.seaLevel.toFixed(2)}${dateStr ? " \u00b7 " + dateStr : ""}</div>
      </div>
      <div class="save-item-actions">
        <button class="save-btn-sm" data-action="load" title="Load">&#9654;</button>
        <button class="save-btn-sm danger" data-action="delete" title="Delete">&times;</button>
      </div>
    `;

    // Click on info area → load
    item.querySelector(".save-item-info")!.addEventListener("click", () => {
      applyConfig(save.config);
      onLoad();
    });

    // Load button
    item.querySelector('[data-action="load"]')!.addEventListener("click", (e) => {
      e.stopPropagation();
      applyConfig(save.config);
      onLoad();
    });

    // Delete button
    item.querySelector('[data-action="delete"]')!.addEventListener("click", (e) => {
      e.stopPropagation();
      deleteSave(index);
      renderSavedList(onLoad);
    });

    container.appendChild(item);
  });
}

function escapeHtml(s: string): string {
  const div = document.createElement("div");
  div.textContent = s;
  return div.innerHTML;
}

/* ── Public API ──────────────────────────── */

export function bindSaveControls(generateFn: () => void): void {
  const nameInput = document.getElementById("world-name-input") as HTMLInputElement;
  const btnSave = document.getElementById("btn-save-world")!;
  const btnExport = document.getElementById("btn-export-world")!;
  const importInput = document.getElementById("import-file-input") as HTMLInputElement;

  // Save button
  btnSave.addEventListener("click", () => {
    const config = currentConfig();
    const save = createSave(nameInput.value, config);
    addSave(save);
    renderSavedList(generateFn);
    nameInput.value = "";
  });

  // Export button
  btnExport.addEventListener("click", () => {
    const config = currentConfig();
    const save = createSave(nameInput.value || "Export", config);
    exportSave(save);
  });

  // Import file input
  importInput.addEventListener("change", async () => {
    const file = importInput.files?.[0];
    if (!file) return;
    importInput.value = ""; // reset for re-import of same file

    const save = await parseImportFile(file);
    if (!save) {
      alert("Invalid save file. Must be a Nexus world JSON with formatVersion 1.");
      return;
    }

    // Optionally add to saves list
    addSave(save);
    applyConfig(save.config);
    renderSavedList(generateFn);
    generateFn();
  });

  // Initial render
  renderSavedList(generateFn);
}

export function autoSave(): void {
  const config = currentConfig();
  const save = createSave("", config);
  saveAutoSave(save);
}

export function restoreLastWorld(): boolean {
  const save = loadAutoSave();
  if (!save) return false;
  applyConfig(save.config);
  return true;
}
