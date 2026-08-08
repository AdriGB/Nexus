# NEXUS World Engine

A procedural world generation tool built with Rust, WebAssembly, wgpu, and TypeScript.

## What it does

- Generates worlds using fractal Brownian motion (Perlin noise)
- Classifies terrain: ocean, beach, plains, forest, mountains, desert, tundra, swamp
- Renders primarily through wgpu/WebGPU, with a frozen Canvas 2D compatibility fallback
- Supports pan, zoom, minimap, regions, persistence, and tile inspection
- Generates deterministic food, timber, stone, and iron deposits
- Simulates autonomous populations with hunger, health, aging, reproduction, and death
- Gives each entity local perception, imperfect memory, persistent goals, and action plans
- Runs entirely in the browser — no backend required

## Architecture

```text
Browser / TypeScript
|-- UI, input, persistence, camera, minimap
|-- simulation timing and renderer coordination
|
| WASM bridge
v
Rust Engine
|-- world generation, terrain, regions, resources
|-- traversal and eight-neighbor A*
|-- simulation
|   |-- entity state
|   |-- lifecycle and population
|   `-- autonomy: perception, memory, goals, actions
`-- wgpu renderer
    |-- terrain and resource textures
    |-- routes and selection overlays
    `-- instanced entities
|
v
WebGPU
```

## Render backends

### wgpu / WebGPU (primary)

The GPU-accelerated renderer written in Rust is selected automatically when `navigator.gpu` is available.

### Canvas 2D (compatibility fallback)

Canvas is a frozen fallback that keeps terrain, pan/zoom, basic selection, and the UI functional when WebGPU is unavailable or wgpu initialization/rendering fails. New visual features are developed only for wgpu.

For compatibility testing, Canvas can be forced explicitly:

```text
http://localhost:5173/?renderer=canvas
```

For local renderer telemetry, enable the development overlay:

```text
http://localhost:5173/?debug=renderer
```

It reports the active renderer, CPU frame submission time, world dimensions, and zoom. It is hidden by default.

## Renderer architecture

The Rust engine owns world generation and the wgpu renderer. In GPU mode it uploads one RGBA texture where each pixel represents one tile:

- **R**: terrain
- **G**: altitude
- **B**: moisture
- **A**: temperature

TypeScript continues to own the HTML interface, persistence, input handling, camera state, and minimap. Camera changes cross the WASM boundary as a small uniform update rather than as a full visible-tile buffer.

## Resource deposits

Resources are stored separately from terrain in a parallel grid layer. Their generation uses a seed derived from the world seed, so regenerating the same world produces exactly the same deposits while keeping resource placement independent from terrain noise.

- Food favors plains, grassland, and some swamps.
- Timber favors forests and dense forests.
- Stone favors hills and mountains.
- Iron is rarer and favors hills and mountains.

Each deposit has an initial `u16` amount. The **Terrain / Resources** switch changes the primary wgpu shader to the resource layer, which is uploaded as its own RGBA texture. The tile inspector reports walkability, movement cost, resource kind, and amount. Resource visualization is intentionally unavailable in the frozen Canvas fallback.

## Traversal and route preview

Terrain defines walkability and an initial movement cost. The Rust engine uses eight-neighbor A* with diagonal costs, an octile heuristic, corner-cut prevention, and an iteration limit to find the cheapest route between two tiles. Smoothing is restricted to route visualization; entity movement retains the full tile-by-tile path.

- Click a walkable tile to set the route origin.
- Shift+Click another tile to calculate the destination.
- The resulting route is uploaded to a dedicated wgpu vertex buffer and rendered in a separate overlay pass.

Route visualization is intentionally unavailable in the frozen Canvas 2D fallback.

## Simulation clock

The Rust engine owns a deterministic simulation clock that starts paused at tick zero. One tick represents one biological hour, while rendering remains independent from simulation time:

- **Play** lets TypeScript translate elapsed real time into batched Rust ticks.
- **Pause** stops automatic advancement.
- **Step** advances exactly one tick while paused or running.
- **Speed** controls the tick rate without tying world speed to frame rate.

The browser uses `requestAnimationFrame` only to measure elapsed time; Rust receives an explicit integer tick count. Long or backgrounded frames are capped and discarded to prevent accidental simulation jumps.

WGSL shaders are parsed and validated with Naga during `cargo test`, so invalid identifiers or shader syntax fail CI before reaching WebGPU at runtime.

## Population and entity lifecycle

Every generated world starts with a small population on deterministic walkable positions. `Simulation` owns mutable entity state and receives `&mut Grid` for each world step, keeping spatial data separate from the logic that transforms it.

Entities begin with zero hunger. Each tick increases hunger; at the threshold they search for nearby Food deposits, test candidates with A*, store a resulting path, and follow it without recalculating every tick. On arrival they consume the deposit's finite amount. The consumed amount is removed from `Grid.resources`, empty deposits disappear, the GPU resource texture is refreshed, and competing entities cannot consume the same units twice.

At maximum hunger, health starts falling until the entity dies and is removed. Population, births, deaths, sex distribution, pregnancies, hunger, food seeking, average hunger, and total food consumed are exposed in the sidebar, together with controls for spawning 10 or 100 additional founders.

## Biological time and lifecycle

Biological time is defined centrally in `simulation/time.rs`: one tick is one hour, a day is 24 ticks, and a year is 8,760 ticks. Founder/debug entities receive deterministic ages between 18 and 40 years. Real newborns begin at age zero.

Sex, founder age, and individual lifespan are derived deterministically from the world seed and monotonic entity ID. Lifespans vary from roughly 650,000 to 950,000 ticks, avoiding synchronized natural deaths.

Eligible nearby females and males receive one deterministic conception roll per simulated day. Conception creates a pregnancy rather than a child. After a 40-week gestation, a newborn is placed on a walkable tile adjacent to the mother; the mother then enters a 180-day postpartum period. Reproductive age windows, pregnancy, postpartum state, health, and hunger all constrain conception. Movement speed remains tile-per-tick and is intentionally unaffected by pregnancy until time-based movement is implemented.

Entities are rendered by a dedicated wgpu instancing pipeline. Position, hunger, and activity are uploaded as per-instance data, so the renderer is ready to scale beyond the initial entity without adding one draw call per creature. Canvas remains a terrain-only compatibility fallback.

## Entity cognition

Entities do not query the full resource grid when making decisions. Each one has a `Mind` with a bounded perception radius, imperfect memory, a persistent goal, and a short action plan:

```text
perceive locally -> update memory -> score goals -> retain or choose goal -> plan -> act
```

Perception records nearby resource deposits, explored knowledge chunks, and visible entities. Food memories contain their last observed amount and tick. Memories expire, are corrected when an entity sees that a deposit has been depleted, and are temporarily suppressed when pathfinding reports that a remembered target is unreachable.

The current Utility AI scores `Eat`, `Explore`, and `Rest`. A viable goal persists across ticks instead of being selected again every frame, although urgent hunger can interrupt exploration when remembered food is available. Exploration selects walkable tiles in unknown chunks, so satisfied entities expand their personal knowledge rather than remaining idle. Eating is planned as separate movement and consumption actions, and consumption still deducts the finite amount from the shared world.

The entity debug inspector exposes the current goal, action, retained-goal age, known resources and chunks, visible entities, and the last utility scores. This cognition layer is deterministic and rule-based; it does not use generative AI or machine learning.

## Prerequisites

- Rust (stable)
- wasm-pack
- Node.js (20.19+)

## Quick start

```bash
# Build the WASM engine
cd engine
wasm-pack build --target web --out-dir ../web/src/wasm

# Install and run the frontend
cd ../web
npm install
npm run dev
```
