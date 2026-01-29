//! System command handling for scheduler

use protocol::SystemCommand;
use serde_json::from_slice;
use zenoh::handlers::FifoChannelHandler;
use zenoh::pubsub::Subscriber;
use zenoh::sample::Sample;

/// Process system commands (pause/resume/verbose)
pub fn handle_system_commands(
    subscriber: &Subscriber<FifoChannelHandler<Sample>>,
    paused: &mut bool,
    verbose: &mut bool,
) {
    while let Ok(Some(sample)) = subscriber.try_recv() {
        if let Ok(cmd) = from_slice::<SystemCommand>(&sample.payload().to_bytes()) {
            cmd.apply_with_log("Scheduler", Some(paused), Some(verbose));
        }
    }
}
