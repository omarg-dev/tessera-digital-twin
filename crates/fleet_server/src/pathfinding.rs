//! A* Pathfinding for the fleet server
//!
//! Grid-based pathfinding using A* algorithm.
//! This runs on the fleet_server to calculate paths for robots.

use protocol::GridMap;
use std::collections::{BinaryHeap, HashMap};
use std::cmp::Ordering;

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

/// Manhattan distance heuristic
fn heuristic(x1: usize, y1: usize, x2: usize, y2: usize) -> u32 {
    ((x1 as i32 - x2 as i32).abs() + (y1 as i32 - y2 as i32).abs()) as u32
}

/// 4-directional neighbors
const DIRS: [(i32, i32); 4] = [(0, 1), (1, 0), (0, -1), (-1, 0)];

/// Find path from start to goal using A*
/// Returns grid coordinates, not world coordinates
pub fn find_path(
    map: &GridMap,
    start: (usize, usize),
    goal: (usize, usize),
) -> Option<Vec<(usize, usize)>> {
    if !map.is_walkable(start.0, start.1) || !map.is_walkable(goal.0, goal.1) {
        return None;
    }
    
    if start == goal {
        return Some(vec![start]);
    }
    
    let mut open_set = BinaryHeap::new();
    let mut came_from: HashMap<(usize, usize), (usize, usize)> = HashMap::new();
    let mut g_scores: HashMap<(usize, usize), u32> = HashMap::new();
    
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
            let mut path = vec![current_pos];
            let mut pos = current_pos;
            while let Some(&prev) = came_from.get(&pos) {
                path.push(prev);
                pos = prev;
            }
            path.reverse();
            return Some(path);
        }
        
        let current_g = *g_scores.get(&current_pos).unwrap_or(&u32::MAX);
        
        for (dx, dy) in DIRS {
            let nx = current.x as i32 + dx;
            let ny = current.y as i32 + dy;
            
            if nx < 0 || ny < 0 {
                continue;
            }
            
            let nx = nx as usize;
            let ny = ny as usize;
            
            if nx >= map.width || ny >= map.height {
                continue;
            }
            
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

/// Convert grid path to world coordinates
/// Grid (x, y) → World (x, 0.25, y) with tile_size = 1.0
pub fn grid_to_world_path(grid_path: &[(usize, usize)]) -> Vec<[f32; 3]> {
    grid_path.iter()
        .map(|(x, y)| [*x as f32, 0.25, *y as f32])
        .collect()
}

/// Convert world position to grid coordinates
pub fn world_to_grid(pos: [f32; 3]) -> (usize, usize) {
    ((pos[0] + 0.5) as usize, (pos[2] + 0.5) as usize)
}

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
        
        let path = find_path(&map, (0, 0), (4, 0));
        assert!(path.is_some());
        
        let path = path.unwrap();
        assert_eq!(path.first(), Some(&(0, 0)));
        assert_eq!(path.last(), Some(&(4, 0)));
    }
    
    #[test]
    fn test_no_path() {
        let map_str = r#"
            . # .
            . # .
            . # .
        "#;
        let map = GridMap::parse(map_str).unwrap();
        
        let path = find_path(&map, (0, 0), (2, 0));
        assert!(path.is_none());
    }
}
