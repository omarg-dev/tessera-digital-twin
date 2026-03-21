//! Shared utility helpers for world/grid math and payload validation.

use std::collections::VecDeque;

use crate::GridMap;

/// Convert a world-space position into grid coordinates.
///
/// Returns None when the input is non-finite or rounds to a negative coordinate.
pub fn world_to_grid(pos: [f32; 3]) -> Option<(usize, usize)> {
    Some((round_to_index(pos[0])?, round_to_index(pos[2])?))
}

/// Convert grid coordinates to world-space position.
pub fn grid_to_world(grid: (usize, usize), y: f32) -> [f32; 3] {
    [grid.0 as f32, y, grid.1 as f32]
}

/// Check if a world-space position contains only finite values.
pub fn is_finite_position(pos: [f32; 3]) -> bool {
    pos[0].is_finite() && pos[1].is_finite() && pos[2].is_finite()
}

/// Squared distance in the XZ plane.
pub fn distance_sq_xz(a: [f32; 3], b: [f32; 3]) -> f32 {
    let dx = a[0] - b[0];
    let dz = a[2] - b[2];
    dx * dx + dz * dz
}

/// Distance in the XZ plane.
pub fn distance_xz(a: [f32; 3], b: [f32; 3]) -> f32 {
    distance_sq_xz(a, b).sqrt()
}

/// Manhattan distance in the XZ plane.
pub fn manhattan_distance_xz(a: [f32; 3], b: [f32; 3]) -> f32 {
    (a[0] - b[0]).abs() + (a[2] - b[2]).abs()
}

/// Check whether a walkable path exists from start to goal.
///
/// If goal is non-walkable, this searches for a path to any adjacent walkable tile.
pub fn is_reachable_on_map(map: &GridMap, start: (usize, usize), goal: (usize, usize)) -> bool {
    if start.0 >= map.width || start.1 >= map.height {
        return false;
    }
    if !map.is_walkable(start.0, start.1) {
        return false;
    }

    let goals = reachable_goal_candidates(map, goal);
    if goals.is_empty() {
        return false;
    }

    let mut visited = vec![vec![false; map.width]; map.height];
    let mut queue = VecDeque::new();
    queue.push_back(start);
    visited[start.1][start.0] = true;

    while let Some((x, y)) = queue.pop_front() {
        if goals.iter().any(|g| g.0 == x && g.1 == y) {
            return true;
        }
        for (dx, dy) in [(1i32, 0i32), (-1, 0), (0, 1), (0, -1)] {
            let nx = x as i32 + dx;
            let ny = y as i32 + dy;
            if nx < 0 || ny < 0 {
                continue;
            }
            let nx = nx as usize;
            let ny = ny as usize;
            if nx >= map.width || ny >= map.height {
                continue;
            }
            if visited[ny][nx] || !map.is_walkable(nx, ny) {
                continue;
            }
            visited[ny][nx] = true;
            queue.push_back((nx, ny));
        }
    }

    false
}

fn reachable_goal_candidates(map: &GridMap, goal: (usize, usize)) -> Vec<(usize, usize)> {
    let mut goals = Vec::new();
    if goal.0 < map.width && goal.1 < map.height && map.is_walkable(goal.0, goal.1) {
        goals.push(goal);
        return goals;
    }

    for (dx, dy) in [(1i32, 0i32), (-1, 0), (0, 1), (0, -1)] {
        let nx = goal.0 as i32 + dx;
        let ny = goal.1 as i32 + dy;
        if nx < 0 || ny < 0 {
            continue;
        }
        let nx = nx as usize;
        let ny = ny as usize;
        if nx >= map.width || ny >= map.height {
            continue;
        }
        if map.is_walkable(nx, ny) {
            goals.push((nx, ny));
        }
    }
    goals
}

fn round_to_index(value: f32) -> Option<usize> {
    if !value.is_finite() {
        return None;
    }
    let rounded = value.round();
    if rounded < 0.0 {
        return None;
    }
    Some(rounded as usize)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn simple_map() -> GridMap {
        GridMap::parse(
            r#"
            . . .
            . # .
            . . .
            "#,
        )
        .expect("test map should parse")
    }

    #[test]
    fn test_world_to_grid_valid() {
        assert_eq!(world_to_grid([2.4, 0.0, 7.6]), Some((2, 8)));
    }

    #[test]
    fn test_world_to_grid_invalid() {
        assert_eq!(world_to_grid([f32::NAN, 0.0, 1.0]), None);
        assert_eq!(world_to_grid([-0.6, 0.0, 1.0]), None);
    }

    #[test]
    fn test_distance_xz() {
        let a = [1.0, 10.0, 1.0];
        let b = [4.0, -5.0, 5.0];
        assert!((distance_sq_xz(a, b) - 25.0).abs() < 1e-6);
        assert!((distance_xz(a, b) - 5.0).abs() < 1e-6);
    }

    #[test]
    fn test_manhattan_distance_xz() {
        let a = [1.0, 0.0, 1.0];
        let b = [4.0, 0.0, 5.0];
        assert!((manhattan_distance_xz(a, b) - 7.0).abs() < 1e-6);
    }

    #[test]
    fn test_is_reachable_on_map_walkable_goal() {
        let map = simple_map();
        assert!(is_reachable_on_map(&map, (0, 0), (2, 2)));
    }

    #[test]
    fn test_is_reachable_on_map_non_walkable_goal_adjacent() {
        let map = simple_map();
        assert!(is_reachable_on_map(&map, (0, 0), (1, 1)));
    }

    #[test]
    fn test_is_reachable_on_map_invalid_start() {
        let map = simple_map();
        assert!(!is_reachable_on_map(&map, (99, 99), (1, 1)));
    }
}