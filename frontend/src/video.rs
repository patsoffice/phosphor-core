use sdl2::pixels::PixelFormatEnum;
use sdl2::render::{Canvas, TextureCreator};
use sdl2::video::{Window, WindowContext};

pub struct Video {
    canvas: Canvas<Window>,
    texture_creator: TextureCreator<WindowContext>,
    width: u32,
    height: u32,
}

impl Video {
    /// Create an SDL window and renderer for the given native resolution.
    pub fn new(
        sdl_video: &sdl2::VideoSubsystem,
        title: &str,
        native_width: u32,
        native_height: u32,
        scale: u32,
    ) -> Self {
        let window = sdl_video
            .window(title, native_width * scale, native_height * scale)
            .position_centered()
            .build()
            .expect("Failed to create window");

        let canvas = window
            .into_canvas()
            .accelerated()
            .build()
            .expect("Failed to create canvas");

        let texture_creator = canvas.texture_creator();

        Self {
            canvas,
            texture_creator,
            width: native_width,
            height: native_height,
        }
    }

    /// Upload an RGB24 framebuffer to the texture and present it.
    pub fn present(&mut self, framebuffer: &[u8]) {
        let mut texture = self
            .texture_creator
            .create_texture_streaming(PixelFormatEnum::RGB24, self.width, self.height)
            .expect("Failed to create texture");

        texture
            .update(None, framebuffer, (self.width * 3) as usize)
            .expect("Failed to update texture");

        self.canvas.clear();
        self.canvas
            .copy(&texture, None, None)
            .expect("Failed to copy texture");
        self.canvas.present();
    }
}
