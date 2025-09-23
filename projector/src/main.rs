// Thank you: https://github.com/dwiandhikaap/rust-raylib-gstreamer/blob/main/src/main.rs

use std::env;
use anyhow::{anyhow, Error};
use gstreamer_app::gst;
use raylib::color::Color;
use raylib::ffi::KeyboardKey;
use raylib::math::Vector2;
use raylib::prelude::RaylibDraw;
use shared::ProjectorCommand;
use crate::ipc::start_ipc_server;
use crate::video::RaylibVideo;

mod video;
mod ipc;

fn main() -> Result<(), Error> {
    gst::init()?;

    // ipc
    let mut rx = start_ipc_server();

    /*let args: Vec<String> = env::args().collect();
    if args.len() != 2 {
        return Err(anyhow!("Usage {} VIDEO_FILE_PATH", args[0]));
    }
    let path = &args[1];*/

    // raylib
    let (mut rl, thread) = raylib::init()
        .size(800, 600)
        .resizable()
        .title("SimplePresenter Projector")
        .build();

    let mut video: Option<RaylibVideo> = None;

    let mut scale = 1.0;
    let mut rotation = 0.0;
    let mut pos = Vector2::new(0.0, 0.0);

    while !rl.window_should_close() {
        if let Some(key) = rl.get_key_pressed() {
            match key {
                KeyboardKey::KEY_F11 => {
                    rl.toggle_fullscreen();
                }
                _ => {}
            }
        }

        while let Ok(cmd) = rx.try_recv() {
            match cmd {
                ProjectorCommand::Start { path, skip } => {
                    println!("Starting video, {:?} with skip {}", path, skip);

                    if let Ok(mut v) = RaylibVideo::new(&path, &mut rl, &thread) {
                        v.play();
                        video = Some(v);
                    }
                }
                _ => {}
            }
        }


        let time = rl.get_time();
        let mut d = rl.begin_drawing(&thread);
        d.clear_background(Color::BLACK);

        if let Some(ref mut v) = video {
            v.update();

            let screen_width = d.get_render_width() as f32;
            let screen_height = d.get_render_height() as f32;

            let scale_x = screen_width / v.width as f32;
            let scale_y = screen_height / v.height as f32;
            let scale = scale_x.min(scale_y);

            let draw_width = v.width as f32 * scale;
            let draw_height = v.height as f32 * scale;

            let pos_x = (screen_width - draw_width) as f64 * 0.5;
            let pos_y = (screen_height - draw_height) as f64 * 0.5;
            let pos = Vector2::new(pos_x as f32, pos_y as f32);

            d.draw_texture_ex(&v.frame_texture, pos, rotation, scale, Color::WHITE);
        }

        d.draw_fps(0, 0);
    }

    Ok(())
}
