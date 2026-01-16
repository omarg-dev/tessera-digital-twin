use bevy::prelude::*;
use crate::resources::{RobotUpdates, RobotIndex};
use crate::components::Robot;

// Applies `RobotUpdate`s to matching robots (by `Robot.id`) in the world
pub fn sync_robots(
    mut robot_updates: ResMut<RobotUpdates>,
    index: Res<RobotIndex>,
    mut robots: Query<(&mut Transform, &mut Robot)>,
) {
    // Drain all updates collected this frame
    for update in robot_updates.updates.drain(..) {
        // Lookup entity by id
        if let Some(&entity) = index.by_id.get(&update.id) {
            if let Ok((mut transform, mut robot)) = robots.get_mut(entity) {
                robot.state = update.state.clone();
                let pos = Vec3::new(update.position[0], update.position[1], update.position[2]);
                robot.position = pos;
                transform.translation = pos;
            }
        }
    }
}