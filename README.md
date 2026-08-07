# NEXUS World Engine

A procedural world generation tool built with Rust, WebAssembly, wgpu, and TypeScript.

## What it does

- Generates worlds using fractal Brownian motion (Perlin noise)
- Classifies terrain: ocean, beach, plains, forest, mountains, desert, tundra, swamp
- Renders primarily through wgpu/WebGPU, with a frozen Canvas 2D compatibility fallback
- Supports pan, zoom, minimap, regions, persistence, and tile inspection
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
