use crate::world::{Grid, Region, RegionKind, Terrain};
use std::collections::VecDeque;

fn is_water(terrain: Terrain) -> bool {
    matches!(terrain, Terrain::DeepWater | Terrain::ShallowWater)
}

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
                if cx < region.min_x {
                    region.min_x = cx;
                }
                if cx > region.max_x {
                    region.max_x = cx;
                }
                if cy < region.min_y {
                    region.min_y = cy;
                }
                if cy > region.max_y {
                    region.max_y = cy;
                }
                if cx == 0
                    || cy == 0
                    || cx == grid.width - 1
                    || cy == grid.height - 1
                {
                    region.touches_border = true;
                }

                for (nx, ny) in
                    neighbors_4(cx, cy, grid.width, grid.height)
                {
                    let nidx = (ny * grid.width + nx) as usize;
                    if grid.region_ids[nidx] != u32::MAX {
                        continue;
                    }
                    let n_is_water =
                        is_water(grid.tiles[nidx].terrain);
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

fn neighbors_4(
    x: u32,
    y: u32,
    w: u32,
    h: u32,
) -> Vec<(u32, u32)> {
    let mut out = Vec::with_capacity(4);
    if x > 0 {
        out.push((x - 1, y));
    }
    if x + 1 < w {
        out.push((x + 1, y));
    }
    if y > 0 {
        out.push((x, y - 1));
    }
    if y + 1 < h {
        out.push((x, y + 1));
    }
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
                    altitude: if is_water(terrain) {
                        -0.5
                    } else {
                        0.5
                    },
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

    #[test]
    fn every_tile_receives_a_region() {
        let mut grid = make_test_grid(&[
            "LLWWW", "LLWWW", "WWLWW", "WWLLW",
        ]);
        detect_regions(&mut grid);
        for (i, &rid) in grid.region_ids.iter().enumerate() {
            assert_ne!(rid, u32::MAX, "tile {} has no region", i);
        }
    }

    #[test]
    fn correct_number_of_regions() {
        let mut grid = make_test_grid(&[
            "LLWWW", "LLWWW", "WWLWW", "WWLLW",
        ]);
        detect_regions(&mut grid);
        assert_eq!(grid.regions.len(), 7);
    }

    #[test]
    fn connected_land_has_same_region() {
        let mut grid = make_test_grid(&[
            "LLWWW", "LLWWW", "WWLWW", "WWLLW",
        ]);
        detect_regions(&mut grid);
        let r00 = grid.region_ids[0];
        let r10 = grid.region_ids[1];
        let r01 = grid.region_ids[5];
        let r11 = grid.region_ids[6];
        assert_eq!(r00, r10);
        assert_eq!(r00, r01);
        assert_eq!(r00, r11);
    }

    #[test]
    fn land_separated_by_water_has_different_regions() {
        let mut grid = make_test_grid(&[
            "LLWWW", "LLWWW", "WWLWW", "WWLLW",
        ]);
        detect_regions(&mut grid);
        let region_a = grid.region_ids[0];
        let region_d = grid.region_ids[12];
        let region_g = grid.region_ids[17];
        assert_ne!(region_a, region_d);
        assert_ne!(region_a, region_g);
        assert_ne!(region_d, region_g);
    }

    #[test]
    fn water_and_land_never_share_region() {
        let mut grid = make_test_grid(&[
            "LLWWW", "LLWWW", "WWLWW", "WWLLW",
        ]);
        detect_regions(&mut grid);
        for (i, tile) in grid.tiles.iter().enumerate() {
            let rid = grid.region_ids[i];
            let region = &grid.regions[rid as usize];
            if tile.terrain.is_water() {
                assert_eq!(region.kind, RegionKind::Water);
            } else {
                assert_eq!(region.kind, RegionKind::Land);
            }
        }
    }

    #[test]
    fn region_detection_is_deterministic() {
        let mut g1 = make_test_grid(&[
            "LLWWW", "LLWWW", "WWLWW", "WWLLW",
        ]);
        let mut g2 = make_test_grid(&[
            "LLWWW", "LLWWW", "WWLWW", "WWLLW",
        ]);
        detect_regions(&mut g1);
        detect_regions(&mut g2);
        assert_eq!(g1.region_ids, g2.region_ids);
        assert_eq!(g1.regions.len(), g2.regions.len());
        for (a, b) in
            g1.regions.iter().zip(g2.regions.iter())
        {
            assert_eq!(a.kind, b.kind);
            assert_eq!(a.tile_count, b.tile_count);
        }
    }

    #[test]
    fn isolated_single_tile_is_its_own_region() {
        let mut grid = make_test_grid(&[
            "LLWWW", "LLWWW", "WWLWW", "WWLLW",
        ]);
        detect_regions(&mut grid);
        let rid = grid.region_ids[12];
        let region = &grid.regions[rid as usize];
        assert_eq!(region.kind, RegionKind::Land);
        assert_eq!(region.tile_count, 1);
        assert!(!region.touches_border);
    }

    #[test]
    fn large_water_body_is_detected() {
        let mut grid = make_test_grid(&[
            "WWWWW", "WLLLW", "WLLLW", "WWWWW",
        ]);
        detect_regions(&mut grid);
        let water_regions: Vec<_> = grid
            .regions
            .iter()
            .filter(|r| r.kind == RegionKind::Water)
            .collect();
        assert_eq!(water_regions.len(), 1);
        assert_eq!(water_regions[0].tile_count, 16);
    }
}
