use std::time::{Duration, Instant};

use phosphor_core::core::machine::Machine;
use sdl2::event::Event;
use sdl2::keyboard::Scancode;

use crate::input::KeyMap;
use crate::video::Video;

pub fn run(machine: &mut dyn Machine, key_map: &KeyMap, scale: u32) {
    let sdl_context = sdl2::init().expect("Failed to initialize SDL2");
    let sdl_video = sdl_context.video().expect("Failed to init SDL video");
    let sdl_audio = sdl_context.audio().expect("Failed to init SDL audio");

    let (width, height) = machine.display_size();
    let mut video = Video::new(&sdl_video, "Phosphor Emulator", width, height, scale);
    let mut event_pump = sdl_context.event_pump().expect("Failed to get event pump");

    let audio_state = crate::audio::init(&sdl_audio, machine.audio_sample_rate());
    let mut audio_started = false;

    let buffer_size = (width * height * 3) as usize;
    let mut framebuffer = vec![0u8; buffer_size];
    let mut audio_scratch = vec![0i16; 2048];

    let frame_duration = Duration::from_secs_f64(1.0 / machine.frame_rate_hz());
    let mut next_frame_time = Instant::now() + frame_duration;
    let mut throttle = true;

    // FPS overlay state (F10 to toggle)
    let mut show_fps = false;
    let mut fps_text = String::new();
    let mut fps_frame_count: u32 = 0;
    let mut fps_last_update = Instant::now();

    'main: loop {
        // Poll all pending SDL events, translate to machine input
        for event in event_pump.poll_iter() {
            match event {
                Event::Quit { .. } => break 'main,

                Event::KeyDown {
                    scancode: Some(Scancode::Escape),
                    ..
                } => break 'main,

                Event::KeyDown {
                    scancode: Some(Scancode::F9),
                    repeat: false,
                    ..
                } => {
                    throttle = !throttle;
                    if throttle {
                        next_frame_time = Instant::now() + frame_duration;
                    }
                }

                Event::KeyDown {
                    scancode: Some(Scancode::F10),
                    repeat: false,
                    ..
                } => {
                    show_fps = !show_fps;
                    fps_frame_count = 0;
                    fps_last_update = Instant::now();
                }

                Event::KeyDown {
                    scancode: Some(sc),
                    repeat: false,
                    ..
                } => {
                    if let Some(button_id) = key_map.get(sc) {
                        machine.set_input(button_id, true);
                    }
                }

                Event::KeyUp {
                    scancode: Some(sc), ..
                } => {
                    if let Some(button_id) = key_map.get(sc) {
                        machine.set_input(button_id, false);
                    }
                }

                _ => {}
            }
        }

        // Run one frame of emulation
        machine.run_frame();

        // Drain audio samples from machine into SDL ring buffer
        if let Some((ref device, ref ring)) = audio_state {
            let n = machine.fill_audio(&mut audio_scratch);
            if n > 0 {
                let mut buf = ring.lock().unwrap();
                const MAX_RING_SIZE: usize = 4096;
                while buf.len() + n > MAX_RING_SIZE {
                    buf.pop_front();
                }
                buf.extend(&audio_scratch[..n]);

                // Start playback after the first batch of real samples is buffered,
                // so the callback never transitions from silence to audio (no pop).
                if !audio_started {
                    device.resume();
                    audio_started = true;
                }
            }
        }

        // Render the framebuffer and present
        machine.render_frame(&mut framebuffer);

        // FPS overlay: update counter and draw text onto framebuffer
        if show_fps {
            fps_frame_count += 1;
            let elapsed = fps_last_update.elapsed();
            if elapsed >= Duration::from_millis(500) {
                let fps = fps_frame_count as f64 / elapsed.as_secs_f64();
                fps_text = format!("{fps:.1}");
                fps_frame_count = 0;
                fps_last_update = Instant::now();
            }
            crate::overlay::draw_fps(&mut framebuffer, width as usize, &fps_text);
        }

        video.present(&framebuffer);

        // Frame throttling (F9 to toggle): sleep until the target presentation time.
        // Advancing next_frame_time by a fixed duration each frame
        // automatically corrects sub-millisecond drift.
        if throttle {
            let now = Instant::now();
            if next_frame_time > now {
                std::thread::sleep(next_frame_time - now);
            }
            next_frame_time += frame_duration;

            // If we've fallen more than one frame behind, reset the deadline
            // rather than burst-catching-up (which would cause choppy audio).
            if next_frame_time < Instant::now() {
                next_frame_time = Instant::now() + frame_duration;
            }
        }
    }

    // Stop audio callback before the device is dropped, avoiding an exit pop.
    if let Some((ref device, _)) = audio_state {
        device.pause();
    }
}
