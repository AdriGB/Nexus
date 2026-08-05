use crate::world::{Grid, Region, RegionKind, Terrain};
use std::collections::VecDeque;

fn is_water(terrain: Terrain) -> bool {
    matches!(terrain, Terrain::DeepWater | Terrain::ShallowWater)
}

/// Flood-fill detection of connected land and water regions.
/// Uses 4-directional connectivity (no diagonals).
pub fn detect_regions(grid: &mut Grid) {
    let total = (grid.width * grid.height) as usize;
    grid.region_ids = vec![u32::MAX; total];
    grid.regions.clear();

    let mut next_id: u32 = 0;
    let mut queue: VecDeque<(u32, u32)> = VecDeque::new();

    for y in 0..grid.height {
        for x in 0..grid.width {
            let idx = (y * grid.width + x) as usize;
            if grid.region_ids[idx] != u32::MAX {
                continue;
            }

            let kind = if is_water(grid.tiles[idx].terrain) {
                RegionKind::Water
            } else {
                RegionKind::Land
            };

            let mut region = Region {
                kind,
                tile_count: 0,
                min_x: x,
                min_y: y,
                max_x: x,
                max_y: y,
                touches_border: false,
            };

            queue.push_back((x, y));
            grid.region_ids[idx] = next_id;

            while let Some((cx, cy)) = queue.pop_front() {
                region.tile_count += 1;
                if cx < region.min_x { region.min_x = cx; }
                if cx > region.max_x { region.max_x = cx; }
                if cy < region.min_y { region.min_y = cy; }
                if cy > region.max_y { region.max_y = cy; }
                if cx == 0 || cy == 0 || cx == grid.width - 1 || cy == grid.height - 1 {
                    region.touches_border = true;
                }

                for (nx, ny) in neighbors_4(cx, cy, grid.width, grid.height) {
                    let nidx = (ny * grid.width + nx) as usize;
                    if grid.region_ids[nidx] != u32::MAX {
                        continue;
                    }
                    let n_is_water = is_water(grid.tiles[nidx].terrain);
                    let n_kind = if n_is_water {
                        RegionKind::Water
                    } else {
                        RegionKind::Land
                    };
                    if n_kind == kind {
                        grid.region_ids[nidx] = next_id;
                        queue.push_back((nx, ny));
                    }
                }
            }

            grid.regions.push(region);
            next_id += 1;
        }
    }
}

fn neighbors_4(x: u32, y: u32, w: u32, h: u32) -> Vec<(u32, u32)> {
    let mut out = Vec::with_capacity(4);
    if x > 0 { out.push((x - 1, y)); }
    if x + 1 < w { out.push((x + 1, y)); }
    if y > 0 { out.push((x, y - 1)); }
    if y + 1 < h { out.push((x, y + 1)); }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::world::Tile;

    fn make_test_grid(pattern: &[&str]) -> Grid {
        let width = pattern[0].len() as u32;
        let height = pattern.len() as u32;
        let mut tiles = Vec::new();
        for row in pattern {
            for c in row.chars() {
                let terrain = match c {
                    'L' => Terrain::Grassland,
                    'W' => Terrain::DeepWater,
                    _ => Terrain::Plains,
                };
                tiles.push(Tile {
                    terrain,
                    altitude: if is_water(terrain) { -0.5 } else { 0.5 },
                    moisture: 0.5,
                    temperature: 0.5,
                });
            }
        }
        let tc = tiles.len();
        Grid {
            width,
            height,
            tiles,
            region_ids: vec![u32::MAX; tc],
            regions: Vec::new(),
        }
    }

    // Pattern:
    //   LLWWW   y=0
    //   LLWWW   y=1
    //   WWLWW   y=2
    //   WWLLW   y=3
    //
    // Expected regions:
    //   Land A:  (0,0)(1,0)(0,1)(1,1) = 4 tiles, touches border
    //   Water B: (2,0)(3,0)(4,0)(2,1)(3,1)(4,1) = 6 tiles, touches border
    //   Water C: (0,2)(1,2) = 2 tiles, touches border
    //   Land D:  (2,2) = 1 tile, no border
    //   Water E: (3,2)(4,2)(4,3) = 3 tiles, touches border
    //   Water F: (0,3)(1,3) = 2 tiles, touches border
    //   Land G:  (2,3)(3,3) = 2 tiles, touches border

    #[test]
    fn every_tile_receives_a_region() {
        let mut grid = make_test_grid(&["LLWWW", "LLWWW", "WWLWW", "WWLLW"]);
        detect_regions(&mut grid);
        for (i, &rid) in grid.region_ids.iter().enumerate() {
            assert_ne!(rid, u32::MAX, "tile {} has no region", i);
        }
    }

    #[test]
    fn correct_number_of_regions() {
        let mut grid = make_test_grid(&["LLWWW", "LLWWW", "WWLWW", "WWLLW"]);
        detect_regions(&mut grid);
        assert_eq!(grid.regions.len(), 7);
    }

    #[test]
    fn connected_land_has_same_region() {
        let mut grid = make_test_grid(&["LLWWW", "LLWWW", "WWLWW", "WWLLW"]);
        detect_regions(&mut grid);
        let r00 = grid.region_ids[0];
        let r10 = grid.region_ids[1];
        let r01 = grid.region_ids[5]; // y=1, x=0 → index 1*5+0 = 5
        let r11 = grid.region_ids[6]; // y=1, x=1 → index 1*5+1 = 6
        assert_eq!(r00, r10);
        assert_eq!(r00, r01);
        assert_eq!(r00, r11);
    }

    #[test]
    fn land_separated_by_water_has_different_regions() {
        let mut grid = make_test_grid(&["LLWWW", "LLWWW", "WWLWW", "WWLLW"]);
        detect_regions(&mut grid);
        let region_a = grid.region_ids[0]; // (0,0) Land A
        let region_d = grid.region_ids[12]; // (2,2) y=2*5+2=12 Land D
        let region_g = grid.region_ids[17]; // (2,3) y=3*5+2=17 Land G
        assert_ne!(region_a, region_d);
        assert_ne!(region_a, region_g);
        assert_ne!(region_d, region_g);
    }

    #[test]
    fn water_and_land_never_share_region() {
        let mut grid = make_test_grid(&["LLWWW", "LLWWW", "WWLWW", "WWLLW"]);
        detect_regions(&mut grid);
        let land_ids: Vec<u32> = grid
            .regions
            .iter()
            .filter(|r| r.kind == RegionKind::Land)
            .map(|r| r.id)
            .collect();
        let water_ids: Vec<u32> = grid
            .regions
            .iter()
            .filter(|r| r.kind == RegionKind::Water)
            .map(|r| r.id)
            .collect();
        for lid in &land_ids {
            for wid in &water_ids {
                assert_ne!(lid, wid);
            }
        }
    }

    #[test]
    fn region_detection_is_deterministic() {
        let mut g1 = make_test_grid(&["LLWWW", "LLWWW", "WWLWW", "WWLLW"]);
        let mut g2 = make_test_grid(&["LLWWW", "LLWWW", "WWLWW", "WWLLW"]);
        detect_regions(&mut g1);
        detect_regions(&mut g2);
        assert_eq!(g1.region_ids, g2.region_ids);
        assert_eq!(g1.regions.len(), g2.regions.len());
        for (a, b) in g1.regions.iter().zip(g2.regions.iter()) {
            assert_eq!(a.id, b.id);
            assert_eq!(a.kind, b.kind);
            assert_eq!(a.tile_count, b.tile_count);
        }
    }

    #[test]
    fn isolated_single_tile_is_its_own_region() {
        let mut grid = make_test_grid(&["LLWWW", "LLWWW", "WWLWW", "WWLLW"]);
        detect_regions(&mut grid);
        let region_d = grid.regions.iter().find(|r| r.id == grid.region_ids[12]).unwrap();
        assert_eq!(region_d.kind, RegionKind::Land);
        assert_eq!(region_d.tile_count, 1);
        assert!(!region_d.touches_border);
    }

    #[test]
    fn large_water_body_is_detected() {
        let mut grid = make_test_grid(&["WWWWW", "WLLLW", "WLLLW", "WWWWW"]);
        detect_regions(&mut grid);
        let water_regions: Vec<_> = grid.regions.iter().filter(|r| r.kind == RegionKind::Water).collect();
        // One ring of water surrounding one block of land
        assert_eq!(water_regions.len(), 1);
        assert_eq!(water_regions[0].tile_count, 16); // 25 - 9 = 16
    }
}
