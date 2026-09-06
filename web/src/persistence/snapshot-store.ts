import type {
  SavedSnapshotRecord,
  SavedSnapshotRecordMeta,
} from "./world-save";

const DB_NAME = "nexus_db";
const STORE_META = "snapshot_meta";
const STORE_PAYLOAD = "snapshot_payload";
const DB_VERSION = 2;

// In-memory fallback for environments without IndexedDB (e.g. Node tests, strict iframe sandbox)
const memoryMetaStore = new Map<string, SavedSnapshotRecordMeta>();
const memoryPayloadStore = new Map<string, string>();

function isIndexedDbAvailable(): boolean {
  return typeof indexedDB !== "undefined";
}

function openDb(): Promise<IDBDatabase> {
  return new Promise((resolve, reject) => {
    if (!isIndexedDbAvailable()) {
      reject(new Error("IndexedDB is not available"));
      return;
    }

    const request = indexedDB.open(DB_NAME, DB_VERSION);

    request.onupgradeneeded = () => {
      const db = request.result;
      const tx = request.transaction;

      if (!db.objectStoreNames.contains(STORE_META)) {
        const metaStore = db.createObjectStore(STORE_META, { keyPath: "id" });
        metaStore.createIndex("createdAt", "createdAt", { unique: false });
        metaStore.createIndex("tick", "tick", { unique: false });
      }

      if (!db.objectStoreNames.contains(STORE_PAYLOAD)) {
        db.createObjectStore(STORE_PAYLOAD, { keyPath: "id" });
      }

      // If migrating from v1 where monolithic "snapshots" store existed
      if (db.objectStoreNames.contains("snapshots") && tx) {
        const oldStore = tx.objectStore("snapshots");
        const metaStore = tx.objectStore(STORE_META);
        const payloadStore = tx.objectStore(STORE_PAYLOAD);

        const cursorReq = oldStore.openCursor();
        cursorReq.onsuccess = () => {
          const cursor = cursorReq.result;
          if (cursor) {
            const val = cursor.value as SavedSnapshotRecord;
            metaStore.put(toMeta(val));
            payloadStore.put({ id: val.id, snapshotJson: val.snapshotJson });
            cursor.continue();
          } else {
            db.deleteObjectStore("snapshots");
          }
        };
      }
    };

    request.onsuccess = () => resolve(request.result);
    request.onerror = () =>
      reject(request.error || new Error("Failed to open IndexedDB"));
  });
}

/**
 * Persists a snapshot record. In browser environments with IndexedDB, this saves
 * the metadata to `snapshot_meta` and the full JSON body to `snapshot_payload`
 * within a single atomic transaction.
 *
 * If IndexedDB fails, this promise rejects with an actionable error—it NEVER
 * silently falls back to ephemeral memory.
 */
export async function saveSnapshotRecord(
  record: SavedSnapshotRecord,
): Promise<void> {
  const meta = toMeta(record);

  if (!isIndexedDbAvailable()) {
    memoryMetaStore.set(meta.id, meta);
    memoryPayloadStore.set(record.id, record.snapshotJson);
    return;
  }

  const db = await openDb();
  return new Promise((resolve, reject) => {
    const tx = db.transaction([STORE_META, STORE_PAYLOAD], "readwrite");
    const metaStore = tx.objectStore(STORE_META);
    const payloadStore = tx.objectStore(STORE_PAYLOAD);

    metaStore.put(meta);
    payloadStore.put({ id: record.id, snapshotJson: record.snapshotJson });

    tx.oncomplete = () => {
      db.close();
      resolve();
    };
    tx.onerror = () => {
      const err = tx.error || new Error("Failed to save snapshot to IndexedDB");
      db.close();
      reject(err);
    };
    tx.onabort = () => {
      const err =
        tx.error ||
        new Error("Transaction aborted while saving snapshot to IndexedDB");
      db.close();
      reject(err);
    };
  });
}

/**
 * Lists metadata for all saved snapshots. Queries ONLY the `snapshot_meta`
 * store, avoiding loading and deserializing multi-megabyte JSON payloads.
 */
export async function listSnapshotRecords(): Promise<
  SavedSnapshotRecordMeta[]
> {
  if (!isIndexedDbAvailable()) {
    return Array.from(memoryMetaStore.values()).sort((a, b) =>
      b.createdAt.localeCompare(a.createdAt),
    );
  }

  const db = await openDb();
  return new Promise((resolve, reject) => {
    const tx = db.transaction(STORE_META, "readonly");
    const store = tx.objectStore(STORE_META);
    const req = store.getAll();

    req.onsuccess = () => {
      const metas = (req.result as SavedSnapshotRecordMeta[]) || [];
      metas.sort((a, b) => b.createdAt.localeCompare(a.createdAt));
      resolve(metas);
    };
    req.onerror = () => {
      reject(req.error || new Error("Failed to list snapshots from IndexedDB"));
    };
    tx.oncomplete = () => db.close();
    tx.onerror = () => {
      reject(tx.error || new Error("Transaction error while listing snapshots"));
    };
  });
}

/**
 * Loads a full snapshot record by ID, recombining metadata with the JSON payload.
 */
export async function loadSnapshotRecord(
  id: string,
): Promise<SavedSnapshotRecord | null> {
  if (!isIndexedDbAvailable()) {
    const meta = memoryMetaStore.get(id);
    const snapshotJson = memoryPayloadStore.get(id);
    if (!meta || !snapshotJson) return null;
    return { ...meta, snapshotJson };
  }

  const db = await openDb();
  return new Promise((resolve, reject) => {
    const tx = db.transaction([STORE_META, STORE_PAYLOAD], "readonly");
    const metaStore = tx.objectStore(STORE_META);
    const payloadStore = tx.objectStore(STORE_PAYLOAD);

    const metaReq = metaStore.get(id);
    const payloadReq = payloadStore.get(id);

    let meta: SavedSnapshotRecordMeta | null = null;
    let payload: { id: string; snapshotJson: string } | null = null;

    metaReq.onsuccess = () => {
      meta = (metaReq.result as SavedSnapshotRecordMeta) || null;
    };
    payloadReq.onsuccess = () => {
      payload =
        (payloadReq.result as { id: string; snapshotJson: string }) || null;
    };

    tx.oncomplete = () => {
      db.close();
      if (!meta || !payload) {
        resolve(null);
      } else {
        resolve({
          ...meta,
          snapshotJson: payload.snapshotJson,
        });
      }
    };
    tx.onerror = () => {
      const err =
        tx.error || new Error("Failed to load snapshot from IndexedDB");
      db.close();
      reject(err);
    };
  });
}

/**
 * Deletes a snapshot record from both metadata and payload stores atomically.
 */
export async function deleteSnapshotRecord(id: string): Promise<void> {
  if (!isIndexedDbAvailable()) {
    memoryMetaStore.delete(id);
    memoryPayloadStore.delete(id);
    return;
  }

  const db = await openDb();
  return new Promise((resolve, reject) => {
    const tx = db.transaction([STORE_META, STORE_PAYLOAD], "readwrite");
    tx.objectStore(STORE_META).delete(id);
    tx.objectStore(STORE_PAYLOAD).delete(id);

    tx.oncomplete = () => {
      db.close();
      resolve();
    };
    tx.onerror = () => {
      const err =
        tx.error || new Error("Failed to delete snapshot from IndexedDB");
      db.close();
      reject(err);
    };
  });
}

export function _clearMemoryStoreForTesting(): void {
  memoryMetaStore.clear();
  memoryPayloadStore.clear();
}

function toMeta(record: SavedSnapshotRecord): SavedSnapshotRecordMeta {
  return {
    id: record.id,
    name: record.name,
    createdAt: record.createdAt,
    tick: record.tick,
    population: record.population,
    households: record.households,
    stateHash: record.stateHash,
    worldWidth: record.worldWidth,
    worldHeight: record.worldHeight,
    worldSeed: record.worldSeed,
    seaLevel: record.seaLevel,
  };
}
