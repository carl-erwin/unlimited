//
extern crate termion;

//
use self::termion::event::{Event, Key, MouseEvent};
use self::termion::input::{TermRead, MouseTerminal};
use self::termion::raw::IntoRawMode;
use self::termion::terminal_size;

use std::io::{self, Write, stdin, Stdout};

//
use core::screen::Screen;
use core::codepointinfo::CodepointInfo;

use core::editor::Editor;

fn fill_screen(screen: &mut Screen, data: &[u8]) {

    screen.clear();

    let mut offset = 0;
    for c in data {

        let mut displayed_cp: char = *c as char;
        if *c as char == '\n' as char {
            displayed_cp = ' ';
        }

        let cpi = CodepointInfo {
            cp: *c as char,
            displayed_cp: displayed_cp,
            offset: offset,
        };

        let (ok, _) = screen.push(cpi);
        offset += 1;
        if ok == false {
            break;
        }
    }
}


fn draw_screen(screen: &Screen, stdout: &mut Stdout) {

    write!(stdout, "{}", termion::cursor::Goto(1, 1)).unwrap();

    for l in 0..screen.height {
        let line = &screen.line[l];
        if line.used == 0 {
            break;
        }

        for c in 0..line.width {
            let cpi = &line.chars[c];
            write!(stdout, "{}", cpi.displayed_cp).unwrap();
        }

        if l < screen.height - 1 {
            write!(stdout, "\r\n").unwrap();
        }
    }

    stdout.flush().unwrap();
}



fn draw_buffer(buf: &Option<&Box<::core::buffer::Buffer>>, mut scr: &mut Screen, mut stdout: &mut Stdout)
{
     match *buf {
            Some(ref b) => {
                match b.byte_buffer {

                    Some(ref byte_buf) => {
                        fill_screen(&mut scr, &byte_buf.data);
                        draw_screen(&scr, &mut stdout);
                    }
                    _ => {},                    
                }
            }
            _ => {},
        }
}

pub fn main_loop(editor: &mut Editor) {

    let mut stdout = MouseTerminal::from(io::stdout().into_raw_mode().unwrap());

    write!(stdout, "{}{}", termion::cursor::Hide, termion::clear::All).unwrap();

    stdout.flush().unwrap();

    let mut keys: Vec<Event> = Vec::new();

    let mut quit = false;
    let mut clear_screen = true;

    // select file
    let mut bid = 2;
    let mut buf = editor.buffer_map.get(&bid);
    
    while !quit {
        let (width, height) = terminal_size().unwrap();
        let mut scr = Screen::new(width as usize, height as usize);

        if clear_screen == true {            
            clear_screen = false;
            write!(stdout, "{}", termion::clear::All).unwrap();
            stdout.flush().unwrap();
        }

        draw_buffer(&buf, &mut scr, &mut stdout);
        for evt in stdin().events() {

            draw_buffer(&buf, &mut scr, &mut stdout);

            write!(stdout, "{}", termion::cursor::Goto(1, 1)).unwrap();

            keys.push(evt.unwrap());

            let evt = keys[keys.len() - 1].clone();

            // Print recieved Events...
            match evt {

                Event::Key(k) => {
                    match k {
                        // Exit.
                        Key::Ctrl('r') => {
                            keys.clear();
                        }

                        Key::Ctrl('c') => {
                            if keys.len() > 1 {
                                if let Event::Key(prev_event) = keys[keys.len() - 2] {
                                    if let Key::Ctrl(prev_char) = prev_event {
                                        if prev_char == 'x' {
                                            quit = true;
                                            break;
                                        }
                                    }
                                }
                            }
                        }

                        Key::Char(c) => print!("'{}'", c),
                        Key::Alt(c) => print!("Alt-{}", c),
                        Key::Ctrl(c) => print!("Ctrl-{}", c),

                        Key::F(1) => {
                            if bid > 0 {
                                bid -= 1;
                            }
                            buf = editor.buffer_map.get(&bid);
                            clear_screen = true;
                            break
                        },

                        Key::F(2) => {
                            bid += 1;
                            buf = editor.buffer_map.get(&bid);
                            clear_screen = true;
                            break
                        },


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
            print!("        ");
            stdout.flush().unwrap();
        }
    }

    write!(stdout, "{}{}", termion::clear::All, termion::cursor::Show).unwrap();
    stdout.flush().unwrap();
}
