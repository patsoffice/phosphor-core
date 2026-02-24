use std::path::Path;
use std::time::{Duration, Instant};

use phosphor_core::core::machine::Machine;
use sdl2::event::Event;
use sdl2::keyboard::Scancode;

use crate::debug_ui::{self, DebugState, RunMode};
use crate::input::{self, ControllerMap, KeyMap};
use crate::video::Video;

pub fn run(
    machine: &mut dyn Machine,
    key_map: &KeyMap,
    controller_map: &ControllerMap,
    scale: u32,
    save_path: &Path,
) {
    // Enable controller backends before SDL init — needed for Xbox on macOS
    sdl2::hint::set("SDL_JOYSTICK_HIDAPI", "1");
    sdl2::hint::set("SDL_JOYSTICK_HIDAPI_XBOX", "1");
    sdl2::hint::set("SDL_JOYSTICK_MFI", "1");

    let sdl_context = sdl2::init().expect("Failed to initialize SDL2");
    let sdl_video = sdl_context.video().expect("Failed to init SDL video");
    let sdl_audio = sdl_context.audio().expect("Failed to init SDL audio");

    // Initialize game controller and joystick subsystems for joypad support
    let controller_subsystem = sdl_context
        .game_controller()
        .expect("Failed to init SDL game controller");
    let joystick_subsystem = sdl_context.joystick().expect("Failed to init SDL joystick");

    // Load community controller database (gamecontrollerdb.txt) if present.
    // Download from: https://github.com/mdqinc/SDL_GameControllerDB
    let db_paths: Vec<std::path::PathBuf> = {
        let mut paths = vec![std::path::PathBuf::from("gamecontrollerdb.txt")];
        if let Some(home) = std::env::var_os("HOME") {
            paths.push(std::path::Path::new(&home).join(".config/phosphor/gamecontrollerdb.txt"));
        }
        paths
    };
    for path in &db_paths {
        if path.exists() {
            match controller_subsystem.load_mappings(path) {
                Ok(n) => eprintln!("Loaded {n} controller mappings from {}", path.display()),
                Err(e) => eprintln!("Failed to load {}: {e}", path.display()),
            }
        }
    }
    let mut controllers: Vec<sdl2::controller::GameController> = Vec::new();
    let num_joysticks = joystick_subsystem.num_joysticks().unwrap_or(0);
    if num_joysticks == 0 {
        eprintln!("No joysticks detected");
    }
    for i in 0..num_joysticks {
        let name = joystick_subsystem
            .name_for_index(i)
            .unwrap_or_else(|_| "unknown".into());
        if controller_subsystem.is_game_controller(i) {
            if let Ok(gc) = controller_subsystem.open(i) {
                eprintln!("Controller {i}: {}", gc.name());
                controllers.push(gc);
            } else {
                eprintln!("Controller {i}: {name} (failed to open)");
            }
        } else {
            eprintln!("Joystick {i}: {name} (not in controller database)");
        }
    }

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
    let mut last_render_time = Instant::now();

    // FPS overlay state (F10 to toggle)
    let mut show_fps = false;
    let mut fps_text = String::new();
    let mut fps_smoothed: f64 = machine.frame_rate_hz();
    let mut fps_last_instant = Instant::now();

    // Mouse grab for trackball games (F11 to toggle)
    let has_analog = !machine.analog_map().is_empty();
    let analog_axes: Vec<u8> = machine.analog_map().iter().map(|a| a.id).collect();
    let mut mouse_grabbed = false;
    if has_analog {
        sdl_context.mouse().set_relative_mouse_mode(true);
        mouse_grabbed = true;
    }

    // Debug state
    let has_debug = machine.as_debuggable().is_some();
    let mut debug_state = DebugState::new();
    if let Some(dbg) = machine.as_debuggable() {
        debug_state.cpu_count = dbg.debug_cpu_count();
        debug_state.cpu_name = dbg.debug_cpu_name().to_string();
    }
    let mut prev_selected_cpu: usize = 0;

    'main: loop {
        // Poll all pending SDL events, translate to machine input
        for event in event_pump.poll_iter() {
            // Forward every event to egui first
            video.process_event(event.clone());

            match event {
                Event::Quit { .. } => break 'main,

                Event::KeyDown {
                    scancode: Some(Scancode::Escape),
                    ..
                } => break 'main,

                // F1: Toggle debug mode
                Event::KeyDown {
                    scancode: Some(Scancode::F1),
                    repeat: false,
                    ..
                } => {
                    if has_debug {
                        debug_state.active = !debug_state.active;
                        if debug_state.active {
                            video.resize_window(width * scale + 240, height * scale);
                            debug_state.run_mode = RunMode::Paused;
                            if let Some(dbg) = machine.as_debuggable() {
                                debug_state.refresh(dbg);
                            }
                        } else {
                            video.resize_window(width * scale, height * scale);
                            debug_state.run_mode = RunMode::Running;
                        }
                    }
                }

                // F2: Step instruction (debug + paused)
                Event::KeyDown {
                    scancode: Some(Scancode::F2),
                    repeat: false,
                    ..
                } => {
                    if debug_state.active && debug_state.run_mode == RunMode::Paused {
                        debug_state.run_mode = RunMode::StepInstruction;
                    }
                }

                // F3: Step cycle (debug + paused)
                Event::KeyDown {
                    scancode: Some(Scancode::F3),
                    repeat: false,
                    ..
                } => {
                    if debug_state.active && debug_state.run_mode == RunMode::Paused {
                        debug_state.run_mode = RunMode::StepCycle;
                    }
                }

                // F4: Continue (resume running)
                Event::KeyDown {
                    scancode: Some(Scancode::F4),
                    repeat: false,
                    ..
                } => {
                    if debug_state.active {
                        debug_state.run_mode = RunMode::Running;
                    }
                }

                Event::KeyDown {
                    scancode: Some(Scancode::F5),
                    repeat: false,
                    ..
                } => {
                    machine.reset();
                }

                // Quick Save (F6)
                Event::KeyDown {
                    scancode: Some(Scancode::F6),
                    repeat: false,
                    ..
                } => {
                    if let Some(data) = machine.save_state() {
                        match std::fs::write(save_path, &data) {
                            Ok(()) => eprintln!("Save state written ({} bytes)", data.len()),
                            Err(e) => eprintln!("Save state failed: {e}"),
                        }
                    } else {
                        eprintln!("Save states not supported for this machine");
                    }
                }

                // Quick Load (F7)
                Event::KeyDown {
                    scancode: Some(Scancode::F7),
                    repeat: false,
                    ..
                } => match std::fs::read(save_path) {
                    Ok(data) => match machine.load_state(&data) {
                        Ok(()) => eprintln!("Save state loaded"),
                        Err(e) => eprintln!("Load state failed: {e}"),
                    },
                    Err(e) => eprintln!("No save file found: {e}"),
                },

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
                    fps_smoothed = machine.frame_rate_hz();
                    fps_last_instant = Instant::now();
                }

                // Mouse grab toggle (F11)
                Event::KeyDown {
                    scancode: Some(Scancode::F11),
                    repeat: false,
                    ..
                } => {
                    mouse_grabbed = !mouse_grabbed;
                    sdl_context.mouse().set_relative_mouse_mode(mouse_grabbed);
                }

                // Keyboard input — only pass to game if egui doesn't want it
                Event::KeyDown {
                    scancode: Some(sc),
                    repeat: false,
                    ..
                } => {
                    if !video.wants_keyboard()
                        && let Some(button_id) = key_map.get(sc)
                    {
                        machine.set_input(button_id, true);
                    }
                }

                Event::KeyUp {
                    scancode: Some(sc), ..
                } => {
                    if !video.wants_keyboard()
                        && let Some(button_id) = key_map.get(sc)
                    {
                        machine.set_input(button_id, false);
                    }
                }

                // Game controller button press/release (egui never intercepts these)
                Event::ControllerButtonDown { button, .. } => {
                    if let Some(button_id) = controller_map.get_button(button) {
                        machine.set_input(button_id, true);
                    }
                }

                Event::ControllerButtonUp { button, .. } => {
                    if let Some(button_id) = controller_map.get_button(button) {
                        machine.set_input(button_id, false);
                    }
                }

                // Game controller analog stick → digital directions
                Event::ControllerAxisMotion { axis, value, .. } => {
                    for (button_id, pressed) in controller_map.axis_to_digital(axis, value) {
                        machine.set_input(button_id, pressed);
                    }
                }

                // Controller hotplug
                Event::ControllerDeviceAdded { which, .. } => {
                    if let Ok(gc) = controller_subsystem.open(which) {
                        eprintln!("Controller connected: {}", gc.name());
                        controllers.push(gc);
                    }
                }

                Event::ControllerDeviceRemoved { which, .. } => {
                    controllers.retain(|c| c.instance_id() != which);
                    eprintln!("Controller disconnected");
                }

                // Mouse motion → analog axes (trackball games)
                Event::MouseMotion { xrel, yrel, .. } => {
                    if !video.wants_pointer() && mouse_grabbed && analog_axes.len() >= 2 {
                        machine.set_analog(analog_axes[0], xrel);
                        machine.set_analog(analog_axes[1], yrel);
                    }
                }

                // Mouse buttons → fire (trackball games)
                Event::MouseButtonDown { mouse_btn, .. } => {
                    if !video.wants_pointer()
                        && mouse_grabbed
                        && let Some(id) =
                            input::mouse_button_to_input(machine.input_map(), mouse_btn)
                    {
                        machine.set_input(id, true);
                    }
                }

                Event::MouseButtonUp { mouse_btn, .. } => {
                    if !video.wants_pointer()
                        && mouse_grabbed
                        && let Some(id) =
                            input::mouse_button_to_input(machine.input_map(), mouse_btn)
                    {
                        machine.set_input(id, false);
                    }
                }

                _ => {}
            }
        }

        // Sync CPU selection if changed via debug panel
        if debug_state.selected_cpu != prev_selected_cpu {
            if let Some(dbg) = machine.as_debuggable() {
                dbg.debug_select_cpu(debug_state.selected_cpu);
                debug_state.refresh(dbg);
            }
            prev_selected_cpu = debug_state.selected_cpu;
        }

        // Execute based on debug state
        let frame_executed = debug_ui::execute_frame(machine, &mut debug_state);

        // Drain audio samples only when a full frame was executed
        if frame_executed && let Some((ref device, ref ring, _)) = audio_state {
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

        // Render: always render when paused (to show debug UI), otherwise respect throttle
        let should_render = throttle
            || debug_state.run_mode == RunMode::Paused
            || last_render_time.elapsed() >= frame_duration;

        if should_render {
            machine.render_frame(&mut framebuffer);

            // FPS overlay onto framebuffer (only when debug panel is not active)
            if show_fps && !debug_state.active {
                crate::overlay::draw_fps(&mut framebuffer, width as usize, &fps_text);
            }

            video.update_game_texture(&framebuffer);

            if debug_state.active {
                video.present_with_debug(|ctx, tex_id, native_size| {
                    debug_ui::draw_debug_ui(ctx, tex_id, native_size, &mut debug_state);
                });
            } else {
                video.present_game_only();
            }
            last_render_time = Instant::now();
        }

        // FPS: exponential moving average (α = 0.05) for a stable readout
        if show_fps {
            let now = Instant::now();
            let dt = now.duration_since(fps_last_instant).as_secs_f64();
            fps_last_instant = now;
            if dt > 0.0 {
                let instant_fps = 1.0 / dt;
                fps_smoothed += 0.05 * (instant_fps - fps_smoothed);
                fps_text = format!("{fps_smoothed:.1}");
            }
        }

        // Frame throttling
        if debug_state.run_mode == RunMode::Paused {
            // When paused, sleep to keep UI responsive without burning CPU
            std::thread::sleep(Duration::from_millis(16));
        } else if throttle {
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

    // Signal fade-out, wait for the ramp to complete, then stop the callback.
    if let Some((ref device, _, ref fade_out)) = audio_state {
        fade_out.store(true, std::sync::atomic::Ordering::Relaxed);
        std::thread::sleep(crate::audio::fade_out_duration());
        device.pause();
    }
}
