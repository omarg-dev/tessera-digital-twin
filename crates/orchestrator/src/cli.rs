//! CLI command parsing and help display

use crate::processes::CRATE_ORDER;

/// Parsed command from user input
#[derive(Debug, Clone, PartialEq)]
pub enum Command {
    // Process management
    RunAll,
    Run(String),
    KillAll,
    Kill(String),
    Restart,
    Status,

    // Output visibility (takes effect on next spawn)
    ShowOutput(String, bool),

    // Runtime commands (broadcast via Zenoh)
    Pause,
    Resume,
    Verbose(bool),
    Chaos(bool),

    // Robot control (individual robots)
    RobotEnable(u32),
    RobotDisable(u32),
    RobotRestart(u32),

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
            ["run"] | ["run", "all"] | ["up"] | ["up", "all"] => Command::RunAll,
            ["run", name] | ["up", name] => Command::Run(name.to_string()),
            
            ["kill"] | ["kill", "all"] | ["down"] | ["down", "all"] => Command::KillAll,
            ["kill", name] | ["down", name] => Command::Kill(name.to_string()),
            
            ["restart"] | ["reset"] => Command::Restart,
            ["status"] | ["ps"] => Command::Status,

            // output visibility (takes effect on next spawn)
            ["show", "all"] => Command::ShowOutput("all".to_string(), true),
            ["hide", "all"] => Command::ShowOutput("all".to_string(), false),
            ["show", name] => Command::ShowOutput(name.to_string(), true),
            ["hide", name] => Command::ShowOutput(name.to_string(), false),
            
            // Runtime commands
            ["pause"] | ["p"] => Command::Pause,
            ["resume"] | ["r"] => Command::Resume,
            ["verbose", "on"] | ["v", "on"] => Command::Verbose(true),
            ["verbose", "off"] | ["v", "off"] => Command::Verbose(false),
            ["chaos", "on"] => Command::Chaos(true),
            ["chaos", "off"] => Command::Chaos(false),
            
            // Robot control (use enable/disable to avoid conflict with up/down crate)
            ["enable", "robot", id] | ["robot", "enable", id] => {
                id.parse().map(Command::RobotEnable).unwrap_or(Command::Unknown(input))
            }
            ["disable", "robot", id] | ["robot", "disable", id] => {
                id.parse().map(Command::RobotDisable).unwrap_or(Command::Unknown(input))
            }
            ["restart", "robot", id] | ["robot", "restart", id] => {
                id.parse().map(Command::RobotRestart).unwrap_or(Command::Unknown(input))
            }
            
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
    println!("│  run, up        - Run all crates                │");
    println!("│  run <crate>    - Run specific crate            │");
    println!("│  kill, down     - Kill all crates               │");
    println!("│  kill <crate>   - Kill specific crate           │");
    println!("│  restart        - Kill + run all                │");
    println!("│  status, ps     - Show process status           │");
    println!("├─────────────────────────────────────────────────┤");
    println!("│  OUTPUT VISIBILITY (takes effect on next spawn) │");
    println!("├─────────────────────────────────────────────────┤");
    println!("│  show <crate>   - Open crate in a window        │");
    println!("│  hide <crate>   - Run crate silently            │");
    println!("│  show all       - Window all crates             │");
    println!("│  hide all       - Silence all crates (default)  │");
    println!("├─────────────────────────────────────────────────┤");
    println!("│  RUNTIME COMMANDS (broadcast via Zenoh)         │");
    println!("├─────────────────────────────────────────────────┤");
    println!("│  pause, p       - Pause simulation              │");
    println!("│  resume, r      - Resume simulation             │");
    println!("│  verbose on/off - Toggle verbose output         │");
    println!("│  chaos on/off   - Toggle chaos engineering      │");
    println!("├─────────────────────────────────────────────────┤");
    println!("│  ROBOT CONTROL                                  │");
    println!("├─────────────────────────────────────────────────┤");
    println!("│  enable robot <id>      - Enable robot          │");
    println!("│  disable robot <id>    - Disable robot          │");
    println!("│  restart robot <id> - Reset robot to station    │");
    println!("├─────────────────────────────────────────────────┤");
    println!("│  quit, q        - Kill all and exit             │");
    println!("│  help, h        - Show this help                │");
    println!("╰─────────────────────────────────────────────────╯");
    println!();
    println!("Available crates: {:?}", CRATE_ORDER);
}

/// Print process status table
pub fn print_status(running: &[String], show_output: &std::collections::HashSet<String>) {
    use crate::processes::is_process_running;

    println!("╭───────────────────────────────────────────────────────╮");
    println!("│  PROCESS STATUS                                       │");
    println!("├───────────────────────────────────────────────────────┤");

    for name in CRATE_ORDER {
        let status = if is_process_running(name) {
            "🟢 running"
        } else if running.contains(&name.to_string()) {
            "🔴 exited"
        } else {
            "⚫ not started"
        };
        let output = if show_output.contains(*name) { "[window]" } else { "[silent]" };
        println!("│  {:17} {:18}  {:8}  │", format!("{}:", name), status, output);
    }
    println!("╰───────────────────────────────────────────────────────╯");
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_run_all() {
        assert_eq!(Command::parse("run"), Command::RunAll);
        assert_eq!(Command::parse("run all"), Command::RunAll);
        assert_eq!(Command::parse("up"), Command::RunAll);
        assert_eq!(Command::parse("UP ALL"), Command::RunAll);
    }

    #[test]
    fn test_parse_run_specific() {
        assert_eq!(Command::parse("run mock_firmware"), Command::Run("mock_firmware".to_string()));
        assert_eq!(Command::parse("run visualizer"), Command::Run("visualizer".to_string()));
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
        assert_eq!(Command::parse("chaos on"), Command::Chaos(true));
        assert_eq!(Command::parse("chaos off"), Command::Chaos(false));
    }

    #[test]
    fn test_parse_robot_control() {
        assert_eq!(Command::parse("enable robot 1"), Command::RobotEnable(1));
        assert_eq!(Command::parse("robot enable 2"), Command::RobotEnable(2));
        assert_eq!(Command::parse("disable robot 3"), Command::RobotDisable(3));
        assert_eq!(Command::parse("robot disable 4"), Command::RobotDisable(4));
        assert_eq!(Command::parse("restart robot 5"), Command::RobotRestart(5));
        assert_eq!(Command::parse("robot restart 6"), Command::RobotRestart(6));
    }

    #[test]
    fn test_parse_show_hide() {
        assert_eq!(Command::parse("show visualizer"), Command::ShowOutput("visualizer".to_string(), true));
        assert_eq!(Command::parse("hide coordinator"), Command::ShowOutput("coordinator".to_string(), false));
        assert_eq!(Command::parse("show all"), Command::ShowOutput("all".to_string(), true));
        assert_eq!(Command::parse("hide all"), Command::ShowOutput("all".to_string(), false));
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
