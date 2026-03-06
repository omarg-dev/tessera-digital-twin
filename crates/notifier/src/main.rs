// dev tool: plays the default build-complete arpeggio.
//
// usage:  cargo build -p <target> && cargo notify
//
// the melody and timing are configured in protocol::config::notify.

fn main() {
    notifier::play_default();
}
