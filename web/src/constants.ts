import type { TerrainDef } from "./types";

export const TERRAIN: TerrainDef[] = [
  { id: 0, name: "Deep Water", h: 215, s: 55, l: 14 },
  { id: 1, name: "Shallow Water", h: 205, s: 45, l: 24 },
  { id: 2, name: "Beach", h: 42, s: 50, l: 58 },
  { id: 3, name: "Plains", h: 85, s: 35, l: 38 },
  { id: 4, name: "Grassland", h: 95, s: 42, l: 32 },
  { id: 5, name: "Forest", h: 110, s: 45, l: 24 },
  { id: 6, name: "Dense Forest", h: 120, s: 50, l: 16 },
  { id: 7, name: "Hills", h: 50, s: 22, l: 38 },
  { id: 8, name: "Mountain", h: 30, s: 8, l: 44 },
  { id: 9, name: "Snow Peak", h: 210, s: 12, l: 88 },
  { id: 10, name: "Desert", h: 35, s: 58, l: 52 },
  { id: 11, name: "Swamp", h: 78, s: 30, l: 22 },
  { id: 12, name: "Tundra", h: 180, s: 10, l: 56 },
];

export const BASE_TILE = 16;
export const MIN_ZOOM = 0.01;
export const MAX_ZOOM = 12;
