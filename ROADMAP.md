# NEXUS Roadmap

## Phase 1 — World Viewer ✅

- [x] Perlin noise world generation
- [x] Canvas renderer with pan/zoom
- [x] Minimap
- [x] Tile inspector
- [x] Terrain legend
- [x] TypeScript + Vite + WASM architecture

## Phase 1.1 — Stabilization

- [x] CI checks for cargo fmt, clippy, tests, WASM, TypeScript, and production build
- [x] Reuse the WASM artifact between CI jobs
- [x] Save/load seed and parameters (localStorage)
- [x] Export/import world data as JSON
- [ ] LOD rendering or full-world image for extreme zoom-out

## Phase 1.2 — Geography

- [x] Region detection and connectivity analysis
- [ ] River generation (erosion simulation)
- [ ] Biome refinement (temperature + moisture gradients)

## Phase 1.3 — GPU Renderer

- [x] Add wgpu and create a WebGPU surface over a dedicated canvas
- [x] Draw a fullscreen primitive
- [x] Upload the world as one RGBA texture
- [x] Shade terrain colors in WGSL
- [x] Camera pan/zoom through a uniform buffer
- [x] Resize and device-pixel-ratio handling
- [x] Hover and selected-tile overlays
- [x] Keep the current minimap working in GPU mode
- [x] Preserve Canvas 2D behind `?renderer=canvas` and automatic fallback
- [x] Make wgpu/WebGPU the default renderer
- [x] Freeze Canvas 2D as a basic compatibility fallback
- [x] Validate functional parity and performance

## Phase 1.4 — Traversal

- [x] Terrain movement costs
- [x] Walkability rules
- [x] Eight-neighbor A* with diagonal cost √2 and an octile heuristic
- [x] Corner-cut prevention and configurable iteration limit
- [x] Walkability-based path smoothing for visualization/debug only
- [x] Keep full unsmoothed A* paths for tile-by-tile entity movement
- [x] WASM route bridge
- [x] GPU route overlay with a dedicated render pass
- [x] Artificial-map and large-world pathfinding tests

## Phase 2 — Living World

### Phase 2.1 — Resources

- [x] Parallel resource-deposit layer separated from terrain
- [x] Deterministic seed-derived generation
- [x] Food, timber, stone, and iron amounts
- [x] Terrain-conditioned distribution and generation tests
- [x] Tile inspector resource and traversal details
- [x] Dedicated GPU resource texture and visualization mode
- [x] Keep resource visualization out of the frozen Canvas fallback

### Phase 2.2 — Simulation

- [x] Rust-owned simulation clock and pause state
- [x] Deterministic batched tick advancement independent from rendering
- [x] Manual single-tick stepping
- [x] WASM clock controls
- [x] Play, pause, step, speed, and tick UI
- [x] Background-frame and delta-time safeguards
- [x] Unit tests for pause, resume, stepping, and advancement
- [x] WGSL parsing and validation in the Rust test suite

### Phase 2.3 — First Entity

- [x] Entity ID, position, and Simulation-owned storage
- [x] Deterministic spawn on walkable terrain
- [x] Tick-driven movement with stored A* paths
- [x] GPU-instanced entity rendering
- [x] Hunger need and nearby Food search
- [x] Food consumption and resource depletion
- [x] Entity inspector and activity debug state
- [x] Simulation mutates Grid through explicit world steps

### Phase 2.4 — Population

- [x] Deterministic multiple-entity spawning
- [x] Population statistics and spawn controls
- [x] Health, starvation, death, and entity removal
- [x] Monotonic entity IDs without reuse
- [x] Initial tick-based aging and proximity reproduction prototype
- [x] Finite resource consumption and competition tests
- [x] 10, 100, and 1,000 entity scale tests

### Phase 2.5 — Autonomy

- [x] Split simulation orchestration, entity state, lifecycle, and autonomy
- [x] Entity Mind with perception, memory, goals, and action plans
- [x] Local terrain, resource, and nearby-entity perception
- [x] Remember discovered Food with estimated amount and observation tick
- [x] Expire stale memories and correct depleted-resource beliefs
- [x] Utility scoring for Eat, Explore, and Rest
- [x] Persistent goals with urgent-hunger interruption
- [x] Explore unknown knowledge chunks instead of remaining idle
- [x] Plan movement and consumption from remembered information only
- [x] Record failed paths and temporarily avoid unreachable targets
- [x] Cognition state and utility scores in the entity debug inspector

### Phase 2.6 — Biology

- [x] One tick represents one hour
- [x] Female and male sex
- [x] Deterministic founder demographics
- [x] Individual natural lifespan
- [x] Natural death
- [x] Reproductive age windows
- [x] Male and female reproduction requirement
- [x] Deterministic daily conception
- [x] Pregnancy state
- [x] 40-week gestation
- [x] Birth after gestation
- [x] Postpartum cooldown
- [x] Biology population statistics
- [x] Biology inspector/debug state
- [x] Time-based movement
- [x] Pregnancy mobility penalty
- [x] Life stages
- [x] Child dependency

### Future Entity Systems

- [ ] Personality and relationships
  - [x] Deterministic personality traits
  - [x] Personality affects utility scoring
    - [x] Curiosity and caution affect explore and rest utilities
    - [x] Persistence affects goal retention
  - [x] Persistent relationship memory
  - [ ] Relationship affinity
  - [ ] Social interaction
  - [ ] Relationship-driven behavior
- [ ] Factions and groups

## Phase 3 — User Rules & Consequences

- [ ] Rule editor (define social/economic/cultural rules)
- [ ] Consequence engine (migrate NEXUS rule matching to Rust)
- [ ] Rules affect entities and society
- [ ] First and second order consequences

## Phase 4 — Emergent Society

- [ ] Economy (resources, trade, markets)
- [ ] Politics (laws, leaders, conflict)
- [ ] Culture generation (rituals, objects, professions)
- [ ] Rumor propagation system
- [ ] Ideology mutation engine

## Phase 5 — History

- [ ] Historical event logging
- [ ] Timeline viewer
- [ ] Legend mode (query world history)
- [ ] Export world as narrative document

## Phase 6 — Polish

- [ ] City growth and expansion
- [ ] Trade routes
- [ ] Tauri desktop application

## Future

- [ ] Native wgpu renderer for the desktop build
- [ ] Reusable entity instance buffers with `queue.write_buffer`
- [ ] Partial resource-texture updates
- [ ] Chunk-based spatial indexes for entities and resources
- [ ] Compute-driven spatial simulations
- [ ] Multiplayer (shared world editing)
- [ ] AI-assisted consequence suggestions
