import {
  type WorldSaveV1,
  type SavedSnapshotRecordMeta,
  validateSave,
  detectSaveFormat,
  parseSnapshotMeta,
} from "./world-save";

export type ImportPayload =
  | { type: "snapshot"; json: string; meta: SavedSnapshotRecordMeta }
  | { type: "legacy_config"; save: WorldSaveV1 }
  | { type: "invalid"; error: string };

export function exportSnapshot(
  json: string,
  name = "World",
  tick: number | bigint = 0,
): void {
  const blob = new Blob([json], { type: "application/json" });
  if (typeof URL !== "undefined" && typeof document !== "undefined") {
    const url = URL.createObjectURL(blob);
    const a = document.createElement("a");
    const safeName = name.replace(/[^a-z0-9_-]/gi, "_").toLowerCase() || "world";
    a.href = url;
    a.download = `nexus-${safeName}-tick-${tick}.json`;
    a.click();
    URL.revokeObjectURL(url);
  }
}

export function exportSave(save: WorldSaveV1): void {
  const json = JSON.stringify(save, null, 2);
  const blob = new Blob([json], { type: "application/json" });
  if (typeof URL !== "undefined" && typeof document !== "undefined") {
    const url = URL.createObjectURL(blob);
    const a = document.createElement("a");
    const safeName = save.name.replace(/[^a-z0-9_-]/gi, "_").toLowerCase() || "world";
    a.href = url;
    a.download = `nexus-${safeName}.json`;
    a.click();
    URL.revokeObjectURL(url);
  }
}

export async function parseImportFile(file: File): Promise<ImportPayload> {
  try {
    const rawText = await file.text();
    const data = JSON.parse(rawText);
    const format = detectSaveFormat(data);

    if (format === "snapshot") {
      const meta = parseSnapshotMeta(rawText, file.name.replace(/\.json$/i, ""));
      if (!meta) {
        return { type: "invalid", error: "Snapshot header missing or invalid" };
      }
      return { type: "snapshot", json: rawText, meta };
    } else if (format === "legacy_config") {
      const validated = validateSave(data);
      if (validated) {
        return { type: "legacy_config", save: validated };
      } else {
        return { type: "invalid", error: "Invalid legacy world configuration" };
      }
    } else {
      return {
        type: "invalid",
        error: "Unrecognized save format. Expected a Nexus snapshot (nexus-snapshot/v1) or legacy config.",
      };
    }
  } catch (err) {
    return {
      type: "invalid",
      error: err instanceof Error ? err.message : "Failed to parse JSON file",
    };
  }
}
