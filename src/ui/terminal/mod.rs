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



fn draw_buffer(buf: &Option<&Box<::core::buffer::Buffer>>,
               mut scr: &mut Screen,
               mut stdout: &mut Stdout) {
    match *buf {
        Some(ref b) => {
            match b.byte_buffer {

                Some(ref byte_buf) => {
                    fill_screen(&mut scr, &byte_buf.data);
                    draw_screen(&scr, &mut stdout);
                }
                _ => {}
            }
        }
        _ => {}
    }
}

fn terminal_clear_current_line(mut stdout: &mut Stdout, line_width: u16) {
    for _ in 0..line_width {
        write!(stdout, " ").unwrap();
    }
}

fn terminal_cursor_to(mut stdout: &mut Stdout, x: u16, y: u16) {
    write!(stdout, "{}", termion::cursor::Goto(x, y)).unwrap();
}


fn terminal_clear_screen(stdout: &mut Stdout, clear_toggle_flag: &mut bool) {
    if *clear_toggle_flag == true {
        write!(stdout, "{}", termion::clear::All).unwrap();
        stdout.flush().unwrap();
    }
    *clear_toggle_flag = false;
}

pub fn main_loop(editor: &mut Editor) {

    let mut stdout = MouseTerminal::from(io::stdout().into_raw_mode().unwrap());

    write!(stdout, "{}{}", termion::cursor::Hide, termion::clear::All).unwrap();
    stdout.flush().unwrap();

    let mut keys: Vec<Event> = Vec::new();

    let mut quit = false;
    let mut clear_toggle_flag = true;

    //
    let display_status = true;

    // select file
    let mut bid = 2;
    let mut buf = editor.buffer_map.get(&bid);
    let mut status_line_y = 0 as u16;
    let mut status = String::new();

    while !quit {
        let (width, height) = terminal_size().unwrap();
        let mut scr = Screen::new(width as usize, height as usize);

        status_line_y += 1;
        status_line_y %= height;

        draw_buffer(&buf, &mut scr, &mut stdout);
        if display_status == true {
            display_status_line(&buf, &status, status_line_y, width, &mut stdout);
        }

        for evt in stdin().events() {

            keys.push(evt.unwrap());
            let evt = keys[keys.len() - 1].clone();

            draw_buffer(&buf, &mut scr, &mut stdout);

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

                        Key::Char(c) => {
                            if c == '\n' {
                                status = format!("'{}'", "<newline>");
                            } else {
                                status = format!("'{}'", c);
                            }
                        }
                        Key::Alt(c) => status = format!("Alt-{}", c),
                        Key::Ctrl(c) => status = format!("Ctrl-{}", c),

                        Key::F(1) => {
                            if bid > 0 {
                                bid -= 1;
                            }
                            buf = editor.buffer_map.get(&bid);
                            clear_toggle_flag = true;
                            break;
                        }

                        Key::F(2) => {
                            bid = ::std::cmp::min(bid + 1, (editor.buffer_map.len() - 1) as u64);
                            buf = editor.buffer_map.get(&bid);
                            clear_toggle_flag = true;
                            break;
                        }

                        Key::F(f) => status = format!("F{:?}", f),
                        Key::Left => status = format!("<left>"),
                        Key::Right => status = format!("<right>"),
                        Key::Up => status = format!("<up>"),
                        Key::Down => status = format!("<down>"),
                        Key::Backspace => status = format!("<backspace>"),
                        Key::Home => status = format!("<Home>"),
                        Key::End => status = format!("<End>"),
                        Key::PageUp => status = format!("<PageUp>"),
                        Key::PageDown => status = format!("<PageDown>"),
                        Key::Delete => status = format!("<Delete>"),
                        Key::Insert => status = format!("<Insert>"),
                        Key::Esc => status = format!("<Esc>"),
                        _ => status = format!("Other"),
                    }
                }

                Event::Mouse(m) => {
                    match m {
                        MouseEvent::Press(mb, x, y) => {
                            status = format!("MouseEvent::Press => MouseButton {:?} @ ({}, {})",
                                             mb,
                                             x,
                                             y);
                        }

                        MouseEvent::Release(x, y) => {
                            status = format!("MouseEvent::Release => @ ({}, {})", x, y);
                        }

                        MouseEvent::Hold(x, y) => {
                            status = format!("MouseEvent::Hold => @ ({}, {})", x, y);
                        }
                    };
                }

                Event::Unsupported(_) => {}

            }

            if display_status == true {
                display_status_line(&buf, &status, status_line_y, width, &mut stdout);
            }
            break;
        }
    }

    // quit
    // clear, restore cursor
    write!(stdout, "{}{}", termion::clear::All, termion::cursor::Show).unwrap();
    stdout.flush().unwrap();
}


fn display_status_line(buf: &Option<&Box<::core::buffer::Buffer>>,
                       status: &str,
                       line: u16,
                       width: u16,
                       mut stdout: &mut Stdout) {
    // select/clear last line
    let name = match *buf {
        Some(ref b) => b.name.as_str(),
        None => "",
    };

    let file_name = match *buf {
        Some(ref b) => {
            match b.byte_buffer {
                Some(ref bb) => bb.file_name.as_str(),
                None => "",
            }
        }
        None => "",
    };


    terminal_cursor_to(&mut stdout, 1, line);
    terminal_clear_current_line(&mut stdout, width);
    terminal_cursor_to(&mut stdout, 1, line);

    let status_str = format!("line {} buffer_name '{}', file: '{}', event '{}'",
                             line,
                             name,
                             file_name,
                             status);

    print!("{}", status_str);
    stdout.flush().unwrap();
}
