use bevy::prelude::*;
use crate::components::Robot;
use crate::resources::RobotIndex;

/// Build the initial robot index at startup
pub fn build_robot_index(mut index: ResMut<RobotIndex>, robots: Query<(Entity, &Robot)>) {
    index.by_id.clear();
    for (entity, robot) in robots.iter() {
        index.by_id.insert(robot.id, entity);
    }
}

/// Track newly spawned robots and add them to the index
pub fn index_new_robots(mut index: ResMut<RobotIndex>, robots: Query<(Entity, &Robot), Added<Robot>>) {
    for (entity, robot) in robots.iter() {
        index.by_id.insert(robot.id, entity);
    }
}
