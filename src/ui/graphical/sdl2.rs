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

use std::env;
use std::fs::File;
use std::io::prelude::*;
use std::io::BufReader;

use crate::core::codepointinfo::CodepointInfo;
use crate::core::screen::*;

pub type Result<T> = std::result::Result<T, ErrorKind>;
pub type ErrorKind = io::Error;

pub fn main_loop_sdl(
    ui_rx: &Receiver<EventMessage<'static>>,
    ui_tx: &Sender<EventMessage<'static>>,
    core_tx: &Sender<EventMessage<'static>>,
) -> Result<()> {
    let sdl_context = sdl2::init().unwrap();

    let video_subsystem = sdl_context.video().unwrap();

    let font_w = 16;
    let font_h = 20;

    let mut width = 90 * font_w;
    let mut height = 30 * font_h;

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
        //canvas.set_draw_color(Color::RGB(c, c, c));
        //canvas.fill_rects(&rects).unwrap();

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

///////////////////////////////////////////////////////////////////////////////////////////////////

use sdl2::video::SwapInterval;

extern crate gl;
// include the OpenGL type aliases
use gl::types::*;

#[derive(Copy, Clone, Default)]
struct Cell {
    c: char,
    real_c: char,
    offset: u64,
    attr: u32,
}

pub fn main_loop_sdl_gl(
    ui_rx: &Receiver<EventMessage<'static>>,
    ui_tx: &Sender<EventMessage<'static>>,
    core_tx: &Sender<EventMessage<'static>>,
) -> Result<()> {
    let sdl_context = sdl2::init().unwrap();

    let video_subsystem = sdl_context.video().unwrap();

    let font_w = 10;
    let font_h = 20;

    let mut width = 90 * font_w;
    let mut height = 30 * font_h;

    let window = video_subsystem
        .window("unlimitED", width, height)
        .opengl()
        .resizable()
        .position_centered()
        .build()
        .expect("could not initialize video subsystem");

    let gl_context = window.gl_create_context().unwrap();

    video_subsystem
        .gl_set_swap_interval(SwapInterval::Immediate)
        .unwrap();

    let _gl =
        gl::load_with(|s| video_subsystem.gl_get_proc_address(s) as *const std::os::raw::c_void);

    unsafe {
        gl::ClearColor(0.0, 0.0, 0.0, 0.0);
        gl::Clear(gl::COLOR_BUFFER_BIT);
    }

    let mut event_pump = sdl_context.event_pump().unwrap();

    let mut fps = 0;
    let mut t0 = std::time::Instant::now();

    let mut i = 0.0;
    let mut dir = 1;

    let mut out = Vec::with_capacity(width as usize * height as usize);

    for filename in env::args().skip(1) {
        let file = File::open(filename.clone());
        if file.is_err() {
            continue;
        }
        println!("opening {}", filename);
        let file = file.unwrap();

        let mut rbuf = BufReader::new(file);

        let mut buf = Vec::new();
        let mut flush = false;
        buf.resize(1024 * 1024 * 100, 0);

        let mut x = 0;
        let mut y = 0;
        let mut offset = 0;

        let mut tstart = std::time::Instant::now();

        'running: loop {
            for event in event_pump.poll_iter() {
                //eprintln!("sdl event {:?}", event);
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

            let rd_sz = rbuf.read(&mut buf[..]).unwrap();
            if rd_sz == 0 {
                break;
            }

            for c in &buf[..rd_sz] {
                let cell = Cell {
                    c: *c as char,
                    real_c: *c as char,
                    offset,
                    attr: 0,
                };
                out.push(cell);

                x += 1;
                if x >= width || *c == b'\n' {
                    x = 0;
                    y += 1;
                }
                if y >= height {
                    flush = true;
                }

                if flush {
                    //
                    unsafe {
                        let r = i;
                        let g = i;
                        let b = i;
                        let a = 1.0;
                        gl::ClearColor(r, g, b, a);
                        gl::Clear(gl::COLOR_BUFFER_BIT);
                    }

                    // calling a function
                    window.gl_swap_window();
                    fps += 1;

                    //
                    if dir >= 0 {
                        i += 0.01;
                        if i > 1.0 {
                            dir = -1;
                        }
                    }

                    if dir < 0 {
                        i -= 0.01;
                        if i < 0.0 {
                            dir = 1;
                        }
                    }

                    //
                    out.clear();

                    x = 0;
                    y = 0;

                    flush = false;
                }

                offset += 1;
            }

            let d = t0.elapsed();
            if d >= Duration::from_millis(1000) {
                println!("fps = {}", fps);
                t0 = std::time::Instant::now();
                fps = 0;
            }
        }

        let d = tstart.elapsed();
        println!("time to parse  = {}", d.as_millis());
    }

    println!("end");
    println!("type ctrl+c");

    Ok(())
}
