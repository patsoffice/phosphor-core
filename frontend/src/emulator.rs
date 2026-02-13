use phosphor_core::core::machine::Machine;
use sdl2::event::Event;
use sdl2::keyboard::Scancode;

use crate::input::KeyMap;
use crate::video::Video;

pub fn run(machine: &mut dyn Machine, key_map: &KeyMap, scale: u32) {
    let sdl_context = sdl2::init().expect("Failed to initialize SDL2");
    let sdl_video = sdl_context.video().expect("Failed to init SDL video");

    let (width, height) = machine.display_size();
    let mut video = Video::new(&sdl_video, "Phosphor Emulator", width, height, scale);
    let mut event_pump = sdl_context.event_pump().expect("Failed to get event pump");

    let buffer_size = (width * height * 3) as usize;
    let mut framebuffer = vec![0u8; buffer_size];

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

        // Render the framebuffer and present
        machine.render_frame(&mut framebuffer);
        video.present(&framebuffer);

        // Frame timing handled by VSync (set in Video::new via present_vsync)
    }
}
