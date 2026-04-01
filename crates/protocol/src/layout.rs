//! Runtime layout selection and resolution helpers.
//!
//! This module owns non-constant layout logic used by orchestrator and runtime crates.

use std::path::Path;
use std::{fs, io};

/// Default warehouse layout file (relative to workspace root)
pub const LAYOUT_FILE_PATH: &str = "assets/layouts/l1_basic_small.layout";

/// Directory containing all discoverable layout files.
pub const LAYOUTS_DIR: &str = "assets/layouts";

/// Supported file extension for discoverable layouts.
pub const LAYOUT_FILE_EXTENSION: &str = "layout";

/// Environment variable used to override the active layout at runtime.
pub const LAYOUT_OVERRIDE_ENV: &str = "TESSERA_LAYOUT";

/// Discoverable layout metadata.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LayoutEntry {
    pub file_name: String,
    pub stem: String,
    pub path: String,
}

fn read_layout_entries_from_dir(layouts_dir: &Path) -> io::Result<Vec<LayoutEntry>> {
    let mut entries = Vec::new();

    for entry in fs::read_dir(layouts_dir)? {
        let entry = entry?;
        if !entry.file_type()?.is_file() {
            continue;
        }

        let file_name_os = entry.file_name();
        let Some(file_name) = file_name_os.to_str() else {
            continue;
        };

        let path = Path::new(file_name);
        let Some(extension) = path.extension().and_then(|value| value.to_str()) else {
            continue;
        };

        if !extension.eq_ignore_ascii_case(LAYOUT_FILE_EXTENSION) {
            continue;
        }

        let stem = path
            .file_stem()
            .and_then(|value| value.to_str())
            .unwrap_or(file_name)
            .to_string();

        entries.push(LayoutEntry {
            file_name: file_name.to_string(),
            stem,
            path: format!("{}/{}", LAYOUTS_DIR, file_name),
        });
    }

    entries.sort_by(|left, right| {
        left.file_name
            .to_ascii_lowercase()
            .cmp(&right.file_name.to_ascii_lowercase())
            .then_with(|| left.file_name.cmp(&right.file_name))
    });

    Ok(entries)
}

/// Discover all available layouts under assets/layouts in stable alphabetical order.
pub fn discover_layout_entries() -> io::Result<Vec<LayoutEntry>> {
    read_layout_entries_from_dir(Path::new(LAYOUTS_DIR))
}

fn select_layout_path(entries: &[LayoutEntry], selector: &str) -> Option<String> {
    let normalized = selector.trim().to_ascii_lowercase();
    if normalized.is_empty() {
        return None;
    }

    if let Ok(index) = normalized.parse::<usize>() {
        if (1..=entries.len()).contains(&index) {
            return Some(entries[index - 1].path.clone());
        }
    }

    entries
        .iter()
        .find(|entry| {
            entry.stem.eq_ignore_ascii_case(&normalized)
                || entry.file_name.eq_ignore_ascii_case(&normalized)
        })
        .map(|entry| entry.path.clone())
}

/// Resolve a user-friendly layout selector to a concrete layout file path.
///
/// Accepted selectors include 1-based numeric indices and file stem/full-name aliases.
pub fn layout_path_from_selector(selector: &str) -> io::Result<Option<String>> {
    let entries = discover_layout_entries()?;
    Ok(select_layout_path(&entries, selector))
}

/// Resolve the active layout path for runtime crates.
///
/// If the orchestrator set an override via environment variable, that path wins.
/// Otherwise the first discovered layout is used, with a static fallback.
pub fn resolve_layout_path() -> String {
    std::env::var(LAYOUT_OVERRIDE_ENV)
        .ok()
        .filter(|value| !value.trim().is_empty())
        .or_else(|| {
            discover_layout_entries()
                .ok()
                .and_then(|entries| entries.into_iter().next().map(|entry| entry.path))
        })
        .unwrap_or_else(|| LAYOUT_FILE_PATH.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;
    use std::time::{SystemTime, UNIX_EPOCH};

    fn unique_temp_dir(prefix: &str) -> PathBuf {
        let unique = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("system clock before unix epoch")
            .as_nanos();
        std::env::temp_dir().join(format!("tessera_{prefix}_{unique}"))
    }

    #[test]
    fn discover_filters_non_layout_and_sorts_alphabetically() {
        let dir = unique_temp_dir("layout_discovery");
        fs::create_dir_all(&dir).expect("failed to create temp dir");
        fs::create_dir(dir.join("nested")).expect("failed to create nested temp dir");

        fs::write(dir.join("l3_gamma.layout"), "#").expect("failed to write layout file");
        fs::write(dir.join("L1_alpha.layout"), "#").expect("failed to write layout file");
        fs::write(dir.join("readme.txt"), "ignore").expect("failed to write non-layout file");
        fs::write(dir.join("l2_beta.layout"), "#").expect("failed to write layout file");

        let entries = read_layout_entries_from_dir(&dir).expect("failed to discover layouts");

        assert_eq!(entries.len(), 3);
        assert_eq!(entries[0].file_name, "L1_alpha.layout");
        assert_eq!(entries[1].file_name, "l2_beta.layout");
        assert_eq!(entries[2].file_name, "l3_gamma.layout");
        assert_eq!(entries[0].stem, "L1_alpha");
        assert_eq!(entries[1].stem, "l2_beta");

        fs::remove_dir_all(&dir).expect("failed to clean temp dir");
    }

    #[test]
    fn selector_supports_index_stem_and_filename() {
        let entries = vec![
            LayoutEntry {
                file_name: "l1_basic_small.layout".to_string(),
                stem: "l1_basic_small".to_string(),
                path: "assets/layouts/l1_basic_small.layout".to_string(),
            },
            LayoutEntry {
                file_name: "l2_basic_medium.layout".to_string(),
                stem: "l2_basic_medium".to_string(),
                path: "assets/layouts/l2_basic_medium.layout".to_string(),
            },
        ];

        assert_eq!(
            select_layout_path(&entries, "1"),
            Some("assets/layouts/l1_basic_small.layout".to_string())
        );
        assert_eq!(
            select_layout_path(&entries, "2"),
            Some("assets/layouts/l2_basic_medium.layout".to_string())
        );
        assert_eq!(
            select_layout_path(&entries, "l2_basic_medium"),
            Some("assets/layouts/l2_basic_medium.layout".to_string())
        );
        assert_eq!(
            select_layout_path(&entries, "L1_BASIC_SMALL.LAYOUT"),
            Some("assets/layouts/l1_basic_small.layout".to_string())
        );
        assert_eq!(select_layout_path(&entries, "0"), None);
        assert_eq!(select_layout_path(&entries, "999"), None);
        assert_eq!(select_layout_path(&entries, "unknown"), None);
    }
}
