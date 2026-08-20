const SPATIAL_CELL_SIZE: u32 = 8;

#[derive(Clone, Copy, Debug, PartialEq)]
pub(super) struct EntitySnapshot {
    pub id: u32,
    pub x: u32,
    pub y: u32,
    pub hunger: f32,
}

#[derive(Clone, Debug, Default)]
pub(super) struct SpatialGrid {
    cells_wide: u32,
    cells_high: u32,
    cells: Vec<Vec<usize>>,
}

impl SpatialGrid {
    pub(super) fn prepare(&mut self, world_width: u32, world_height: u32) {
        let cells_wide = world_width.div_ceil(SPATIAL_CELL_SIZE);
        let cells_high = world_height.div_ceil(SPATIAL_CELL_SIZE);

        if self.cells_wide != cells_wide || self.cells_high != cells_high {
            self.cells_wide = cells_wide;
            self.cells_high = cells_high;
            self.cells.clear();
            self.cells
                .resize_with((cells_wide * cells_high) as usize, Vec::new);
        } else {
            for cell in &mut self.cells {
                cell.clear();
            }
        }
    }

    pub(super) fn insert(&mut self, snapshot_index: usize, x: u32, y: u32) {
        if self.cells_wide == 0 || self.cells_high == 0 {
            return;
        }

        let cell_x = x / SPATIAL_CELL_SIZE;
        let cell_y = y / SPATIAL_CELL_SIZE;

        if cell_x >= self.cells_wide || cell_y >= self.cells_high {
            return;
        }

        let index = (cell_y * self.cells_wide + cell_x) as usize;
        self.cells[index].push(snapshot_index);
    }

    pub(super) fn visit_candidates(
        &self,
        x: u32,
        y: u32,
        radius: u32,
        mut visit: impl FnMut(usize),
    ) {
        if self.cells_wide == 0 || self.cells_high == 0 {
            return;
        }

        let min_x = x.saturating_sub(radius) / SPATIAL_CELL_SIZE;
        let min_y = y.saturating_sub(radius) / SPATIAL_CELL_SIZE;
        let max_x = (x.saturating_add(radius) / SPATIAL_CELL_SIZE).min(self.cells_wide - 1);
        let max_y = (y.saturating_add(radius) / SPATIAL_CELL_SIZE).min(self.cells_high - 1);

        for cell_y in min_y..=max_y {
            for cell_x in min_x..=max_x {
                let index = (cell_y * self.cells_wide + cell_x) as usize;
                for &snapshot_index in &self.cells[index] {
                    visit(snapshot_index);
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn candidates_include_same_and_neighbor_cells() {
        let mut grid = SpatialGrid::default();
        grid.prepare(32, 32);

        grid.insert(0, 7, 7);
        grid.insert(1, 9, 7);
        grid.insert(2, 25, 25);

        let mut candidates = Vec::new();
        grid.visit_candidates(7, 7, 6, |index| candidates.push(index));
        candidates.sort_unstable();

        assert_eq!(candidates, vec![0, 1]);
    }

    #[test]
    fn prepare_clears_previous_tick() {
        let mut grid = SpatialGrid::default();

        grid.prepare(32, 32);
        grid.insert(0, 4, 4);
        grid.insert(1, 10, 10);

        grid.prepare(32, 32);

        assert!(grid.cells.iter().all(Vec::is_empty));
        assert_eq!(grid.cells_wide, 4);
        assert_eq!(grid.cells_high, 4);
    }

    #[test]
    fn spatial_query_clamps_at_world_edges() {
        let mut grid = SpatialGrid::default();
        grid.prepare(32, 32);

        grid.insert(0, 0, 0);
        grid.insert(1, 31, 31);

        let mut candidates = Vec::new();
        grid.visit_candidates(0, 0, 6, |index| candidates.push(index));

        assert_eq!(candidates, vec![0]);
    }
}
