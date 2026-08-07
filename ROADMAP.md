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
- [ ] Resource deposits (ore, fertile soil, timber)

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
- [ ] Make wgpu the default after wider browser testing
- [ ] Remove the main Canvas 2D renderer after parity is accepted

## Phase 2 — Living World

- [ ] Entity system (creatures with needs, personality, AI)
- [ ] Pathfinding (A* on terrain)
- [ ] Reproduction, aging, death
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
- [ ] GPU-instanced entity rendering
- [ ] Compute-driven spatial simulations
- [ ] Multiplayer (shared world editing)
- [ ] AI-assisted consequence suggestions
