export interface IWorldBridge {
  width(): number;
  height(): number;
  simulation_tick(): bigint;
  simulation_is_paused(): boolean;
  simulation_advance(ticks: number): bigint;
  simulation_step(): bigint;
  simulation_pause(): void;
  simulation_resume(): void;
  simulation_world_revision(): bigint;
  entity_count(): number;
  spawn_entities(count: number): number;
  population_stats(): string;
  first_entity_info(): string;
  first_entity_relationships(): string;
  recent_interaction_events(entityId?: number): string;
  entity_info(id: number): string;
  get_tile_data(
    vx: number,
    vy: number,
    vw: number,
    vh: number,
  ): Uint8Array;
  tile_info(x: number, y: number): string;
  region_stats(): string;
  find_path(
    startX: number,
    startY: number,
    goalX: number,
    goalY: number,
  ): Uint32Array;
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

export interface IGpuRendererBridge {
  resize(width: number, height: number, dpr: number): void;
  upload_world(world: IWorldBridge): void;
  upload_route(coordinates: Uint32Array): void;
  upload_entities(world: IWorldBridge): void;
  upload_resources(world: IWorldBridge): void;
  render(
    panX: number,
    panY: number,
    zoom: number,
    hoverX: number,
    hoverY: number,
    selectedX: number,
    selectedY: number,
    showGrid: boolean,
    showResources: boolean,
  ): void;
  free(): void;
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
  walkable: boolean;
  movement_cost: number | null;
  resource: {
    kind: "Food" | "Timber" | "Stone" | "Iron";
    amount: number;
  } | null;
}

export interface EntityInfo {
  id: number;
  x: number;
  y: number;
  hunger: number;
  health: number;
  sex: "Female" | "Male";
  age_ticks: number;
  age_years: number;
  lifespan_ticks: number;
  pregnant: boolean;
  pregnancy_due_tick: number | null;
  activity:
    | "Idle"
    | "Seeking food"
    | "Moving"
    | "Starving"
    | "Exploring"
    | "Resting"
    | "Socializing";
  remaining_path: number;
  goal: "None" | "Eat" | "Explore" | "Follow" | "Rest" | "Socialize";
  action: string;
  goal_age_ticks: number;
  known_resources: number;
  known_entities: number;
  known_chunks: number;
  visible_entities: number;
  movement_credit: number;
  life_stage: string;
  stage_movement_factor: number;
  caregiver_id: number | null;
  personality: {
    curiosity: number;
    sociability: number;
    cooperativeness: number;
    caution: number;
    persistence: number;
  };
  utilities: {
    eat: number;
    explore: number;
    rest: number;
  };
}

export interface KnownRelationshipInfo {
  id: number;
  affinity: number;
  interaction_count: number;
  first_seen_tick: number;
  last_seen_tick: number;
  last_interaction_tick: number;
  last_seen_x: number;
  last_seen_y: number;
  observed_ticks: number;
  seek_retry_after_tick: number | null;
}

export interface InteractionEvent {
  id: string;
  tick: string;
  relative_time: string;
  location: TileCoord;
  actor_id: number;
  target_id: number;
  related_entity_ids: number[];
  kind: "interaction";
  cause: "mutual_social_contact";
  actor_affinity_delta: number;
  target_affinity_delta: number;
}

export interface PopulationStats {
  population: number;
  births: number;
  deaths: number;
  females: number;
  males: number;
  pregnant: number;
  hungry: number;
  seeking_food: number;
  average_hunger: number;
  food_consumed: number;
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
