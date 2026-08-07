# NEXUS World Engine

A procedural world generation tool built with Rust, WebAssembly, wgpu, and TypeScript.

## What it does

- Generates worlds using fractal Brownian motion (Perlin noise)
- Classifies terrain: ocean, beach, plains, forest, mountains, desert, tundra, swamp
- Renders primarily through wgpu/WebGPU, with a frozen Canvas 2D compatibility fallback
- Supports pan, zoom, minimap, regions, persistence, and tile inspection
- Generates deterministic food, timber, stone, and iron deposits
- Runs entirely in the browser — no backend required

## Architecture

```text
Browser / TypeScript
├── UI (HTML/CSS)
├── Input handling
├── Persistence (localStorage)
├── Camera state
└── Minimap
          │
          │ WASM bridge (function calls + uniform updates)
          ▼
Rust Engine
├── World generation (fBm Perlin noise)
├── Region detection (biome classification)
└── wgpu renderer (GPU texture uploads)
          │
          ▼
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

Terrain defines walkability and an initial movement cost. The Rust engine uses four-neighbor A* with a Manhattan heuristic to find the cheapest route between two tiles.

- Click a walkable tile to set the route origin.
- Shift+Click another tile to calculate the destination.
- The resulting route is uploaded to a dedicated wgpu vertex buffer and rendered in a separate overlay pass.

Route visualization is intentionally unavailable in the frozen Canvas 2D fallback.

## Simulation clock

The Rust engine owns a deterministic simulation clock that starts paused at tick zero. Rendering remains independent from simulation time:

- **Play** lets TypeScript translate elapsed real time into batched Rust ticks.
- **Pause** stops automatic advancement.
- **Step** advances exactly one tick while paused or running.
- **Speed** controls the tick rate without tying world speed to frame rate.

The browser uses `requestAnimationFrame` only to measure elapsed time; Rust receives an explicit integer tick count. Long or backgrounded frames are capped and discarded to prevent accidental simulation jumps.

WGSL shaders are parsed and validated with Naga during `cargo test`, so invalid identifiers or shader syntax fail CI before reaching WebGPU at runtime.

## First entity

Every generated world starts with one entity on the nearest walkable tile to the world center. `Simulation` owns its mutable state and receives `&mut Grid` for each world step, keeping spatial data separate from the logic that transforms it.

The entity begins with zero hunger. Each tick increases hunger; at the threshold it searches for nearby Food deposits, tests candidates with A*, stores one resulting path, and follows that path without recalculating it every tick. On arrival it consumes Food and updates the resource layer.

Entities are rendered by a dedicated wgpu instancing pipeline. Position, hunger, and activity are uploaded as per-instance data, so the renderer is ready to scale beyond the initial entity without adding one draw call per creature. Canvas remains a terrain-only compatibility fallback.

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
