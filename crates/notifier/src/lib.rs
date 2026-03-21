// notification sound library.
//
// exposes two public functions:
//   play_default()              — plays the arpeggio from protocol::config::notify
//   play_sequence(seq)          — plays any caller-supplied (freq_hz, duration_ms) pairs
//
// both block the calling thread until the sound finishes.
// call from a spawned thread if you don't want to stall the caller.

use rodio::{OutputStream, Sink, Source, source::SineWave};
use std::time::Duration;
use protocol::config::notify as cfg;

/// Play the default build-complete arpeggio defined in `protocol::config::notify`.
pub fn play_default() {
    play_sequence(cfg::DEFAULT_SEQUENCE);
}

/// Play a custom note sequence.
///
/// Each element is `(frequency_hz, duration_ms)`. Uses the same amplitude as
/// `play_default`. Silently exits on any audio device error.
pub fn play_sequence(sequence: &[(f32, u64)]) {
    let Ok((_stream, handle)) = OutputStream::try_default() else {
        return;
    };
    let Ok(sink) = Sink::try_new(&handle) else {
        return;
    };

    for &(freq, ms) in sequence {
        sink.append(
            SineWave::new(freq)
                .take_duration(Duration::from_millis(ms))
                .amplify(cfg::AMPLITUDE),
        );
    }

    sink.sleep_until_end();
}
