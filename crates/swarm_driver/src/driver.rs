//! Swarm driver main loop

use zenoh::Session;
use tokio::time;
use protocol::*;
use protocol::config::physics::TICK_INTERVAL_MS;
use serde_json::to_vec;
use std::time::Instant;

use crate::robot::SimRobot;
use crate::commands::{handle_system_commands, handle_path_commands};

/// Run the swarm driver main loop
pub async fn run(session: Session, map: GridMap) {
    // Publishers
    let update_publisher = session
        .declare_publisher(topics::ROBOT_UPDATES)
        .await
        .expect("Failed to declare ROBOT_UPDATES publisher");
    
    // Subscribers
    let cmd_subscriber = session
        .declare_subscriber(topics::PATH_COMMANDS)
        .await
        .expect("Failed to declare PATH_COMMANDS subscriber");
    
    let control_subscriber = session
        .declare_subscriber(topics::ADMIN_CONTROL)
        .await
        .expect("Failed to declare ADMIN_CONTROL subscriber");
    
    // Spawn robots at station positions
    let mut robots = spawn_robots_from_map(&map);
    
    println!("✓ Swarm Driver running with {} robot(s)", robots.len());
    
    let mut paused = false;
    let mut last_tick = Instant::now();
    let mut tick_count: u64 = 0;
    
    loop {
        let now = Instant::now();
        let dt = now.duration_since(last_tick).as_secs_f32();
        last_tick = now;
        
        // Handle system commands (pause/resume)
        handle_system_commands(&control_subscriber, &mut paused);
        
        // Handle path commands from fleet_server
        handle_path_commands(&cmd_subscriber, &mut robots);
        
        // Physics update for all robots
        for robot in &mut robots {
            robot.update_physics(dt, paused);
        }
        
        // Batch and publish all robot updates
        publish_batch_update(&update_publisher, &robots, tick_count).await;
        
        tick_count += 1;
        time::sleep(std::time::Duration::from_millis(TICK_INTERVAL_MS)).await;
    }
}

/// Spawn one robot per station found in the map
fn spawn_robots_from_map(map: &GridMap) -> Vec<SimRobot> {
    let stations = map.get_stations();
    
    let mut robots: Vec<SimRobot> = stations.iter().enumerate().map(|(i, station)| {
        let id = (i + 1) as u32;
        let pos = [station.x as f32, 0.25, station.y as f32];
        println!("+ Spawning Robot {} at station [{}, {}]", id, station.x, station.y);
        SimRobot::new(id, pos)
    }).collect();
    
    if robots.is_empty() {
        println!("⚠ No stations found in map. Spawning 1 robot at origin.");
        robots.push(SimRobot::new(1, [1.0, 0.25, 1.0]));
    }
    
    robots
}

/// Batch all robot updates into a single message and publish
async fn publish_batch_update(
    publisher: &zenoh::pubsub::Publisher<'_>,
    robots: &[SimRobot],
    tick: u64,
) {
    let batch = RobotUpdateBatch {
        updates: robots.iter().map(|r| r.to_update()).collect(),
        tick,
    };
    
    let payload = to_vec(&batch).expect("Failed to serialize RobotUpdateBatch");
    publisher.put(payload).await.ok();
}
