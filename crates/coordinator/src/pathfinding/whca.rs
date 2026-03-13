//! Windowed Hierarchical Cooperative A* (WHCA*) Pathfinding
//!
//! Multi-robot pathfinding with temporal collision avoidance.
//! Each robot reserves its planned path in a space-time reservation table,
//! and subsequent robots plan around those reservations.
//!
//! ## Algorithm Overview
//!
//! WHCA* extends A* with:
//! 1. **Space-Time Search**: Nodes are (x, y, t) instead of just (x, y)
//! 2. **Reservation Table**: Tracks which cells are occupied at which timesteps
//! 3. **Windowed Planning**: Only plans W timesteps ahead (configurable)
//! 4. **Wait Actions**: Robots can wait in place if path is blocked
//!
//! ## Usage
//!
//! ```ignore
//! let mut pathfinder = WHCAPathfinder::new(32); // 32-step window
//! 
//! // Plan for robot 1
//! let path1 = pathfinder.find_path_avoiding(&map, start1, goal1, &[]);
//! pathfinder.reserve_path(1, &path1);
//!
//! // Plan for robot 2 (avoids robot 1's path)
//! let path2 = pathfinder.find_path_avoiding(&map, start2, goal2, &[(1, start1)]);
//! pathfinder.reserve_path(2, &path2);
//! ```

use super::{GridPos, PathResult, Pathfinder, grid_to_world_path};
use protocol::{GridMap, logs};
use protocol::config::coordinator as coord_config;
use protocol::config::coordinator::whca::{
    WINDOW_SIZE_MS,
    MAX_WAIT_TIME,
    RESERVATION_TOLERANCE_MS,
    MOVE_TIME_MS,
    STATIONARY_HISTORY_TILES,
    STATIONARY_RESERVATION_MS,
    COLLISION_BUFFER_TILES,
};
use std::collections::{BinaryHeap, HashMap, HashSet};
use std::cmp::Ordering;
use std::sync::Mutex;
use std::time::Instant;

#[derive(Debug, Clone, Copy, Default)]
pub struct WHCAStatsSnapshot {
    pub searches_total: u64,
    pub searches_succeeded: u64,
    pub searches_failed: u64,
    pub nodes_expanded_total: u64,
    pub reservation_probe_calls_total: u64,
    pub edge_collision_checks_total: u64,
    pub wait_actions_added_total: u64,
    pub open_set_peak_observed: u64,
    pub reservation_entries_peak: u64,
    pub total_search_time_us: u64,
    pub last_search_time_us: u64,
}

#[derive(Debug, Default)]
struct WHCAStats {
    searches_total: u64,
    searches_succeeded: u64,
    searches_failed: u64,
    nodes_expanded_total: u64,
    reservation_probe_calls_total: u64,
    edge_collision_checks_total: u64,
    wait_actions_added_total: u64,
    open_set_peak_observed: u64,
    reservation_entries_peak: u64,
    total_search_time_us: u64,
    last_search_time_us: u64,
}

#[derive(Debug, Default)]
struct SearchStatsDelta {
    nodes_expanded: u64,
    reservation_probe_calls: u64,
    edge_collision_checks: u64,
    wait_actions_added: u64,
    open_set_peak: u64,
}

/// WHCA* pathfinder with reservation table
pub struct WHCAPathfinder {
    /// Planning window size (milliseconds)
    window_size_ms: u64,
    /// Space-time reservation table: (x, y, time_ms) → robot_id
    reservations: HashMap<(usize, usize, u64), u32>,
    /// Per-robot reservation index for fast cleanup
    robot_reservations: HashMap<u32, HashSet<(usize, usize, u64)>>,
    /// Start time for millisecond calculations
    start_time: Instant,
    /// Aggregated search/runtime metrics for profiling and benchmark reporting
    stats: Mutex<WHCAStats>,
}

impl WHCAPathfinder {
    pub fn new(window_size_ms: u64) -> Self {
        WHCAPathfinder {
            window_size_ms,
            reservations: HashMap::new(),
            robot_reservations: HashMap::new(),
            start_time: Instant::now(),
            stats: Mutex::new(WHCAStats::default()),
        }
    }

    /// Create with default window size
    pub fn with_defaults() -> Self {
        Self::new(WINDOW_SIZE_MS)
    }
    
    /// Get current time in milliseconds since start
    fn current_time_ms(&self) -> u64 {
        self.start_time.elapsed().as_millis() as u64
    }

    fn insert_reservation(&mut self, key: (usize, usize, u64), robot_id: u32) {
        self.reservations.insert(key, robot_id);
        self.robot_reservations
            .entry(robot_id)
            .or_default()
            .insert(key);
    }

    fn record_search_result(&self, success: bool, delta: SearchStatsDelta, elapsed_us: u64) {
        if let Ok(mut stats) = self.stats.lock() {
            stats.searches_total += 1;
            if success {
                stats.searches_succeeded += 1;
            } else {
                stats.searches_failed += 1;
            }
            stats.nodes_expanded_total += delta.nodes_expanded;
            stats.reservation_probe_calls_total += delta.reservation_probe_calls;
            stats.edge_collision_checks_total += delta.edge_collision_checks;
            stats.wait_actions_added_total += delta.wait_actions_added;
            if delta.open_set_peak > stats.open_set_peak_observed {
                stats.open_set_peak_observed = delta.open_set_peak;
            }
            stats.total_search_time_us += elapsed_us;
            stats.last_search_time_us = elapsed_us;
        }
    }

    fn update_reservation_peak_metric(&self) {
        if let Ok(mut stats) = self.stats.lock() {
            let current = self.reservations.len() as u64;
            if current > stats.reservation_entries_peak {
                stats.reservation_entries_peak = current;
            }
        }
    }

    pub fn stats_snapshot(&self) -> WHCAStatsSnapshot {
        match self.stats.lock() {
            Ok(stats) => WHCAStatsSnapshot {
                searches_total: stats.searches_total,
                searches_succeeded: stats.searches_succeeded,
                searches_failed: stats.searches_failed,
                nodes_expanded_total: stats.nodes_expanded_total,
                reservation_probe_calls_total: stats.reservation_probe_calls_total,
                edge_collision_checks_total: stats.edge_collision_checks_total,
                wait_actions_added_total: stats.wait_actions_added_total,
                open_set_peak_observed: stats.open_set_peak_observed,
                reservation_entries_peak: stats.reservation_entries_peak,
                total_search_time_us: stats.total_search_time_us,
                last_search_time_us: stats.last_search_time_us,
            },
            Err(_) => WHCAStatsSnapshot::default(),
        }
    }

    pub fn reset_stats(&self) {
        if let Ok(mut stats) = self.stats.lock() {
            *stats = WHCAStats::default();
        }
    }

    fn reserve_cell_with_buffer(&mut self, pos: GridPos, time_ms: u64, robot_id: u32) {
        let radius = COLLISION_BUFFER_TILES as i32;
        for dx in -radius..=radius {
            for dy in -radius..=radius {
                if dx.abs() + dy.abs() > radius {
                    continue;
                }
                let x = (pos.0 as i32 + dx).max(0) as usize;
                let y = (pos.1 as i32 + dy).max(0) as usize;
                self.insert_reservation((x, y, time_ms), robot_id);
            }
        }
    }

    /// Clear old reservations
    ///
    /// Called periodically to remove stale reservations.
    /// Keeps reservations that are still within the planning window.
    pub fn tick(&mut self) {
        let now_ms = self.current_time_ms();
        // Keep reservations that haven't expired yet (future or recent)
        // A reservation at time t is stale if t < now - tolerance
        let cutoff = now_ms.saturating_sub(RESERVATION_TOLERANCE_MS as u64 + MOVE_TIME_MS);
        self.reservations.retain(|&(_, _, t), _| t >= cutoff);
        self.rebuild_robot_reservation_index();
        self.update_reservation_peak_metric();
    }

    fn rebuild_robot_reservation_index(&mut self) {
        self.robot_reservations.clear();
        for (&key, &owner) in &self.reservations {
            self.robot_reservations
                .entry(owner)
                .or_default()
                .insert(key);
        }
    }

    /// Reserve a path for a robot in the reservation table
    ///
    /// Called after each pathfinding call to reserve the planned path.
    /// Uses robot's velocity to predict arrival times.
    pub fn reserve_path(&mut self, robot_id: u32, path: &[GridPos], velocity: [f32; 3]) {
        if path.is_empty() {
            return;
        }
        
        let mut speed = (velocity[0].powi(2) + velocity[2].powi(2)).sqrt();
        if speed < 0.01 {
            // Robot velocity is often zero at assignment time; fall back to default speed.
            speed = coord_config::DEFAULT_SPEED.max(0.01);
        }
        
        let mut time_ms = self.current_time_ms();

        for i in 0..path.len() - 1 {
            let from = path[i];
            let to = path[i + 1];

            // Reserve the current cell at the segment start time
            for offset in 0..=RESERVATION_TOLERANCE_MS {
                let t = time_ms + offset as u64;
                self.reserve_cell_with_buffer(from, t, robot_id);
            }
            
            // Calculate travel time for this segment
            let dx = to.0 as f32 - from.0 as f32;
            let dz = to.1 as f32 - from.1 as f32;
            let dist = (dx * dx + dz * dz).sqrt();
            let travel_time_ms = ((dist / speed) * 1000.0) as u64;
            
            time_ms += travel_time_ms;
            
            // Reserve destination cell at predicted arrival time with tolerance
            for offset in 0..=RESERVATION_TOLERANCE_MS {
                let t = time_ms + offset as u64;
                self.reserve_cell_with_buffer(to, t, robot_id);
            }
        }

        // Reserve the final cell for dwell time (pickup/dropoff) to avoid head-on conflicts.
        let dwell_secs = coord_config::PICKUP_DELAY_SECS.max(coord_config::DROPOFF_DELAY_SECS);
        let dwell_ms = ((dwell_secs * 1000.0) as u64).min(self.window_size_ms);
        if let Some(&end) = path.last() {
            for offset_ms in 0..=dwell_ms {
                let t = time_ms + offset_ms;
                self.reserve_cell_with_buffer(end, t, robot_id);
            }
        }
        
        if path.len() > 1 {
            println!("[WHCA*] Reserved {} waypoints for robot {} ({}ms window, speed={:.2})", 
                path.len(), robot_id, time_ms - self.current_time_ms(), speed);
        }
        self.update_reservation_peak_metric();
    }

    /// Reserve a stationary robot's position throughout the planning window
    ///
    /// Called for robots that are idle, picking, dropping, or charging.
    /// Prevents other robots from pathfinding through stationary robots.
    pub fn reserve_stationary(&mut self, robot_id: u32, pos: GridPos) {
        let now_ms = self.current_time_ms();
        let duration_ms = STATIONARY_RESERVATION_MS.min(self.window_size_ms);
        // Reserve current position for a short stationary window
        for offset_ms in 0..=duration_ms {
            let time = now_ms + offset_ms;
            self.reserve_cell_with_buffer(pos, time, robot_id);
        }
        self.update_reservation_peak_metric();
        // suppress per-tick stationary log (verbose builds may re-enable)
    }

    /// Reserve a short history of stationary positions (for large robots)
    pub fn reserve_stationary_history(
        &mut self,
        robot_id: u32,
        positions: &std::collections::VecDeque<GridPos>,
        current_pos: GridPos,
    ) {
        let mut history: Vec<GridPos> = positions.iter().copied().collect();
        if history.is_empty() {
            history.push(current_pos);
        }
        let start = history.len().saturating_sub(STATIONARY_HISTORY_TILES.max(1));
        for pos in history[start..].iter() {
            self.reserve_stationary(robot_id, *pos);
        }
    }

    /// Clear all reservations for a specific robot
    ///
    /// Called when a robot's task completes or times out
    pub fn clear_robot_reservations(&mut self, robot_id: u32) {
        if let Some(keys) = self.robot_reservations.remove(&robot_id) {
            for key in keys {
                self.reservations.remove(&key);
            }
            return;
        }
        // fallback if index is missing (e.g., legacy state)
        self.reservations.retain(|_, &mut id| id != robot_id);
        self.rebuild_robot_reservation_index();
    }

    fn robot_ids_in_window(
        &self,
        x: usize,
        y: usize,
        start_t: u64,
        end_t: u64,
        exclude_robot: Option<u32>,
    ) -> HashSet<u32> {
        let mut ids = HashSet::new();
        for t in start_t..=end_t {
            if let Some(robot_id) = self.reservations.get(&(x, y, t)) {
                if let Some(exclude) = exclude_robot {
                    if *robot_id == exclude {
                        continue;
                    }
                }
                ids.insert(*robot_id);
            }
        }
        ids
    }

    /// Check if a cell is reserved at a given time (by another robot)
    fn is_reserved(&self, x: usize, y: usize, time_ms: u64, exclude_robot: Option<u32>) -> bool {
        // Check reservation window ±TOLERANCE
        for offset in -RESERVATION_TOLERANCE_MS..=RESERVATION_TOLERANCE_MS {
            let t = (time_ms as i64 + offset) as u64;
            if let Some(reserved_by) = self.reservations.get(&(x, y, t)) {
                if let Some(exclude) = exclude_robot {
                    if *reserved_by != exclude {
                        return true;
                    }
                } else {
                    return true;
                }
            }
        }
        false
    }

    /// Check if a cell is reserved right now (by another robot)
    pub fn is_reserved_now(&self, pos: GridPos, exclude_robot: Option<u32>) -> bool {
        let now_ms = self.current_time_ms();
        self.is_reserved(pos.0, pos.1, now_ms, exclude_robot)
    }

    /// Check if a cell is reserved at a future time (by another robot)
    pub fn is_reserved_soon(
        &self,
        pos: GridPos,
        offset_ms: u64,
        exclude_robot: Option<u32>,
    ) -> bool {
        let t = self.current_time_ms() + offset_ms;
        self.is_reserved(pos.0, pos.1, t, exclude_robot)
    }

    /// Check for edge collision (two robots swapping positions)
    fn has_edge_collision(&self, from: GridPos, to: GridPos, time_ms: u64, exclude_robot: Option<u32>) -> bool {
        // Our move is from `from@time_ms` to `to@(time_ms + MOVE_TIME_MS)`.
        // A swap conflict exists if the same robot is on `to` around departure and
        // on `from` around our arrival.
        let tolerance = RESERVATION_TOLERANCE_MS as u64;
        let to_start = time_ms.saturating_sub(tolerance);
        let to_end = time_ms;
        let from_center = time_ms + MOVE_TIME_MS;
        let from_start = from_center.saturating_sub(tolerance);
        let from_end = from_center + tolerance;

        let to_ids = self.robot_ids_in_window(to.0, to.1, to_start, to_end, exclude_robot);
        if to_ids.is_empty() {
            return false;
        }
        let from_ids = self.robot_ids_in_window(from.0, from.1, from_start, from_end, exclude_robot);

        to_ids.iter().any(|id| from_ids.contains(id))
    }

    /// Core WHCA* algorithm
    fn find_path_whca(
        &self,
        map: &GridMap,
        start: GridPos,
        goal: GridPos,
        robot_id: Option<u32>,
    ) -> Option<PathResult> {
        let search_started = Instant::now();
        let mut stats_delta = SearchStatsDelta::default();
        // Validate start and goal
        if !map.is_walkable(start.0, start.1) || !map.is_walkable(goal.0, goal.1) {
            self.record_search_result(false, stats_delta, search_started.elapsed().as_micros() as u64);
            return None;
        }

        // Already at goal
        if start == goal {
            self.record_search_result(true, stats_delta, search_started.elapsed().as_micros() as u64);
            return Some(PathResult {
                grid_path: vec![start],
                world_path: grid_to_world_path(&[start]),
                cost: 0,
            });
        }

        let mut open_set = BinaryHeap::new();
        let mut came_from: HashMap<SpaceTimeNode, SpaceTimeNode> = HashMap::new();
        let mut g_scores: HashMap<SpaceTimeNode, u32> = HashMap::new();
        let mut closed_set: HashSet<SpaceTimeNode> = HashSet::new();

        let start_time_ms = self.current_time_ms();
        let start_node = SpaceTimeNode { x: start.0, y: start.1, t: start_time_ms };
        g_scores.insert(start_node, 0);
        open_set.push(WHCANode {
            pos: start_node,
            g_cost: 0,
            f_cost: heuristic(start.0, start.1, goal.0, goal.1),
        });
        stats_delta.open_set_peak = 1;

        while let Some(current) = open_set.pop() {
            let current_pos = current.pos;

            // Goal check
            if current_pos.x == goal.0 && current_pos.y == goal.1 {
                let path = reconstruct_spacetime_path(&came_from, current_pos);
                let grid_path: Vec<GridPos> = path.iter().map(|n| (n.x, n.y)).collect();
                self.record_search_result(true, stats_delta, search_started.elapsed().as_micros() as u64);
                return Some(PathResult {
                    world_path: grid_to_world_path(&grid_path),
                    grid_path,
                    cost: current.g_cost,
                });
            }

            // Window limit (milliseconds)
            if current_pos.t >= start_time_ms + self.window_size_ms {
                continue;
            }

            if closed_set.contains(&current_pos) {
                continue;
            }
            closed_set.insert(current_pos);
            stats_delta.nodes_expanded += 1;

            let current_g = *g_scores.get(&current_pos).unwrap_or(&u32::MAX);

            // Generate successors (4 directions + wait)
            let mut successors = Vec::new();
                
            // Movement actions
            for (dx, dy) in DIRS {
                let nx = current_pos.x as i32 + dx;
                let ny = current_pos.y as i32 + dy;

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

                let next_t = current_pos.t + MOVE_TIME_MS;

                // Check vertex collision
                stats_delta.reservation_probe_calls += 1;
                if self.is_reserved(nx, ny, next_t, robot_id) {
                    continue;
                }

                // Check edge collision (swap)
                stats_delta.edge_collision_checks += 1;
                if self.has_edge_collision((current_pos.x, current_pos.y), (nx, ny), current_pos.t, robot_id) {
                    continue;
                }

                successors.push(SpaceTimeNode { x: nx, y: ny, t: next_t });
            }

            // Wait action (stay in place)
            let wait_t = current_pos.t + MOVE_TIME_MS;
            stats_delta.reservation_probe_calls += 1;
            if !self.is_reserved(current_pos.x, current_pos.y, wait_t, robot_id) {
                // Count consecutive waits to prevent infinite waiting
                let wait_count = count_waits_in_path(&came_from, current_pos);
                if wait_count < MAX_WAIT_TIME {
                    successors.push(SpaceTimeNode { x: current_pos.x, y: current_pos.y, t: wait_t });
                    stats_delta.wait_actions_added += 1;
                }
            }

            for neighbor in successors {
                if closed_set.contains(&neighbor) {
                    continue;
                }

                let tentative_g = current_g + 1;

                if tentative_g < *g_scores.get(&neighbor).unwrap_or(&u32::MAX) {
                    came_from.insert(neighbor, current_pos);
                    g_scores.insert(neighbor, tentative_g);

                    let f = tentative_g + heuristic(neighbor.x, neighbor.y, goal.0, goal.1);
                    open_set.push(WHCANode {
                        pos: neighbor,
                        g_cost: tentative_g,
                        f_cost: f,
                    });
                    let open_len = open_set.len() as u64;
                    if open_len > stats_delta.open_set_peak {
                        stats_delta.open_set_peak = open_len;
                    }
                }
            }
        }

        self.record_search_result(false, stats_delta, search_started.elapsed().as_micros() as u64);

        None // No path found
    }
}

impl WHCAPathfinder {
    /// Find path with self-exclusion (robot won't collide with its own reservations)
    ///
    /// This is the primary method to use from the coordinator/task manager.
    /// Falls back to plain A* if WHCA* fails (reservation congestion).
    pub fn find_path_for_robot(
        &self,
        map: &GridMap,
        start: GridPos,
        goal: GridPos,
        robot_id: u32,
    ) -> Option<PathResult> {
        let result = self.find_path_whca(map, start, goal, Some(robot_id));
        if result.is_none() {
            logs::save_log(
                "Coordinator",
                &format!(
                    "WHCA* no-path for robot {} from ({},{}) to ({},{})",
                    robot_id, start.0, start.1, goal.0, goal.1
                ),
            );
        }
        result
    }

    /// Find path to non-walkable tile (e.g. shelf) with self-exclusion
    ///
    /// Falls back to A* if WHCA* fails.
    pub fn find_path_to_non_walkable_for_robot(
        &self,
        map: &GridMap,
        start: GridPos,
        goal: GridPos,
        robot_id: u32,
    ) -> Option<PathResult> {
        // Start must be walkable
        if !map.is_walkable(start.0, start.1) {
            return None;
        }

        // For walkable goals, use normal robot-aware pathfinding
        if map.is_walkable(goal.0, goal.1) {
            return self.find_path_for_robot(map, start, goal, robot_id);
        }

        // Goal is non-walkable - find path to adjacent tile
        if start == goal || is_adjacent(start, goal) {
            return Some(PathResult {
                grid_path: vec![start],
                world_path: grid_to_world_path(&[start]),
                cost: 0,
            });
        }

        // Try WHCA* to each walkable neighbor, pick cheapest
        let mut best_path: Option<PathResult> = None;
        let mut best_cost = u32::MAX;

        for (dx, dy) in DIRS {
            let nx = goal.0 as i32 + dx;
            let ny = goal.1 as i32 + dy;
            if nx < 0 || ny < 0 { continue; }
            let (nx, ny) = (nx as usize, ny as usize);
            if nx >= map.width || ny >= map.height { continue; }
            if !map.is_walkable(nx, ny) { continue; }

            // Use self-exclusion for each attempt
            if let Some(path) = self.find_path_whca(map, start, (nx, ny), Some(robot_id)) {
                if path.cost < best_cost {
                    best_cost = path.cost;
                    best_path = Some(path);
                }
            }
        }

        if best_path.is_none() {
            logs::save_log(
                "Coordinator",
                &format!(
                    "WHCA* no-path to non-walkable goal for robot {} from ({},{}) to ({},{})",
                    robot_id, start.0, start.1, goal.0, goal.1
                ),
            );
        }

        best_path
    }
}

impl Default for WHCAPathfinder {
    fn default() -> Self {
        Self::with_defaults()
    }
}

impl Pathfinder for WHCAPathfinder {
    fn find_path(
        &self,
        map: &GridMap,
        start: GridPos,
        goal: GridPos,
    ) -> Option<PathResult> {
        let result = self.find_path_whca(map, start, goal, None);
        if result.is_none() {
            logs::save_log(
                "Coordinator",
                &format!(
                    "WHCA* strict no-path (no robot context) from ({},{}) to ({},{})",
                    start.0, start.1, goal.0, goal.1
                ),
            );
        }
        result
    }

    fn find_path_to_non_walkable(
        &self,
        map: &GridMap,
        start: GridPos,
        goal: GridPos,
    ) -> Option<PathResult> {
        // Start must be walkable
        if !map.is_walkable(start.0, start.1) {
            return None;
        }
        if map.is_walkable(goal.0, goal.1) {
            return self.find_path(map, start, goal);
        }
        if start == goal || is_adjacent(start, goal) {
            return Some(PathResult {
                grid_path: vec![start],
                world_path: grid_to_world_path(&[start]),
                cost: 0,
            });
        }

        let mut best_path: Option<PathResult> = None;
        let mut best_cost = u32::MAX;

        for (dx, dy) in DIRS {
            let nx = goal.0 as i32 + dx;
            let ny = goal.1 as i32 + dy;
            if nx < 0 || ny < 0 {
                continue;
            }

            let (nx, ny) = (nx as usize, ny as usize);
            if nx >= map.width || ny >= map.height {
                continue;
            }
            if !map.is_walkable(nx, ny) {
                continue;
            }

            if let Some(path) = self.find_path_whca(map, start, (nx, ny), None) {
                if path.cost < best_cost {
                    best_cost = path.cost;
                    best_path = Some(path);
                }
            }
        }

        if best_path.is_none() {
            logs::save_log(
                "Coordinator",
                &format!(
                    "WHCA* strict no-path to non-walkable goal (no robot context) from ({},{}) to ({},{})",
                    start.0, start.1, goal.0, goal.1
                ),
            );
        }

        best_path
    }

    fn find_path_avoiding(
        &self,
        map: &GridMap,
        start: GridPos,
        goal: GridPos,
        _other_robots: &[(u32, GridPos)],
    ) -> Option<PathResult> {
        // The reservation table already handles collision avoidance.
        // This trait method has no robot_id, so strict behavior applies without fallback.
        self.find_path(map, start, goal)
    }

    fn name(&self) -> &'static str {
        "WHCA*"
    }
}

// ============================================================================
// Helper Types and Functions
// ============================================================================

/// Space-time node: position + timestep
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
struct SpaceTimeNode {
    x: usize,
    y: usize,
    t: u64,  // Time in milliseconds
}

/// Node for the priority queue
#[derive(Clone, Eq, PartialEq)]
struct WHCANode {
    pos: SpaceTimeNode,
    g_cost: u32,
    f_cost: u32,
}

impl Ord for WHCANode {
    fn cmp(&self, other: &Self) -> Ordering {
        other.f_cost.cmp(&self.f_cost)
            .then_with(|| other.g_cost.cmp(&self.g_cost))
    }
}

impl PartialOrd for WHCANode {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

/// Manhattan distance heuristic
fn heuristic(x1: usize, y1: usize, x2: usize, y2: usize) -> u32 {
    ((x1 as i32 - x2 as i32).abs() + (y1 as i32 - y2 as i32).abs()) as u32
}

/// 4-directional movement
const DIRS: [(i32, i32); 4] = [(0, 1), (1, 0), (0, -1), (-1, 0)];

/// Check if two positions are orthogonally adjacent
fn is_adjacent(pos: GridPos, target: GridPos) -> bool {
    let (x1, y1) = pos;
    let (x2, y2) = target;
    (x1.abs_diff(x2) + y1.abs_diff(y2)) == 1
}

/// Reconstruct path from came_from map
fn reconstruct_spacetime_path(
    came_from: &HashMap<SpaceTimeNode, SpaceTimeNode>,
    goal: SpaceTimeNode,
) -> Vec<SpaceTimeNode> {
    let mut path = vec![goal];
    let mut pos = goal;
    while let Some(&prev) = came_from.get(&pos) {
        path.push(prev);
        pos = prev;
    }
    path.reverse();
    path
}

/// Count consecutive waits in path (to prevent infinite waiting)
fn count_waits_in_path(came_from: &HashMap<SpaceTimeNode, SpaceTimeNode>, current: SpaceTimeNode) -> u32 {
    let mut count = 0;
    let mut pos = current;
    
    while let Some(&prev) = came_from.get(&pos) {
        if prev.x == pos.x && prev.y == pos.y {
            count += 1;
            pos = prev;
        } else {
            break;
        }
    }
    
    count
}

// ============================================================================
// Tests
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_simple_path_no_conflicts() {
        let map_str = ". . . . .";
        let map = GridMap::parse(map_str).unwrap();
        let pathfinder = WHCAPathfinder::with_defaults();

        let result = pathfinder.find_path(&map, (0, 0), (4, 0));
        assert!(result.is_some());

        let result = result.unwrap();
        assert_eq!(result.grid_path.first(), Some(&(0, 0)));
        assert_eq!(result.grid_path.last(), Some(&(4, 0)));
    }

    #[test]
    fn test_path_with_reservation() {
        let map_str = ". . . . .";
        let map = GridMap::parse(map_str).unwrap();
        let mut pathfinder = WHCAPathfinder::with_defaults();

        // Reserve the direct path for robot 1 (velocity of 2.0 units/sec)
        let velocity = [2.0, 0.0, 0.0];
        pathfinder.reserve_path(1, &[(1, 0), (2, 0), (3, 0)], velocity);

        // Robot 2 should still find a path (may wait or go around)
        let result = pathfinder.find_path(&map, (0, 0), (4, 0));
        assert!(result.is_some());
    }

    #[test]
    fn test_already_at_goal() {
        let map_str = ". . .";
        let map = GridMap::parse(map_str).unwrap();
        let pathfinder = WHCAPathfinder::with_defaults();

        let result = pathfinder.find_path(&map, (1, 0), (1, 0));
        assert!(result.is_some());

        let result = result.unwrap();
        assert_eq!(result.grid_path.len(), 1);
        assert_eq!(result.cost, 0);
    }

    #[test]
    fn test_no_path() {
        let map_str = r#"
            . # .
            . # .
            . # .
        "#;
        let map = GridMap::parse(map_str).unwrap();
        let pathfinder = WHCAPathfinder::with_defaults();

        let result = pathfinder.find_path(&map, (0, 0), (2, 0));
        assert!(result.is_none());
    }

    #[test]
    fn test_pathfinder_name() {
        let pathfinder = WHCAPathfinder::with_defaults();
        assert_eq!(pathfinder.name(), "WHCA*");
    }

    #[test]
    fn test_tick_clears_old_reservations() {
        let mut pathfinder = WHCAPathfinder::with_defaults();
        
        // Sleep to ensure time advances significantly
        std::thread::sleep(std::time::Duration::from_millis(1500));
        let now_ms = pathfinder.current_time_ms();
        
        // Add reservations: one from start time (should be cleared), one future (kept)
        pathfinder.reservations.insert((1, 1, 10), 1);             // Very old (should clear)
        pathfinder.reservations.insert((2, 2, now_ms + 5000), 2);  // Future (should keep)
        
        // Tick clears reservations where t + 1000 < now
        // So reservation at t=10: 10 + 1000 = 1010 < now_ms (which is ~1500+)
        pathfinder.tick();
        
        // Old reservation should be gone
        assert!(!pathfinder.reservations.contains_key(&(1, 1, 10)));
        // Future reservation should remain
        assert!(pathfinder.reservations.contains_key(&(2, 2, now_ms + 5000)));
    }

    #[test]
    fn test_clear_robot_reservations() {
        let mut pathfinder = WHCAPathfinder::with_defaults();
        let velocity = [2.0, 0.0, 0.0];
        
        pathfinder.reserve_path(1, &[(0, 0), (1, 0), (2, 0)], velocity);
        pathfinder.reserve_path(2, &[(0, 1), (1, 1), (2, 1)], velocity);
        
        pathfinder.clear_robot_reservations(1);
        
        // Robot 1's reservations should be gone
        assert!(!pathfinder.reservations.values().any(|&id| id == 1));
        // Robot 2's should remain
        assert!(pathfinder.reservations.values().any(|&id| id == 2));
    }

    #[test]
    fn test_find_path_to_non_walkable() {
        let map_str = r#"
            . . . . .
            . x . . .
            . . . . .
        "#;
        let map = GridMap::parse(map_str).unwrap();
        let pathfinder = WHCAPathfinder::with_defaults();

        let result = pathfinder.find_path_to_non_walkable(&map, (0, 0), (1, 1));
        assert!(result.is_some());

        let result = result.unwrap();
        let last_pos = result.grid_path.last().unwrap();
        
        // Should end adjacent to the shelf, not on it
        assert!(*last_pos != (1, 1));
        assert!(is_adjacent(*last_pos, (1, 1)));
    }

    #[test]
    fn test_find_path_for_robot_strict_no_fallback_when_window_blocked() {
        let map_str = ". .";
        let map = GridMap::parse(map_str).unwrap();
        let mut pathfinder = WHCAPathfinder::new(MOVE_TIME_MS);

        // Robot 1 occupies both corridor cells for the full stationary window.
        pathfinder.reserve_stationary(1, (0, 0));
        pathfinder.reserve_stationary(1, (1, 0));

        // Strict WHCA behavior: no A* fallback, so blocked window returns None.
        let result = pathfinder.find_path_for_robot(&map, (1, 0), (0, 0), 2);
        assert!(result.is_none());
    }

    #[test]
    fn test_edge_swap_conflict_blocks_unsafe_path() {
        let map_str = ". .";
        let map = GridMap::parse(map_str).unwrap();
        let mut pathfinder = WHCAPathfinder::with_defaults();

        // Reserve robot 1 crossing from left to right.
        pathfinder.reserve_path(1, &[(0, 0), (1, 0)], [2.0, 0.0, 0.0]);

        // Robot 2 attempting the opposite direction should not get an unsafe swap path.
        let result = pathfinder.find_path_for_robot(&map, (1, 0), (0, 0), 2);
        // In a strict 2-cell corridor with conflicting reservation, returning None
        // is preferred over dispatching a potentially colliding swap path.
        assert!(result.is_none());
    }

    #[test]
    fn test_tradeoff_strict_vs_trait_head_on_corridor() {
        let map_str = ". .";
        let map = GridMap::parse(map_str).unwrap();

        let trials = 25;
        let mut strict_success = 0;
        let mut trait_success = 0;

        for _ in 0..trials {
            let mut pathfinder = WHCAPathfinder::with_defaults();
            // Robot 1 reserves a left->right crossing through the only corridor.
            pathfinder.reserve_path(1, &[(0, 0), (1, 0)], [2.0, 0.0, 0.0]);

            if pathfinder.find_path_for_robot(&map, (1, 0), (0, 0), 2).is_some() {
                strict_success += 1;
            }
            // Trait pathfinder lacks robot_id context but now remains strict (no fallback).
            if pathfinder.find_path(&map, (1, 0), (0, 0)).is_some() {
                trait_success += 1;
            }
        }

        println!(
            "WHCA strict benchmark: strict_success={}/{} trait_success={}/{}",
            strict_success, trials, trait_success, trials
        );

        assert_eq!(strict_success, 0);
        assert_eq!(trait_success, 0);
    }

    #[test]
    fn test_whca_stats_snapshot_and_reset() {
        let map_str = ". . . . .";
        let map = GridMap::parse(map_str).unwrap();
        let pathfinder = WHCAPathfinder::with_defaults();

        pathfinder.reset_stats();
        let _ = pathfinder.find_path(&map, (0, 0), (4, 0));
        let _ = pathfinder.find_path(&map, (0, 0), (0, 0));
        let _ = pathfinder.find_path(&map, (0, 0), (99, 0)); // invalid goal -> failed search

        let stats = pathfinder.stats_snapshot();
        assert!(stats.searches_total >= 3);
        assert!(stats.searches_succeeded >= 2);
        assert!(stats.searches_failed >= 1);
        assert!(stats.total_search_time_us >= stats.last_search_time_us);

        pathfinder.reset_stats();
        let reset = pathfinder.stats_snapshot();
        assert_eq!(reset.searches_total, 0);
        assert_eq!(reset.nodes_expanded_total, 0);
        assert_eq!(reset.total_search_time_us, 0);
    }
}
