export interface IWorldBridge {
  width(): number;
  height(): number;
  get_tile_data(
    vx: number,
    vy: number,
    vw: number,
    vh: number,
  ): Uint8Array;
  tile_info(x: number, y: number): string;
  region_stats(): string;
  free(): void;
}

export interface WorldBridgeConstructor {
  new (
    seed: number,
    width: number,
    height: number,
    seaLevel: number,
  ): IWorldBridge;
}

export interface TileCoord {
  x: number;
  y: number;
}

export interface TileInfo {
  terrain: string;
  altitude: number;
  moisture: number;
  temperature: number;
  x: number;
  y: number;
  region_id: number;
  region_type: "Land" | "Water" | "Unknown";
  region_area: number;
  coastal: boolean;
}

export interface RegionStats {
  land_regions: number;
  water_regions: number;
  land_tiles: number;
  water_tiles: number;
  total_tiles: number;
  land_coverage: number;
  largest_landmass_pct: number;
  islands: number;
}

export interface TerrainDef {
  id: number;
  name: string;
  h: number;
  s: number;
  l: number;
}
