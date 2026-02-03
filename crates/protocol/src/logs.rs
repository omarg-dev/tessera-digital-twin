//! Logs utility functions
//! 
//! Logs are saved in the workspace root's logs directory on a per-hour basis.
//! Structure: logs/YYYY-MM-DD_HH/{crate}.log + merged.log
//! 
//! Each crate writes to its own file to prevent interleaving.
//! On shutdown, merge_logs() combines all hourly logs chronologically,
//! with deduplication of repetitive firmware logs.

use chrono::Local;
use std::fs::{create_dir_all, OpenOptions, read_dir};
use std::io::{Write, BufRead, BufReader};
use std::path::PathBuf;
use std::sync::OnceLock;
use std::collections::HashMap;
use crate::config::LOG_DIR;

/// Cached workspace root path
static WORKSPACE_ROOT: OnceLock<PathBuf> = OnceLock::new();

/// Find the workspace root by looking for Cargo.toml with [workspace]
fn find_workspace_root() -> PathBuf {
    // Start from current directory and walk up
    let mut current = std::env::current_dir().unwrap_or_else(|_| PathBuf::from("."));
    
    for _ in 0..10 {
        let cargo_toml = current.join("Cargo.toml");
        if cargo_toml.exists() {
            if let Ok(contents) = std::fs::read_to_string(&cargo_toml) {
                if contents.contains("[workspace]") {
                    return current;
                }
            }
        }
        if !current.pop() {
            break;
        }
    }
    
    // Fallback to current directory
    std::env::current_dir().unwrap_or_else(|_| PathBuf::from("."))
}

/// Get the logs directory path (workspace_root/logs)
fn get_log_dir() -> PathBuf {
    let root = WORKSPACE_ROOT.get_or_init(find_workspace_root);
    root.join(LOG_DIR)
}

/// Get the current hourly subdirectory (logs/YYYY-MM-DD_HH/)
fn get_hourly_dir() -> PathBuf {
    let now = Local::now();
    get_log_dir().join(now.format("%Y-%m-%d_%H").to_string())
}

/// Formatted timestamp HH:MM:SS.mmm
pub fn timestamp() -> String {
    let now = Local::now();
    now.format("%H:%M:%S%.3f").to_string()
}

/// Save a log entry to a per-crate file
/// 
/// Creates hourly subdirectories: logs/YYYY-MM-DD_HH/{crate}.log
pub fn save_log(crate_name: &str, log: &str) -> String {
    let hourly_dir = get_hourly_dir();
    let crate_lower = crate_name.to_lowercase();
    let log_file_path = hourly_dir.join(format!("{}.log", crate_lower));
    let log_file_str = log_file_path.to_string_lossy().to_string();

    // Create hourly directory if it doesn't exist
    if !hourly_dir.exists() {
        if let Err(e) = create_dir_all(&hourly_dir) {
            eprintln!("⚠ Failed to create log directory '{}': {}", hourly_dir.display(), e);
            return log_file_str;
        }
    }

    // Append log to file (OS-level atomic append, no locking needed)
    match OpenOptions::new()
        .create(true)
        .append(true)
        .open(&log_file_path)
    {
        Ok(mut file) => {
            // Individual crate logs: just [time] message
            if let Err(e) = writeln!(file, "[{}] {}", timestamp(), log) {
                eprintln!("⚠ Failed to write to log file '{}': {}", log_file_str, e);
            }
            // File automatically closes and flushes on drop
        }
        Err(e) => {
            eprintln!("⚠ Failed to open log file '{}': {}", log_file_str, e);
        }
    }

    log_file_str
}

/// Merge all hourly log files into a single chronological file with deduplication
/// 
/// Call this at shutdown to create logs/YYYY-MM-DD_HH/merged.log
/// Deduplicates consecutive repeated firmware logs to reduce clutter.
pub fn merge_logs() {
    let hourly_dir = get_hourly_dir();
    
    if !hourly_dir.exists() {
        return; // No logs to merge
    }

    let merged_path = hourly_dir.join("merged.log");
    
    // Read all log entries from crate files
    let mut entries: Vec<(String, String, String)> = Vec::new(); // (timestamp_for_sort, formatted_line, crate_name)

    if let Ok(entries_iter) = read_dir(&hourly_dir) {
        for entry in entries_iter {
            if let Ok(entry) = entry {
                let path = entry.path();
                if path.is_file() && path.extension().map_or(false, |ext| ext == "log") {
                    if path.file_name().map_or(false, |name| name != "merged.log") {
                        // Get crate name from filename (e.g., "scheduler.log" -> "Scheduler")
                        let crate_name = path
                            .file_stem()
                            .and_then(|name| name.to_str())
                            .map(|s| {
                                let first_char = s.chars().next().unwrap_or('U').to_uppercase().to_string();
                                first_char + &s[1..]
                            })
                            .unwrap_or_else(|| "Unknown".to_string());
                        
                        if let Ok(file) = std::fs::File::open(&path) {
                            let reader = BufReader::new(file);
                            for line in reader.lines() {
                                if let Ok(line) = line {
                                    // Extract timestamp from format: [HH:MM:SS.mmm] message
                                    if let Some(bracket_end) = line.find(']') {
                                        let timestamp = line[1..bracket_end].to_string();
                                        // Reformat with crate name: [HH:MM:SS.mmm] Crate message
                                        let message = line[bracket_end + 2..].to_string(); // Skip "] "
                                        let formatted_line = format!("[{}] {} {}", timestamp, crate_name, message);
                                        entries.push((timestamp, formatted_line, crate_name.clone()));
                                    }
                                }
                            }
                        }
                    }
                }
            }
        }
    }

    // Sort by timestamp (HH:MM:SS.mmm format sorts lexicographically correctly)
    entries.sort_by(|a, b| a.0.cmp(&b.0));

    // Deduplicate consecutive repeated firmware logs (per-robot basis)
    let mut final_lines: Vec<String> = Vec::new();
    let mut last_command_per_robot: HashMap<u32, String> = HashMap::new();
    
    for (_, line, crate_name) in entries {
        // For firmware logs, check if it's a repeated command execution
        if crate_name == "Firmware" && line.contains("executed command:") {
            // Extract robot ID: find "Robot N " pattern
            let robot_id = if let Some(robot_idx) = line.find("Robot ") {
                line[robot_idx + 6..]
                    .split_whitespace()
                    .next()
                    .and_then(|s| s.parse::<u32>().ok())
            } else {
                None
            };
            
            if let Some(robot_id) = robot_id {
                // Extract the command part (everything from "executed command:")
                let key = if let Some(idx) = line.find("executed command:") {
                    line[idx..].to_string()
                } else {
                    line.clone()
                };
                
                // Check if this robot's last command was identical
                if let Some(last_cmd) = last_command_per_robot.get(&robot_id) {
                    if key == *last_cmd {
                        // Skip this duplicate firmware log for this robot
                        continue;
                    }
                }
                last_command_per_robot.insert(robot_id, key);
            }
        } else {
            // For non-firmware logs, clear the per-robot tracking
            // (we don't need to clear, just let it accumulate)
        }
        
        final_lines.push(line);
    }

    // Write merged file
    if let Ok(mut merged_file) = OpenOptions::new()
        .create(true)
        .write(true)
        .truncate(true)
        .open(&merged_path)
    {
        for line in final_lines {
            let _ = writeln!(merged_file, "{}", line);
        }
        println!("✓ Merged logs into {}", merged_path.display());
    }
}
