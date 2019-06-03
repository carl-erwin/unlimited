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

//
use std::io::{self, Read, Stdout, Write};
use std::thread;
use std::time::Duration;
use std::time::Instant;

use std::collections::HashMap;
use std::sync::mpsc::Receiver;
use std::sync::mpsc::Sender;
//
extern crate termion;

use self::termion::async_stdin;
use self::termion::event::parse_event;
use self::termion::input::MouseTerminal;
use self::termion::raw::IntoRawMode;
use self::termion::screen::{AlternateScreen, ToMainScreen};
use self::termion::terminal_size;

//
use crate::core::event::Event;
use crate::core::event::Event::*;
use crate::core::event::EventMessage;
use crate::core::event::InputEvent;
use crate::core::screen::Screen;

use crate::core::event::Key;

use crate::core::VERSION;

use crate::core::codepointinfo::CodepointInfo;

//
use crate::ui::UiState;

pub fn main_loop(ui_rx: &Receiver<EventMessage>, core_tx: &Sender<EventMessage>) {
    let mut seq: usize = 0;

    fn get_next_seq(seq: &mut usize) -> usize {
        *seq = *seq + 1;
        *seq
    }

    // front-end init code {----
    // init termion
    let stdout = MouseTerminal::from(io::stdout().into_raw_mode().unwrap());
    let mut stdout = AlternateScreen::from(stdout);
    let mut stdin = async_stdin().bytes();

    // ui state
    let mut ui_state = UiState::new();
    ui_state.view_start_line = if ui_state.display_status { 2 } else { 1 };

    // send first event
    let msg = EventMessage::new(get_next_seq(&mut seq), Event::RequestDocumentList);
    core_tx.send(msg).unwrap_or(());

    // ui ctx : TODO move to struct UiCtx
    let mut doc_list;
    let mut current_doc_id = 0;
    let mut current_view_id = 0;
    let mut screen = Box::new(Screen::new(0, 0));
    let mut view_doc_map = HashMap::new();
    let mut prev_screen_rdr_time = Duration::new(0, 0);

    while !ui_state.quit {
        let vec_evt = get_input_event(&mut stdin, &mut ui_state);

        let mut request_layout = vec_evt.len() > 0;
        for ev in vec_evt {
            // send translated input evnts to core
            let msg = EventMessage::new(get_next_seq(&mut seq), Event::InputEvent { ev });
            core_tx.send(msg).unwrap_or(());
        }

        // hack to send multiple page down
        for _ in 0..0 {
            let ev = InputEvent::KeyPress {
                key: Key::PageDown,
                ctrl: false,
                alt: false,
                shift: false,
            };
            let msg = EventMessage::new(get_next_seq(&mut seq), Event::InputEvent { ev });
            core_tx.send(msg).unwrap_or(());
        }

        // resize ?
        if ui_state.resize_flag {
            screen = Box::new(Screen::new(
                ui_state.terminal_width as usize,
                ui_state.terminal_height as usize,
            ));

            request_layout = true;
        }

        // need layout ?
        if request_layout {
            let msg = EventMessage::new(
                get_next_seq(&mut seq),
                Event::RequestLayoutEvent {
                    view_id: current_view_id,
                    doc_id: current_doc_id,
                    screen: screen.clone(),
                },
            );
            core_tx.send(msg).unwrap_or(());
        }

        // evt from core ?
        if let Ok(evt) = ui_rx.recv_timeout(Duration::from_millis(10)) {
            match evt.event {
                Event::ApplicationQuitEvent => {
                    ui_state.quit = true;
                    let msg =
                        EventMessage::new(get_next_seq(&mut seq), Event::ApplicationQuitEvent);
                    core_tx.send(msg).unwrap_or(());
                    break;
                }

                // TODO: add Event::OpenDocument / Event::CloseDocument
                Event::DocumentList { ref list } => {
                    doc_list = list.clone();
                    doc_list.sort_by(|a, b| a.0.cmp(&b.0));

                    if !doc_list.is_empty() {
                        // open first document
                        current_doc_id = doc_list[0].0;

                        let msg = EventMessage::new(
                            get_next_seq(&mut seq),
                            Event::CreateView {
                                width: ui_state.terminal_width as usize,
                                height: ui_state.terminal_height as usize,
                                doc_id: current_doc_id,
                            },
                        );
                        core_tx.send(msg).unwrap_or(());
                    }
                }

                Event::ViewCreated {
                    width: 0,
                    height: 0,
                    doc_id,
                    view_id,
                } => {
                    // save mapping between doc_id and view
                    // remember a document can have multiple view
                    view_doc_map.insert(view_id, doc_id);

                    let msg = EventMessage::new(
                        get_next_seq(&mut seq),
                        Event::RequestLayoutEvent {
                            view_id,
                            doc_id,
                            screen: screen.clone(),
                        },
                    );
                    core_tx.send(msg).unwrap_or(());
                }

                BuildLayoutEvent {
                    view_id,
                    doc_id,
                    mut screen,
                } => {
                    // pending request ? save view_id
                    current_doc_id = doc_id;
                    current_view_id = view_id;

                    let start = Instant::now();

                    if ui_state.resize_flag {
                        // clear screen
                        write!(stdout, "{}{}", termion::cursor::Hide, termion::clear::All).unwrap();
                        ui_state.resize_flag = false;
                    }

                    if ui_state.display_status {
                        let status_line_y = 1;

                        display_status_line(
                            &ui_state,
                            &screen,
                            status_line_y,
                            ui_state.terminal_width,
                            ui_state.terminal_height,
                            &prev_screen_rdr_time,
                            &mut stdout,
                        );
                    }

                    if ui_state.display_view {
                        draw_view(&mut ui_state, &mut screen, &mut stdout);
                    }

                    stdout.flush().unwrap();
                    let end = Instant::now();
                    prev_screen_rdr_time = end.duration_since(start);
                }

                _ => {}
            }
        } else {
            // TODO: handle timeout
        }
    }

    // front-end quit code {----
    // on quit, clear, restore cursor
    write!(stdout, "{}{}", termion::clear::All, termion::cursor::Show).unwrap();
    write!(stdout, "{}{}", ToMainScreen, termion::cursor::Show).unwrap();
    stdout.flush().unwrap();
    // ----}
}

/*

derive_csi_sequence!("Reset SGR parameters.", Reset, "m");
derive_csi_sequence!("Bold text.", Bold, "1m");
derive_csi_sequence!("Fainted text (not widely supported).", Faint, "2m");
derive_csi_sequence!("Italic text.", Italic, "3m");
derive_csi_sequence!("Underlined text.", Underline, "4m");
derive_csi_sequence!("Blinking text (not widely supported).", Blink, "5m");
derive_csi_sequence!("Inverted colors (negative mode).", Invert, "7m");
derive_csi_sequence!("Crossed out text (not widely supported).", CrossedOut, "9m");
derive_csi_sequence!("Undo bold text.", NoBold, "21m");
derive_csi_sequence!("Undo fainted text (not widely supported).", NoFaint, "22m");
derive_csi_sequence!("Undo italic text.", NoItalic, "23m");
derive_csi_sequence!("Undo underlined text.", NoUnderline, "24m");
derive_csi_sequence!("Undo blinking text (not widely supported).", NoBlink, "25m");
derive_csi_sequence!("Undo inverted colors (negative mode).", NoInvert, "27m");
derive_csi_sequence!("Undo crossed out text (not widely supported).", NoCrossedOut, "29m");
derive_csi_sequence!("Framed text (not widely supported).", Framed, "51m");

*/

fn draw_screen(screen: &mut Screen, start_line: usize, mut stdout: &mut Stdout) {
    write!(stdout, "{}", termion::cursor::Hide).unwrap();
    write!(stdout, "{}", termion::cursor::Goto(1, start_line as u16)).unwrap();
    write!(stdout, "{}", termion::style::Reset).unwrap();

    let mut prev_cpi = CodepointInfo::new();

    // default color
    write!(
        stdout,
        "{}",
        termion::color::Fg(termion::color::Rgb(
            prev_cpi.color.0,
            prev_cpi.color.1,
            prev_cpi.color.2
        ))
    )
    .unwrap();

    for l in 0..screen.height {
        terminal_cursor_to(&mut stdout, 1, (start_line + l + 1) as u16);

        let line = screen.get_line(l).unwrap();

        for c in 0..line.width {
            let cpi = line.get_cpi(c).unwrap();

            if prev_cpi.is_selected != cpi.is_selected {
                if cpi.is_selected {
                    write!(stdout, "{}", termion::style::Invert).unwrap();
                } else {
                    write!(stdout, "{}", termion::style::NoInvert).unwrap();
                }
            }

            // detect change
            if prev_cpi.color != cpi.color {
                write!(
                    stdout,
                    "{}",
                    termion::color::Fg(termion::color::Rgb(cpi.color.0, cpi.color.1, cpi.color.2))
                )
                .unwrap();
            }

            write!(stdout, "{}", cpi.displayed_cp).unwrap();

            prev_cpi = *cpi;
        }
    }
}

/*
    TODO:
    1 : be explicit
    2 : create editor internal result type Result<>
    3 : use idomatic    func()? style
*/
fn draw_view(ui_state: &mut UiState, mut screen: &mut Screen, mut stdout: &mut Stdout) {
    let start_line = ui_state.view_start_line;
    draw_screen(&mut screen, start_line, &mut stdout);
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
                        x: i32::from(x - 1),
                        y: i32::from(y - 1),
                        button,
                    };
                }

                self::termion::event::MouseEvent::Release(x, y) => {
                    ui_state.status = format!("MouseEvent::Release => @ ({}, {})", x, y);

                    return InputEvent::ButtonRelease {
                        ctrl: false,
                        alt: false,
                        shift: false,
                        x: i32::from(x - 1),
                        y: i32::from(y - 1),
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

    crate::core::event::InputEvent::NoInputEvent
}

fn get_input_event(
    mut stdin: &mut ::std::io::Bytes<self::termion::AsyncReader>,
    ui_state: &mut UiState,
) -> Vec<InputEvent> {
    let mut v = Vec::<InputEvent>::new();

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
            // TODO: use last input event time
            ui_state.status = " async no event".to_owned();
            ui_state.input_wait_time_ms += 10;
            ui_state.input_wait_time_ms = ::std::cmp::min(ui_state.input_wait_time_ms, 10);
        }

        // check terminal size
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
            ui_state.input_wait_time_ms = 0;
            ui_state.resize_flag = true;
        }

        if ui_state.input_wait_time_ms > 0 {
            thread::sleep(Duration::from_millis(ui_state.input_wait_time_ms));
        }
    }

    v
}

fn display_status_line(
    ui_state: &UiState,
    screen: &Screen,
    line: u16,
    width: u16,
    height: u16,
    prev_screen_rdr_time: &Duration,
    mut stdout: &mut Stdout,
) {
    let name = ""; // TODO: from doc list
    let file_name = ""; // TODO: from doc list

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

    let (cpi, _, _) = screen.find_cpi_by_offset(ui_state.mark_offset);

    let mcp = match cpi {
        Some(cpi) => cpi.cp as u32,
        _ => 0xffd,
    };

    let mut status_str = if name != file_name {
        format!(
            " unlimitED! {}  doc[{}] file[{}], screen_start(@{}):'{:08x}' {} scr_build_time {} prv_rdr_time {}",
            VERSION, name, file_name, screen.first_offset, mcp, ui_state.status,
            screen.time_to_build.as_micros(),
            prev_screen_rdr_time.as_micros()
        )
    } else {
        format!(
            " unlimitED! {}  doc[{}], screen_start(@{}):'{:08x}' {} scr_build_time {} prv_rdr_time {}",
            VERSION, name, screen.first_offset, mcp, ui_state.status,
            screen.time_to_build.as_micros(),
            prev_screen_rdr_time.as_micros()
        )
    };

    status_str.truncate((width + 2) as usize);

    write!(
        stdout,
        "{}{}{}",
        termion::style::Invert,
        status_str,
        termion::style::Reset
    )
    .unwrap();

    // scroolbar
    for h in 0..height + 1 {
        terminal_cursor_to(&mut stdout, width + 2, h + 3);
        write!(
            stdout,
            "{} ",
            termion::color::Bg(termion::color::Rgb(0x00, 0x00, 0xff))
        )
        .unwrap();
    }

    let off = screen.first_offset as f64;
    let max_size = screen.doc_max_offset as f64;

    let pos = ((off / max_size) * f64::from(height)) as u16;

    terminal_cursor_to(&mut stdout, width + 2, 3 + pos);
    write!(
        stdout,
        "{}{} {}",
        termion::color::Bg(termion::color::Rgb(0xff, 0x00, 0x00)),
        termion::style::Invert,
        termion::style::Reset
    )
    .unwrap();
}
