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

    return string;
}
