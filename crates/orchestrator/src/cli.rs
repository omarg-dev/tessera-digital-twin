//! CLI command parsing and help display

use crate::processes::CRATE_ORDER;

/// Parsed command from user input
#[derive(Debug, Clone, PartialEq)]
pub enum Command {
    // Process management
    StartAll,
    Start(String),
    KillAll,
    Kill(String),
    Restart,
    Status,
    
    // Runtime commands (broadcast via Zenoh)
    Pause,
    Resume,
    Verbose(bool),
    
    // Meta
    Help,
    Quit,
    
    // Invalid
    Unknown(String),
    Empty,
}

impl Command {
    /// Parse a command from input string
    pub fn parse(input: &str) -> Self {
        let input = input.trim().to_ascii_lowercase();
        let parts: Vec<&str> = input.split_whitespace().collect();
        
        match parts.as_slice() {
            // Process management
            ["start"] | ["start", "all"] | ["up"] | ["up", "all"] => Command::StartAll,
            ["start", name] | ["up", name] => Command::Start(name.to_string()),
            
            ["kill"] | ["kill", "all"] | ["down"] | ["down", "all"] => Command::KillAll,
            ["kill", name] | ["down", name] => Command::Kill(name.to_string()),
            
            ["restart"] | ["reset"] => Command::Restart,
            ["status"] | ["ps"] => Command::Status,
            
            // Runtime commands
            ["pause"] | ["p"] => Command::Pause,
            ["resume"] | ["r"] => Command::Resume,
            ["verbose", "on"] | ["v", "on"] => Command::Verbose(true),
            ["verbose", "off"] | ["v", "off"] => Command::Verbose(false),
            
            // Meta
            ["help"] | ["h"] | ["?"] => Command::Help,
            ["quit"] | ["exit"] | ["q"] => Command::Quit,
            
            [] => Command::Empty,
            _ => Command::Unknown(input),
        }
    }
}

/// Print the help message
pub fn print_help() {
    println!("╭─────────────────────────────────────────────────╮");
    println!("│  PROCESS MANAGEMENT                             │");
    println!("├─────────────────────────────────────────────────┤");
    println!("│  start, up      - Start all crates              │");
    println!("│  start <crate>  - Start specific crate          │");
    println!("│  kill, down     - Kill all crates               │");
    println!("│  kill <crate>   - Kill specific crate           │");
    println!("│  restart        - Kill + start all              │");
    println!("│  status, ps     - Show process status           │");
    println!("├─────────────────────────────────────────────────┤");
    println!("│  RUNTIME COMMANDS (broadcast via Zenoh)         │");
    println!("├─────────────────────────────────────────────────┤");
    println!("│  pause, p       - Pause simulation              │");
    println!("│  resume, r      - Resume simulation             │");
    println!("│  verbose on/off - Toggle verbose output         │");
    println!("├─────────────────────────────────────────────────┤");
    println!("│  quit, q        - Kill all and exit             │");
    println!("│  help, h        - Show this help                │");
    println!("╰─────────────────────────────────────────────────╯");
    println!();
    println!("Available crates: {:?}", CRATE_ORDER);
}

/// Print process status table
pub fn print_status(running: &[String]) {
    use crate::processes::is_process_running;
    
    println!("╭─────────────────────────────────────────╮");
    println!("│  PROCESS STATUS                         │");
    println!("├─────────────────────────────────────────┤");

    for name in CRATE_ORDER {
        let status = if is_process_running(name) {
            "🟢 running"
        } else if running.contains(&name.to_string()) {
            "🔴 exited"
        } else {
            "⚫ not started"
        };
        println!("│  {:17} {:18} │", format!("{}:", name), status);
    }
    println!("╰─────────────────────────────────────────╯");
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_start_all() {
        assert_eq!(Command::parse("start"), Command::StartAll);
        assert_eq!(Command::parse("start all"), Command::StartAll);
        assert_eq!(Command::parse("up"), Command::StartAll);
        assert_eq!(Command::parse("UP ALL"), Command::StartAll);
    }

    #[test]
    fn test_parse_start_specific() {
        assert_eq!(Command::parse("start mock_firmware"), Command::Start("mock_firmware".to_string()));
        assert_eq!(Command::parse("up visualizer"), Command::Start("visualizer".to_string()));
    }

    #[test]
    fn test_parse_kill() {
        assert_eq!(Command::parse("kill"), Command::KillAll);
        assert_eq!(Command::parse("kill all"), Command::KillAll);
        assert_eq!(Command::parse("down"), Command::KillAll);
        assert_eq!(Command::parse("kill coordinator"), Command::Kill("coordinator".to_string()));
    }

    #[test]
    fn test_parse_runtime_commands() {
        assert_eq!(Command::parse("pause"), Command::Pause);
        assert_eq!(Command::parse("p"), Command::Pause);
        assert_eq!(Command::parse("resume"), Command::Resume);
        assert_eq!(Command::parse("r"), Command::Resume);
        assert_eq!(Command::parse("verbose on"), Command::Verbose(true));
        assert_eq!(Command::parse("verbose off"), Command::Verbose(false));
    }

    #[test]
    fn test_parse_meta() {
        assert_eq!(Command::parse("help"), Command::Help);
        assert_eq!(Command::parse("quit"), Command::Quit);
        assert_eq!(Command::parse(""), Command::Empty);
    }

    #[test]
    fn test_parse_unknown() {
        match Command::parse("foobar") {
            Command::Unknown(s) => assert_eq!(s, "foobar"),
            _ => panic!("Expected Unknown"),
        }
    }
}
