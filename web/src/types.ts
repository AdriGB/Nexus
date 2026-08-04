export interface IWorldBridge {
  width(): number;
  height(): number;
  get_tile_data(vx: number, vy: number, vw: number, vh: number): Uint8Array;
  tile_info(x: number, y: number): string;
  free(): void;
}

export interface WorldBridgeConstructor {
  new (seed: number, width: number, height: number, seaLevel: number): IWorldBridge;
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
}

export interface TerrainDef {
  id: number;
  name: string;
  h: number;
  s: number;
  l: number;
}
