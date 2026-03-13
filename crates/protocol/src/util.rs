//! Shared utility helpers for world/grid math and payload validation.

/// Convert a world-space position into grid coordinates.
///
/// Returns None when the input is non-finite or rounds to a negative coordinate.
pub fn world_to_grid(pos: [f32; 3]) -> Option<(usize, usize)> {
    Some((round_to_index(pos[0])?, round_to_index(pos[2])?))
}

/// Convert grid coordinates to world-space position.
pub fn grid_to_world(grid: (usize, usize), y: f32) -> [f32; 3] {
    [grid.0 as f32, y, grid.1 as f32]
}

/// Check if a world-space position contains only finite values.
pub fn is_finite_position(pos: [f32; 3]) -> bool {
    pos[0].is_finite() && pos[1].is_finite() && pos[2].is_finite()
}

/// Squared distance in the XZ plane.
pub fn distance_sq_xz(a: [f32; 3], b: [f32; 3]) -> f32 {
    let dx = a[0] - b[0];
    let dz = a[2] - b[2];
    dx * dx + dz * dz
}

/// Distance in the XZ plane.
pub fn distance_xz(a: [f32; 3], b: [f32; 3]) -> f32 {
    distance_sq_xz(a, b).sqrt()
}

fn round_to_index(value: f32) -> Option<usize> {
    if !value.is_finite() {
        return None;
    }
    let rounded = value.round();
    if rounded < 0.0 {
        return None;
    }
    Some(rounded as usize)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_world_to_grid_valid() {
        assert_eq!(world_to_grid([2.4, 0.0, 7.6]), Some((2, 8)));
    }

    #[test]
    fn test_world_to_grid_invalid() {
        assert_eq!(world_to_grid([f32::NAN, 0.0, 1.0]), None);
        assert_eq!(world_to_grid([-0.6, 0.0, 1.0]), None);
    }

    #[test]
    fn test_distance_xz() {
        let a = [1.0, 10.0, 1.0];
        let b = [4.0, -5.0, 5.0];
        assert!((distance_sq_xz(a, b) - 25.0).abs() < 1e-6);
        assert!((distance_xz(a, b) - 5.0).abs() < 1e-6);
    }
}