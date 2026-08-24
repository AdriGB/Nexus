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
  transfer_inventory_item(
    sourceId: number,
    targetId: number,
    kind: "food" | "timber" | "stone" | "iron",
    quantity: number,
  ): number;
  deposit_household_item(
    entityId: number,
    kind: "food" | "timber" | "stone" | "iron",
    quantity: number,
  ): number;
  withdraw_household_item(
    entityId: number,
    kind: "food" | "timber" | "stone" | "iron",
    quantity: number,
  ): number;
  population_stats(): string;
  first_entity_info(): string;
  first_entity_relationships(): string;
  first_entity_kinship(): string;
  first_entity_household(): string;
  recent_interaction_events(entityId?: number): string;
  recent_events(entityId?: number): string;
  entity_event_summary(entityId: number): string;
  entity_info(id: number): string;
  entity_kinship(id: number): string;
  entity_household(id: number): string;
  entity_relationship(firstId: number, secondId: number): string;
  entity_family_tree(entityId: number, ancestorDepth: number, descendantDepth: number): string;
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
  action_progress_ticks: number;
  action_duration_ticks: number | null;
  goal_age_ticks: number;
  known_resources: number;
  known_entities: number;
  known_chunks: number;
  visible_entities: number;
  movement_credit: number;
  life_stage: string;
  stage_movement_factor: number;
  caregiver_id: number | null;
  partner_id: number | null;
  household_id: number | null;
  mother_id: number | null;
  father_id: number | null;
  personality: {
    curiosity: number;
    sociability: number;
    cooperativeness: number;
    caution: number;
    persistence: number;
  };
  utilities: {
    eat: number;
    acquire_resource: number;
    explore: number;
    rest: number;
    socialize: number;
    share_food: number;
  };
  decision_explanation: {
    chosen_goal: EntityInfo["goal"];
    highest_utility_goal: EntityInfo["goal"];
    chosen_score: number;
    highest_score: number;
    switch_margin: number;
    reason:
      | "highest_utility"
      | "goal_persistence"
      | "dependent_needs_food"
      | "dependent_follows_caregiver";
  } | null;
  inventory: {
    capacity: number;
    used_capacity: number;
    remaining_capacity: number;
    items: Array<{
      kind: "Food" | "Timber" | "Stone" | "Iron";
      amount: number;
    }>;
  };
}

export interface EntityKinship {
  mother_id: number | null;
  father_id: number | null;
  children_ids: number[];
  sibling_ids: number[];
  ancestors: KinshipGeneration[];
  descendants: KinshipGeneration[];
}

export interface EntityHousehold {
  household_id: number | null;
  member_ids: number[];
  formed_tick: number | null;
  residence_x: number | null;
  residence_y: number | null;
  storage: EntityInfo["inventory"] | null;
}

export interface KinshipGeneration {
  entity_id: number;
  generation: number;
}

export type KinshipRelation =
  | { kind: "same_person" | "parent" | "child" | "full_sibling" | "half_sibling" | "unrelated" }
  | { kind: "ancestor" | "descendant"; generations: number }
  | { kind: "aunt_uncle" | "niece_nephew"; generations_removed: number }
  | { kind: "cousin"; degree: number; removed: number };

export interface FamilyTree {
  focal_id: number;
  nodes: FamilyTreeNode[];
  edges: FamilyTreeEdge[];
}

export interface FamilyTreeNode {
  entity_id: number;
  generation: number;
  alive: boolean;
}

export interface FamilyTreeEdge {
  parent_id: number;
  child_id: number;
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

interface SimulationEventBase {
  id: string;
  caused_by_event_id: string | null;
  tick: string;
  relative_time: string;
  location: TileCoord;
  actor_id: number;
  target_id: number | null;
  related_entity_ids: number[];
  refused?: boolean | null;
  partnership_actor_affinity?: number | null;
  partnership_target_affinity?: number | null;
  compatibility_per_mille?: number | null;
}

export interface InteractionEvent extends SimulationEventBase {
  kind: "interaction";
  cause: "mutual_social_contact";
  actor_affinity_delta: number;
  target_affinity_delta: number;
  child_id: null;
  amount: null;
  resource_kind: null;
  previous_affinity: null;
  new_affinity: null;
  delta: null;
}

export interface BirthEvent extends SimulationEventBase {
  kind: "birth";
  cause: "born";
  actor_affinity_delta: null;
  target_affinity_delta: null;
  child_id: number;
  amount: null;
  resource_kind: null;
  previous_affinity: null;
  new_affinity: null;
  delta: null;
}

export interface DeathEvent extends SimulationEventBase {
  kind: "death";
  cause: "starvation" | "natural_death";
  actor_affinity_delta: null;
  target_affinity_delta: null;
  child_id: null;
  amount: null;
  resource_kind: null;
  previous_affinity: null;
  new_affinity: null;
  delta: null;
}

export interface ConsumptionEvent extends SimulationEventBase {
  kind: "consumption";
  cause: "ate_food";
  actor_affinity_delta: null;
  target_affinity_delta: null;
  child_id: null;
  amount: number;
  resource_kind: null;
  previous_affinity: null;
  new_affinity: null;
  delta: null;
}

export interface ResourceDiscoveryEvent extends SimulationEventBase {
  kind: "discovery";
  cause: "resource_found";
  actor_affinity_delta: null;
  target_affinity_delta: null;
  child_id: null;
  amount: number;
  resource_kind: "food" | "timber" | "stone" | "iron";
  previous_affinity: null;
  new_affinity: null;
  delta: null;
}

export interface EncounterEvent extends SimulationEventBase {
  kind: "encounter";
  cause: "first_encounter";
  target_id: number;
  actor_affinity_delta: null;
  target_affinity_delta: null;
  child_id: null;
  amount: null;
  resource_kind: null;
  previous_affinity: null;
  new_affinity: null;
  delta: null;
}

export interface AffinityChangeEvent extends SimulationEventBase {
  kind: "affinity_change";
  cause: "mutual_social_contact" | "relationship_decay";
  target_id: number;
  actor_affinity_delta: null;
  target_affinity_delta: null;
  child_id: null;
  amount: null;
  resource_kind: null;
  previous_affinity: number;
  new_affinity: number;
  delta: number;
}

interface FoodShareEventBase extends SimulationEventBase {
  target_id: number;
  actor_affinity_delta: null;
  target_affinity_delta: null;
  child_id: null;
  resource_kind: null;
  previous_affinity: null;
  new_affinity: null;
  delta: null;
  refused: boolean;
}

export interface FoodSharedEvent extends FoodShareEventBase {
  kind: "food_shared";
  cause: "food_shared";
  amount: number;
  refused: false;
}

export interface FoodShareRefusedEvent extends FoodShareEventBase {
  kind: "food_share_refused";
  cause: "food_share_refused";
  amount: null;
  refused: true;
}

export interface PartnershipFormedEvent extends SimulationEventBase {
  kind: "partnership_formed";
  cause: "mutual_commitment";
  target_id: number;
  actor_affinity_delta: null;
  target_affinity_delta: null;
  child_id: null;
  amount: null;
  resource_kind: null;
  previous_affinity: null;
  new_affinity: null;
  delta: null;
  partnership_actor_affinity: number;
  partnership_target_affinity: number;
  compatibility_per_mille: number;
}

export interface PartnershipDissolvedEvent extends SimulationEventBase {
  kind: "partnership_dissolved";
  cause: "mutual_social_contact" | "food_share_refused" | "relationship_decay";
  target_id: number;
  actor_affinity_delta: null;
  target_affinity_delta: null;
  child_id: null;
  amount: null;
  resource_kind: null;
  previous_affinity: null;
  new_affinity: null;
  delta: null;
  partnership_actor_affinity: number;
  partnership_target_affinity: number;
  compatibility_per_mille: null;
}

export type SimulationEvent =
  | InteractionEvent
  | BirthEvent
  | DeathEvent
  | ConsumptionEvent
  | ResourceDiscoveryEvent
  | EncounterEvent
  | AffinityChangeEvent
  | FoodSharedEvent
  | FoodShareRefusedEvent
  | PartnershipFormedEvent
  | PartnershipDissolvedEvent;

export interface EntityEventSummary {
  entity_id: number;
  total_events: number;
  first_event_tick: string | null;
  latest_event_tick: string | null;
  births: number;
  deaths: number;
  consumptions: number;
  discoveries: number;
  encounters: number;
  interactions: number;
  affinity_changes: number;
  partnerships_formed: number;
  partnerships_dissolved: number;
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
