//! Pathfinding strategy dispatcher
//!
//! Allows runtime selection of pathfinding algorithm via config.
//! Provides enum-based dispatch to A* or WHCA* while maintaining trait flexibility.

use super::{AStarPathfinder, WHCAPathfinder, Pathfinder, PathResult, GridPos};
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

    /// Clear reservations for a robot (multi-robot algorithms only)
    pub fn clear_robot_reservations(&mut self, robot_id: u32) {
        if let PathfinderInstance::WHCA(whca) = self {
            whca.clear_robot_reservations(robot_id);
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
