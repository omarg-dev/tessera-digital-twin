//! Pathfinding strategy dispatcher
//!
//! Allows runtime selection of pathfinding algorithm via config.
//! Provides enum-based dispatch to A* or WHCA* while maintaining trait flexibility.

use super::{AStarPathfinder, WHCAPathfinder, WHCAStatsSnapshot, Pathfinder, PathResult, GridPos};
use protocol::GridMap;

/// Runtime-selectable pathfinding strategy
pub enum PathfinderInstance {
    /// Single-robot A* pathfinding (no collision avoidance)
    AStar(AStarPathfinder),
    /// Multi-robot WHCA* (space-time collision avoidance)
    WHCA(WHCAPathfinder),
}

impl PathfinderInstance {
    /// Create pathfinder based on config strategy
    pub fn from_config() -> Self {
        match protocol::config::coordinator::PATHFINDING_STRATEGY {
            "astar" => {
                PathfinderInstance::AStar(AStarPathfinder::new())
            }
            "whca" | _ => {
                PathfinderInstance::WHCA(WHCAPathfinder::with_defaults())
            }
        }
    }

    /// Advance the global clock for multi-robot coordination (multi-robot algorithms only)
    pub fn tick(&mut self) {
        if let PathfinderInstance::WHCA(whca) = self {
            whca.tick();
        }
    }

    /// Reserve a path for multi-robot coordination (multi-robot algorithms only)
    pub fn reserve_path(&mut self, robot_id: u32, path: &[GridPos], velocity: [f32; 3]) {
        if let PathfinderInstance::WHCA(whca) = self {
            whca.reserve_path(robot_id, path, velocity);
        }
    }

    /// Reserve a stationary robot's position (multi-robot algorithms only)
    pub fn reserve_stationary(&mut self, robot_id: u32, pos: GridPos) {
        if let PathfinderInstance::WHCA(whca) = self {
            whca.reserve_stationary(robot_id, pos);
        }
    }

    /// Reserve a short history of stationary positions (multi-robot algorithms only)
    pub fn reserve_stationary_history(
        &mut self,
        robot_id: u32,
        positions: &std::collections::VecDeque<GridPos>,
        current_pos: GridPos,
    ) {
        if let PathfinderInstance::WHCA(whca) = self {
            whca.reserve_stationary_history(robot_id, positions, current_pos);
        }
    }

    /// Clear reservations for a robot (multi-robot algorithms only)
    pub fn clear_robot_reservations(&mut self, robot_id: u32) {
        if let PathfinderInstance::WHCA(whca) = self {
            whca.clear_robot_reservations(robot_id);
        }
    }

    /// Check if a cell is reserved right now (multi-robot algorithms only)
    pub fn is_reserved_now(&self, pos: GridPos, exclude_robot: Option<u32>) -> bool {
        match self {
            PathfinderInstance::WHCA(whca) => whca.is_reserved_now(pos, exclude_robot),
            _ => false,
        }
    }

    /// Check if a cell is reserved in the near future (multi-robot algorithms only)
    pub fn is_reserved_soon(
        &self,
        pos: GridPos,
        offset_ms: u64,
        exclude_robot: Option<u32>,
    ) -> bool {
        match self {
            PathfinderInstance::WHCA(whca) => whca.is_reserved_soon(pos, offset_ms, exclude_robot),
            _ => false,
        }
    }

    /// Find path with robot self-exclusion (WHCA* won't collide with own reservations)
    ///
    /// For A* strategy, robot_id is ignored (no reservation table).
    /// For WHCA*, strict no-fallback behavior applies.
    pub fn find_path_for_robot(
        &self,
        map: &GridMap,
        start: GridPos,
        goal: GridPos,
        robot_id: u32,
    ) -> Option<PathResult> {
        match self {
            PathfinderInstance::AStar(astar) => astar.find_path(map, start, goal),
            PathfinderInstance::WHCA(whca) => whca.find_path_for_robot(map, start, goal, robot_id),
        }
    }

    /// Find path to non-walkable tile with robot self-exclusion
    ///
    /// For WHCA*, strict no-fallback behavior applies.
    pub fn find_path_to_non_walkable_for_robot(
        &self,
        map: &GridMap,
        start: GridPos,
        goal: GridPos,
        robot_id: u32,
    ) -> Option<PathResult> {
        match self {
            PathfinderInstance::AStar(astar) => astar.find_path_to_non_walkable(map, start, goal),
            PathfinderInstance::WHCA(whca) => whca.find_path_to_non_walkable_for_robot(map, start, goal, robot_id),
        }
    }

    /// Return WHCA metrics snapshot if the active strategy is WHCA*
    pub fn whca_stats_snapshot(&self) -> Option<WHCAStatsSnapshot> {
        match self {
            PathfinderInstance::WHCA(whca) => Some(whca.stats_snapshot()),
            PathfinderInstance::AStar(_) => None,
        }
    }

    /// Reset WHCA metrics counters when running benchmark windows
    pub fn reset_whca_stats(&self) {
        if let PathfinderInstance::WHCA(whca) = self {
            whca.reset_stats();
        }
    }
}

impl Pathfinder for PathfinderInstance {
    fn find_path(
        &self,
        map: &GridMap,
        start: GridPos,
        goal: GridPos,
    ) -> Option<PathResult> {
        match self {
            PathfinderInstance::AStar(astar) => astar.find_path(map, start, goal),
            PathfinderInstance::WHCA(whca) => whca.find_path(map, start, goal),
        }
    }

    fn find_path_to_non_walkable(
        &self,
        map: &GridMap,
        start: GridPos,
        goal: GridPos,
    ) -> Option<PathResult> {
        match self {
            PathfinderInstance::AStar(astar) => astar.find_path_to_non_walkable(map, start, goal),
            PathfinderInstance::WHCA(whca) => whca.find_path_to_non_walkable(map, start, goal),
        }
    }

    fn find_path_avoiding(
        &self,
        map: &GridMap,
        start: GridPos,
        goal: GridPos,
        obstacles: &[(u32, GridPos)],
    ) -> Option<PathResult> {
        match self {
            PathfinderInstance::AStar(astar) => {
                astar.find_path_avoiding(map, start, goal, obstacles)
            }
            PathfinderInstance::WHCA(whca) => {
                whca.find_path_avoiding(map, start, goal, obstacles)
            }
        }
    }

    fn name(&self) -> &'static str {
        match self {
            PathfinderInstance::AStar(astar) => astar.name(),
            PathfinderInstance::WHCA(whca) => whca.name(),
        }
    }
}
