//! System command handling for mission_control

use protocol::SystemCommand;
use serde_json::from_slice;
use zenoh::handlers::FifoChannelHandler;
use zenoh::pubsub::Subscriber;
use zenoh::sample::Sample;

/// Process system commands (pause/resume)
pub fn handle_system_commands(
    subscriber: &Subscriber<FifoChannelHandler<Sample>>,
    paused: &mut bool,
) {
    while let Ok(Some(sample)) = subscriber.try_recv() {
        if let Ok(cmd) = from_slice::<SystemCommand>(&sample.payload().to_bytes()) {
            match cmd {
                SystemCommand::Pause => {
                    *paused = true;
                    println!("⏸ PAUSED");
                }
                SystemCommand::Resume => {
                    *paused = false;
                    println!("▶ RESUMED");
                }
            }
        }
    }
}
