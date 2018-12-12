// Copyright (c) Carl-Erwin Griffith
//
// Permission is hereby granted, free of charge, to any
// person obtaining a copy of this software and associated
// documentation files (the "Software"), to deal in the
// Software without restriction, including without
// limitation the rights to use, copy, modify, merge,
// publish, distribute, sublicense, and/or sell copies of
// the Software, and to permit persons to whom the Software
// is furnished to do so, subject to the following
// conditions:
//
// The above copyright notice and this permission notice
// shall be included in all copies or substantial portions
// of the Software.
//
// THE SOFTWARE IS PROVIDED "AS IS", WITHOUT WARRANTY OF
// ANY KIND, EXPRESS OR IMPLIED, INCLUDING BUT NOT LIMITED
// TO THE WARRANTIES OF MERCHANTABILITY, FITNESS FOR A
// PARTICULAR PURPOSE AND NONINFRINGEMENT. IN NO EVENT
// SHALL THE AUTHORS OR COPYRIGHT HOLDERS BE LIABLE FOR ANY
// CLAIM, DAMAGES OR OTHER LIABILITY, WHETHER IN AN ACTION
// OF CONTRACT, TORT OR OTHERWISE, ARISING FROM, OUT OF OR
// IN CONNECTION WITH THE SOFTWARE OR THE USE OR OTHER

use std::env;
use std::io::BufWriter;
use std::io::{self, Write};

fn main() {
    let os_args = env::args();
    let args: Vec<_> = os_args.collect();

    if args.len() != 4 {
        println!("usage : {} start numline width", args[0]);
        return;
    }

    let start_num = args[1].trim_right().parse::<u64>().unwrap_or(0);
    let stop_num = args[2].trim_right().parse::<u64>().unwrap_or(0);
    let width_num = args[3].trim_right().parse::<u64>().unwrap_or(0);

    gen_lines(start_num, stop_num, width_num);
}

fn gen_lines(start: u64, stop: u64, linewidth: u64) -> () {
    let string = gen_line(linewidth);


    let stdout = io::stdout();
    let mut buff = BufWriter::new(stdout);
    for x in start..start + stop + 1 {
        buff.write_fmt(format_args!("{:012} {}", x, string)).unwrap();
    }
}

fn gen_line(linewidth: u64) -> String {
    let mut string = String::new();

    let table = ['0', '1', '2', '3', '4', '5', '6', '7', '8', '9', 'a', 'b', 'c', 'd', 'e', 'f',
                 'g', 'h', 'i', 'j', 'k', 'l', 'm', 'n', 'o', 'p', 'q', 'r', 's', 't', 'u', 'v',
                 'w', 'x', 'y', 'z'];

    for x in 0..linewidth {
        string.push(table[x as usize % table.len()]);
    }
    string.push('\n');

    string
}
