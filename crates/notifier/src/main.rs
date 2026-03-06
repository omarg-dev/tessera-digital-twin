// dev tool: plays a short ascending arpeggio after compilation finishes.
//
// usage:  cargo build -p <target> && cargo notify
//
// the sound is synthesized entirely in code — no audio file required.

use rodio::{OutputStream, Sink, Source, source::SineWave};
use std::time::Duration;

// note frequencies (Hz)
const C5: f32 = 523.25;
const E5: f32 = 659.25;
const G5: f32 = 783.99;
const C6: f32 = 1046.50;

// duration of each note in ms
const NOTE_MS: u64 = 110;
const FINAL_MS: u64 = 280;

// volume (0.0 – 1.0); pure sine waves are louder than they sound, keep this gentle
const AMPLITUDE: f32 = 0.22;

fn note(freq: f32, ms: u64) -> impl Source<Item = f32> {
    SineWave::new(freq)
        .take_duration(Duration::from_millis(ms))
        .amplify(AMPLITUDE)
}

fn main() {
    let Ok((_stream, handle)) = OutputStream::try_default() else {
        // no audio device — silently exit rather than crashing
        return;
    };
    let Ok(sink) = Sink::try_new(&handle) else {
        return;
    };

    sink.append(note(C5, NOTE_MS));
    sink.append(note(E5, NOTE_MS));
    sink.append(note(G5, NOTE_MS));
    sink.append(note(C6, FINAL_MS));

    sink.sleep_until_end();
}
