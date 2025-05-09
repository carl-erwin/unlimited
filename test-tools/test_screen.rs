extern crate unlimited;

use std::env;
use std::fs::File;

use std::io::prelude::*;

use std::io::BufReader;

//use std::io::{self, IoSlice, Write};

use std::time::Duration;

use unlimited::core::codepointinfo::CodepointInfo;
use unlimited::core::screen::*;

extern crate unicode_width;
use unicode_width::UnicodeWidthChar;

struct Cell {
    pub c: char,
    pub real_c: char,
    pub attr: u32,
    pub fg: u32,
    pub bg: u32,
    pub off: Option<u64>,
    pub size: u8,
    pub cw: u8,
}

fn main() -> std::io::Result<()> {
    let os_args = env::args();
    let args: Vec<_> = os_args.collect();

    if args.len() != 4 {
        println!("usage : {} [width] [height] [file]", args[0]);
        return Ok(());
    }

    let width = args[1].trim_end().parse::<usize>().unwrap_or(128);
    let height = args[2].trim_end().parse::<usize>().unwrap_or(32);

    let mut buf = Vec::with_capacity(1024 * 32);
    buf.resize(buf.capacity(), 0);

    let mut out_buf: Vec<Cell> = Vec::with_capacity(width * height);
    let mut screen = Screen::new(width, height);

    let mut fps = 0;
    let mut t0 = std::time::Instant::now();

    println!("screen dimension {}x{}", width, height);

    for file in env::args().skip(3) {
        println!("opening {}", file);

        let file = File::open(file).unwrap();
        let mut rbuf = BufReader::new(file);
        let mut count: u64 = 0;
        let mut total_frame: u64 = 0;

        loop {
            let rd_sz = rbuf.read(&mut buf[..]).unwrap();
            if rd_sz == 0 {
                break;
            }

            for c in &buf[..rd_sz] {
                if true {
                    if out_buf.len() == out_buf.capacity() {
                        out_buf.clear();
                        fps += 1;
                        total_frame += 1;
                    }

                    count += 1;

                    let cw = UnicodeWidthChar::width(*c as char).unwrap_or(1) as u8;

                    let cell = Cell {
                        c: *c as char,
                        real_c: (*c + 1) as char,
                        attr: *c as u32,
                        fg: (*c * 3) as u32,
                        bg: (*c * 4) as u32,
                        off: Some(count),
                        size: 1,
                        cw,
                    };

                    out_buf.push(cell);
                    continue;
                } else {
                    let mut cpi = CodepointInfo::new();
                    cpi.cp = *c as char;
                    cpi.displayed_cp = *c as char;
                    'retry: loop {
                        let (ok, _line_index) = screen.push(&cpi);
                        if !ok {
                            screen.clear();
                            fps += 1;
                            break 'retry;
                        }
                        break;
                    }
                    count += 1;
                }
            }

            let d = t0.elapsed();
            if d >= Duration::from_millis(1000) {
                println!("fps = {}", fps);
                t0 = std::time::Instant::now();
                fps = 0;
            }
        }

        println!("push count = {}", count);
        println!("total_frame = {}", total_frame);
    }

    Ok(())
}
