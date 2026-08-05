import { type WorldSaveV1, validateSave } from "./world-save";

const SAVES_KEY = "nexus_saves";
const AUTOSAVE_KEY = "nexus_autosave";

export function loadAllSaves(): WorldSaveV1[] {
  try {
    const raw = localStorage.getItem(SAVES_KEY);
    if (!raw) return [];
    const arr = JSON.parse(raw);
    if (!Array.isArray(arr)) return [];
    return arr.map(validateSave).filter((s): s is WorldSaveV1 => s !== null);
  } catch {
    return [];
  }
}

export function addSave(save: WorldSaveV1): void {
  const all = loadAllSaves();
  all.push(save);
  try {
    localStorage.setItem(SAVES_KEY, JSON.stringify(all));
  } catch (e) {
    console.warn("Failed to save to localStorage:", e);
  }
}

export function deleteSave(index: number): void {
  const all = loadAllSaves();
  all.splice(index, 1);
  localStorage.setItem(SAVES_KEY, JSON.stringify(all));
}

export function saveAutoSave(save: WorldSaveV1): void {
  try {
    localStorage.setItem(AUTOSAVE_KEY, JSON.stringify(save));
  } catch {
    /* ignore quota errors */
  }
}

export function loadAutoSave(): WorldSaveV1 | null {
  try {
    const raw = localStorage.getItem(AUTOSAVE_KEY);
    if (!raw) return null;
    return validateSave(JSON.parse(raw));
  } catch {
    return null;
  }
}
