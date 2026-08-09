use crate::world::Grid;
use std::cmp::Ordering;
use std::collections::BinaryHeap;

const SQRT2: f32 = std::f32::consts::SQRT_2;
const DEFAULT_MAX_ITERATIONS: usize = 50_000;

#[derive(Clone, Copy, Debug)]
struct OpenNode {
    position: usize,
    estimated_total: f32,
}

impl PartialEq for OpenNode {
    fn eq(&self, other: &Self) -> bool {
        self.position == other.position
            && self.estimated_total.to_bits() == other.estimated_total.to_bits()
    }
}

impl Eq for OpenNode {}

impl PartialOrd for OpenNode {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

impl Ord for OpenNode {
    fn cmp(&self, other: &Self) -> Ordering {
        other
            .estimated_total
            .total_cmp(&self.estimated_total)
            .then_with(|| other.position.cmp(&self.position))
    }
}

#[derive(Clone, Debug, Default)]
pub(crate) struct PathfindingWorkspace {
    open: BinaryHeap<OpenNode>,
    came_from: Vec<Option<usize>>,
    costs: Vec<f32>,
    generation: Vec<u32>,
    current_gen: u32,
}

impl PathfindingWorkspace {
    pub(crate) fn new() -> Self {
        Self::default()
    }

    fn prepare(&mut self, tile_count: usize) {
        self.open.clear();

        self.current_gen = self.current_gen.wrapping_add(1);

        if self.current_gen == 0 {
            self.current_gen = 1;
            self.generation.fill(0);
        }

        if self.came_from.len() < tile_count {
            self.came_from.resize(tile_count, None);
            self.costs.resize(tile_count, f32::INFINITY);
            self.generation.resize(tile_count, 0);
        }
    }
}

pub fn find_path(grid: &Grid, start: (u32, u32), goal: (u32, u32)) -> Option<Vec<(u32, u32)>> {
    find_path_with_limit(grid, start, goal, None)
}

pub fn find_path_with_limit(
    grid: &Grid,
    start: (u32, u32),
    goal: (u32, u32),
    max_iterations: Option<usize>,
) -> Option<Vec<(u32, u32)>> {
    let mut workspace = PathfindingWorkspace::new();
    find_path_with_workspace_and_limit(&mut workspace, grid, start, goal, max_iterations)
}

pub(crate) fn find_path_with_workspace(
    workspace: &mut PathfindingWorkspace,
    grid: &Grid,
    start: (u32, u32),
    goal: (u32, u32),
) -> Option<Vec<(u32, u32)>> {
    find_path_with_workspace_and_limit(workspace, grid, start, goal, None)
}

pub(crate) fn step_cost(grid: &Grid, from: (u32, u32), to: (u32, u32)) -> Option<f32> {
    let dx = from.0.abs_diff(to.0);
    let dy = from.1.abs_diff(to.1);

    if dx > 1 || dy > 1 || (dx == 0 && dy == 0) {
        return None;
    }

    let terrain_cost = grid.get(to.0, to.1)?.terrain.movement_cost()?;
    let base_cost = if dx == 1 && dy == 1 { SQRT2 } else { 1.0 };

    Some(base_cost * terrain_cost)
}

fn find_path_with_workspace_and_limit(
    workspace: &mut PathfindingWorkspace,
    grid: &Grid,
    start: (u32, u32),
    goal: (u32, u32),
    max_iterations: Option<usize>,
) -> Option<Vec<(u32, u32)>> {
    if !grid.get(start.0, start.1)?.terrain.is_walkable()
        || !grid.get(goal.0, goal.1)?.terrain.is_walkable()
    {
        return None;
    }

    if start == goal {
        return Some(vec![start]);
    }

    if let (Some(start_region), Some(goal_region)) = (
        grid.region_id_at(start.0, start.1),
        grid.region_id_at(goal.0, goal.1),
    ) {
        if start_region != goal_region {
            return None;
        }
    }

    let max_iters = max_iterations.unwrap_or(DEFAULT_MAX_ITERATIONS);
    let start_index = index(grid, start);
    let goal_index = index(grid, goal);

    workspace.prepare(grid.tiles.len());

    workspace.generation[start_index] = workspace.current_gen;
    workspace.costs[start_index] = 0.0;
    workspace.open.push(OpenNode {
        position: start_index,
        estimated_total: octile(start, goal),
    });

    let mut iterations = 0;

    while let Some(current) = workspace.open.pop() {
        iterations += 1;
        if iterations > max_iters {
            return None;
        }

        let current_cost = workspace.costs[current.position];
        let best_estimate = current_cost + octile(coordinate(grid, current.position), goal);

        if current.estimated_total > best_estimate {
            continue;
        }

        if current.position == goal_index {
            return Some(reconstruct_path(
                grid,
                &workspace.came_from,
                start_index,
                goal_index,
            ));
        }

        let current_coordinate = coordinate(grid, current.position);

        for (neighbor, _) in neighbors_8dir(grid, current_coordinate) {
            let neighbor_index = index(grid, neighbor);
            let Some(step_cost) = step_cost(grid, current_coordinate, neighbor) else {
                continue;
            };

            let candidate_cost = current_cost + step_cost;

            if workspace.generation[neighbor_index] != workspace.current_gen
                || candidate_cost < workspace.costs[neighbor_index]
            {
                workspace.generation[neighbor_index] = workspace.current_gen;
                workspace.costs[neighbor_index] = candidate_cost;
                workspace.came_from[neighbor_index] = Some(current.position);
                workspace.open.push(OpenNode {
                    position: neighbor_index,
                    estimated_total: candidate_cost + octile(neighbor, goal),
                });
            }
        }
    }

    None
}

pub fn smooth_path(grid: &Grid, path: Vec<(u32, u32)>) -> Vec<(u32, u32)> {
    if path.len() <= 2 {
        return path;
    }

    let mut smoothed = vec![path[0]];
    let mut anchor = 0;

    for i in 1..path.len() {
        if !has_line_of_sight(grid, path[anchor], path[i]) {
            let waypoint = path[i - 1];
            if smoothed.last() != Some(&waypoint) {
                smoothed.push(waypoint);
            }
            anchor = i - 1;
        }
    }

    smoothed.push(*path.last().unwrap());
    smoothed
}

fn has_line_of_sight(grid: &Grid, from: (u32, u32), to: (u32, u32)) -> bool {
    let (mut x0, mut y0) = (from.0 as i32, from.1 as i32);
    let (x1, y1) = (to.0 as i32, to.1 as i32);
    let dx = (x1 - x0).abs();
    let dy = -(y1 - y0).abs();
    let sx: i32 = if x0 < x1 { 1 } else { -1 };
    let sy: i32 = if y0 < y1 { 1 } else { -1 };
    let mut err = dx + dy;

    loop {
        if x0 < 0 || y0 < 0 || x0 >= grid.width as i32 || y0 >= grid.height as i32 {
            return false;
        }
        if !grid
            .get(x0 as u32, y0 as u32)
            .is_some_and(|t| t.terrain.is_walkable())
        {
            return false;
        }

        if x0 == x1 && y0 == y1 {
            break;
        }

        let e2 = 2 * err;
        if e2 >= dy {
            err += dy;
            x0 += sx;
        }
        if e2 <= dx {
            err += dx;
            y0 += sy;
        }
    }

    true
}

fn index(grid: &Grid, coordinate: (u32, u32)) -> usize {
    (coordinate.1 * grid.width + coordinate.0) as usize
}

fn coordinate(grid: &Grid, index: usize) -> (u32, u32) {
    let index = index as u32;
    (index % grid.width, index / grid.width)
}

fn octile(left: (u32, u32), right: (u32, u32)) -> f32 {
    let dx = left.0.abs_diff(right.0) as f32;
    let dy = left.1.abs_diff(right.1) as f32;
    dx.max(dy) + (SQRT2 - 1.0) * dx.min(dy)
}

fn neighbors_8dir(grid: &Grid, (x, y): (u32, u32)) -> impl Iterator<Item = ((u32, u32), f32)> {
    let w = grid.width;
    let h = grid.height;

    let walkable =
        |px: u32, py: u32| -> bool { grid.get(px, py).is_some_and(|t| t.terrain.is_walkable()) };

    let up = y > 0 && walkable(x, y - 1);
    let down = y + 1 < h && walkable(x, y + 1);
    let left = x > 0 && walkable(x - 1, y);
    let right = x + 1 < w && walkable(x + 1, y);

    [
        up.then(|| ((x, y - 1), 1.0)),
        left.then(|| ((x - 1, y), 1.0)),
        right.then(|| ((x + 1, y), 1.0)),
        down.then(|| ((x, y + 1), 1.0)),
        (up && left && walkable(x - 1, y - 1)).then(|| ((x - 1, y - 1), SQRT2)),
        (up && right && walkable(x + 1, y - 1)).then(|| ((x + 1, y - 1), SQRT2)),
        (down && left && walkable(x - 1, y + 1)).then(|| ((x - 1, y + 1), SQRT2)),
        (down && right && walkable(x + 1, y + 1)).then(|| ((x + 1, y + 1), SQRT2)),
    ]
    .into_iter()
    .flatten()
}

fn reconstruct_path(
    grid: &Grid,
    came_from: &[Option<usize>],
    start: usize,
    goal: usize,
) -> Vec<(u32, u32)> {
    let mut path = vec![coordinate(grid, goal)];
    let mut current = goal;

    while current != start {
        current = came_from[current].expect("reachable path nodes have a predecessor");
        path.push(coordinate(grid, current));
    }

    path.reverse();
    path
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::world::{Region, Terrain, Tile};

    fn grid_from_rows(rows: &[&str]) -> Grid {
        let height = rows.len() as u32;
        let width = rows.first().map_or(0, |row| row.len()) as u32;
        assert!(rows.iter().all(|row| row.len() == width as usize));

        let tiles = rows
            .iter()
            .flat_map(|row| row.chars())
            .map(|symbol| Tile {
                terrain: match symbol {
                    'P' => Terrain::Plains,
                    'M' => Terrain::Mountain,
                    '#' => Terrain::DeepWater,
                    _ => panic!("unknown artificial terrain symbol: {symbol}"),
                },
                altitude: 0.0,
                moisture: 0.5,
                temperature: 0.5,
            })
            .collect();

        Grid {
            width,
            height,
            tiles,
            region_ids: Vec::new(),
            regions: Vec::<Region>::new(),
            resources: vec![None; (width * height) as usize],
        }
    }

    #[test]
    fn finds_a_path_across_open_ground() {
        let grid = grid_from_rows(&["PPPPP", "PPPPP", "PPPPP"]);
        let path = find_path(&grid, (0, 1), (4, 1)).unwrap();

        assert_eq!(path.first(), Some(&(0, 1)));
        assert_eq!(path.last(), Some(&(4, 1)));
        assert_eq!(path.len(), 5);
    }

    #[test]
    fn routes_around_an_obstacle() {
        let grid = grid_from_rows(&["PPPPP", "P###P", "PPPPP"]);
        let path = find_path(&grid, (0, 1), (4, 1)).unwrap();

        assert!(path
            .iter()
            .all(|&(x, y)| grid.get(x, y).unwrap().terrain.is_walkable()));
        assert!(path.len() > 5);
    }

    #[test]
    fn returns_none_when_the_goal_is_isolated() {
        let grid = grid_from_rows(&["P#P", "#P#", "P#P"]);
        assert!(find_path(&grid, (0, 0), (1, 1)).is_none());
    }

    #[test]
    fn start_equal_to_goal_returns_one_tile() {
        let grid = grid_from_rows(&["P"]);
        assert_eq!(find_path(&grid, (0, 0), (0, 0)), Some(vec![(0, 0)]));
    }

    #[test]
    fn prefers_cheap_ground_over_a_shorter_mountain_route() {
        let grid = grid_from_rows(&["PPPPP", "PMMMP", "PPPPP"]);
        let path = find_path(&grid, (0, 1), (4, 1)).unwrap();

        assert!(path
            .iter()
            .all(|&(x, y)| grid.get(x, y).unwrap().terrain != Terrain::Mountain));
        assert_eq!(path.len(), 5);
    }

    #[test]
    fn rejects_unwalkable_endpoints_and_out_of_bounds_coordinates() {
        let grid = grid_from_rows(&["P#"]);
        assert!(find_path(&grid, (0, 0), (1, 0)).is_none());
        assert!(find_path(&grid, (2, 0), (0, 0)).is_none());
    }

    #[test]
    fn rejects_endpoints_in_different_known_regions() {
        let mut grid = grid_from_rows(&["PPP"]);
        grid.region_ids = vec![0, 0, 1];

        assert!(find_path(&grid, (0, 0), (2, 0)).is_none());
    }

    #[test]
    fn finds_a_route_across_a_large_world() {
        let row = "P".repeat(256);
        let rows: Vec<_> = (0..256).map(|_| row.as_str()).collect();
        let grid = grid_from_rows(&rows);
        let path = find_path(&grid, (0, 0), (255, 255)).unwrap();

        assert_eq!(path.first(), Some(&(0, 0)));
        assert_eq!(path.last(), Some(&(255, 255)));
        assert_eq!(path.len(), 256);
    }

    #[test]
    fn diagonal_movement_is_shorter_than_cardinal() {
        let row = "P".repeat(10);
        let rows: Vec<_> = (0..10).map(|_| row.as_str()).collect();
        let grid = grid_from_rows(&rows);

        let path = find_path(&grid, (0, 0), (9, 9)).unwrap();
        assert_eq!(path.len(), 10);
    }

    #[test]
    fn diagonal_blocked_through_impassable_corner() {
        let grid = grid_from_rows(&["PP#", "PP#", "PPP"]);
        let path = find_path(&grid, (0, 0), (2, 2)).unwrap();

        assert!(path
            .iter()
            .all(|&(x, y)| grid.get(x, y).unwrap().terrain.is_walkable()));
        assert!(path.len() >= 4);
    }

    #[test]
    fn smoothed_path_removes_collinear_points() {
        let grid = grid_from_rows(&["PPPPP", "PPPPP", "PPPPP"]);
        let path = find_path(&grid, (0, 1), (4, 1)).unwrap();
        let smoothed = smooth_path(&grid, path);

        assert_eq!(smoothed.first(), Some(&(0, 1)));
        assert_eq!(smoothed.last(), Some(&(4, 1)));
        assert_eq!(smoothed.len(), 2);
    }

    #[test]
    fn smoothed_path_preserves_necessary_turns() {
        let grid = grid_from_rows(&["PPPPP", "P###P", "PPPPP"]);
        let path = find_path(&grid, (0, 1), (4, 1)).unwrap();
        let smoothed = smooth_path(&grid, path.clone());

        assert_eq!(smoothed.first(), Some(&(0, 1)));
        assert_eq!(smoothed.last(), Some(&(4, 1)));
        assert!(smoothed.len() < path.len());
        assert!(smoothed.len() >= 3);
        assert!(smoothed
            .iter()
            .all(|&(x, y)| grid.get(x, y).unwrap().terrain.is_walkable()));
    }

    #[test]
    fn smoothed_path_segments_have_line_of_sight() {
        let grid = grid_from_rows(&["PPPPP", "P###P", "PPPPP"]);
        let path = find_path(&grid, (0, 1), (4, 1)).unwrap();
        let smoothed = smooth_path(&grid, path);

        for window in smoothed.windows(2) {
            assert!(has_line_of_sight(&grid, window[0], window[1]));
        }
    }

    #[test]
    fn smoothed_path_collapses_diagonal_line() {
        let row = "P".repeat(10);
        let rows: Vec<_> = (0..10).map(|_| row.as_str()).collect();
        let grid = grid_from_rows(&rows);

        let path = find_path(&grid, (0, 0), (9, 9)).unwrap();
        let smoothed = smooth_path(&grid, path);

        assert_eq!(smoothed.len(), 2);
        assert_eq!(smoothed.first(), Some(&(0, 0)));
        assert_eq!(smoothed.last(), Some(&(9, 9)));
    }

    #[test]
    fn respects_max_iterations_limit() {
        let row = "P".repeat(200);
        let rows: Vec<_> = (0..200).map(|_| row.as_str()).collect();
        let grid = grid_from_rows(&rows);

        let path = find_path_with_limit(&grid, (0, 0), (199, 199), Some(100));
        assert!(path.is_none());
    }

    #[test]
    fn reusable_workspace_does_not_leak_state_between_searches() {
        let grid = grid_from_rows(&["PPPPPP", "PPPPPP", "PPPPPP", "PPPPPP"]);
        let mut workspace = PathfindingWorkspace::new();

        let first = find_path_with_workspace(&mut workspace, &grid, (0, 0), (5, 3));
        let second = find_path_with_workspace(&mut workspace, &grid, (5, 3), (0, 0));

        assert_eq!(first, find_path(&grid, (0, 0), (5, 3)));
        assert_eq!(second, find_path(&grid, (5, 3), (0, 0)));
    }

    #[test]
    fn workspace_recovers_after_limited_search_failure() {
        let row = "P".repeat(32);
        let rows: Vec<_> = (0..32).map(|_| row.as_str()).collect();
        let grid = grid_from_rows(&rows);
        let mut workspace = PathfindingWorkspace::new();

        let failed =
            find_path_with_workspace_and_limit(&mut workspace, &grid, (0, 0), (31, 31), Some(1));
        assert!(failed.is_none());

        let recovered = find_path_with_workspace(&mut workspace, &grid, (0, 0), (31, 31));
        assert_eq!(recovered, find_path(&grid, (0, 0), (31, 31)));
    }

    #[test]
    fn workspace_survives_generation_overflow() {
        let grid = grid_from_rows(&["PPPPPP", "PPPPPP", "PPPPPP", "PPPPPP"]);
        let mut workspace = PathfindingWorkspace::new();
        workspace.current_gen = u32::MAX;

        let path = find_path_with_workspace(&mut workspace, &grid, (0, 0), (5, 3));
        assert_eq!(path, find_path(&grid, (0, 0), (5, 3)));

        let path2 = find_path_with_workspace(&mut workspace, &grid, (5, 3), (0, 0));
        assert_eq!(path2, find_path(&grid, (5, 3), (0, 0)));
    }
}
