import type {
  SavedSnapshotRecord,
  SavedSnapshotRecordMeta,
} from "./world-save";

const DB_NAME = "nexus_db";
const STORE_NAME = "snapshots";
const DB_VERSION = 1;

// In-memory fallback for environments without IndexedDB (e.g. Node tests, strict iframe sandbox)
const memoryStore = new Map<string, SavedSnapshotRecord>();

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
      if (!db.objectStoreNames.contains(STORE_NAME)) {
        const store = db.createObjectStore(STORE_NAME, { keyPath: "id" });
        store.createIndex("createdAt", "createdAt", { unique: false });
        store.createIndex("tick", "tick", { unique: false });
      }
    };

    request.onsuccess = () => resolve(request.result);
    request.onerror = () => reject(request.error || new Error("Failed to open IndexedDB"));
  });
}

export async function saveSnapshotRecord(record: SavedSnapshotRecord): Promise<void> {
  if (!isIndexedDbAvailable()) {
    memoryStore.set(record.id, record);
    return;
  }

  try {
    const db = await openDb();
    return new Promise((resolve, reject) => {
      const tx = db.transaction(STORE_NAME, "readwrite");
      const store = tx.objectStore(STORE_NAME);
      const req = store.put(record);

      req.onsuccess = () => resolve();
      req.onerror = () => reject(req.error || new Error("Failed to save snapshot to IndexedDB"));
      tx.oncomplete = () => db.close();
    });
  } catch (err) {
    // Fallback to memory store if opening or writing IndexedDB fails
    memoryStore.set(record.id, record);
  }
}

export async function listSnapshotRecords(): Promise<SavedSnapshotRecordMeta[]> {
  if (!isIndexedDbAvailable()) {
    return Array.from(memoryStore.values())
      .map(toMeta)
      .sort((a, b) => b.createdAt.localeCompare(a.createdAt));
  }

  try {
    const db = await openDb();
    return new Promise((resolve, reject) => {
      const tx = db.transaction(STORE_NAME, "readonly");
      const store = tx.objectStore(STORE_NAME);
      const req = store.getAll();

      req.onsuccess = () => {
        const records = (req.result as SavedSnapshotRecord[]) || [];
        const metas = records
          .map(toMeta)
          .sort((a, b) => b.createdAt.localeCompare(a.createdAt));
        resolve(metas);
      };
      req.onerror = () => reject(req.error || new Error("Failed to list snapshots from IndexedDB"));
      tx.oncomplete = () => db.close();
    });
  } catch {
    return Array.from(memoryStore.values())
      .map(toMeta)
      .sort((a, b) => b.createdAt.localeCompare(a.createdAt));
  }
}

export async function loadSnapshotRecord(id: string): Promise<SavedSnapshotRecord | null> {
  if (!isIndexedDbAvailable()) {
    return memoryStore.get(id) || null;
  }

  try {
    const db = await openDb();
    return new Promise((resolve, reject) => {
      const tx = db.transaction(STORE_NAME, "readonly");
      const store = tx.objectStore(STORE_NAME);
      const req = store.get(id);

      req.onsuccess = () => {
        const result = (req.result as SavedSnapshotRecord) || null;
        resolve(result);
      };
      req.onerror = () => reject(req.error || new Error("Failed to load snapshot from IndexedDB"));
      tx.oncomplete = () => db.close();
    });
  } catch {
    return memoryStore.get(id) || null;
  }
}

export async function deleteSnapshotRecord(id: string): Promise<void> {
  memoryStore.delete(id);

  if (!isIndexedDbAvailable()) {
    return;
  }

  try {
    const db = await openDb();
    return new Promise((resolve, reject) => {
      const tx = db.transaction(STORE_NAME, "readwrite");
      const store = tx.objectStore(STORE_NAME);
      const req = store.delete(id);

      req.onsuccess = () => resolve();
      req.onerror = () => reject(req.error || new Error("Failed to delete snapshot from IndexedDB"));
      tx.oncomplete = () => db.close();
    });
  } catch {
    // Ignored on fallback
  }
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
