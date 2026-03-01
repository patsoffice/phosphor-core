use std::collections::VecDeque;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};

use sdl2::audio::{AudioCallback, AudioDevice, AudioSpecDesired};

/// Number of samples over which to fade in/out (~5.8 ms at 44.1 kHz).
const FADE_SAMPLES: u32 = 256;

pub(crate) struct AudioPlayer {
    buffer: Arc<Mutex<VecDeque<i16>>>,
    /// Scratch space for draining samples under the lock.
    drain: Vec<i16>,
    /// Last sample value — held on underrun to avoid pops.
    last_sample: i16,
    fade_in_pos: u32,
    fading_out: Arc<AtomicBool>,
    fade_out_pos: u32,
}

impl AudioCallback for AudioPlayer {
    type Channel = i16;
    fn callback(&mut self, out: &mut [i16]) {
        // Drain available samples under the lock, then release it.
        let available = {
            let mut buf = self.buffer.lock().unwrap();
            let n = out.len().min(buf.len());
            self.drain.clear();
            self.drain.extend(buf.drain(..n));
            n
        };

        // Process drained samples (lock is released).
        for (i, sample) in out.iter_mut().enumerate() {
            let raw = if i < available {
                let s = self.drain[i];
                self.last_sample = s;
                s
            } else {
                // Underrun: hold last sample instead of jumping to zero
                self.last_sample
            };

            if self.fade_in_pos < FADE_SAMPLES {
                let gain = self.fade_in_pos as f32 / FADE_SAMPLES as f32;
                *sample = (raw as f32 * gain) as i16;
                self.fade_in_pos += 1;
            } else if self.fading_out.load(Ordering::Relaxed) {
                if self.fade_out_pos < FADE_SAMPLES {
                    let gain = 1.0 - (self.fade_out_pos as f32 / FADE_SAMPLES as f32);
                    *sample = (raw as f32 * gain) as i16;
                    self.fade_out_pos += 1;
                } else {
                    *sample = 0;
                }
            } else {
                *sample = raw;
            }
        }
    }
}

/// Shared audio ring buffer. The emulator thread pushes samples in;
/// the SDL audio callback thread pops them out.
pub type AudioRing = Arc<Mutex<VecDeque<i16>>>;

/// Handle for signalling the audio callback to fade out before shutdown.
pub type FadeOut = Arc<AtomicBool>;

/// Initialize SDL2 audio playback.
///
/// Returns the audio device (must be kept alive), a shared ring buffer
/// for feeding samples, and a fade-out signal for clean shutdown.
///
/// If `sample_rate` is 0, returns `None` (machine has no audio).
pub fn init(
    sdl_audio: &sdl2::AudioSubsystem,
    sample_rate: u32,
) -> Option<(AudioDevice<AudioPlayer>, AudioRing, FadeOut)> {
    if sample_rate == 0 {
        return None;
    }

    let ring: AudioRing = Arc::new(Mutex::new(VecDeque::with_capacity(8192)));
    let fade_out: FadeOut = Arc::new(AtomicBool::new(false));

    let desired_spec = AudioSpecDesired {
        freq: Some(sample_rate as i32),
        channels: Some(1),
        samples: Some(1024), // ~23.2 ms at 44100 Hz
    };

    let device = sdl_audio
        .open_playback(None, &desired_spec, |spec| AudioPlayer {
            buffer: Arc::clone(&ring),
            drain: Vec::with_capacity(spec.samples as usize),
            last_sample: 0,
            fade_in_pos: 0,
            fading_out: Arc::clone(&fade_out),
            fade_out_pos: 0,
        })
        .expect("Failed to open SDL audio device");

    // Device starts paused; the emulator loop resumes it after the first
    // frame of audio has been buffered.
    Some((device, ring, fade_out))
}

/// Duration to sleep after signalling fade-out, allowing the callback
/// to ramp down before the device is paused.
pub fn fade_out_duration() -> std::time::Duration {
    // FADE_SAMPLES at 44100 Hz ≈ 5.8 ms; round up to 10 ms for safety.
    std::time::Duration::from_millis(10)
}
