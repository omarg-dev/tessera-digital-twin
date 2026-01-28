//! Command-line interface for mission control

use crate::allocator::RobotInfo;
use crate::queue::TaskQueue;
use protocol::GridMap;
use std::collections::HashMap;
use tokio::io::{AsyncBufReadExt, BufReader};
use tokio::sync::mpsc;

/// Stdin command variants
pub enum StdinCmd {
    Status,
    AddTask {
        pickup: (usize, usize),
        dropoff: (usize, usize),
    },
    ListShelves,
    ListDropoffs,
    ListStations,
    Map,
    Help,
}

/// Spawn a background task to read stdin commands
pub fn spawn_stdin_reader(tx: mpsc::Sender<StdinCmd>) {
    tokio::spawn(async move {
        let stdin = tokio::io::stdin();
        let reader = BufReader::new(stdin);
        let mut lines = reader.lines();

        while let Ok(Some(line)) = lines.next_line().await {
            let parts: Vec<&str> = line.trim().split_whitespace().collect();
            if parts.is_empty() {
                continue;
            }

            let cmd = match parts[0] {
                "status" | "s" => Some(StdinCmd::Status),
                "list" if parts.len() >= 2 => match parts[1] {
                    "shelves" | "shelf" => Some(StdinCmd::ListShelves),
                    "dropoffs" | "dropoff" | "drop" => Some(StdinCmd::ListDropoffs),
                    "stations" | "station" => Some(StdinCmd::ListStations),
                    _ => {
                        println!("Usage: list <shelves|dropoffs|stations>");
                        None
                    }
                },
                "list" => {
                    println!("Usage: list <shelves|dropoffs|stations>");
                    None
                }
                "map" => Some(StdinCmd::Map),
                "add" if parts.len() == 3 => {
                    // Named location format: add S1 D1 or add S1 S2
                    let pickup_str = parts[1].to_uppercase();
                    let dropoff_str = parts[2].to_uppercase();
                    Some(StdinCmd::AddTask {
                        pickup: parse_named_location(&pickup_str).unwrap_or((0, 0)),
                        dropoff: parse_named_location(&dropoff_str).unwrap_or((0, 0)),
                    })
                }
                "add" if parts.len() == 5 => {
                    let px = parts[1].parse().ok();
                    let py = parts[2].parse().ok();
                    let dx = parts[3].parse().ok();
                    let dy = parts[4].parse().ok();
                    match (px, py, dx, dy) {
                        (Some(px), Some(py), Some(dx), Some(dy)) => {
                            Some(StdinCmd::AddTask {
                                pickup: (px, py),
                                dropoff: (dx, dy),
                            })
                        }
                        _ => {
                            println!("Usage: add <pickup_x> <pickup_y> <dropoff_x> <dropoff_y>");
                            None
                        }
                    }
                }
                "add" => {
                    println!("Usage: add <S#> <D#|S#>  or  add <px> <py> <dx> <dy>");
                    println!("  Examples: add S1 D1, add S2 S5, add 5 5 8 8");
                    None
                }
                "help" | "h" => {
                    print_help();
                    Some(StdinCmd::Help)
                }
                "pause" | "resume" | "reset" | "kill" => {
                    println!("System commands moved to control_plane crate.");
                    println!("Run: cargo run -p control_plane");
                    None
                }
                _ => {
                    println!("Unknown command: {}. Type 'help' for available commands.", parts[0]);
                    None
                }
            };

            if let Some(cmd) = cmd {
                tx.send(cmd).await.ok();
            }
        }
    });
}

fn print_help() {
    println!("\n╔════════════════════════════════════════════════════╗");
    println!("║            MISSION CONTROL COMMANDS                ║");
    println!("╠════════════════════════════════════════════════════╣");
    println!("║ TASKS:                                             ║");
    println!("║   add S1 D1             - Add task (shelf→dropoff) ║");
    println!("║   add S1 S2             - Add task (shelf→shelf)   ║");
    println!("║   add <px> <py> <dx> <dy> - Add task by coords     ║");
    println!("║   status, s             - Show full status         ║");
    println!("╠════════════════════════════════════════════════════╣");
    println!("║ LOCATIONS:                                         ║");
    println!("║   list shelves          - List all shelves (S#)    ║");
    println!("║   list dropoffs         - List all dropoffs (D#)   ║");
    println!("║   list stations         - List charging stations   ║");
    println!("║   map                   - Show warehouse map       ║");
    println!("╠════════════════════════════════════════════════════╣");
    println!("║ SYSTEM (run control_plane for pause/resume/reset): ║");
    println!("║   help, h               - Show this help           ║");
    println!("╚════════════════════════════════════════════════════╝\n");
}

/// Print current status (enhanced with map info)
pub fn print_status(
    queue: &dyn TaskQueue,
    robots: &HashMap<u32, RobotInfo>,
    map: &GridMap,
    paused: bool,
    verbose: bool,
) {
    let shelves = map.get_shelves();
    let dropoffs = map.get_dropoffs();
    let stations = map.get_stations();
    let idle_robots = robots.values().filter(|r| r.assigned_task.is_none()).count();

    println!("\n╔════════════════════════════════════════════════════╗");
    println!("║            MISSION CONTROL STATUS                  ║");
    println!("╠════════════════════════════════════════════════════╣");
    println!("║ State: {:<8}  Verbose: {:<3}                      ║",
        if paused { "PAUSED" } else { "RUNNING" },
        if verbose { "ON" } else { "OFF" });
    println!("║ Queue: {} pending / {} total                         ║", queue.pending_count(), queue.total_count());
    println!("║ Robots: {} online ({} idle, {} busy)                  ║",
        robots.len(), idle_robots, robots.len() - idle_robots);
    println!("║ Map: {}x{} | {} shelves | {} dropoffs | {} stations   ║",
        map.width, map.height, shelves.len(), dropoffs.len(), stations.len());
    println!("╠════════════════════════════════════════════════════╣");

    if verbose {
        // Show tasks
        if !queue.all_tasks().is_empty() {
            println!("║ TASKS:");
            for task in queue.all_tasks() {
                let (pickup, dropoff) = match &task.task_type {
                    protocol::TaskType::PickAndDeliver { pickup, dropoff, .. } => {
                        (format!("({},{})", pickup.0, pickup.1), format!("({},{})", dropoff.0, dropoff.1))
                    }
                    _ => ("?".to_string(), "?".to_string()),
                };
                println!("║   #{}: {} → {} | {:?}", task.id, pickup, dropoff, task.status);
            }
            println!("╠════════════════════════════════════════════════════╣");
        }

        // Show robots
        if !robots.is_empty() {
            println!("║ ROBOTS:");
            let mut robot_list: Vec<_> = robots.values().collect();
            robot_list.sort_by_key(|r| r.id);
            for robot in robot_list {
                let assigned = robot.assigned_task
                    .map(|t| format!("Task#{}", t))
                    .unwrap_or_else(|| "idle".to_string());
                println!("║   R{}: {:?} @ ({:.0},{:.0}) bat:{:.0}% [{}]",
                    robot.id, robot.state, robot.position[0], robot.position[2],
                    robot.battery, assigned);
            }
        }
    } else {
        println!("║ (Use 'verbose on' in control_plane for details)    ║");
    }

    println!("╚════════════════════════════════════════════════════╝\n");
}

/// List shelves with named IDs
pub fn print_shelves(map: &GridMap) {
    let shelves = map.get_shelves();
    println!("\n┌─ SHELVES ({} total) ─────────────────────────────┐", shelves.len());
    for (i, shelf) in shelves.iter().enumerate() {
        let capacity = match shelf.tile_type {
            protocol::grid_map::TileType::Shelf(c) => c,
            _ => 0,
        };
        print!("│ S{:<3} ({:2},{:2}) cap:{} ", i + 1, shelf.x, shelf.y, capacity);
        if (i + 1) % 3 == 0 || i == shelves.len() - 1 {
            println!("│");
        }
    }
    println!("└──────────────────────────────────────────────────┘");
    println!("  Use: add S1 D1  or  add S1 S2\n");
}

/// List dropoffs with named IDs
pub fn print_dropoffs(map: &GridMap) {
    let dropoffs = map.get_dropoffs();
    println!("\n┌─ DROPOFFS ({} total) ───────────────────────────┐", dropoffs.len());
    for (i, dropoff) in dropoffs.iter().enumerate() {
        println!("│ D{:<3} ({:2},{:2})                                    │", i + 1, dropoff.x, dropoff.y);
    }
    println!("└──────────────────────────────────────────────────┘");
    println!("  Use: add S# D#  to deliver from shelf to dropoff\n");
}

/// List charging stations
pub fn print_stations(map: &GridMap) {
    let stations = map.get_stations();
    println!("\n┌─ CHARGING STATIONS ({} total) ────────────────────┐", stations.len());
    for (i, station) in stations.iter().enumerate() {
        println!("│ C{:<3} ({:2},{:2})                                    │", i + 1, station.x, station.y);
    }
    println!("└──────────────────────────────────────────────────┘\n");
}

/// Print ASCII map of the warehouse
pub fn print_map(map: &GridMap, robots: &HashMap<u32, RobotInfo>) {
    println!("\n┌─ WAREHOUSE MAP ({}x{}) ─────────────────────────────┐", map.width, map.height);
    
    // Build a grid
    let mut grid = vec![vec![' '; map.width]; map.height];
    
    // Fill with tiles
    for tile in &map.tiles {
        let ch = match tile.tile_type {
            protocol::grid_map::TileType::Empty => '~',
            protocol::grid_map::TileType::Ground => '.',
            protocol::grid_map::TileType::Wall => '#',
            protocol::grid_map::TileType::Shelf(_) => 'S',
            protocol::grid_map::TileType::Station => '_',
            protocol::grid_map::TileType::Dropoff => 'D',
        };
        if tile.y < map.height && tile.x < map.width {
            grid[tile.y][tile.x] = ch;
        }
    }
    
    // Overlay robots
    for robot in robots.values() {
        let x = robot.position[0] as usize;
        let y = robot.position[2] as usize;
        if y < map.height && x < map.width {
            grid[y][x] = 'R';
        }
    }
    
    // Print header row with column numbers
    print!("│   ");
    for x in 0..map.width.min(20) {
        print!("{}", x % 10);
    }
    if map.width > 20 { print!("..."); }
    println!();
    
    // Print grid
    for (y, row) in grid.iter().enumerate().take(25) {
        print!("│ {:2} ", y);
        for ch in row.iter().take(20) {
            print!("{}", ch);
        }
        if map.width > 20 { print!("..."); }
        println!();
    }
    if map.height > 25 { println!("│ ... ({} more rows)", map.height - 25); }
    
    println!("└──────────────────────────────────────────────────┘");
    println!("  Legend: # Wall, S Shelf, D Dropoff, _ Station, R Robot, . Ground");
    println!();
}

/// Parse named location like S1, D2, etc.
/// Returns (0,0) for invalid - caller should validate before using
pub fn parse_named_location(name: &str) -> Option<(usize, usize)> {
    // This is a placeholder - actual lookup happens in server.rs with map access
    // We store the name prefix and index, then resolve in handle_stdin
    if name.len() < 2 {
        return None;
    }
    let prefix = &name[0..1];
    let idx: usize = name[1..].parse().ok()?;
    // Return a marker value with prefix encoded
    // S = 10000+idx, D = 20000+idx (will be resolved in server)
    match prefix {
        "S" => Some((10000 + idx, 0)),
        "D" => Some((20000 + idx, 0)),
        _ => None,
    }
}
