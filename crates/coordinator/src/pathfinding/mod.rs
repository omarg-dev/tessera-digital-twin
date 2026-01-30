//! Pathfinding abstraction
//!
//! This module defines the `Pathfinder` trait and provides implementations.
//! To add a new pathfinding strategy, create a new file and implement the trait.
//!
//! ## Available Implementations
//! - `AStarPathfinder` - Single-robot A* (default, no collision avoidance)
//!
//! ## Future Implementations
//! - `WHCAPathfinder` - Windowed Hierarchical Cooperative A* (multi-robot)
//! - `CBSPathfinder` - Conflict-Based Search (optimal multi-robot)

mod astar;

pub use astar::AStarPathfinder;

use protocol::GridMap;

/// Grid position type alias for clarity
pub type GridPos = (usize, usize);

/// World position type alias (x, y, z)
pub type WorldPos = [f32; 3];

/// Result of a pathfinding query
#[derive(Debug, Clone)]
pub struct PathResult {
    /// Grid coordinates of the path (used by WHCA* for reservation table)
    #[allow(dead_code)]
    pub grid_path: Vec<GridPos>,
    /// World coordinates of the path (derived from grid_path)
    pub world_path: Vec<WorldPos>,
    /// Total path cost (number of steps for A*, time for WHCA*)
    pub cost: u32,
}

/// Trait for pathfinding algorithm implementations
///
/// Implement this trait to create custom pathfinding strategies:
/// - `AStarPathfinder` - Simple single-robot A* (default)
/// - `WHCAPathfinder` - Multi-robot with time windows (future)
/// - `CBSPathfinder` - Conflict-based search (future)
pub trait Pathfinder: Send + Sync {
    /// Find a path from start to goal
    ///
    /// Returns `None` if no path exists or if start/goal are not walkable.
    fn find_path(
        &self,
        map: &GridMap,
        start: GridPos,
        goal: GridPos,
    ) -> Option<PathResult>;
    
    /// Find a path avoiding other robots (for multi-robot planners)
    ///
    /// Default implementation ignores other robots (single-robot behavior).
    /// Override for WHCA* or other multi-robot algorithms.
    #[allow(dead_code, unused_variables)]
    fn find_path_avoiding(
        &self,
        map: &GridMap,
        start: GridPos,
        goal: GridPos,
        other_robots: &[(u32, GridPos)],
    ) -> Option<PathResult> {
        // Default: ignore other robots
        self.find_path(map, start, goal)
    }
    
    /// Name of the algorithm (for logging)
    fn name(&self) -> &'static str;
}

// ============================================================================
// Coordinate Conversion Utilities
// ============================================================================

use protocol::config::physics::ROBOT_HEIGHT;

/// Convert grid coordinates to world position
/// Grid (x, y) → World (x, ROBOT_HEIGHT, y)
pub fn grid_to_world(pos: GridPos) -> WorldPos {
    [pos.0 as f32, ROBOT_HEIGHT, pos.1 as f32]
}

/// Convert world position to grid coordinates
/// World (x, _, z) → Grid (round(x), round(z))
pub fn world_to_grid(pos: WorldPos) -> GridPos {
    ((pos[0] + 0.5) as usize, (pos[2] + 0.5) as usize)
}

/// Convert a grid path to world coordinates
pub fn grid_to_world_path(grid_path: &[GridPos]) -> Vec<WorldPos> {
    grid_path.iter().map(|&pos| grid_to_world(pos)).collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_grid_to_world() {
        let grid = (5, 10);
        let world = grid_to_world(grid);
        assert_eq!(world[0], 5.0);
        assert_eq!(world[1], ROBOT_HEIGHT);
        assert_eq!(world[2], 10.0);
    }

    #[test]
    fn test_world_to_grid() {
        // Center of tile
        assert_eq!(world_to_grid([5.0, 0.25, 10.0]), (5, 10));
        // Slightly off-center (should round to same tile)
        assert_eq!(world_to_grid([5.3, 0.25, 10.4]), (5, 10));
        // Edge case: just past halfway rounds up
        assert_eq!(world_to_grid([5.6, 0.25, 10.6]), (6, 11));
    }

    #[test]
    fn test_grid_to_world_path() {
        let grid_path = vec![(0, 0), (1, 0), (2, 0)];
        let world_path = grid_to_world_path(&grid_path);
        assert_eq!(world_path.len(), 3);
        assert_eq!(world_path[0], [0.0, ROBOT_HEIGHT, 0.0]);
        assert_eq!(world_path[2], [2.0, ROBOT_HEIGHT, 0.0]);
    }
}
