//! Logs utility functions
//! 
//! Logs are saved in the workspace root's logs directory on a per-session basis.
//! Structure: logs/SESSION_START/RUN_START/{crate}.log + merged.log
//! 
//! Each crate writes to its own file to prevent interleaving.
//! On shutdown, merge_logs() combines all session logs chronologically,
//! with deduplication of repetitive firmware logs.

use chrono::Local;
use std::fs::{create_dir_all, OpenOptions, read_dir};
use std::io::{Write, BufRead, BufReader};
use std::path::PathBuf;
use std::sync::OnceLock;
use std::collections::HashMap;
use crate::config::{LOG_DIR, LOG_MERGE_EXCLUDE};

/// Cached workspace root path
static WORKSPACE_ROOT: OnceLock<PathBuf> = OnceLock::new();
static ORCH_SESSION_DIR: OnceLock<PathBuf> = OnceLock::new();
const ORCH_SESSION_FILE: &str = "orchestrator_session.txt";
const RUN_SESSION_FILE: &str = "current_run.txt";

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

/// Initialize or load the orchestrator session directory (logs/SESSION_START/)
///
/// The orchestrator should call `start_orchestrator_session()` once at startup.
fn get_orchestrator_session_dir() -> PathBuf {
    ORCH_SESSION_DIR
        .get_or_init(|| {
            let log_dir = get_log_dir();
            let _ = create_dir_all(&log_dir);
            let session_file = log_dir.join(ORCH_SESSION_FILE);

            if let Ok(existing) = std::fs::read_to_string(&session_file) {
                let stamp = existing.trim();
                if !stamp.is_empty() {
                    let existing_dir = log_dir.join(stamp);
                    if existing_dir.exists() {
                        return existing_dir;
                    }
                }
            }

            let now = Local::now().format("%Y-%m-%d_%H-%M").to_string();
            let new_dir = log_dir.join(&now);
            let _ = create_dir_all(&new_dir);

            let _ = std::fs::write(&session_file, format!("{}\n", now));
            new_dir
        })
        .clone()
}

/// Initialize or load the current run directory (logs/SESSION_START/RUN_START/)
///
/// The orchestrator should call `start_run_session()` when `run/up` is executed.
fn get_run_session_dir() -> PathBuf {
    let orch_dir = get_orchestrator_session_dir();
    let session_file = orch_dir.join(RUN_SESSION_FILE);

    if let Ok(existing) = std::fs::read_to_string(&session_file) {
        let stamp = existing.trim();
        if !stamp.is_empty() {
            let existing_dir = orch_dir.join(stamp);
            if existing_dir.exists() {
                return existing_dir;
            }
        }
    }

    // If no run session is active, fall back to orchestrator dir
    orch_dir
}

/// Formatted timestamp HH:MM:SS.mmm
pub fn timestamp() -> String {
    let now = Local::now();
    now.format("%H:%M:%S%.3f").to_string()
}

/// Save a log entry to a per-crate file
/// 
/// Creates run subdirectories: logs/SESSION_START/RUN_START/{crate}.log
pub fn save_log(crate_name: &str, log: &str) -> String {
    let session_dir = get_run_session_dir();
    let crate_lower = crate_name.to_lowercase();
    let log_file_path = session_dir.join(format!("{}.log", crate_lower));
    let log_file_str = log_file_path.to_string_lossy().to_string();

    // Create session directory if it doesn't exist
    if !session_dir.exists() {
        if let Err(e) = create_dir_all(&session_dir) {
            eprintln!("⚠ Failed to create log directory '{}': {}", session_dir.display(), e);
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

/// Merge all run log files into a single chronological file with deduplication
/// 
/// Call this at shutdown to create logs/SESSION_START/RUN_START/merged.log
/// Deduplicates consecutive repeated firmware logs to reduce clutter.
pub fn merge_logs() {
    let session_dir = get_run_session_dir();
    
    if !session_dir.exists() {
        return; // No logs to merge
    }

    let merged_path = session_dir.join("merged.log");
    
    // Read all log entries from crate files
    let mut entries: Vec<(String, String, String)> = Vec::new(); // (timestamp_for_sort, formatted_line, crate_name)

    if let Ok(entries_iter) = read_dir(&session_dir) {
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

                        let crate_name_lower = crate_name.to_lowercase();
                        if LOG_MERGE_EXCLUDE.iter().any(|name| *name == crate_name_lower) {
                            continue;
                        }
                        
                        if let Ok(file) = std::fs::File::open(&path) {
                            let reader = BufReader::new(file);
                            for line in reader.lines() {
                                if let Ok(line) = line {
                                    // Expected format: [HH:MM:SS.mmm] message
                                    // Guard: line must start with '[' and contain a closing ']'
                                    // to avoid panicking on embedded ']' in message content or
                                    // continuation lines from multi-line log entries.
                                    if !line.starts_with('[') {
                                        continue;
                                    }
                                    if let Some(bracket_end) = line[1..].find(']').map(|i| i + 1) {
                                        let timestamp = line[1..bracket_end].to_string();
                                        // skip "] " (2 chars) to get the message body
                                        let msg_start = (bracket_end + 2).min(line.len());
                                        let message = line[msg_start..].to_string();
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

    // Clear the run marker so the next run starts a new run directory
    let orch_dir = get_orchestrator_session_dir();
    let run_file = orch_dir.join(RUN_SESSION_FILE);
    if let Ok(existing) = std::fs::read_to_string(&run_file) {
        let stamp = existing.trim();
        if !stamp.is_empty() && orch_dir.join(stamp) == session_dir {
            let _ = std::fs::remove_file(&run_file);
        }
    }
}

/// Start a new orchestrator session (logs/SESSION_START/)
///
/// Call this once when orchestrator starts.
pub fn start_orchestrator_session() -> PathBuf {
    get_orchestrator_session_dir()
}

/// Start a new run session (logs/SESSION_START/RUN_START/)
///
/// Call this when `run/up` is executed.
pub fn start_run_session() -> PathBuf {
    let orch_dir = get_orchestrator_session_dir();
    let run_stamp = Local::now().format("%H-%M").to_string();
    let run_dir = orch_dir.join(&run_stamp);
    let _ = create_dir_all(&run_dir);
    let _ = std::fs::write(orch_dir.join(RUN_SESSION_FILE), format!("{}\n", run_stamp));
    run_dir
}
