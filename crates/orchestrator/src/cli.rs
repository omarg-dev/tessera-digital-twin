//! CLI command parsing and help display

use crate::processes::CRATE_ORDER;

/// Parsed command from user input
#[derive(Debug, Clone, PartialEq)]
pub enum Command {
    // Process management
    RunAll(Option<String>),
    Run { name: String, layout: Option<String> },
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

        if matches!(parts.first(), Some(&"run") | Some(&"up")) {
            return Self::parse_run_command(&parts, &input);
        }
        
        match parts.as_slice() {
            // Process management
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
            
            // Meta
            ["help"] | ["h"] | ["?"] => Command::Help,
            ["quit"] | ["exit"] | ["q"] => Command::Quit,
            
            [] => Command::Empty,
            _ => Command::Unknown(input),
        }
    }

    fn parse_run_command(parts: &[&str], input: &str) -> Self {
        let mut layout = None;
        let mut target = None;
        let mut i = 1;

        while i < parts.len() {
            match parts[i] {
                "-l" | "--layout" => {
                    i += 1;
                    let Some(selector) = parts.get(i) else {
                        return Command::Unknown(input.to_string());
                    };
                    layout = Some((*selector).to_string());
                }
                "all" => {
                    if target.is_some() {
                        return Command::Unknown(input.to_string());
                    }
                    target = Some("all".to_string());
                }
                value => {
                    if target.is_some() {
                        return Command::Unknown(input.to_string());
                    }
                    target = Some(value.to_string());
                }
            }
            i += 1;
        }

        match target.as_deref() {
            None | Some("all") => Command::RunAll(layout),
            Some(name) => Command::Run {
                name: name.to_string(),
                layout,
            },
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
    println!("│  run -l <id>    - Run all with layout preset    │");
    println!("│  run <crate> --layout <id> - Run crate layout   │");
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
    println!("│  quit, q        - Kill all and exit             │");
    println!("│  help, h        - Show this help                │");
    println!("╰─────────────────────────────────────────────────╯");
    println!();
    println!("Available crates: {:?}", CRATE_ORDER);
    println!("Available layout presets:");
    println!("  0/default/layout           -> assets/data/layout.txt");
    println!("  1/layout1                  -> assets/data/layout1.txt");
    println!("  2/layout2                  -> assets/data/layout2.txt");
    println!("  3/cinematic1/cinematic_ring -> assets/data/layout3_cinematic_ring.txt");
    println!("  4/cinematic2/cinematic_crossroads -> assets/data/layout4_cinematic_crossroads.txt");
    println!("  5/cinematic3/cinematic_runway -> assets/data/layout5_cinematic_runway.txt");
    println!("  6/test1/test_bottleneck    -> assets/data/layout6_test_bottleneck.txt");
    println!("  7/test2/test_openfield     -> assets/data/layout7_test_openfield.txt");
    println!("  8/test3/test_lane_swap     -> assets/data/layout8_test_lane_swap.txt");
    println!("  9/mega/massive_factory     -> assets/data/layout9_massive_factory.txt");
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
        assert_eq!(Command::parse("run"), Command::RunAll(None));
        assert_eq!(Command::parse("run all"), Command::RunAll(None));
        assert_eq!(Command::parse("up"), Command::RunAll(None));
        assert_eq!(Command::parse("UP ALL"), Command::RunAll(None));
        assert_eq!(Command::parse("run -l 3"), Command::RunAll(Some("3".to_string())));
        assert_eq!(
            Command::parse("run --layout cinematic1"),
            Command::RunAll(Some("cinematic1".to_string()))
        );
        assert_eq!(Command::parse("run --layout 9"), Command::RunAll(Some("9".to_string())));
    }

    #[test]
    fn test_parse_run_specific() {
        assert_eq!(
            Command::parse("run mock_firmware"),
            Command::Run {
                name: "mock_firmware".to_string(),
                layout: None,
            }
        );
        assert_eq!(
            Command::parse("run visualizer --layout 2"),
            Command::Run {
                name: "visualizer".to_string(),
                layout: Some("2".to_string()),
            }
        );
        assert_eq!(
            Command::parse("run -l test1 scheduler"),
            Command::Run {
                name: "scheduler".to_string(),
                layout: Some("test1".to_string()),
            }
        );
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
