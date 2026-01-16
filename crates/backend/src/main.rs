use zenoh::*;
use tokio::time;
use tokio::io::{self, AsyncBufReadExt, BufReader};
use tokio::sync::mpsc;
use protocol::*;
use serde_json::to_vec;

#[tokio::main]
async fn main() {
    println!("Starting mock brain backend...");
    
    let session = open(Config::default())
        .await
        .expect("Failed to open Zenoh session");
    
    run_mock_brain(session).await;
}

async fn run_mock_brain(zenoh_session: Session) {
    let publisher = zenoh_session
        .declare_publisher("factory/robots")
        .await
        .expect("Failed to declare publisher");

    // Channel to receive movement toggle commands from stdin
    let (tx, mut rx) = mpsc::channel::<MovementCmd>(4);

    // Spawn a task to read stdin and send commands
    tokio::spawn(async move {
        let mut lines = BufReader::new(io::stdin()).lines();
        println!("Commands: 'pause', 'resume'");
        while let Ok(Some(line)) = lines.next_line().await {
            let cmd = line.trim().to_ascii_lowercase();
            let msg = match cmd.as_str() {
                "pause" => Some(MovementCmd::Set(false)),
                "resume" => Some(MovementCmd::Set(true)),
                _ => None,
            };
            if let Some(cmd) = msg {
                let _ = tx.send(cmd).await;
            }
        }
    });

    let robot_id: u32 = 0;
    let mut position = [0.0f32, 0.0, 0.0];
    let mut should_move = false; // valve: whether to advance the mock position
    let mut last_log = std::time::Instant::now();

    loop {
        // Handle any pending movement command (non-blocking)
        if let Ok(cmd) = rx.try_recv() {
            match cmd {
                MovementCmd::Set(v) => should_move = v,
            }
            println!("Movement {}", if should_move { "enabled" } else { "paused" });
        }

        let update = RobotUpdate {
            id: robot_id,
            position: position,
            state: if should_move { RobotState::Moving } else { RobotState::Idle },
        };

                // Simulate movement only when allowed
        if should_move {
            position[0] += 0.1;
            position[2] += 0.1;
        }

        let payload = to_vec(&update).expect("Failed to serialize RobotUpdate");

        publisher
            .put(payload)
            .await
            .expect("Failed to publish RobotUpdate");

        // For debugging: print published update at most once per second while movement occurs
        if should_move && last_log.elapsed() >= std::time::Duration::from_secs(2) {
            println!("Published RobotUpdate_ID: {:?}, State: {:?}, Position: {:?}", update.id, update.state, update.position);
            last_log = std::time::Instant::now();
        }
        time::sleep(std::time::Duration::from_millis(100)).await;
    }
}

/// Simple movement control commands from stdin
enum MovementCmd {
    Set(bool)
}
