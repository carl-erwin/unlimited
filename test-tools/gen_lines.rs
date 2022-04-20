use std::env;
use std::io::BufWriter;
use std::io::{self, Write};

/// Simple line generator to test very large file indexing/line numbering
/// each line starts with a line number and is followed by an abitrary number of characters.
fn main() {
    let os_args = env::args();
    let args: Vec<_> = os_args.collect();

    if args.len() != 4 {
        println!("usage : {} [start] [number of line] [width]", args[0]);
        return;
    }

    let start_num = args[1].trim_end().parse::<u64>().unwrap_or(0);
    let stop_num = args[2].trim_end().parse::<u64>().unwrap_or(0);
    let width_num = args[3].trim_end().parse::<u64>().unwrap_or(0);

    gen_lines(start_num, stop_num, width_num);
}

fn gen_lines(start: u64, stop: u64, line_width: u64) {

    let stdout = io::stdout();
    let mut buff = BufWriter::new(stdout);

    let v = gen_line(line_width);
    let sz = v.len();
    for line_number in start..start + stop {
        let s = &v[line_number as usize % sz];
        buff.write_fmt(format_args!("{:012} {}\n", line_number, s)).unwrap();
    }
}

fn gen_line(line_width: u64) -> Vec<String> {
    let mut v = vec![];

    let table = [
        '0', '1', '2', '3', '4', '5', '6', '7', '8', '9', 'a', 'b', 'c', 'd', 'e', 'f', 'g', 'h',
        'i', 'j', 'k', 'l', 'm', 'n', 'o', 'p', 'q', 'r', 's', 't', 'u', 'v', 'w', 'x', 'y', 'z',
    ];

    for c in &table {
        let mut string = String::new();
        for _ in 0..line_width {
          string.push(*c as char);
        }
        v.push(string);
    }

    v
}
