import { describe, it, expect, vi, beforeEach } from "vitest";
import {
  detectSaveFormat,
  parseSnapshotMeta,
  validateSave,
  createSave,
  type SnapshotHeaderV1,
} from "./world-save";
import {
  saveSnapshotRecord,
  listSnapshotRecords,
  loadSnapshotRecord,
  deleteSnapshotRecord,
  _clearMemoryStoreForTesting,
} from "./snapshot-store";
import {
  exportSnapshot,
  parseImportFile,
} from "./file-transfer";

const MOCK_SNAPSHOT_V1 = {
  header: {
    format: "nexus-snapshot/v1",
    engine_version: "0.1.0",
    hash_version: 1,
    created_at_tick: 420,
    paused: true,
    simulation_seed: 1234567,
    world_seed: 42,
    sea_level: 0.35,
    world_width: 256,
    world_height: 256,
    state_hash: "0xdeadbeefcafebabe1234567890abcdef",
  } satisfies SnapshotHeaderV1,
  grid: {
    width: 256,
    height: 256,
    tiles: [],
  },
  simulation: {
    scalars: {
      tick: 420,
      seed: 1234567,
      births: 15,
      deaths: 2,
      food_consumed: 350,
      next_entity_id: 200,
      next_household_id: 10,
    },
    entities: [{ id: 1 }, { id: 2 }, { id: 3 }],
    households: [{ id: 1 }, { id: 2 }],
  },
};

const MOCK_LEGACY_SAVE = {
  formatVersion: 1 as const,
  generatorVersion: "0.1.0",
  name: "Old World",
  createdAt: "2026-08-01T12:00:00.000Z",
  config: {
    seed: 99,
    width: 256,
    height: 256,
    seaLevel: 0.35,
  },
};

describe("Save Format Detection & Metadata Parsing", () => {
  it("detects Snapshot V1 format correctly", () => {
    expect(detectSaveFormat(MOCK_SNAPSHOT_V1)).toBe("snapshot");
  });

  it("detects legacy config format correctly", () => {
    expect(detectSaveFormat(MOCK_LEGACY_SAVE)).toBe("legacy_config");
  });

  it("detects invalid payloads", () => {
    expect(detectSaveFormat(null)).toBe("invalid");
    expect(detectSaveFormat({})).toBe("invalid");
    expect(detectSaveFormat({ header: { format: "unknown-v9" } })).toBe("invalid");
    expect(detectSaveFormat("string")).toBe("invalid");
  });

  it("parses Snapshot V1 metadata accurately", () => {
    const json = JSON.stringify(MOCK_SNAPSHOT_V1);
    const meta = parseSnapshotMeta(json, "My Colony", "test_id_1");

    expect(meta).not.toBeNull();
    expect(meta?.id).toBe("test_id_1");
    expect(meta?.name).toBe("My Colony");
    expect(meta?.tick).toBe(420);
    expect(meta?.population).toBe(3);
    expect(meta?.households).toBe(2);
    expect(meta?.stateHash).toBe("0xdeadbeefcafebabe1234567890abcdef");
    expect(meta?.worldWidth).toBe(256);
    expect(meta?.worldHeight).toBe(256);
    expect(meta?.worldSeed).toBe(42);
    expect(meta?.seaLevel).toBe(0.35);
  });

  it("returns null when parsing invalid JSON as snapshot metadata", () => {
    expect(parseSnapshotMeta("not-json")).toBeNull();
    expect(parseSnapshotMeta(JSON.stringify(MOCK_LEGACY_SAVE))).toBeNull();
  });
});

describe("Snapshot Store (IndexedDB / Memory fallback)", () => {
  beforeEach(() => {
    _clearMemoryStoreForTesting();
  });

  it("saves, lists, loads, and deletes snapshot records", async () => {
    const json = JSON.stringify(MOCK_SNAPSHOT_V1);
    const meta = parseSnapshotMeta(json, "Alpha Save", "slot_alpha");
    expect(meta).not.toBeNull();

    await saveSnapshotRecord({
      ...meta!,
      snapshotJson: json,
    });

    const list = await listSnapshotRecords();
    const found = list.find((s) => s.id === "slot_alpha");
    expect(found).toBeDefined();
    expect(found?.name).toBe("Alpha Save");
    expect(found?.tick).toBe(420);
    // listSnapshotRecords must return metadata only, never materializing snapshotJson
    expect((found as any).snapshotJson).toBeUndefined();

    const loaded = await loadSnapshotRecord("slot_alpha");
    expect(loaded).not.toBeNull();
    expect(loaded?.snapshotJson).toBe(json);

    await deleteSnapshotRecord("slot_alpha");
    const loadedAfterDelete = await loadSnapshotRecord("slot_alpha");
    expect(loadedAfterDelete).toBeNull();
  });

  it("rejects when IndexedDB open fails instead of silently falling back", async () => {
    const originalIDB = globalThis.indexedDB;
    try {
      const mockReq: {
        onerror: ((e: any) => void) | null;
        onsuccess: ((e: any) => void) | null;
        error: Error;
      } = {
        onerror: null,
        onsuccess: null,
        error: new Error("Disk quota exceeded or database locked"),
      };
      (globalThis as any).indexedDB = {
        open: vi.fn(() => {
          setTimeout(() => mockReq.onerror?.(new Event("error")), 0);
          return mockReq;
        }),
      };

      const json = JSON.stringify(MOCK_SNAPSHOT_V1);
      const meta = parseSnapshotMeta(json, "Beta Save", "slot_beta")!;
      await expect(
        saveSnapshotRecord({ ...meta, snapshotJson: json }),
      ).rejects.toThrow("Disk quota exceeded or database locked");

      // Verify it did NOT quietly save to memory
      (globalThis as any).indexedDB = undefined;
      const list = await listSnapshotRecords();
      expect(list.find((s) => s.id === "slot_beta")).toBeUndefined();
    } finally {
      globalThis.indexedDB = originalIDB;
    }
  });
});

describe("File Transfer (Import & Export)", () => {
  beforeEach(() => {
    vi.restoreAllMocks();
  });

  it("parses an imported Snapshot V1 file", async () => {
    const json = JSON.stringify(MOCK_SNAPSHOT_V1);
    const file = new File([json], "nexus-colony-tick-420.json", {
      type: "application/json",
    });

    const result = await parseImportFile(file);
    expect(result.type).toBe("snapshot");
    if (result.type === "snapshot") {
      expect(result.meta.tick).toBe(420);
      expect(result.meta.stateHash).toBe("0xdeadbeefcafebabe1234567890abcdef");
      expect(result.json).toBe(json);
    }
  });

  it("parses an imported legacy save file", async () => {
    const json = JSON.stringify(MOCK_LEGACY_SAVE);
    const file = new File([json], "legacy.json", {
      type: "application/json",
    });

    const result = await parseImportFile(file);
    expect(result.type).toBe("legacy_config");
    if (result.type === "legacy_config") {
      expect(result.save.name).toBe("Old World");
      expect(result.save.config.seed).toBe(99);
    }
  });

  it("returns invalid on corrupt JSON file", async () => {
    const file = new File(["{ broken json"], "corrupt.json", {
      type: "application/json",
    });

    const result = await parseImportFile(file);
    expect(result.type).toBe("invalid");
  });

  it("triggers file download on exportSnapshot", () => {
    const createObjectURLMock = vi.fn().mockReturnValue("blob:nexus-mock");
    const revokeObjectURLMock = vi.fn();
    const originalURL = globalThis.URL;
    const originalDocument = globalThis.document;

    const clickMock = vi.fn();
    const mockElement = {
      click: clickMock,
      href: "",
      download: "",
    };

    globalThis.URL = {
      ...originalURL,
      createObjectURL: createObjectURLMock,
      revokeObjectURL: revokeObjectURLMock,
    } as unknown as typeof URL;

    globalThis.document = {
      createElement: vi.fn().mockReturnValue(mockElement),
    } as unknown as Document;

    try {
      exportSnapshot(JSON.stringify(MOCK_SNAPSHOT_V1), "My Settlement", 420);

      expect(createObjectURLMock).toHaveBeenCalled();
      expect(clickMock).toHaveBeenCalled();
      expect(mockElement.download).toBe("nexus-my_settlement-tick-420.json");
      expect(revokeObjectURLMock).toHaveBeenCalledWith("blob:nexus-mock");
    } finally {
      globalThis.URL = originalURL;
      globalThis.document = originalDocument;
    }
  });
});
