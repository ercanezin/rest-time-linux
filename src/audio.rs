use rodio::{OutputStream, Sink, source::SineWave, Source};
use std::time::Duration;
use tracing::warn;

pub struct AudioEngine;

impl AudioEngine {
    pub fn play_break_start(volume: f32) {
        tokio::task::spawn_blocking(move || {
            if let Ok((_stream, handle)) = OutputStream::try_default() {
                if let Ok(sink) = Sink::try_new(&handle) {
                    sink.set_volume(volume);
                    // Harmonic chime
                    let chord = SineWave::new(523.25)
                        .take_duration(Duration::from_millis(400))
                        .amplify(0.5);
                    sink.append(chord);
                    sink.sleep_until_end();
                }
            } else {
                warn!("Audio device unavailable for break chime.");
            }
        });
    }

    pub fn play_break_end(volume: f32) {
        tokio::task::spawn_blocking(move || {
            if let Ok((_stream, handle)) = OutputStream::try_default() {
                if let Ok(sink) = Sink::try_new(&handle) {
                    sink.set_volume(volume);
                    // Ascending chime
                    let chime = SineWave::new(783.99)
                        .take_duration(Duration::from_millis(500))
                        .amplify(0.5);
                    sink.append(chime);
                    sink.sleep_until_end();
                }
            } else {
                warn!("Audio device unavailable for break chime.");
            }
        });
    }
}
