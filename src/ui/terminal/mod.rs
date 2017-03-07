extern crate termion;

use self::termion::event::{Event, Key, MouseEvent};
use self::termion::input::{TermRead, MouseTerminal};
use self::termion::raw::IntoRawMode;
use self::termion::terminal_size;

use std::io::{self, Write, stdin};

pub fn main_loop() {

    let mut stdout = MouseTerminal::from(io::stdout().into_raw_mode().unwrap());

    write!(stdout,
           "{}{}",
           termion::cursor::Goto(1, 1),
           termion::clear::All)
        .unwrap();

    stdout.flush().unwrap();

    let mut quit = false;
    while !quit {
        let stdin = stdin();

        for evt in stdin.events() {
            let evt = evt.unwrap();

            write!(stdout,
                   "{}{}",
                   termion::clear::All,
                   termion::cursor::Goto(1, 1))
                .unwrap();

            let (width, height) = terminal_size().unwrap();

            println!("terminal size ({},{})\r", width, height);

            write!(stdout, "{}", termion::cursor::Goto(1, 2)).unwrap();

            // Print recieved Events...
            match evt {

                Event::Key(k) => {
                    match k {
                        // Exit.
                        Key::Char('q') => {
                            quit = true;
                            break;
                        }
                        Key::Char(c) => print!("{}", c),
                        Key::Alt(c) => print!("Alt-{}", c),
                        Key::Ctrl(c) => print!("Ctrl-{}", c),
                        Key::F(f) => print!("F{:?}", f),
                        Key::Left => print!("<left>"),
                        Key::Right => print!("<right>"),
                        Key::Up => print!("<up>"),
                        Key::Down => print!("<down>"),
                        Key::Backspace => print!("<backspace>"),
                        Key::Home => print!("<Home>"),
                        Key::End => print!("<End>"),
                        Key::PageUp => print!("<PageUp>"),
                        Key::PageDown => print!("<PageDown>"),
                        Key::Delete => print!("<Delete>"),
                        Key::Insert => print!("<Insert>"),
                        Key::Esc => print!("<Esc>"),

                        _ => print!("Other"),
                    }
                }

                Event::Mouse(m) => {
                    match m {
                        MouseEvent::Press(mb, x, y) => {
                            print!("MouseEvent::Press => MouseButton {:?} @ ({}, {})", mb, x, y);
                        }

                        MouseEvent::Release(x, y) => {
                            print!("MouseEvent::Release => @ ({}, {})", x, y);
                        }

                        MouseEvent::Hold(x, y) => {
                            print!("MouseEvent::Hold => @ ({}, {})", x, y);
                        }
                    }
                }

                Event::Unsupported(_) => {}
            }

            // Flush again.
            stdout.flush().unwrap();
        }
    }
}
