extern crate termion;

use self::termion::event::Key;
use self::termion::input::TermRead;
use self::termion::raw::IntoRawMode;

use std::io::{Write, stdout, stdin};



pub fn main_loop() {

    let mut stdout = stdout().into_raw_mode().unwrap();

    write!(stdout,
           "{}{}",
           termion::cursor::Goto(1, 1),
           termion::clear::All)
        .unwrap();
    stdout.flush().unwrap();

    let mut quit = false;
    while !quit {
        let stdin = stdin();
        for c in stdin.keys() {

            write!(stdout,
                   "{}{}",
                   termion::cursor::Goto(1, 1),
                   termion::clear::All)
                .unwrap();

            // Print the key we type...
            match c.unwrap() {
                // Exit.
                Key::Char('q') => {
                    quit = true;
                    break;
                }
                Key::Char(c) => println!("{}", c),
                Key::Alt(c) => println!("Alt-{}", c),
                Key::Ctrl(c) => println!("Ctrl-{}", c),
                Key::Left => println!("<left>"),
                Key::Right => println!("<right>"),
                Key::Up => println!("<up>"),
                Key::Down => println!("<down>"),
                _ => println!("Other"),
            }

            // Flush again.
            stdout.flush().unwrap();
        }
    }
}
