//
extern crate termion;


//
use self::termion::screen::AlternateScreen;
use self::termion::event::{Event, Key, MouseEvent};
use self::termion::input::{TermRead, MouseTerminal};
use self::termion::raw::IntoRawMode;
use self::termion::terminal_size;

use std::io::{self, Write, stdin, Stdout};

//
use core::view::View;
use core::screen::Screen;
use core::codepointinfo::CodepointInfo;

use core::editor::Editor;

fn fill_screen(view: &mut View) {

    match view.buffer {

        Some(ref buf) => {

            let data = &buf.byte_buffer.data;

            view.screen.clear();

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
                    is_selected: false,
                };

                let (ok, _) = view.screen.push(cpi);
                offset += 1;
                if ok == false {
                    break;
                }
            }

            for m in &buf.moving_marks {

                if m.offset >= view.start_offset && m.offset >= view.start_offset {}
            }
        }
        None => {}
    }
}


fn draw_screen(screen: &mut Screen, mut stdout: &mut Stdout) {

    write!(stdout, "{}", termion::cursor::Goto(1, 1)).unwrap();

    for l in 0..screen.height {

        terminal_cursor_to(&mut stdout, 1, (l + 1) as u16);

        let line = screen.get_line(l).unwrap();

        for c in 0..line.used {

            let cpi = line.get_cpi(c).unwrap();

            if cpi.is_selected == true {
                write!(stdout, "{}", termion::style::Invert).unwrap();
            }

            write!(stdout, "{}", cpi.displayed_cp).unwrap();
            write!(stdout, "{}", termion::style::Reset).unwrap();
        }

        for _ in line.used..line.width {
            write!(stdout, " ").unwrap();
        }
    }

    stdout.flush().unwrap();
}


/*
    TODO:
    1 : be explicit
    2 : create editor internal result type Result<>
    3 : use idomatic    func()? style
*/
fn draw_view(mut view: &mut View, mut stdout: &mut Stdout) {

    fill_screen(&mut view);
    draw_screen(&mut view.screen, &mut stdout);
}

fn terminal_clear_current_line(mut stdout: &mut Stdout, line_width: u16) {
    for _ in 0..line_width {
        write!(stdout, " ").unwrap();
    }
}

fn terminal_cursor_to(mut stdout: &mut Stdout, x: u16, y: u16) {
    write!(stdout, "{}", termion::cursor::Goto(x, y)).unwrap();
}


/*
 TODO: create a view per buffer
*/
fn setup_views(editor: &mut Editor, width: usize, height: usize) {

    let mut views = Vec::new();

    let mut vid = 0;

    for (_, b) in &editor.buffer_map {

        let view = View::new(vid,
                             0 as u64,
                             width as usize,
                             height as usize,
                             Some(b.clone()));
        views.push(view);
        vid += 1;
    }

    for view in views {
        &editor.view_map.insert(view.id, Box::new(view));
    }
}


pub fn main_loop(mut editor: &mut Editor) {

    let (width, height) = terminal_size().unwrap();

    setup_views(editor, width as usize, height as usize);

    //
    let stdout = MouseTerminal::from(io::stdout().into_raw_mode().unwrap());
    let mut stdout = AlternateScreen::from(stdout);

    write!(stdout, "{}{}", termion::cursor::Hide, termion::clear::All).unwrap();
    stdout.flush().unwrap();

    let mut keys: Vec<Event> = Vec::new();
    let mut quit = false;

    //
    let mut status = String::new();
    let display_status = true;

    let display_view = true;

    // select view
    let mut vid = 0;

    while !quit {

        let nb_view = editor.view_map.len();
        let mut view = editor.view_map.get_mut(&vid);

        let status_line_y = height;

        if display_view == true {
            draw_view(&mut view.as_mut().unwrap(), &mut stdout);
        }

        if display_status == true {
            display_status_line(&mut view.as_mut().unwrap(), &status, status_line_y, width, &mut stdout);
        }

        for evt in stdin().events() {

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
                            if vid > 0 {
                                vid -= 1;
                            }
                            break;
                        }

                        Key::F(2) => {
                            vid = ::std::cmp::min(vid + 1, (nb_view - 1) as u64);
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

                Event::Unsupported(e) => {
                    status = format!("Event::Unsupported {:?}", e);
                }

            }

/*
            if display_status == true {
                //display_status_line(&buf, &status, status_line_y, width, &mut stdout);
            }
*/
            break;
        }
    }

    // quit
    // clear, restore cursor
    write!(stdout, "{}{}", termion::clear::All, termion::cursor::Show).unwrap();
    stdout.flush().unwrap();
}


fn display_status_line(view: &View,
                       status: &str,
                       line: u16,
                       width: u16,
                       mut stdout: &mut Stdout) {
    // select/clear last line
    let name = match view.buffer {
        Some(ref b) => b.name.as_str(),
        None => "",
    };

    let file_name = match view.buffer {
        Some(ref b) => b.byte_buffer.file_name.as_str(),
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
