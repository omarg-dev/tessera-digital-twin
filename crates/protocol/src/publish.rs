//! Shared helpers for JSON serialization + publish result logging.

use std::fmt::Display;
use std::future::Future;

use serde::Serialize;

use crate::logs::save_log;

/// Serialize a message to JSON and publish it through the provided async closure.
///
/// Returns `true` on publish success, `false` on serialization or publish failure.
pub async fn publish_json_logged<T, F, Fut, E>(
    layer: &str,
    context: &str,
    message: &T,
    publish: F,
) -> bool
where
    T: Serialize,
    F: FnOnce(Vec<u8>) -> Fut,
    Fut: Future<Output = Result<(), E>>,
    E: Display,
{
    let payload = match serde_json::to_vec(message) {
        Ok(payload) => payload,
        Err(err) => {
            save_log(
                layer,
                &format!("serialization failed for {}: {}", context, err),
            );
            return false;
        }
    };

    match publish(payload).await {
        Ok(()) => true,
        Err(err) => {
            save_log(layer, &format!("publish failed for {}: {}", context, err));
            false
        }
    }
}
