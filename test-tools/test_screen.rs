extern crate unlimited;

use std::env;
use std::fs::File;

use std::io::prelude::*;

use std::io::BufReader;

//use std::io::{self, IoSlice, Write};

use std::time::Duration;

use unlimited::core::codepointinfo::CodepointInfo;
use unlimited::core::screen::*;

fn main() -> std::io::Result<()> {
    let mut buf = Vec::with_capacity(1024 * 64);
    buf.resize(buf.capacity(), 0);

    let width = 350;
    let height = 85;
    let mut screen = Screen::new(width, height);

    let mut fps = 0;
    let mut t0 = std::time::Instant::now();

    for file in env::args().skip(1) {
        println!("opening {}", file);

        let file = File::open(file).unwrap();
        let mut rbuf = BufReader::new(file);

        loop {
            let rd_sz = rbuf.read(&mut buf[..]).unwrap();
            if rd_sz == 0 {
                break;
            }

            for c in &buf[..rd_sz] {
                let mut cpi = CodepointInfo::new();
                cpi.cp = *c as char;
                cpi.displayed_cp = *c as char;
                'retry: loop {
                    let (ok, _line_index) = screen.push(cpi);
                    if !ok {
                        screen.clear();
                        fps += 1;
                        break 'retry;
                    }
                    break;
                }
            }

            let d = t0.elapsed();
            if d >= Duration::from_millis(1000) {
                println!("fps = {}", fps);
                t0 = std::time::Instant::now();
                fps = 0;
            }
        }
    }

    Ok(())
}
