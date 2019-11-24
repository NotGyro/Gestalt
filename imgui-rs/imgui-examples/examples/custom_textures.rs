use std::borrow::Cow;
use std::error::Error;
use std::io::Cursor;
use std::rc::Rc;

use glium::{
    backend::Facade,
    texture::{ClientFormat, RawImage2d},
    Texture2d,
};
use image::{jpeg::JPEGDecoder, ImageDecoder};
use imgui::*;

mod support;

#[derive(Default)]
struct CustomTexturesApp {
    my_texture_id: Option<TextureId>,
    lenna: Option<Lenna>,
}

struct Lenna {
    texture_id: TextureId,
    size: [f32; 2],
}

impl CustomTexturesApp {
    fn register_textures<F>(
        &mut self,
        gl_ctx: &F,
        textures: &mut Textures<Rc<Texture2d>>,
    ) -> Result<(), Box<dyn Error>>
    where
        F: Facade,
    {
        const WIDTH: usize = 100;
        const HEIGHT: usize = 100;

        if self.my_texture_id.is_none() {
            // Generate dummy texture
            let mut data = Vec::with_capacity(WIDTH * HEIGHT);
            for i in 0..WIDTH {
                for j in 0..HEIGHT {
                    // Insert RGB values
                    data.push(i as u8);
                    data.push(j as u8);
                    data.push((i + j) as u8);
                }
            }

            let raw = RawImage2d {
                data: Cow::Owned(data),
                width: WIDTH as u32,
                height: HEIGHT as u32,
                format: ClientFormat::U8U8U8,
            };
            let gl_texture = Texture2d::new(gl_ctx, raw)?;
            let texture_id = textures.insert(Rc::new(gl_texture));

            self.my_texture_id = Some(texture_id);
        }

        if self.lenna.is_none() {
            self.lenna = Some(Lenna::new(gl_ctx, textures)?);
        }

        Ok(())
    }

    fn show_textures(&self, ui: &Ui) {
        Window::new(im_str!("Hello textures"))
            .size([400.0, 600.0], Condition::FirstUseEver)
            .build(ui, || {
                ui.text(im_str!("Hello textures!"));
                if let Some(my_texture_id) = self.my_texture_id {
                    ui.text("Some generated texture");
                    Image::new(my_texture_id, [100.0, 100.0]).build(ui);
                }

                if let Some(lenna) = &self.lenna {
                    ui.text("Say hello to Lenna.jpg");
                    lenna.show(ui);
                }
            });
    }
}

impl Lenna {
    fn new<F>(gl_ctx: &F, textures: &mut Textures<Rc<Texture2d>>) -> Result<Self, Box<dyn Error>>
    where
        F: Facade,
    {
        let lenna_bytes = include_bytes!("../../resources/Lenna.jpg");
        let byte_stream = Cursor::new(lenna_bytes.as_ref());
        let decoder = JPEGDecoder::new(byte_stream)?;

        let (width, height) = decoder.dimensions();
        let image = decoder.read_image()?;
        let raw = RawImage2d {
            data: Cow::Owned(image),
            width: width as u32,
            height: height as u32,
            format: ClientFormat::U8U8U8,
        };
        let gl_texture = Texture2d::new(gl_ctx, raw)?;
        let texture_id = textures.insert(Rc::new(gl_texture));
        Ok(Lenna {
            texture_id,
            size: [width as f32, height as f32],
        })
    }

    fn show(&self, ui: &Ui) {
        Image::new(self.texture_id, self.size).build(ui);
    }
}

fn main() {
    let mut my_app = CustomTexturesApp::default();

    let mut system = support::init(file!());
    my_app
        .register_textures(system.display.get_context(), system.renderer.textures())
        .expect("Failed to register textures");
    system.main_loop(|_, ui| my_app.show_textures(ui));
}
