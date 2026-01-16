use bevy::prelude::*;
use zenoh::prelude::*;
use tokio::time;
// use crate::protocol;  // Can't use crate:: in a separate binary
use serde_json::to_vec;
use serde::{Serialize, Deserialize};

// Define protocol types locally (or move to a shared lib)
mod protocol {
    use super::*;
    
    #[derive(Debug, Clone, Serialize, Deserialize)]
    pub enum RobotState {
        Idle,
        Moving,
        Loading,
        Unloading,
    }

    #[derive(Debug, Clone, Serialize, Deserialize)]
    pub struct RobotUpdate {
        pub id: u32,
        pub position: [f32; 3],  // Use array instead of Vec3 for serialization
        pub state: RobotState,
    }
}

#[tokio::main]
async fn main() {
    println!("Starting mock brain backend...");
    
    let session = zenoh::open(zenoh::Config::default())
        .await
        .expect("Failed to open Zenoh session");
    
    run_mock_brain(session).await;
}

async fn run_mock_brain(zenoh_session: Session) {
    let publisher = zenoh_session
        .declare_publisher("/factory/robots")
        .await
        .expect("Failed to declare publisher");

    let robot_id: u32 = 1;
    let mut position = [0.0f32, 0.0, 0.0];

    loop {
        let update = protocol::RobotUpdate {
            id: robot_id,
            position,
            state: protocol::RobotState::Moving,
        };

        let payload = to_vec(&update).expect("Failed to serialize RobotUpdate");

        publisher
            .put(payload)
            .await
            .expect("Failed to publish RobotUpdate");

        // Simulate movement
        position[0] += 0.1;
        position[2] += 0.1;

        time::sleep(std::time::Duration::from_millis(100)).await;
    }
}