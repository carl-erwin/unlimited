//
use std::thread;
use std::time::Duration;
use std::rc::Rc;
use std::cell::RefCell;
use std::io::{self, Read, Stdout, Write};

//
extern crate termion;

use self::termion::screen::{AlternateScreen, ToMainScreen};
use self::termion::input::MouseTerminal;
use self::termion::raw::IntoRawMode;
use self::termion::terminal_size;
use self::termion::async_stdin;
use self::termion::event::parse_event;

//
use core::view::{build_screen_layout, View};
use core::screen::Screen;
use core::event::InputEvent;
use core::event::Key;
use core::editor::Editor;

//
struct UiState {
    keys: Vec<InputEvent>,
    quit: bool,
    status: String,
    display_status: bool,
    display_view: bool,
    vid: u64,
    nb_view: usize,
    last_offset: u64,
    mark_offset: u64,
    input_wait_time_ms: u64,
    terminal_width: u16,
    terminal_height: u16,
    view_start_line: usize,
}

impl UiState {
    fn new() -> UiState {
        UiState {
            keys: Vec::new(),
            quit: false,
            status: String::new(),
            display_status: true,
            display_view: true,
            vid: 0,
            nb_view: 0,
            last_offset: 0,
            mark_offset: 0,
            input_wait_time_ms: 20,
            terminal_width: 0,
            terminal_height: 0,
            view_start_line: 0,
        }
    }
}

pub fn main_loop(editor: &mut Editor) {
    let mut ui_state = UiState::new();

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

    // quit
    // clear, restore cursor
    write!(stdout, "{}{}", termion::clear::All, termion::cursor::Show).unwrap();
    write!(stdout, "{}{}", ToMainScreen, termion::cursor::Show).unwrap();

    stdout.flush().unwrap();
}

fn setup_views(editor: &mut Editor, width: usize, height: usize) {
    let mut views = Vec::new();

    for (vid, b) in editor.document_map.iter().enumerate() {
        let view = View::new(
            vid as u64,
            0 as u64,
            width as usize,
            height as usize,
            Some(b.1.clone()),
        );
        views.push(view);
    }

    for view in views {
        editor.view_map.push((view.id, Rc::new(RefCell::new(view))));
    }
}

/// Fills the screen using the view start offset
fn fill_screen(ui_state: &mut UiState, view: &mut View) {
    if let Some(ref _buf) = view.document {
        let mut screen = &mut view.screen;

        screen.clear();

        let mut data = vec![];
        let max_size = (screen.width * screen.height * 4) as usize;
        let doc = view.document.as_ref().unwrap().borrow_mut();
        doc.read(view.start_offset, max_size, &mut data);

        let max_offset = doc.buffer.size as u64;

        view.end_offset = build_screen_layout(&data, view.start_offset, max_offset, &mut screen);

        ui_state.last_offset = view.end_offset;

        // render marks

        // brute force for now
        for m in view.moving_marks.borrow().iter() {
            // TODO: screen.find_line_by_offset(m.offset) -> Option<&mut Line>
            if m.offset >= view.start_offset && m.offset <= view.end_offset {
                for l in 0..screen.height {
                    let line = screen.get_mut_line(l).unwrap();
                    for c in 0..line.nb_cells {
                        let cpi = line.get_mut_cpi(c).unwrap();

                        //if cpi.offset > m.offset {
                        //break;
                        //}

                        if cpi.offset == m.offset {
                            cpi.is_selected = true;
                            ui_state.mark_offset = m.offset;
                        }
                    }
                }
            }
        }
    }
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
    fill_screen(&mut ui_state, &mut view);
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

    // prepare async read
    loop {
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
            {
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

                    // TODO: push resize event to remove size checks in main loop
                    break;
                }
            }

            if !v.is_empty() {
                break;
            }

            // TODO: use last input event time
            ui_state.status = " async no event".to_owned();
            ui_state.input_wait_time_ms += 10;
            ui_state.input_wait_time_ms = ::std::cmp::max(ui_state.input_wait_time_ms, 20);
            thread::sleep(Duration::from_millis(ui_state.input_wait_time_ms));
        }
    }

    v
}

fn process_input_events(ui_state: &mut UiState, view: &mut View, ev: &InputEvent) {
    if *ev == ::core::event::InputEvent::NoInputEvent {
        // ignore no input event event :-)
        return;
    }

    ui_state.keys.push(ev.clone());

    let mut clear_keys = true;
    match *ev {
        InputEvent::KeyPress {
            ctrl: true,
            alt: false,
            shift: false,
            key: Key::UNICODE('q'),
        } => {
            ui_state.quit = true;
        }

        InputEvent::KeyPress {
            ctrl: true,
            alt: false,
            shift: false,
            key: Key::UNICODE('u'),
        } => {
            view.undo();
        }

        InputEvent::KeyPress {
            ctrl: true,
            alt: false,
            shift: false,
            key: Key::UNICODE('r'),
        } => {
            view.redo();
        }

        InputEvent::KeyPress {
            ctrl: true,
            alt: false,
            shift: false,
            key: Key::UNICODE('x'),
        } => {
            clear_keys = false;
        }

        // ctrl+a
        InputEvent::KeyPress {
            ctrl: true,
            alt: false,
            shift: false,
            key: Key::UNICODE('a'),
        }
        | InputEvent::KeyPress {
            ctrl: false,
            alt: false,
            shift: false,
            key: Key::Home,
        } => {
            view.move_marks_to_beginning_of_line();
        }

        // ctrl+e
        InputEvent::KeyPress {
            ctrl: true,
            alt: false,
            shift: false,
            key: Key::UNICODE('e'),
        }
        | InputEvent::KeyPress {
            ctrl: false,
            alt: false,
            shift: false,
            key: Key::End,
        } => {
            view.move_marks_to_end_of_line();
        }

        // ctrl+d
        InputEvent::KeyPress {
            ctrl: true,
            alt: false,
            shift: false,
            key: Key::UNICODE('d'),
        } => {
            view.remove_codepoint();
        }

        // ctrl+s
        InputEvent::KeyPress {
            ctrl: true,
            alt: false,
            shift: false,
            key: Key::UNICODE('s'),
        } => {
            view.save_document();
        }

        // ctrl+k
        InputEvent::KeyPress {
            ctrl: true,
            alt: false,
            shift: false,
            key: Key::UNICODE('k'),
        } => {
            view.cut_to_end_of_line();
        }

        // ctrl+y
        InputEvent::KeyPress {
            ctrl: true,
            alt: false,
            shift: false,
            key: Key::UNICODE('y'),
        } => {
            view.paste();
        }

        // ctrl+l
        InputEvent::KeyPress {
            ctrl: true,
            alt: false,
            shift: false,
            key: Key::UNICODE('l'),
        } => {
            view.center_arround_mark();
        }

        // ctrl+?
        InputEvent::KeyPress {
            ctrl: true,
            alt: false,
            shift: false,
            key: Key::UNICODE(c),
        } => {
            ui_state.status = format!("ctrl+<{}>", c);
        }

        // left
        InputEvent::KeyPress {
            ctrl: false,
            alt: false,
            shift: false,
            key: Key::Left,
        } => {
            view.move_marks_backward();

            ui_state.status = "<left>".to_owned();
        }

        // up
        InputEvent::KeyPress {
            ctrl: false,
            alt: false,
            shift: false,
            key: Key::Up,
        } => {
            view.move_marks_to_previous_line();

            ui_state.status = "<up>".to_owned();
        }

        // down
        InputEvent::KeyPress {
            ctrl: false,
            alt: false,
            shift: false,
            key: Key::Down,
        } => {
            view.move_marks_to_next_line();

            ui_state.status = "<down>".to_owned();
        }

        // right
        InputEvent::KeyPress {
            ctrl: false,
            alt: false,
            shift: false,
            key: Key::Right,
        } => {
            view.move_marks_forward();

            ui_state.status = "<right>".to_owned();
        }

        // page_up
        InputEvent::KeyPress {
            ctrl: false,
            alt: false,
            shift: false,
            key: Key::PageUp,
        } => {
            view.scroll_to_previous_screen();
            ui_state.status = "<page_up>".to_owned();
        }

        // page_down
        InputEvent::KeyPress {
            ctrl: false,
            alt: false,
            shift: false,
            key: Key::PageDown,
        } => {
            view.scroll_to_next_screen();
            ui_state.status = "<page_down>".to_owned();
        }

        // alt+< goto beginning of file
        InputEvent::KeyPress {
            ctrl: false,
            alt: true,
            shift: false,
            key: Key::UNICODE('<'),
        } => {
            view.move_mark_to_beginning_of_file();
            ui_state.status = "<move to beginning of file>".to_owned();
        }

        // alt+> goto end of file
        InputEvent::KeyPress {
            ctrl: false,
            alt: true,
            shift: false,
            key: Key::UNICODE('>'),
        } => {
            view.move_mark_to_end_of_file();
            ui_state.status = "<move to end of file>".to_owned();
        }

        // delete
        InputEvent::KeyPress {
            ctrl: false,
            alt: false,
            shift: false,
            key: Key::Delete,
        } => {
            view.remove_codepoint();
            ui_state.status = "<del>".to_owned();
        }

        // backspace
        InputEvent::KeyPress {
            ctrl: false,
            alt: false,
            shift: false,
            key: Key::BackSpace,
        } => {
            view.remove_previous_codepoint();

            ui_state.status = "<backspace>".to_owned();
        }

        // insert text
        InputEvent::KeyPress {
            ctrl: false,
            alt: false,
            shift: false,
            key: Key::UNICODE(cp),
        } => {
            view.insert_codepoint(cp);
            ui_state.status = format!("<insert [0x{:x}]>", cp as u32);
        }

        // mouse button pressed
        InputEvent::ButtonPress {
            ctrl: false,
            alt: false,
            shift: false,
            x,
            y,
            button,
        } => match button {
            0 | 1 => {
                view.button_press(button, x, y - 2);
                ui_state.status = format!("<click({},@({},{}))]>", button, x, y - 2);
            }
            3 => {
                view.scroll_up(3);
                ui_state.status = format!("<click({},@({},{}))]>", button, x, y - 2);
            }
            4 => {
                view.scroll_down(3);
                ui_state.status = format!("<click({},@({},{}))]>", button, x, y - 2);
            }

            _ => {}
        },

        // mouse button released
        InputEvent::ButtonRelease {
            ctrl: false,
            alt: false,
            shift: false,
            x,
            y,
            button,
        } => {
            view.button_release(button, x, y);
            ui_state.status = format!("<unclick({},@({},{}))]>", button, x, y);
        }

        _ => {}
    }

    if clear_keys {
        ui_state.keys.clear();
    }
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