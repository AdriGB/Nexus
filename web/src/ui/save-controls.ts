import { readParams } from "./controls";
import { state } from "../state";
import {
  type WorldConfig,
  type SavedSnapshotRecordMeta,
  parseSnapshotMeta,
} from "../persistence/world-save";
import {
  saveSnapshotRecord,
  listSnapshotRecords,
  loadSnapshotRecord,
  deleteSnapshotRecord,
} from "../persistence/snapshot-store";
import {
  loadAllSaves,
  deleteSave,
  saveAutoSave,
  loadAutoSave,
} from "../persistence/local-storage";
import {
  exportSnapshot,
  parseImportFile,
} from "../persistence/file-transfer";

export type RestoreSnapshotCallback = (
  json: string,
  meta?: SavedSnapshotRecordMeta | null,
) => Promise<void> | void;

let statusTimeout: number | null = null;

export function showStatusMessage(
  message: string,
  type: "success" | "error",
): void {
  const el = document.getElementById("save-status-msg");
  if (!el) return;

  el.textContent = message;
  el.className = `save-status-msg ${type}`;
  el.hidden = false;

  if (statusTimeout !== null) {
    clearTimeout(statusTimeout);
  }
  statusTimeout = window.setTimeout(() => {
    el.hidden = true;
    statusTimeout = null;
  }, 4500);
}

function currentConfig(): WorldConfig {
  const p = readParams();
  return { seed: p.seed, width: p.width, height: p.height, seaLevel: p.sea };
}

function applyConfig(config: WorldConfig): void {
  const seedInput = document.getElementById("seed-input") as HTMLInputElement | null;
  const widthInput = document.getElementById("width-input") as HTMLInputElement | null;
  const heightInput = document.getElementById("height-input") as HTMLInputElement | null;
  const seaSlider = document.getElementById("sea-slider") as HTMLInputElement | null;
  const seaVal = document.getElementById("sea-val");

  if (seedInput) seedInput.value = String(config.seed);
  if (widthInput) widthInput.value = String(config.width);
  if (heightInput) heightInput.value = String(config.height);
  if (seaSlider) {
    seaSlider.value = String(config.seaLevel);
    if (seaVal) seaVal.textContent = config.seaLevel.toFixed(2);
  }
}

/* ── Saved list rendering ────────────────── */

async function renderSavedList(
  onGenerate: () => void,
  onRestoreSnapshot: RestoreSnapshotCallback,
): Promise<void> {
  const container = document.getElementById("saved-worlds-list");
  if (!container) return;

  const snapshots = await listSnapshotRecords();
  const legacySaves = loadAllSaves();

  if (snapshots.length === 0 && legacySaves.length === 0) {
    container.innerHTML = '<div class="saved-list-empty">No saved worlds or snapshots yet</div>';
    return;
  }

  container.innerHTML = "";

  // Render modern Snapshot V1 items
  snapshots.forEach((save) => {
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

    const shortHash = save.stateHash ? save.stateHash.slice(0, 8) : "—";

    item.innerHTML = `
      <div class="save-item-info">
        <div class="save-item-name">${escapeHtml(save.name)}</div>
        <div class="save-item-meta">Tick ${save.tick.toLocaleString()} \u00b7 Pop ${save.population} \u00b7 <code>${escapeHtml(shortHash)}</code>${dateStr ? " \u00b7 " + dateStr : ""}</div>
      </div>
      <div class="save-item-actions">
        <button class="save-btn-sm" data-action="load" title="Load snapshot">&#9654;</button>
        <button class="save-btn-sm" data-action="export" title="Export snapshot JSON">&#8595;</button>
        <button class="save-btn-sm danger" data-action="delete" title="Delete snapshot">&times;</button>
      </div>
    `;

    const triggerLoad = async () => {
      try {
        const full = await loadSnapshotRecord(save.id);
        if (!full) {
          showStatusMessage("Snapshot record not found in storage", "error");
          return;
        }
        await onRestoreSnapshot(full.snapshotJson, full);
        showStatusMessage(`Loaded snapshot "${save.name}" (Tick ${save.tick})`, "success");
      } catch (err) {
        showStatusMessage(`Failed to load snapshot: ${(err as Error).message || err}`, "error");
      }
    };

    item.querySelector(".save-item-info")!.addEventListener("click", triggerLoad);
    item.querySelector('[data-action="load"]')!.addEventListener("click", (e) => {
      e.stopPropagation();
      triggerLoad();
    });

    item.querySelector('[data-action="export"]')!.addEventListener("click", async (e) => {
      e.stopPropagation();
      const full = await loadSnapshotRecord(save.id);
      if (full) {
        exportSnapshot(full.snapshotJson, full.name, full.tick);
      }
    });

    item.querySelector('[data-action="delete"]')!.addEventListener("click", async (e) => {
      e.stopPropagation();
      await deleteSnapshotRecord(save.id);
      renderSavedList(onGenerate, onRestoreSnapshot);
    });

    container.appendChild(item);
  });

  // Render legacy config items (if any)
  legacySaves.forEach((legacy, index) => {
    const item = document.createElement("div");
    item.className = "save-item legacy";

    item.innerHTML = `
      <div class="save-item-info">
        <div class="save-item-name">${escapeHtml(legacy.name)} <span style="font-size:8px;color:var(--text-muted);">[Legacy seed]</span></div>
        <div class="save-item-meta">seed ${legacy.config.seed} \u00b7 ${legacy.config.width}\u00d7${legacy.config.height} \u00b7 sea ${legacy.config.seaLevel.toFixed(2)}</div>
      </div>
      <div class="save-item-actions">
        <button class="save-btn-sm" data-action="load" title="Generate from seed">&#9654;</button>
        <button class="save-btn-sm danger" data-action="delete" title="Delete">&times;</button>
      </div>
    `;

    const triggerLegacyLoad = () => {
      applyConfig(legacy.config);
      onGenerate();
      showStatusMessage(`Generated world from legacy seed (${legacy.config.seed})`, "success");
    };

    item.querySelector(".save-item-info")!.addEventListener("click", triggerLegacyLoad);
    item.querySelector('[data-action="load"]')!.addEventListener("click", (e) => {
      e.stopPropagation();
      triggerLegacyLoad();
    });

    item.querySelector('[data-action="delete"]')!.addEventListener("click", (e) => {
      e.stopPropagation();
      deleteSave(index);
      renderSavedList(onGenerate, onRestoreSnapshot);
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

export function bindSaveControls(
  onGenerate: () => void,
  onRestoreSnapshot: RestoreSnapshotCallback,
): void {
  const nameInput = document.getElementById("world-name-input") as HTMLInputElement;
  const btnSave = document.getElementById("btn-save-world")!;
  const btnExport = document.getElementById("btn-export-world")!;
  const importInput = document.getElementById("import-file-input") as HTMLInputElement;

  // Save button
  btnSave.addEventListener("click", async () => {
    if (!state.world) {
      showStatusMessage("No active simulation to save", "error");
      return;
    }

    try {
      const cfg = currentConfig();
      const json = state.world.save_snapshot_pretty(cfg.seed, cfg.seaLevel);
      const meta = parseSnapshotMeta(json, nameInput.value);

      if (!meta) {
        showStatusMessage("Failed to generate snapshot metadata", "error");
        return;
      }

      await saveSnapshotRecord({ ...meta, snapshotJson: json });
      showStatusMessage(`Saved snapshot "${meta.name}" (Tick ${meta.tick})`, "success");
      nameInput.value = "";
      renderSavedList(onGenerate, onRestoreSnapshot);
    } catch (err) {
      showStatusMessage(`Save failed: ${(err as Error).message || err}`, "error");
    }
  });

  // Export button
  btnExport.addEventListener("click", () => {
    if (!state.world) {
      showStatusMessage("No active simulation to export", "error");
      return;
    }

    try {
      const cfg = currentConfig();
      const json = state.world.save_snapshot_pretty(cfg.seed, cfg.seaLevel);
      const meta = parseSnapshotMeta(json, nameInput.value);
      exportSnapshot(json, nameInput.value || "world", meta?.tick ?? 0);
      showStatusMessage("Snapshot exported to JSON file", "success");
    } catch (err) {
      showStatusMessage(`Export failed: ${(err as Error).message || err}`, "error");
    }
  });

  // Import file input
  importInput.addEventListener("change", async () => {
    const file = importInput.files?.[0];
    if (!file) return;
    importInput.value = ""; // Reset for re-import

    const payload = await parseImportFile(file);

    if (payload.type === "snapshot") {
      try {
        await onRestoreSnapshot(payload.json, payload.meta);
        await saveSnapshotRecord({ ...payload.meta, snapshotJson: payload.json });
        renderSavedList(onGenerate, onRestoreSnapshot);
        showStatusMessage(
          `Restored snapshot: Tick ${payload.meta.tick} (Hash verified: ${payload.meta.stateHash.slice(0, 8)}…)`,
          "success",
        );
      } catch (err) {
        showStatusMessage(`Failed to restore snapshot: ${(err as Error).message || err}`, "error");
      }
    } else if (payload.type === "legacy_config") {
      applyConfig(payload.save.config);
      onGenerate();
      showStatusMessage(
        `Loaded legacy configuration "${payload.save.name}" (Tick 0)`,
        "success",
      );
    } else {
      showStatusMessage(`Import error: ${payload.error}`, "error");
    }
  });

  // Initial render
  renderSavedList(onGenerate, onRestoreSnapshot);
}

export function autoSave(): void {
  const config = currentConfig();
  const save = {
    formatVersion: 1 as const,
    generatorVersion: "0.1.0",
    name: "",
    createdAt: new Date().toISOString(),
    config,
  };
  saveAutoSave(save);
}

export function restoreLastWorld(): boolean {
  const save = loadAutoSave();
  if (!save) return false;
  applyConfig(save.config);
  return true;
}

