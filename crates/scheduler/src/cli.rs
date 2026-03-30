//! Command-line interface for scheduler

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
    RandomTask,
    MassAdd {
        count: u32,
        dropoff_probability: Option<f32>,
    },
    CancelTask { task_id: u64 },
    SetPriority { task_id: u64, priority: protocol::Priority },
    History,
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
                "random" | "rand" => Some(StdinCmd::RandomTask),
                "mass_add" | "massadd" | "mass" if parts.len() == 2 || parts.len() == 3 => {
                    let count = parts[1].parse::<u32>().ok().filter(|count| *count > 0);
                    let dropoff_probability = if parts.len() == 3 {
                        parse_dropoff_percentage(parts[2]).map(Some)
                    } else {
                        Some(None)
                    };

                    match (count, dropoff_probability) {
                        (Some(count), Some(dropoff_probability)) => {
                            Some(StdinCmd::MassAdd {
                                count,
                                dropoff_probability,
                            })
                        }
                        _ => {
                            println!("Usage: mass_add <count> [dropoff_%]");
                            println!("  Example: mass_add 250 60");
                            None
                        }
                    }
                }
                "mass_add" | "massadd" | "mass" => {
                    println!("Usage: mass_add <count> [dropoff_%]");
                    println!("  Example: mass_add 250 60");
                    None
                }
                "cancel" if parts.len() == 2 => {
                    if let Ok(task_id) = parts[1].parse::<u64>() {
                        Some(StdinCmd::CancelTask { task_id })
                    } else {
                        println!("Usage: cancel <task_id>");
                        None
                    }
                }
                "priority" if parts.len() == 3 => {
                    let task_id = parts[1].parse::<u64>().ok();
                    let priority = match parts[2].to_lowercase().as_str() {
                        "low" | "l" => Some(protocol::Priority::Low),
                        "normal" | "n" => Some(protocol::Priority::Normal),
                        "high" | "h" => Some(protocol::Priority::High),
                        "critical" | "c" => Some(protocol::Priority::Critical),
                        _ => None,
                    };
                    match (task_id, priority) {
                        (Some(id), Some(p)) => Some(StdinCmd::SetPriority { task_id: id, priority: p }),
                        _ => {
                            println!("Usage: priority <task_id> <low|normal|high|critical>");
                            None
                        }
                    }
                }
                "history" | "hist" => Some(StdinCmd::History),
                "help" | "h" => Some(StdinCmd::Help),
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

// Print help command list
pub fn print_help() {
    println!("\n╔════════════════════════════════════════════════════╗");
    println!("║            SCHEDULER COMMANDS                      ║");
    println!("╠════════════════════════════════════════════════════╣");
    println!("║ TASKS:                                             ║");
    println!("║   add S1 D1             - Add task (shelf→dropoff) ║");
    println!("║   add S1 S2             - Add task (shelf→shelf)   ║");
    println!("║   add <px> <py> <dx> <dy> - Add task by coords     ║");
    println!("║   random, rand          - Add random shelf→dropoff ║");
    println!("║   mass_add <n> [pct]    - Add n random tasks       ║");
    println!("║                           (pct = dropoff %, def 60)║");
    println!("║   cancel <id>           - Cancel pending task      ║");
    println!("║   priority <id> <level> - Set task priority        ║");
    println!("║                          (low/normal/high/critical)║");
    println!("║   status, s             - Show full status         ║");
    println!("║   history               - Show completed tasks     ║");
    println!("╠════════════════════════════════════════════════════╣");
    println!("║ LOCATIONS:                                         ║");
    println!("║   list shelves          - List all shelves (S#)    ║");
    println!("║   list dropoffs         - List all dropoffs (D#)   ║");
    println!("║   list stations         - List charging stations   ║");
    println!("║   map                   - Show warehouse map       ║");
    println!("╠════════════════════════════════════════════════════╣");
    println!("║ SYSTEM (run orchestrator for pause/resume/reset):  ║");
    println!("║   help, h               - Show this help           ║");
    println!("╚════════════════════════════════════════════════════╝\n");
}

/// Helper to format a status line with dynamic alignment
fn format_status_line(content: &str, box_width: usize) -> String {
    let available = box_width.saturating_sub(4); // "║ " + " ║"
    if content.len() <= available {
        let padding = available - content.len();
        format!("║ {}{}║", content, " ".repeat(padding))
    } else {
        // Truncate if too long
        format!("║ {}...║", &content[..available.saturating_sub(3)])
    }
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
    let box_width = 52; // Total width of box

    println!("\n╔════════════════════════════════════════════════════╗");
    println!("║            SCHEDULER STATUS                        ║");
    println!("╠════════════════════════════════════════════════════╣");
    let state_line = format!("State: {}  Verbose: {}",
        if paused { "PAUSED" } else { "RUNNING" },
        if verbose { "ON" } else { "OFF" });
    println!("{}", format_status_line(&state_line, box_width));
    
    // Queue status - now handles any size numbers
    let queue_line = format!("Queue: {} pending / {} total",
        queue.pending_count(),
        queue.total_count());
    println!("{}", format_status_line(&queue_line, box_width));
    
    // Robot status
    let robot_line = format!("Robots: {} online ({} idle, {} busy)",
        robots.len(),
        idle_robots,
        robots.len() - idle_robots);
    println!("{}", format_status_line(&robot_line, box_width));
    
    // Map status
    let map_line = format!("Map: {}x{} | {} shelves | {} dropoffs | {} stations",
        map.width,
        map.height,
        shelves.len(),
        dropoffs.len(),
        stations.len());
    println!("{}", format_status_line(&map_line, box_width));
    
    println!("╠════════════════════════════════════════════════════╣");

    if verbose {
        // Show tasks
        if !queue.all_tasks().is_empty() {
            println!("║ TASKS:                                             ║");
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
            println!("║ ROBOTS:                                            ║");
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
        println!("║ (Use 'verbose on' in orchestrator for details)     ║");
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

/// Print completed/failed/cancelled task history
pub fn print_history(queue: &dyn TaskQueue) {
    let tasks: Vec<_> = queue.all_tasks().into_iter()
        .filter(|t| matches!(t.status, 
            protocol::TaskStatus::Completed | 
            protocol::TaskStatus::Failed { .. } | 
            protocol::TaskStatus::Cancelled))
        .collect();
    
    println!("\n┌─ TASK HISTORY ({} entries) ─────────────────────────┐", tasks.len());
    if tasks.is_empty() {
        println!("│ No completed/failed/cancelled tasks yet.            │");
    } else {
        for task in tasks {
            let status_str = match &task.status {
                protocol::TaskStatus::Completed => "✓ DONE".to_string(),
                protocol::TaskStatus::Failed { reason } => format!("✗ FAIL: {}", reason),
                protocol::TaskStatus::Cancelled => "⊘ CANCEL".to_string(),
                _ => "?".to_string(),
            };
            let (pickup, dropoff) = match &task.task_type {
                protocol::TaskType::PickAndDeliver { pickup, dropoff, .. } => {
                    (format!("({},{})", pickup.0, pickup.1), format!("({},{})", dropoff.0, dropoff.1))
                }
                _ => ("?".to_string(), "?".to_string()),
            };
            println!("│ #{:<4} {} → {} | {}", task.id, pickup, dropoff, status_str);
        }
    }
    println!("└──────────────────────────────────────────────────────┘\n");
}

/// Print ASCII map of the warehouse
pub fn print_map(map: &GridMap, robots: &HashMap<u32, RobotInfo>) {
    let row_label_width = map.height.saturating_sub(1).to_string().len();
    
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
    
    // Build header line and borders
    let col_header: String = (0..map.width)
        .map(|x| char::from(b'0' + (x % 10) as u8))
        .collect();
    let header_line = format!(" {:>width$} {}", "", col_header, width = row_label_width);
    let border = "─".repeat(header_line.len());
    let title = format!(" WAREHOUSE MAP ({}x{})", map.width, map.height);
    let title_line = format!("{}{}", title, " ".repeat(border.len().saturating_sub(title.len())));

    println!("\n┌{}┐", border);
    println!("│{}│", title_line);
    println!("│{}│", header_line);
    
    // Print grid
    for (y, row) in grid.iter().enumerate() {
        let row_str: String = row.iter().collect();
        let line = format!(" {:>width$} {}", y, row_str, width = row_label_width);
        println!("│{}│", line);
    }

    println!("└{}┘", border);
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
    // S = SHELF_MARKER_BASE+idx, D = DROPOFF_MARKER_BASE+idx (will be resolved in server)
    match prefix {
        "S" => Some((protocol::config::scheduler::SHELF_MARKER_BASE + idx, 0)),
        "D" => Some((protocol::config::scheduler::DROPOFF_MARKER_BASE + idx, 0)),
        _ => None,
    }
}

fn parse_dropoff_percentage(input: &str) -> Option<f32> {
    let pct = input.parse::<f32>().ok()?;
    if !pct.is_finite() || !(0.0..=100.0).contains(&pct) {
        return None;
    }
    Some((pct / 100.0).clamp(0.0, 1.0))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_shelf_location() {
        assert_eq!(parse_named_location("S1"), Some((10001, 0)));
        assert_eq!(parse_named_location("S10"), Some((10010, 0)));
        assert_eq!(parse_named_location("S99"), Some((10099, 0)));
    }

    #[test]
    fn test_parse_dropoff_location() {
        assert_eq!(parse_named_location("D1"), Some((20001, 0)));
        assert_eq!(parse_named_location("D5"), Some((20005, 0)));
    }

    #[test]
    fn test_parse_invalid_location() {
        assert_eq!(parse_named_location("X1"), None);  // Invalid prefix
        assert_eq!(parse_named_location("S"), None);   // Too short
        assert_eq!(parse_named_location(""), None);    // Empty
        assert_eq!(parse_named_location("Sabc"), None); // Non-numeric suffix
    }

    #[test]
    fn test_marker_values_distinguishable() {
        // Ensure shelf and dropoff markers don't overlap
        let s1 = parse_named_location("S1").expect("S1 should parse as a shelf marker");
        let d1 = parse_named_location("D1").expect("D1 should parse as a dropoff marker");
        assert!(s1.0 < protocol::config::scheduler::DROPOFF_MARKER_BASE);  // Shelf range
        assert!(d1.0 >= protocol::config::scheduler::DROPOFF_MARKER_BASE); // Dropoff range
    }

    #[test]
    fn test_parse_dropoff_percentage_valid() {
        assert_eq!(parse_dropoff_percentage("0"), Some(0.0));
        assert_eq!(parse_dropoff_percentage("60"), Some(0.6));
        assert_eq!(parse_dropoff_percentage("100"), Some(1.0));
    }

    #[test]
    fn test_parse_dropoff_percentage_invalid() {
        assert_eq!(parse_dropoff_percentage("-1"), None);
        assert_eq!(parse_dropoff_percentage("101"), None);
        assert_eq!(parse_dropoff_percentage("abc"), None);
    }
}
