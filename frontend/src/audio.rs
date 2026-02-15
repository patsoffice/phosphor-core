use std::collections::VecDeque;
use std::sync::{Arc, Mutex};

use sdl2::audio::{AudioCallback, AudioDevice, AudioSpecDesired};

pub(crate) struct AudioPlayer {
    buffer: Arc<Mutex<VecDeque<i16>>>,
}

impl AudioCallback for AudioPlayer {
    type Channel = i16;
    fn callback(&mut self, out: &mut [i16]) {
        let mut buf = self.buffer.lock().unwrap();
        for sample in out.iter_mut() {
            *sample = buf.pop_front().unwrap_or(0);
        }
    }
}

/// Shared audio ring buffer. The emulator thread pushes samples in;
/// the SDL audio callback thread pops them out.
pub type AudioRing = Arc<Mutex<VecDeque<i16>>>;

/// Initialize SDL2 audio playback.
///
/// Returns the audio device (must be kept alive) and a shared ring buffer
/// for feeding samples from the emulation loop.
///
/// If `sample_rate` is 0, returns `None` (machine has no audio).
pub fn init(
    sdl_audio: &sdl2::AudioSubsystem,
    sample_rate: u32,
) -> Option<(AudioDevice<AudioPlayer>, AudioRing)> {
    if sample_rate == 0 {
        return None;
    }

    let ring: AudioRing = Arc::new(Mutex::new(VecDeque::with_capacity(4096)));

    let desired_spec = AudioSpecDesired {
        freq: Some(sample_rate as i32),
        channels: Some(1),
        samples: Some(512), // ~11.6 ms at 44100 Hz
    };

    let device = sdl_audio
        .open_playback(None, &desired_spec, |_spec| AudioPlayer {
            buffer: Arc::clone(&ring),
        })
        .expect("Failed to open SDL audio device");

    device.resume();

    Some((device, ring))
}
