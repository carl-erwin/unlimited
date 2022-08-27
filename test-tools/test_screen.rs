extern crate unlimited;

use std::env;
use std::fs::File;

use std::io;
use std::io::prelude::*;

use std::io::BufReader;
use std::io::BufWriter;

//use std::io::{self, IoSlice, Write};

use std::time::Duration;
use std::time::Instant;

use unlimited::core::codepointinfo::CodepointInfo;
use unlimited::core::screen::*;

fn main() -> std::io::Result<()> {
    let stdout = io::stdout();
    let stdout = stdout.lock();

    let mut wbuf = BufWriter::with_capacity(1024 * 16, stdout);

    let buf = &mut [0; 1024 * 16];

    let width = 200;
    let height = 100;
    let mut screen = Screen::new(width, height);

    let mut w = 0;
    let mut h = 0;

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

                let (ok, _line_index) = screen.push(cpi);
                if !ok {
                    screen.clear();
                    fps += 1;
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
