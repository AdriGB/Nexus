# NEXUS World Engine

A procedural world generation tool built with Rust + WebAssembly + TypeScript.

## What it does

- Generates worlds using fractal Brownian motion (Perlin noise)
- Classifies terrain: ocean, beach, plains, forest, mountains, desert, tundra, swamp
- Renders in Canvas 2D with pan, zoom, minimap, and tile inspection
- Runs entirely in the browser — no backend required

## Prerequisites

- [Rust](https://rustup.rs/) (stable)
- [wasm-pack](https://rustwasm.github.io/wasm-pack/installer/)
- [Node.js](https://nodejs.org/) (18+)

## Quick start

```bash
# Build the WASM engine
cd engine
wasm-pack build --target web --out-dir ../web/public/wasm

# Install and run the frontend
cd ../web
npm install
npm run dev
