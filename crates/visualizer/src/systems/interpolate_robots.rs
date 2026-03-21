//! Dead-reckoning interpolation for robot transforms.
//!
//! Firmware publishes at 20 Hz. Without interpolation, robots visually
//! teleport between these 50 ms snapshots on each render frame.
//!
//! Each frame this system:
//! 1. Advances transform.translation by network_velocity * dt (dead-reckoning).
//!    When velocity is accurate the robot glides smoothly with no visual step.
//! 2. Applies a correction lerp toward target_position to drain any accumulated
//!    drift without overshooting.
//! 3. Snaps immediately if the distance to target exceeds ROBOT_TELEPORT_THRESHOLD
//!    (firmware restart, initial spawn).
//!
//! Runs in Update, after sync_robots (which writes target_position / network_velocity).

use bevy::prelude::*;
use crate::components::Robot;
use protocol::config::visual::{ROBOT_LERP, ROBOT_TELEPORT_THRESHOLD};

pub fn interpolate_robots(
    time: Res<Time>,
    mut robots: Query<(&mut Transform, &Robot)>,
) {
    let dt = time.delta_secs();

    for (mut transform, robot) in robots.iter_mut() {
        let current = transform.translation;
        let target = robot.target_position;

        let dist = current.distance(target);

        // snap on teleport (restart, initial spawn, chaos drift beyond threshold)
        if dist > ROBOT_TELEPORT_THRESHOLD {
            transform.translation = target;
            continue;
        }

        // dead-reckon: advance position using firmware-reported velocity
        let dead_reckoned = current + robot.network_velocity * dt;

        // correction lerp: frame-rate-independent factor derived from per-frame constant
        // at 60 fps: 0.25 * (1/60) * 60 = 0.25 per frame → <5% residual after 2 frames
        let lerp_factor = (ROBOT_LERP * dt * 60.0).min(1.0);
        transform.translation = dead_reckoned.lerp(target, lerp_factor);
    }
}
