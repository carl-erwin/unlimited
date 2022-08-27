use std::io;
use std::time::Duration;

use sdl2::event::Event;
use sdl2::event::WindowEvent::SizeChanged;
use sdl2::keyboard::Keycode;
use sdl2::pixels::Color;
use sdl2::rect::Point;
use sdl2::rect::Rect;

use crate::core::event::EventMessage;
use std::sync::mpsc::Receiver;
use std::sync::mpsc::Sender;

pub type Result<T> = std::result::Result<T, ErrorKind>;
pub type ErrorKind = io::Error;

pub fn main_loop(
    ui_rx: &Receiver<EventMessage<'static>>,
    ui_tx: &Sender<EventMessage<'static>>,
    core_tx: &Sender<EventMessage<'static>>,
) -> Result<()> {
    let sdl_context = sdl2::init().unwrap();

    let video_subsystem = sdl_context.video().unwrap();

    let mut width = 3000;
    let mut height = 1800;

    let window = video_subsystem
        .window("unlimitED", width, height)
        .resizable()
        .position_centered()
        .build()
        .expect("could not initialize video subsystem");

    let mut canvas = window
        .into_canvas()
        .build()
        .expect("could not make a canvas");

    canvas.set_draw_color(Color::RGB(0, 0, 0));
    canvas.clear();
    canvas.present();

    let mut event_pump = sdl_context.event_pump().unwrap();

    let mut fps = 0;
    let mut t0 = std::time::Instant::now();

    let font_w = 16;
    let font_h = 20;

    let max_rects = width / font_w * height / font_h;
    let mut rects = Vec::with_capacity(max_rects as usize);

    let mut i = 0;
    'running: loop {
        //
        for event in event_pump.poll_iter() {
            eprintln!("sdl event {:?}", event);
            match event {
                Event::Quit { .. }
                | Event::KeyDown {
                    keycode: Some(Keycode::Escape),
                    ..
                } => {
                    break 'running;
                }

                Event::Window {
                    win_event: SizeChanged(w, h),
                    ..
                } => {
                    width = w as u32;
                    height = h as u32;
                }

                _ => {}
            }
        }

        //

        rects.clear();
        for y in (0..height).step_by(font_h as usize) {
            for x in (0..width).step_by(font_w as usize) {
                let rect = Rect::new(x as i32, y as i32, (font_w - 1) as u32, (font_h - 1) as u32);
                rects.push(rect);
            }
        }

        canvas.set_draw_color(Color::RGB(255, 255, 255));
        canvas.clear();

        //canvas.set_draw_color(Color::RGB(101, 130, 143));

        let c = i % 255;
        canvas.set_draw_color(Color::RGB(c, c, c));
        canvas.fill_rects(&rects).unwrap();

        // The rest of the game loop goes here...
        canvas.present();
        fps += 1;
        i += 1;
        if i == 255 {
            i = 0
        };

        //let wait = std::time::Duration::from_millis(1);
        // std::thread::sleep(wait);

        let d = t0.elapsed();
        if d >= Duration::from_millis(1000) {
            println!("fps = {}", fps);
            t0 = std::time::Instant::now();
            fps = 0;
        }
    }

    Ok(())
}
