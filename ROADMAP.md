
---

## ROADMAP reorder: `ROADMAP.md`

```markdown
# NEXUS Roadmap

## Phase 1 — World Viewer ✅
- [x] Perlin noise world generation
- [x] Canvas renderer with pan/zoom
- [x] Minimap
- [x] Tile inspector
- [x] Terrain legend
- [x] TypeScript + Vite + WASM architecture

## Phase 1.1 — Stabilization
- [ ] CI passing (cargo fmt, clippy, tsc, build)
- [ ] Save/load seed and parameters (localStorage)
- [ ] Export/import world data as JSON
- [ ] LOD rendering or full-world image for extreme zoom-out

## Phase 1.2 — Geography
- [ ] River generation (erosion simulation)
- [ ] Biome refinement (temperature + moisture gradients)
- [ ] Resource deposits (ore, fertile soil, timber)
- [ ] Region detection and connectivity analysis

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
- [ ] WebGL renderer (larger worlds)
- [ ] Multiplayer (shared world editing)
- [ ] AI-assisted consequence suggestions
