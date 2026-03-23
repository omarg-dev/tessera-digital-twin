//! Mock firmware main loop - Firmware layer simulation

use zenoh::Session;
use tokio::time;
use protocol::*;
use protocol::config::firmware::physics::TICK_INTERVAL_MS;
use std::time::Instant;

use crate::robot::SimRobot;
use crate::commands::{handle_system_commands, handle_path_commands, handle_robot_control};

/// Run the mock firmware main loop
pub async fn run(session: Session, map: GridMap) {
    // Publishers
    let update_publisher = session
        .declare_publisher(topics::ROBOT_UPDATES)
        .await
        .expect("Failed to declare ROBOT_UPDATES publisher");
    
    let response_publisher = session
        .declare_publisher(topics::COMMAND_RESPONSES)
        .await
        .expect("Failed to declare COMMAND_RESPONSES publisher");
    
    // Subscribers
    let cmd_subscriber = session
        .declare_subscriber(topics::PATH_COMMANDS)
        .await
        .expect("Failed to declare PATH_COMMANDS subscriber");
    
    let control_subscriber = session
        .declare_subscriber(topics::ADMIN_CONTROL)
        .await
        .expect("Failed to declare ADMIN_CONTROL subscriber");
    
    let robot_control_subscriber = session
        .declare_subscriber(topics::ROBOT_CONTROL)
        .await
        .expect("Failed to declare ROBOT_CONTROL subscriber");
    
    // Spawn robots at station positions
    let mut robots = spawn_robots_from_map(&map);
    
    println!("✓ Mock Firmware running with {} robot(s)", robots.len());
    
    let mut paused = false;
    let mut chaos = protocol::config::chaos::ENABLED;
    let mut time_scale: f32 = 1.0;
    let mut last_tick = Instant::now();
    let mut tick_count: u64 = 0;
    
    loop {
        let now = Instant::now();
        let dt = now.duration_since(last_tick).as_secs_f32();
        last_tick = now;
        
        // Handle system commands (pause/resume/chaos/time_scale)
        handle_system_commands(&control_subscriber, &mut paused, &mut chaos, &mut time_scale);
        
        // Handle robot control commands (up/down/restart)
        handle_robot_control(&robot_control_subscriber, &mut robots);
        
        // Handle path commands from coordinator
        handle_path_commands(&cmd_subscriber, &response_publisher, &mut robots, chaos).await;
        
        // Physics update for all robots (dt scaled by time_scale)
        let scaled_dt = dt * time_scale;
        for robot in &mut robots {
            robot.update_physics(scaled_dt, paused, chaos, &map);
        }
        
        // Batch and publish all robot updates (with chaos packet loss)
        publish_batch_update(&update_publisher, &robots, tick_count, chaos).await;
        
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
    chaos: bool,
) {
    // Chaos: occasionally drop the entire batch (simulates network loss)
    if protocol::chaos::should_drop_packet(chaos) {
        protocol::chaos::log_chaos_event("Dropped RobotUpdateBatch packet", "Firmware");
        return;
    }
    
    let batch = RobotUpdateBatch {
        // Include all robots, even disabled ones (receiver filters by enabled flag)
        updates: robots.iter().map(|r| r.to_update()).collect(),
        tick,
    };

    let _ = protocol::publish_json_logged(
        "Firmware",
        &format!("RobotUpdateBatch tick={}", tick),
        &batch,
        |payload| async move { publisher.put(payload).await.map(|_| ()) },
    )
    .await;
}
