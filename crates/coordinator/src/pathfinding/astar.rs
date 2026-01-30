//! A* Pathfinding Algorithm
//!
//! Grid-based single-robot pathfinding using the A* algorithm.
//! This is the default pathfinder - simple and fast but no multi-robot coordination.

use super::{GridPos, PathResult, Pathfinder, grid_to_world_path};
use protocol::GridMap;
use std::collections::{BinaryHeap, HashMap};
use std::cmp::Ordering;

/// A* pathfinder implementation
///
/// Simple single-robot pathfinder using Manhattan distance heuristic.
/// Does not consider other robots - use WHCA* for multi-robot scenarios.
pub struct AStarPathfinder;

impl AStarPathfinder {
    pub fn new() -> Self {
        AStarPathfinder
    }
}

impl Default for AStarPathfinder {
    fn default() -> Self {
        Self::new()
    }
}

impl Pathfinder for AStarPathfinder {
    fn find_path(
        &self,
        map: &GridMap,
        start: GridPos,
        goal: GridPos,
    ) -> Option<PathResult> {
        find_path_astar(map, start, goal)
    }
    
    fn name(&self) -> &'static str {
        "A*"
    }
}

// ============================================================================
// A* Algorithm Implementation
// ============================================================================

/// A node in the A* search
#[derive(Clone, Eq, PartialEq)]
struct Node {
    x: usize,
    y: usize,
    g_cost: u32,  // Cost from start
    f_cost: u32,  // g + heuristic
}

impl Ord for Node {
    fn cmp(&self, other: &Self) -> Ordering {
        // Reverse ordering for min-heap (BinaryHeap is max-heap by default)
        other.f_cost.cmp(&self.f_cost)
            .then_with(|| other.g_cost.cmp(&self.g_cost))
    }
}

impl PartialOrd for Node {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

/// Manhattan distance heuristic (admissible for 4-directional movement)
fn heuristic(x1: usize, y1: usize, x2: usize, y2: usize) -> u32 {
    ((x1 as i32 - x2 as i32).abs() + (y1 as i32 - y2 as i32).abs()) as u32
}

/// 4-directional movement (N, E, S, W)
const DIRS: [(i32, i32); 4] = [(0, 1), (1, 0), (0, -1), (-1, 0)];

/// Core A* algorithm
fn find_path_astar(
    map: &GridMap,
    start: GridPos,
    goal: GridPos,
) -> Option<PathResult> {
    // Validate start and goal
    if !map.is_walkable(start.0, start.1) || !map.is_walkable(goal.0, goal.1) {
        return None;
    }
    
    // Already at goal
    if start == goal {
        return Some(PathResult {
            grid_path: vec![start],
            world_path: grid_to_world_path(&[start]),
            cost: 0,
        });
    }
    
    let mut open_set = BinaryHeap::new();
    let mut came_from: HashMap<GridPos, GridPos> = HashMap::new();
    let mut g_scores: HashMap<GridPos, u32> = HashMap::new();
    
    g_scores.insert(start, 0);
    open_set.push(Node {
        x: start.0,
        y: start.1,
        g_cost: 0,
        f_cost: heuristic(start.0, start.1, goal.0, goal.1),
    });
    
    while let Some(current) = open_set.pop() {
        let current_pos = (current.x, current.y);
        
        if current_pos == goal {
            // Reconstruct path
            let grid_path = reconstruct_path(&came_from, current_pos);
            let cost = current.g_cost;
            return Some(PathResult {
                world_path: grid_to_world_path(&grid_path),
                grid_path,
                cost,
            });
        }
        
        let current_g = *g_scores.get(&current_pos).unwrap_or(&u32::MAX);
        
        for (dx, dy) in DIRS {
            let nx = current.x as i32 + dx;
            let ny = current.y as i32 + dy;
            
            // Bounds check
            if nx < 0 || ny < 0 {
                continue;
            }
            
            let nx = nx as usize;
            let ny = ny as usize;
            
            if nx >= map.width || ny >= map.height {
                continue;
            }
            
            // Walkability check
            if !map.is_walkable(nx, ny) {
                continue;
            }
            
            let neighbor_pos = (nx, ny);
            let tentative_g = current_g + 1;
            
            if tentative_g < *g_scores.get(&neighbor_pos).unwrap_or(&u32::MAX) {
                came_from.insert(neighbor_pos, current_pos);
                g_scores.insert(neighbor_pos, tentative_g);
                
                let f = tentative_g + heuristic(nx, ny, goal.0, goal.1);
                open_set.push(Node {
                    x: nx,
                    y: ny,
                    g_cost: tentative_g,
                    f_cost: f,
                });
            }
        }
    }
    
    None // No path found
}

/// Reconstruct path from came_from map
fn reconstruct_path(came_from: &HashMap<GridPos, GridPos>, goal: GridPos) -> Vec<GridPos> {
    let mut path = vec![goal];
    let mut pos = goal;
    while let Some(&prev) = came_from.get(&pos) {
        path.push(prev);
        pos = prev;
    }
    path.reverse();
    path
}

// ============================================================================
// Tests
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_simple_path() {
        let map_str = r#"
            . . . . .
            . # # # .
            . . . . .
        "#;
        let map = GridMap::parse(map_str).unwrap();
        let pathfinder = AStarPathfinder::new();
        
        let result = pathfinder.find_path(&map, (0, 0), (4, 0));
        assert!(result.is_some());
        
        let result = result.unwrap();
        assert_eq!(result.grid_path.first(), Some(&(0, 0)));
        assert_eq!(result.grid_path.last(), Some(&(4, 0)));
        assert!(!result.world_path.is_empty());
    }
    
    #[test]
    fn test_no_path() {
        let map_str = r#"
            . # .
            . # .
            . # .
        "#;
        let map = GridMap::parse(map_str).unwrap();
        let pathfinder = AStarPathfinder::new();
        
        let result = pathfinder.find_path(&map, (0, 0), (2, 0));
        assert!(result.is_none());
    }
    
    #[test]
    fn test_already_at_goal() {
        let map_str = ". . .";
        let map = GridMap::parse(map_str).unwrap();
        let pathfinder = AStarPathfinder::new();
        
        let result = pathfinder.find_path(&map, (1, 0), (1, 0));
        assert!(result.is_some());
        
        let result = result.unwrap();
        assert_eq!(result.grid_path.len(), 1);
        assert_eq!(result.cost, 0);
    }
    
    #[test]
    fn test_path_around_obstacle() {
        let map_str = r#"
            . . . . .
            . . # . .
            . . . . .
        "#;
        let map = GridMap::parse(map_str).unwrap();
        let pathfinder = AStarPathfinder::new();
        
        let result = pathfinder.find_path(&map, (0, 1), (4, 1));
        assert!(result.is_some());
        
        let result = result.unwrap();
        // Path should go around the obstacle
        assert!(!result.grid_path.contains(&(2, 1)));
    }
    
    #[test]
    fn test_unwalkable_start_or_goal() {
        let map_str = r#"
            . # .
            . . .
        "#;
        let map = GridMap::parse(map_str).unwrap();
        let pathfinder = AStarPathfinder::new();
        
        // Start on wall
        assert!(pathfinder.find_path(&map, (1, 0), (2, 0)).is_none());
        // Goal on wall
        assert!(pathfinder.find_path(&map, (0, 0), (1, 0)).is_none());
    }

    #[test]
    fn test_pathfinder_name() {
        let pathfinder = AStarPathfinder::new();
        assert_eq!(pathfinder.name(), "A*");
    }
}
