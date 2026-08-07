use crate::world::Grid;
use std::cmp::Ordering;
use std::collections::BinaryHeap;

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

pub fn find_path(grid: &Grid, start: (u32, u32), goal: (u32, u32)) -> Option<Vec<(u32, u32)>> {
    if !grid.get(start.0, start.1)?.terrain.is_walkable()
        || !grid.get(goal.0, goal.1)?.terrain.is_walkable()
    {
        return None;
    }

    if start == goal {
        return Some(vec![start]);
    }

    let start_index = index(grid, start);
    let goal_index = index(grid, goal);
    let mut open = BinaryHeap::new();
    let mut came_from = vec![None; grid.tiles.len()];
    let mut costs = vec![f32::INFINITY; grid.tiles.len()];

    costs[start_index] = 0.0;
    open.push(OpenNode {
        position: start_index,
        estimated_total: manhattan(start, goal),
    });

    while let Some(current) = open.pop() {
        let current_coord = coordinate(grid, current.position);
        let best_estimate = costs[current.position] + manhattan(current_coord, goal);
        if current.estimated_total > best_estimate {
            continue;
        }

        if current.position == goal_index {
            return Some(reconstruct_path(grid, &came_from, start_index, goal_index));
        }

        for neighbor in neighbors(grid, current_coord) {
            let neighbor_index = index(grid, neighbor);
            let movement_cost = grid.get(neighbor.0, neighbor.1)?.terrain.movement_cost();
            let Some(movement_cost) = movement_cost else {
                continue;
            };
            let candidate_cost = costs[current.position] + movement_cost;

            if candidate_cost < costs[neighbor_index] {
                came_from[neighbor_index] = Some(current.position);
                costs[neighbor_index] = candidate_cost;
                open.push(OpenNode {
                    position: neighbor_index,
                    estimated_total: candidate_cost + manhattan(neighbor, goal),
                });
            }
        }
    }

    None
}

fn index(grid: &Grid, coordinate: (u32, u32)) -> usize {
    (coordinate.1 * grid.width + coordinate.0) as usize
}

fn coordinate(grid: &Grid, index: usize) -> (u32, u32) {
    let index = index as u32;
    (index % grid.width, index / grid.width)
}

fn manhattan(left: (u32, u32), right: (u32, u32)) -> f32 {
    (left.0.abs_diff(right.0) + left.1.abs_diff(right.1)) as f32
}

fn neighbors(grid: &Grid, (x, y): (u32, u32)) -> impl Iterator<Item = (u32, u32)> {
    [
        y.checked_sub(1).map(|ny| (x, ny)),
        x.checked_sub(1).map(|nx| (nx, y)),
        (x + 1 < grid.width).then_some((x + 1, y)),
        (y + 1 < grid.height).then_some((x, y + 1)),
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
        assert_eq!(path.len(), 7);
    }

    #[test]
    fn rejects_unwalkable_endpoints_and_out_of_bounds_coordinates() {
        let grid = grid_from_rows(&["P#"]);

        assert!(find_path(&grid, (0, 0), (1, 0)).is_none());
        assert!(find_path(&grid, (2, 0), (0, 0)).is_none());
    }

    #[test]
    fn finds_a_route_across_a_large_world() {
        let row = "P".repeat(256);
        let rows: Vec<_> = (0..256).map(|_| row.as_str()).collect();
        let grid = grid_from_rows(&rows);
        let path = find_path(&grid, (0, 0), (255, 255)).unwrap();

        assert_eq!(path.first(), Some(&(0, 0)));
        assert_eq!(path.last(), Some(&(255, 255)));
        assert_eq!(path.len(), 511);
    }
}
