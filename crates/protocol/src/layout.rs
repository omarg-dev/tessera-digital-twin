//! Runtime layout selection and resolution helpers.
//!
//! This module owns non-constant layout logic used by orchestrator and runtime crates.

/// Default warehouse layout file (relative to workspace root)
pub const LAYOUT_FILE_PATH: &str = "assets/data/layout.txt";

/// Environment variable used to override the active layout at runtime.
pub const LAYOUT_OVERRIDE_ENV: &str = "HYPER_TWIN_LAYOUT";

/// Resolve a user-friendly layout selector to a concrete layout file path.
///
/// Accepted selectors include numeric IDs and aliases.
pub fn layout_path_from_selector(selector: &str) -> Option<&'static str> {
    let normalized = selector.trim().to_ascii_lowercase();
    match normalized.as_str() {
        "0" | "default" | "layout" | "layout0" => Some("assets/data/layout.txt"),
        "1" | "layout1" => Some("assets/data/layout1.txt"),
        "2" | "layout2" => Some("assets/data/layout2.txt"),
        "3" | "layout3" | "cinematic1" | "cinematic_ring" => {
            Some("assets/data/layout3_cinematic_ring.txt")
        }
        "4" | "layout4" | "cinematic2" | "cinematic_crossroads" => {
            Some("assets/data/layout4_cinematic_crossroads.txt")
        }
        "5" | "layout5" | "cinematic3" | "cinematic_runway" => {
            Some("assets/data/layout5_cinematic_runway.txt")
        }
        "6" | "layout6" | "test1" | "test_bottleneck" => {
            Some("assets/data/layout6_test_bottleneck.txt")
        }
        "7" | "layout7" | "test2" | "test_openfield" => {
            Some("assets/data/layout7_test_openfield.txt")
        }
        "8" | "layout8" | "test3" | "test_lane_swap" => {
            Some("assets/data/layout8_test_lane_swap.txt")
        }
        "9" | "layout9" | "mega" | "massive_factory" | "factory100" => {
            Some("assets/data/layout9_massive_factory.txt")
        }
        _ => None,
    }
}

/// Resolve the active layout path for runtime crates.
///
/// If the orchestrator set an override via environment variable, that path wins.
/// Otherwise the global default layout is used.
pub fn resolve_layout_path() -> String {
    std::env::var(LAYOUT_OVERRIDE_ENV)
        .ok()
        .filter(|value| !value.trim().is_empty())
        .unwrap_or_else(|| LAYOUT_FILE_PATH.to_string())
}
