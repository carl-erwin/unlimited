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
use std::io::Error;
use std::io::{self, Stdout, Write};

use std::thread;
use std::time::Duration;
use std::time::Instant;

use std::collections::HashMap;
use std::sync::mpsc::Receiver;
use std::sync::mpsc::Sender;

extern crate libc;

use self::libc::{c_void, read};

//
extern crate termion;

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

use crate::core::codepointinfo::CodepointInfo;

//
use crate::ui::UiState;

fn stdin_thread(tx: &Sender<EventMessage>) {
    loop {
        get_input_events(&tx);
    }
}

pub fn main_loop(
    ui_rx: &Receiver<EventMessage>,
    _ui_tx: &Sender<EventMessage>,
    core_tx: &Sender<EventMessage>,
) {
    let mut seq: usize = 0;

    fn get_next_seq(seq: &mut usize) -> usize {
        *seq = *seq + 1;
        *seq
    }

    // front-end init code {----
    // init termion
    let stdout = MouseTerminal::from(io::stdout().into_raw_mode().unwrap());
    let mut stdout = AlternateScreen::from(stdout);

    let core_tx_clone = core_tx.clone();
    thread::spawn(move || {
        stdin_thread(&core_tx_clone);
    });

    // ui state
    let mut ui_state = UiState::new();

    // send first event
    let msg = EventMessage::new(get_next_seq(&mut seq), Event::RequestDocumentList);
    core_tx.send(msg).unwrap_or(());

    // ui ctx : TODO move to struct UiCtx
    let mut doc_list;
    let mut current_doc_id = 0;
    let mut current_view_id = 0;
    let mut last_screen = Box::new(Screen::new(1, 1)); // last screen ?
    let mut view_doc_map = HashMap::new();
    let mut _prev_screen_rdr_time = Duration::new(0, 0);

    write!(stdout, "{}{}", termion::cursor::Hide, termion::clear::All).unwrap();

    let mut request_layout = false;

    while !ui_state.quit {
        // check terminal size
        let (width, height) = terminal_size().unwrap();

        if ui_state.terminal_width != width || ui_state.terminal_height != height {
            ui_state.terminal_width = width;
            ui_state.terminal_height = height;
            ui_state.resize_flag = true;
        }

        // resize ?
        if ui_state.resize_flag {
            request_layout = true;
        }

        // need layout ?
        if request_layout {
            let msg = EventMessage::new(
                get_next_seq(&mut seq),
                Event::RequestLayoutEvent {
                    view_id: current_view_id,
                    doc_id: current_doc_id,
                    width: ui_state.terminal_width as usize,
                    height: ui_state.terminal_height as usize,
                },
            );
            core_tx.send(msg).unwrap_or(());
            request_layout = false;
        }

        if let Ok(evt) = ui_rx.recv_timeout(Duration::from_millis(1000)) {
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
                    width,
                    height,
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
                            width,
                            height,
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
                        write!(stdout, "{}", termion::clear::All).unwrap();
                        ui_state.resize_flag = false;
                    }

                    draw_view(&mut last_screen, &mut screen, &mut stdout);

                    display_scroll_bar(
                        &screen,
                        ui_state.terminal_width,
                        ui_state.terminal_height,
                        &mut stdout,
                    );

                    stdout.flush().unwrap();
                    let end = Instant::now();
                    _prev_screen_rdr_time = end.duration_since(start);
                    last_screen = screen;
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

fn draw_screen(last_screen: &mut Screen, screen: &mut Screen, mut stdout: &mut Stdout) {
    write!(stdout, "{}", termion::cursor::Goto(1, 1)).unwrap();
    write!(stdout, "{}", termion::style::Reset).unwrap();

    let mut prev_cpi = CodepointInfo::new();

    let check_hash = last_screen.max_width() == screen.max_width()
        && last_screen.max_height() == screen.max_height();

    prev_cpi.color.0 = 0;
    prev_cpi.color.1 = 0;
    prev_cpi.color.2 = 0;

    for l in 0..screen.max_height() {
        let line = screen.get_mut_unclipped_line(l).unwrap();

        terminal_cursor_to(&mut stdout, 1, (1 + l) as u16);

        let mut have_cursor = false;
        for c in 0..line.max_width() {
            let cpi = line.get_unclipped_cpi(c).unwrap();

            if cpi.is_selected {
                have_cursor = true;
                break;
            }
        }

        if check_hash {
            // check previous line
            let prev_line = last_screen.get_mut_unclipped_line(l).unwrap();
            for c in 0..prev_line.width() {
                let cpi = prev_line.get_unclipped_cpi(c).unwrap();
                if cpi.is_selected {
                    have_cursor = true;
                    write!(stdout, "{}", termion::style::NoBold).unwrap();
                    break;
                }
            }
        }

        if check_hash && !have_cursor {
            let prev_line = last_screen.get_mut_unclipped_line(l).unwrap();
            if prev_line.hash() == line.hash() {
                continue;
            }
        }

        for c in 0..line.max_width() {
            let cpi = line.get_unclipped_cpi(c).unwrap();

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

    write!(stdout, "{}", termion::style::Reset).unwrap();
}

/*
    TODO:
    1 : be explicit
    2 : create editor internal result type Result<>
    3 : use idomatic    func()? style
*/
fn draw_view(last_screen: &mut Screen, mut screen: &mut Screen, mut stdout: &mut Stdout) {
    draw_screen(last_screen, &mut screen, &mut stdout);
}

fn _terminal_clear_current_line(stdout: &mut Stdout, line_width: u16) {
    for _ in 0..line_width {
        write!(stdout, " ").unwrap();
    }
}

fn terminal_cursor_to(stdout: &mut Stdout, x: u16, y: u16) {
    write!(stdout, "{}", termion::cursor::Goto(x, y)).unwrap();
}

fn translate_termion_event(evt: self::termion::event::Event) -> InputEvent {
    // translate termion event
    match evt {
        self::termion::event::Event::Key(k) => match k {
            self::termion::event::Key::Ctrl(c) => {
                return InputEvent::KeyPress {
                    ctrl: true,
                    alt: false,
                    shift: false,
                    key: Key::Unicode(c),
                };
            }

            self::termion::event::Key::Char(c) => {
                return InputEvent::KeyPress {
                    ctrl: false,
                    alt: false,
                    shift: false,
                    key: Key::Unicode(c),
                };
            }

            self::termion::event::Key::Alt(c) => {
                return InputEvent::KeyPress {
                    ctrl: false,
                    alt: true,
                    shift: false,
                    key: Key::Unicode(c),
                };
            }

            self::termion::event::Key::F(n) => {
                return InputEvent::KeyPress {
                    ctrl: false,
                    alt: false,
                    shift: false,
                    key: Key::F(n as usize),
                };
            }

            self::termion::event::Key::Left => {
                return InputEvent::KeyPress {
                    ctrl: false,
                    alt: false,
                    shift: false,
                    key: Key::Left,
                };
            }
            self::termion::event::Key::Right => {
                return InputEvent::KeyPress {
                    ctrl: false,
                    alt: false,
                    shift: false,
                    key: Key::Right,
                };
            }
            self::termion::event::Key::Up => {
                return InputEvent::KeyPress {
                    ctrl: false,
                    alt: false,
                    shift: false,
                    key: Key::Up,
                };
            }
            self::termion::event::Key::Down => {
                return InputEvent::KeyPress {
                    ctrl: false,
                    alt: false,
                    shift: false,
                    key: Key::Down,
                };
            }
            self::termion::event::Key::Backspace => {
                return InputEvent::KeyPress {
                    ctrl: false,
                    alt: false,
                    shift: false,
                    key: Key::BackSpace,
                };
            }
            self::termion::event::Key::Home => {
                return InputEvent::KeyPress {
                    ctrl: false,
                    alt: false,
                    shift: false,
                    key: Key::Home,
                };
            }
            self::termion::event::Key::End => {
                return InputEvent::KeyPress {
                    ctrl: false,
                    alt: false,
                    shift: false,
                    key: Key::End,
                };
            }
            self::termion::event::Key::PageUp => {
                return InputEvent::KeyPress {
                    ctrl: false,
                    alt: false,
                    shift: false,
                    key: Key::PageUp,
                };
            }
            self::termion::event::Key::PageDown => {
                return InputEvent::KeyPress {
                    ctrl: false,
                    alt: false,
                    shift: false,
                    key: Key::PageDown,
                };
            }
            self::termion::event::Key::Delete => {
                return InputEvent::KeyPress {
                    ctrl: false,
                    alt: false,
                    shift: false,
                    key: Key::Delete,
                };
            }
            self::termion::event::Key::Insert => {
                return InputEvent::KeyPress {
                    ctrl: false,
                    alt: false,
                    shift: false,
                    key: Key::Insert,
                };
            }
            self::termion::event::Key::Esc => {
                return InputEvent::KeyPress {
                    ctrl: false,
                    alt: false,
                    shift: false,
                    key: Key::Escape,
                };
            }
            _ => {}
        },

        self::termion::event::Event::Mouse(m) => {
            fn termion_mouse_button_to_u32(mb: self::termion::event::MouseButton) -> u32 {
                match mb {
                    self::termion::event::MouseButton::Left => 0,
                    self::termion::event::MouseButton::Right => 1,
                    self::termion::event::MouseButton::Middle => 2,
                    self::termion::event::MouseButton::WheelUp => 3,
                    self::termion::event::MouseButton::WheelDown => 4,
                }
            }

            match m {
                self::termion::event::MouseEvent::Press(mb, x, y) => {
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
                    return InputEvent::ButtonRelease {
                        ctrl: false,
                        alt: false,
                        shift: false,
                        x: i32::from(x - 1),
                        y: i32::from(y - 1),
                        button: 0xff,
                    };
                }

                self::termion::event::MouseEvent::Hold(_x, _y) => {}
            };
        }

        self::termion::event::Event::Unsupported(_e) => {}
    }

    crate::core::event::InputEvent::NoInputEvent
}

fn get_input_events(tx: &Sender<EventMessage>) {
    const BUF_SIZE: usize = 1024 * 32;

    let mut buf = Vec::<u8>::with_capacity(BUF_SIZE);
    unsafe {
        buf.set_len(BUF_SIZE);
    }

    loop {
        let nb_read = unsafe { read(0, buf.as_mut_ptr() as *mut c_void, BUF_SIZE) as usize };
        let mut buf2 = Vec::<Result<u8, Error>>::with_capacity(nb_read);

        let mut raw_data = Vec::<u8>::with_capacity(nb_read);

        for i in 0..nb_read {
            buf2.push(Ok(buf[i]));

            raw_data.push(buf[i]);
        }

        let mut raw_evt = Vec::<_>::with_capacity(BUF_SIZE);
        let mut it = buf2.into_iter();
        loop {
            if let Some(val) = it.next() {
                if let Ok(evt) = parse_event(val.unwrap(), &mut it) {
                    raw_evt.push(evt);
                } else {
                    break;
                }
            } else {
                break;
            }
        }

        // merge consecutive events
        let mut v = vec![];
        let mut codepoints = Vec::<char>::new();

        for evt in &raw_evt {
            let evt = translate_termion_event(evt.clone());
            match evt {
                InputEvent::KeyPress {
                    key: Key::Unicode(c),
                    ctrl: false,
                    alt: false,
                    shift: false,
                } => {
                    codepoints.push(c);
                }

                _ => {
                    if !codepoints.is_empty() {
                        v.push(InputEvent::KeyPress {
                            key: Key::UnicodeArray(codepoints),
                            ctrl: false,
                            alt: false,
                            shift: false,
                        });
                        codepoints = Vec::<char>::new();
                    }
                    v.push(evt);
                }
            }
        }

        // send
        if !codepoints.is_empty() {
            v.push(InputEvent::KeyPress {
                key: Key::UnicodeArray(codepoints),
                ctrl: false,
                alt: false,
                shift: false,
            });
        }

        if !v.is_empty() {
            let msg = EventMessage::new(
                0,
                Event::InputEvent {
                    events: v,
                    raw_data: Some(raw_data),
                },
            );
            tx.send(msg).unwrap_or(());
        }
    }
}

fn display_scroll_bar(screen: &Screen, width: u16, height: u16, mut stdout: &mut Stdout) {
    // scroolbar
    // color
    write!(
        stdout,
        "{}",
        termion::color::Bg(termion::color::Rgb(0x00, 0x00, 0xff))
    )
    .unwrap();

    // spaces
    for h in 0..height + 1 {
        terminal_cursor_to(&mut stdout, width + 2, h + 3);
        write!(stdout, " ",).unwrap();
    }

    let off = screen.first_offset as f64;
    let max_size = screen.doc_max_offset as f64;

    let pos = ((off / max_size) * f64::from(height)) as u16;

    terminal_cursor_to(&mut stdout, width + 2, 3 + pos);

    write!(
        stdout,
        "{}{} {}",
        termion::color::Fg(termion::color::Rgb(0xff, 0xff, 0xff)),
        termion::style::Invert,
        termion::style::Reset,
    )
    .unwrap();
}
