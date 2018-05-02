//
use std::thread;
use std::time::Duration;
use std::io::{self, Read, Stdout, Write};

use std::sync::mpsc::channel;
use std::sync::mpsc::Sender;
use std::sync::mpsc::Receiver;
//
extern crate termion;

use self::termion::screen::{AlternateScreen, ToMainScreen};
use self::termion::input::MouseTerminal;
use self::termion::raw::IntoRawMode;
use self::termion::terminal_size;
use self::termion::async_stdin;
use self::termion::event::parse_event;

//
use core::document;
use core::view::View;
use core::screen::Screen;
use core::event::Event;
use core::event::InputEvent;
use core::event::Event::*;

use core::event::Key;
use core::editor::Editor;

//
use ui::UiState;

pub fn main_loop(ui_rx: Receiver<Event>, core_tx: Sender<Event>) {
    // ui state
    let mut ui_state = UiState::new();

    // ui setup
    let (width, height, start_line) = if ui_state.display_status {
        let (width, height) = terminal_size().unwrap();
        (width - 2, height - 2, 2)
    } else {
        let dim = terminal_size().unwrap();
        (dim.0 - 2, dim.1, 1)
    };

    ui_state.terminal_width = width;
    ui_state.terminal_height = height;
    ui_state.view_start_line = start_line;

    //
    let stdout = MouseTerminal::from(io::stdout().into_raw_mode().unwrap());
    let mut stdout = AlternateScreen::from(stdout);
    write!(stdout, "{}{}", termion::cursor::Hide, termion::clear::All).unwrap();
    stdout.flush().unwrap();
    let mut stdin = async_stdin().bytes();

    // send first event
    let ev = Event::RequestDocumentList;
    core_tx.send(ev);

    let mut doc_list = vec![];
    let mut doc_id = 0;

    let mut screen = Box::new(Screen::new(width as usize, height as usize));

    while !ui_state.quit {
        // send
        let vec_evt = get_input_event(&mut stdin, &mut ui_state);
        for ev in vec_evt {
            let ev = Event::InputEvent { ev };
            core_tx.send(ev);
        }

        // recv
        match ui_rx.recv_timeout(Duration::from_millis(1)) {
            Ok(evt) => {
                println!("ui : recv event : {:?}\r", evt);
                match evt {
                    Event::ApplicationQuitEvent => {
                        ui_state.quit = true;
                        let ev = Event::ApplicationQuitEvent;
                        core_tx.send(ev);
                        break;
                    }

                    Event::DocumentList { ref list } => {
                        doc_list = list.clone();
                        doc_list.sort_by(|a, b| a.0.cmp(&b.0));
                        println!("ui : recv doc list {:?}\r", doc_list);
                        if doc_list.len() > 0 {
                            doc_id = doc_list[0].0;

                            let ev = RequestLayoutEvent {
                                view: 0,
                                doc_id,
                                screen : screen.clone(),
                            };
                            core_tx.send(ev);
                        }
                    }

                    _ => {}
                }
            }

            _ => {
                // TODO: handle timeout
            }
        }
    }

    // quit
    // clear, restore cursor
    write!(stdout, "{}{}", termion::clear::All, termion::cursor::Show).unwrap();
    write!(stdout, "{}{}", ToMainScreen, termion::cursor::Show).unwrap();
    stdout.flush().unwrap();
    return;

    /*
    setup_views(editor, width as usize, height as usize);

    //
    let stdout = MouseTerminal::from(io::stdout().into_raw_mode().unwrap());
    let mut stdout = AlternateScreen::from(stdout);

    write!(stdout, "{}{}", termion::cursor::Hide, termion::clear::All).unwrap();
    stdout.flush().unwrap();

    let mut stdin = async_stdin().bytes();

    while !ui_state.quit {
        ui_state.nb_view = editor.view_map.len();
        let mut view = Some(&editor.view_map[ui_state.vid as usize].1);

        // check screen size
        {
            let mut view = view.as_mut().unwrap().borrow_mut();
            if view.screen.width != ui_state.terminal_width as usize
                || view.screen.height != ui_state.terminal_height as usize
            {
                view.screen = Box::new(Screen::new(
                    ui_state.terminal_width as usize,
                    ui_state.terminal_height as usize,
                ));
            }
        }

        if ui_state.display_view {
            draw_view(
                &mut ui_state,
                &mut view.as_mut().unwrap().borrow_mut(),
                &mut stdout,
            );
        }

        if ui_state.display_status {
            let status_line_y = 1;
            let height = {
                let view = view.as_mut().unwrap().borrow_mut();
                view.screen.height as u16
            };

            display_status_line(
                &ui_state,
                &view.as_mut().unwrap().borrow_mut(),
                status_line_y,
                ui_state.terminal_width,
                height,
                &mut stdout,
            );
        }

        let vec_evt = get_input_event(&mut stdin, &mut ui_state);
        for evt in vec_evt {
            process_input_events(
                &mut ui_state,
                &mut view.as_mut().unwrap().borrow_mut(),
                &evt,
            );

            // re-sync view on each event/update or else the main mark will be offscreen
            // TODO: add a view flag to call this
            fill_screen(&mut ui_state, &mut view.as_mut().unwrap().borrow_mut());
        }
    }
    */
}

fn draw_screen(screen: &mut Screen, start_line: usize, mut stdout: &mut Stdout) {
    write!(stdout, "{}", termion::cursor::Hide).unwrap();
    write!(stdout, "{}", termion::cursor::Goto(1, start_line as u16)).unwrap();
    write!(stdout, "{}", termion::style::Reset).unwrap();
    // stdout.flush().unwrap();

    for l in 0..screen.height {
        terminal_cursor_to(&mut stdout, 1, (start_line + l + 1) as u16);

        let line = screen.get_line(l).unwrap();

        for c in 0..line.width {
            let cpi = line.get_cpi(c).unwrap();

            if cpi.is_selected {
                write!(
                    stdout,
                    "{}{}{}",
                    termion::style::Invert,
                    cpi.displayed_cp,
                    termion::style::Reset
                ).unwrap();
            } else {
                write!(stdout, "{}", cpi.displayed_cp).unwrap();
            }
        }

        /*
        for _ in line.nb_cells..line.width {
            write!(stdout, " ").unwrap();
        }
        */
    }

    stdout.flush().unwrap();
}

/*
    TODO:
    1 : be explicit
    2 : create editor internal result type Result<>
    3 : use idomatic    func()? style
*/
fn draw_view(mut ui_state: &mut UiState, mut view: &mut View, mut stdout: &mut Stdout) {
    let start_line = ui_state.view_start_line;
    draw_screen(&mut view.screen, start_line, &mut stdout);
}

fn _terminal_clear_current_line(stdout: &mut Stdout, line_width: u16) {
    for _ in 0..line_width {
        write!(stdout, " ").unwrap();
    }
}

fn terminal_cursor_to(stdout: &mut Stdout, x: u16, y: u16) {
    write!(stdout, "{}", termion::cursor::Goto(x, y)).unwrap();
}

fn translate_termion_event(evt: self::termion::event::Event, ui_state: &mut UiState) -> InputEvent {
    fn termion_mouse_button_to_u32(mb: self::termion::event::MouseButton) -> u32 {
        match mb {
            self::termion::event::MouseButton::Left => 0,
            self::termion::event::MouseButton::Right => 1,
            self::termion::event::MouseButton::Middle => 2,
            self::termion::event::MouseButton::WheelUp => 3,
            self::termion::event::MouseButton::WheelDown => 4,
        }
    }

    // translate termion event
    match evt {
        self::termion::event::Event::Key(k) => match k {
            self::termion::event::Key::Ctrl('c') => {
                ui_state.status = "Ctrl-c".to_owned();

                return InputEvent::KeyPress {
                    ctrl: true,
                    alt: false,
                    shift: false,
                    key: Key::UNICODE('c'),
                };
            }

            self::termion::event::Key::Char('\n') => {
                ui_state.status = "<newline>".to_owned();

                return InputEvent::KeyPress {
                    ctrl: false,
                    alt: false,
                    shift: false,
                    key: Key::UNICODE('\n'),
                };
            }

            self::termion::event::Key::Char(c) => {
                ui_state.status = format!("{}", c);

                return InputEvent::KeyPress {
                    ctrl: false,
                    alt: false,
                    shift: false,
                    key: Key::UNICODE(c),
                };
            }

            self::termion::event::Key::Alt(c) => {
                ui_state.status = format!("Alt-{}", c);

                return InputEvent::KeyPress {
                    ctrl: false,
                    alt: true,
                    shift: false,
                    key: Key::UNICODE(c),
                };
            }

            self::termion::event::Key::Ctrl(c) => {
                ui_state.status = format!("Ctrl-{}", c);

                return InputEvent::KeyPress {
                    ctrl: true,
                    alt: false,
                    shift: false,
                    key: Key::UNICODE(c),
                };
            }

            self::termion::event::Key::F(1) => {
                if ui_state.vid > 0 {
                    ui_state.vid -= 1;
                }
            }

            self::termion::event::Key::F(2) => {
                ui_state.vid = ::std::cmp::min(ui_state.vid + 1, (ui_state.nb_view - 1) as u64);
            }

            self::termion::event::Key::F(f) => ui_state.status = format!("F{:?}", f),

            self::termion::event::Key::Left => {
                ui_state.status = "<left>".to_owned();
                return InputEvent::KeyPress {
                    ctrl: false,
                    alt: false,
                    shift: false,
                    key: Key::Left,
                };
            }
            self::termion::event::Key::Right => {
                ui_state.status = "<right>".to_owned();
                return InputEvent::KeyPress {
                    ctrl: false,
                    alt: false,
                    shift: false,
                    key: Key::Right,
                };
            }
            self::termion::event::Key::Up => {
                ui_state.status = "<up>".to_owned();
                return InputEvent::KeyPress {
                    ctrl: false,
                    alt: false,
                    shift: false,
                    key: Key::Up,
                };
            }
            self::termion::event::Key::Down => {
                ui_state.status = "<down>".to_owned();
                return InputEvent::KeyPress {
                    ctrl: false,
                    alt: false,
                    shift: false,
                    key: Key::Down,
                };
            }
            self::termion::event::Key::Backspace => {
                ui_state.status = "<backspc>".to_owned();
                return InputEvent::KeyPress {
                    ctrl: false,
                    alt: false,
                    shift: false,
                    key: Key::BackSpace,
                };
            }
            self::termion::event::Key::Home => {
                ui_state.status = "<Home>".to_owned();
                return InputEvent::KeyPress {
                    ctrl: false,
                    alt: false,
                    shift: false,
                    key: Key::Home,
                };
            }
            self::termion::event::Key::End => {
                ui_state.status = "<End>".to_owned();
                return InputEvent::KeyPress {
                    ctrl: false,
                    alt: false,
                    shift: false,
                    key: Key::End,
                };
            }
            self::termion::event::Key::PageUp => {
                ui_state.status = "<PageUp>".to_owned();
                return InputEvent::KeyPress {
                    ctrl: false,
                    alt: false,
                    shift: false,
                    key: Key::PageUp,
                };
            }
            self::termion::event::Key::PageDown => {
                ui_state.status = "<PageDown>".to_owned();
                return InputEvent::KeyPress {
                    ctrl: false,
                    alt: false,
                    shift: false,
                    key: Key::PageDown,
                };
            }
            self::termion::event::Key::Delete => {
                ui_state.status = "<Delete>".to_owned();
                return InputEvent::KeyPress {
                    ctrl: false,
                    alt: false,
                    shift: false,
                    key: Key::Delete,
                };
            }
            self::termion::event::Key::Insert => {
                ui_state.status = "<Insert>".to_owned();
                return InputEvent::KeyPress {
                    ctrl: false,
                    alt: false,
                    shift: false,
                    key: Key::Insert,
                };
            }
            self::termion::event::Key::Esc => {
                ui_state.status = "<Esc>".to_owned();
                return InputEvent::KeyPress {
                    ctrl: false,
                    alt: false,
                    shift: false,
                    key: Key::Escape,
                };
            }
            _ => ui_state.status = "Other".to_owned(),
        },

        self::termion::event::Event::Mouse(m) => {
            match m {
                self::termion::event::MouseEvent::Press(mb, x, y) => {
                    ui_state.status =
                        format!("MouseEvent::Press => MouseButton {:?} @ ({}, {})", mb, x, y);

                    let button = termion_mouse_button_to_u32(mb);

                    return InputEvent::ButtonPress {
                        ctrl: false,
                        alt: false,
                        shift: false,
                        x: (x - 1) as i32,
                        y: (y - 1) as i32,
                        button,
                    };
                }

                self::termion::event::MouseEvent::Release(x, y) => {
                    ui_state.status = format!("MouseEvent::Release => @ ({}, {})", x, y);

                    return InputEvent::ButtonRelease {
                        ctrl: false,
                        alt: false,
                        shift: false,
                        x: (x - 1) as i32,
                        y: (y - 1) as i32,
                        button: 0xff,
                    };
                }

                self::termion::event::MouseEvent::Hold(x, y) => {
                    ui_state.status = format!("MouseEvent::Hold => @ ({}, {})", x, y);
                }
            };
        }

        self::termion::event::Event::Unsupported(e) => {
            ui_state.status = format!("Event::Unsupported {:?}", e);
        }
    }

    ::core::event::InputEvent::NoInputEvent
}

fn get_input_event(
    mut stdin: &mut ::std::io::Bytes<self::termion::AsyncReader>,
    ui_state: &mut UiState,
) -> Vec<InputEvent> {
    let mut v = Vec::<InputEvent>::new();

    // println!("get input {}\r",  ui_state.input_wait_time_ms);
    {
        let b = stdin.next();
        if let Some(b) = b {
            if let Ok(val) = b {
                if let Ok(evt) = parse_event(val, &mut stdin) {
                    let evt = translate_termion_event(evt, ui_state);
                    v.push(evt);
                    ui_state.input_wait_time_ms = 0;
                }
            }
        } else {
            // check terminal size
            if true {
                let (width, height, start_line) = if ui_state.display_status {
                    let (width, height) = terminal_size().unwrap();
                    (width - 2, height - 2, 2)
                } else {
                    let dim = terminal_size().unwrap();
                    (dim.0 - 2, dim.1, 1)
                };

                if ui_state.terminal_width != width || ui_state.terminal_height != height {
                    ui_state.terminal_width = width;
                    ui_state.terminal_height = height;
                    ui_state.view_start_line = start_line;
                    ui_state.input_wait_time_ms = 0; // Warning: to handle resize batch
                }
            }

            // TODO: use last input event time
            ui_state.status = " async no event".to_owned();
            ui_state.input_wait_time_ms += 10;
            ui_state.input_wait_time_ms = ::std::cmp::min(ui_state.input_wait_time_ms, 20);
            thread::sleep(Duration::from_millis(ui_state.input_wait_time_ms));
        }
    }

    v
}

fn display_status_line(
    ui_state: &UiState,
    view: &View,
    line: u16,
    width: u16,
    height: u16,
    mut stdout: &mut Stdout,
) {
    let doc = match view.document {
        Some(ref d) => d.borrow(),
        None => return,
    };

    let name = doc.name.as_str();
    let file_name = doc.buffer.file_name.as_str();

    // select/clear last line
    terminal_cursor_to(&mut stdout, 1, line);
    for _ in 0..width + 2 {
        write!(stdout, "{} ", termion::style::Invert).unwrap();
    }
    terminal_cursor_to(&mut stdout, 1, line);

    terminal_cursor_to(&mut stdout, 1, line + 1);
    for _ in 0..width + 2 {
        write!(stdout, "{} ", termion::style::Reset).unwrap();
    }
    terminal_cursor_to(&mut stdout, 1, line);

    let (cpi, _, _) = view.screen.find_cpi_by_offset(ui_state.mark_offset);

    let mcp = match cpi {
        Some(cpi) => cpi.cp as u32,
        _ => 0xffd,
    };

    let mut status_str = if name != file_name {
        format!(
            " unlimitED! 0.0.3    doc[{}] file[{}], m(@{}):'{:08x}' {}",
            name, file_name, ui_state.mark_offset, mcp, ui_state.status
        )
    } else {
        format!(
            " unlimitED! 0.0.3    doc[{}], m(@{}):'{:08x}' {}",
            name, ui_state.mark_offset, mcp, ui_state.status
        )
    };

    status_str.truncate((width + 2) as usize);

    write!(
        stdout,
        "{}{}{}",
        termion::style::Invert,
        status_str,
        termion::style::Reset
    ).unwrap();

    // scroolbar
    for h in 0..height + 1 {
        terminal_cursor_to(&mut stdout, width + 2, h + 3);
        write!(
            stdout,
            "{} ",
            termion::color::Bg(termion::color::Rgb(0x00, 0x00, 0xff))
        ).unwrap();
    }

    let off = view.start_offset as f64;
    let max_size = view.document.as_ref().unwrap().borrow().buffer.size as f64;

    let pos = ((off / max_size) * height as f64) as u16;

    terminal_cursor_to(&mut stdout, width + 2, 3 + pos);
    write!(
        stdout,
        "{}{} {}",
        termion::color::Bg(termion::color::Rgb(0xff, 0x00, 0x00)),
        termion::style::Invert,
        termion::style::Reset
    ).unwrap();

    stdout.flush().unwrap();
}
