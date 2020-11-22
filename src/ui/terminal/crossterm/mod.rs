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
// extern crate ncurses;

use crate::dbg_println;

use std::io::{stdout, Write};

use crossterm::{
    cursor::{Hide, MoveTo, Show},
    event,
    event::{DisableMouseCapture, EnableMouseCapture},
    queue,
    style::Styler,
    style::{Attribute, Color, Print, ResetColor, SetAttribute, SetForegroundColor},
    terminal::{Clear, ClearType},
    Result,
};

use crossterm::{
    execute,
    terminal::{EnterAlternateScreen, LeaveAlternateScreen},
};

use std::thread;
use std::time::Duration;
use std::time::Instant;

use std::collections::HashMap;
use std::sync::mpsc::Receiver;
use std::sync::mpsc::Sender;

//
//
use crate::core::event::ButtonEvent;
use crate::core::event::Event;
use crate::core::event::Event::*;
use crate::core::event::PointerEvent;

use crate::core::event::EventMessage;
use crate::core::event::InputEvent;
use crate::core::screen::Screen;

use crate::core::event::Key;
use crate::core::event::KeyModifiers;

use crate::core::codepointinfo::CodepointInfo;

//
use crate::ui::UiState;

fn stdin_thread(tx: &Sender<EventMessage>) {
    loop {
        get_input_events(&tx).unwrap();
    }
}

pub fn main_loop(
    ui_rx: &Receiver<EventMessage>,
    _ui_tx: &Sender<EventMessage>,
    core_tx: &Sender<EventMessage>,
) -> Result<()> {
    let mut seq: usize = 0;

    fn get_next_seq(seq: &mut usize) -> usize {
        *seq += 1;
        *seq
    }

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

    let mut request_layout = false;

    let mut stdout = stdout();

    execute!(stdout, EnterAlternateScreen)?;

    execute!(
        stdout,
        EnableMouseCapture,
        Hide,
        SetAttribute(Attribute::Reset),
        Clear(ClearType::All)
    )?;

    crossterm::terminal::enable_raw_mode()?;

    let mut frame_skipped = false;

    while !ui_state.quit {
        // check terminal size
        let (width, height) = crossterm::terminal::size().ok().unwrap();

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

        if let Ok(evt) = ui_rx.recv_timeout(Duration::from_millis(100)) {
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
                        ui_state.resize_flag = false;
                    }

                    let p = crate::core::event::pending_render_event_count();
                    if p <= 1 {
                        let s = Instant::now();
                        draw_view(&mut last_screen, &mut screen, &mut stdout, frame_skipped);
                        let e = Instant::now();
                        dbg_println!("time to draw view = {}\r", (e - s).as_millis());
                        frame_skipped = false;
                    } else {
                        frame_skipped = true;
                    }

                    if p > 0 {
                        crate::core::event::pending_render_event_dec(1);
                    }

                    dbg_println!("pending render events = {}\r", p);

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

    /* Terminate crossterm */
    /* Terminate crossterm */
    execute!(
        stdout,
        SetAttribute(Attribute::Reset),
        Clear(ClearType::All),
        DisableMouseCapture,
        Show
    )?;

    execute!(stdout, LeaveAlternateScreen,)?;

    crossterm::terminal::disable_raw_mode()?;

    Ok(())
}

/*
    TODO:
    1 : be explicit
    2 : create editor internal result type Result<>
    3 : use idomatic    func()? style
*/
fn draw_view(
    last_screen: &mut Screen,
    mut screen: &mut Screen,
    mut stdout: &mut std::io::Stdout,
    full_redraw: bool,
) {
    if full_redraw {
        let _ = draw_screen_dumb(last_screen, &mut screen, &mut stdout);
    } else {
        let _ = draw_screen(last_screen, &mut screen, &mut stdout);
    }
}

fn draw_screen_dumb(
    _last_screen: &mut Screen,
    screen: &mut Screen,
    stdout: &mut std::io::Stdout,
) -> Result<()> {
    queue!(stdout, ResetColor)?;

    for li in 0..screen.height() {
        queue!(stdout, MoveTo(0, li as u16))?;

        let line = screen.get_line(li).unwrap();

        for c in 0..line.width() {
            let cpi = line.get_cpi(c).unwrap();

            // color
            let color = Color::Rgb {
                r: cpi.color.0,
                g: cpi.color.1,
                b: cpi.color.2,
            };
            queue!(stdout, SetForegroundColor(color))?;

            // draw with style
            let s = cpi.displayed_cp.to_string();
            if cpi.is_selected {
                queue!(stdout, ::crossterm::style::PrintStyledContent(s.reverse()))?;
            } else {
                queue!(stdout, Print(cpi.displayed_cp))?;
            }
        }
    }

    /* Update the screen. */
    stdout.flush()?;

    Ok(())
}

fn screen_changed(screen0: &Screen, screen1: &Screen) -> bool {
    !(false
        || (screen0.nb_push > 0 && screen0.first_offset != screen1.first_offset)
        || screen0.max_width() != screen1.max_width()
        || screen0.max_height() != screen1.max_height())
}

fn draw_screen(
    last_screen: &mut Screen,
    screen: &mut Screen,
    stdout: &mut std::io::Stdout,
) -> Result<()> {
    let mut prev_cpi = CodepointInfo::new();

    let check_hash = screen_changed(&last_screen, &screen);

    // set default color
    {
        let color = Color::Rgb {
            r: prev_cpi.color.0,
            g: prev_cpi.color.1,
            b: prev_cpi.color.2,
        };
        queue!(
            stdout,
            SetAttribute(Attribute::Reset),
            SetForegroundColor(color)
        )?;
    }

    for l in 0..screen.max_height() {
        let line = screen.get_mut_unclipped_line(l).unwrap();

        let mut have_cursor = false;
        for c in 0..line.max_width() {
            let cpi = line.get_unclipped_cpi(c).unwrap();

            if cpi.is_selected {
                have_cursor = true;
                break;
            }
        }

        if check_hash {
            // TODO: check attr change
            let prev_line = last_screen.get_mut_unclipped_line(l).unwrap();
            for c in 0..prev_line.width() {
                let cpi = prev_line.get_unclipped_cpi(c).unwrap();
                if cpi.is_selected {
                    have_cursor = true;
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
        queue!(stdout, MoveTo(0, l as u16))?;

        // draw new content
        for c in 0..line.max_width() {
            let cpi = line.get_unclipped_cpi(c).unwrap();

            // color
            let mut set_color = false;

            // default style
            if cpi.is_selected && prev_cpi.is_selected == false {
                queue!(stdout, SetAttribute(Attribute::Reverse))?;
            } else {
                if prev_cpi.is_selected == true {
                    queue!(stdout, SetAttribute(Attribute::Reset))?;
                }
                set_color = true;
            }

            // detect color change
            if prev_cpi.color != cpi.color {
                set_color = true;
            }

            if set_color {
                let color = Color::Rgb {
                    r: cpi.color.0,
                    g: cpi.color.1,
                    b: cpi.color.2,
                };
                queue!(stdout, SetForegroundColor(color))?;
            }

            // draw character
            queue!(stdout, Print(cpi.displayed_cp))?;

            prev_cpi = *cpi;
        }
    }

    // Update the screen
    stdout.flush()?;

    Ok(())
}

fn translate_crossterm_key_modifier(km: ::crossterm::event::KeyModifiers) -> KeyModifiers {
    KeyModifiers {
        ctrl: (km.bits() & event::KeyModifiers::CONTROL.bits()) != 0,
        alt: (km.bits() & event::KeyModifiers::ALT.bits()) != 0,
        shift: (km.bits() & event::KeyModifiers::SHIFT.bits()) != 0,
    }
}

fn key_modifiers_no_shift(km: ::crossterm::event::KeyModifiers) -> KeyModifiers {
    KeyModifiers {
        ctrl: (km.bits() & event::KeyModifiers::CONTROL.bits()) != 0,
        alt: (km.bits() & event::KeyModifiers::ALT.bits()) != 0,
        shift: false,
    }
}

fn translate_crossterm_mouse_button(button: ::crossterm::event::MouseButton) -> u32 {
    match button {
        ::crossterm::event::MouseButton::Left => 0,
        ::crossterm::event::MouseButton::Right => 1,
        ::crossterm::event::MouseButton::Middle => 2,
        // self::termion::event::MouseButton::WheelUp => 3,
        // self::termion::event::MouseButton::WheelDown => 4,
    } //
}

fn translate_crossterm_event(evt: ::crossterm::event::Event) -> InputEvent {
    // translate termion event
    match evt {
        ::crossterm::event::Event::Key(ke) => match ke.code {
            ::crossterm::event::KeyCode::Char(c) => {
                return InputEvent::KeyPress {
                    mods: key_modifiers_no_shift(ke.modifiers),
                    key: Key::Unicode(c),
                };
            }

            ::crossterm::event::KeyCode::Backspace => {
                return InputEvent::KeyPress {
                    mods: translate_crossterm_key_modifier(ke.modifiers),
                    key: Key::BackSpace,
                };
            }

            ::crossterm::event::KeyCode::Enter => {
                return InputEvent::KeyPress {
                    mods: translate_crossterm_key_modifier(ke.modifiers),
                    key: Key::Unicode('\n'),
                };
            }

            ::crossterm::event::KeyCode::Left => {
                return InputEvent::KeyPress {
                    mods: translate_crossterm_key_modifier(ke.modifiers),
                    key: Key::Left,
                };
            }

            ::crossterm::event::KeyCode::Right => {
                return InputEvent::KeyPress {
                    mods: translate_crossterm_key_modifier(ke.modifiers),
                    key: Key::Right,
                };
            }

            ::crossterm::event::KeyCode::Up => {
                return InputEvent::KeyPress {
                    mods: translate_crossterm_key_modifier(ke.modifiers),
                    key: Key::Up,
                };
            }
            ::crossterm::event::KeyCode::Down => {
                return InputEvent::KeyPress {
                    mods: translate_crossterm_key_modifier(ke.modifiers),
                    key: Key::Down,
                };
            }

            ::crossterm::event::KeyCode::Home => {
                return InputEvent::KeyPress {
                    mods: translate_crossterm_key_modifier(ke.modifiers),
                    key: Key::Home,
                };
            }

            ::crossterm::event::KeyCode::End => {
                return InputEvent::KeyPress {
                    mods: translate_crossterm_key_modifier(ke.modifiers),
                    key: Key::End,
                };
            }

            ::crossterm::event::KeyCode::PageUp => {
                return InputEvent::KeyPress {
                    mods: translate_crossterm_key_modifier(ke.modifiers),
                    key: Key::PageUp,
                };
            }
            ::crossterm::event::KeyCode::PageDown => {
                return InputEvent::KeyPress {
                    mods: translate_crossterm_key_modifier(ke.modifiers),
                    key: Key::PageDown,
                };
            }

            ::crossterm::event::KeyCode::Tab => {
                return InputEvent::KeyPress {
                    mods: translate_crossterm_key_modifier(ke.modifiers),
                    key: Key::Unicode('\t'),
                };
            }

            ::crossterm::event::KeyCode::BackTab => {
                return InputEvent::NoInputEvent;
            }

            ::crossterm::event::KeyCode::Delete => {
                return InputEvent::KeyPress {
                    mods: translate_crossterm_key_modifier(ke.modifiers),
                    key: Key::Delete,
                };
            }

            ::crossterm::event::KeyCode::Insert => {
                return InputEvent::KeyPress {
                    mods: translate_crossterm_key_modifier(ke.modifiers),
                    key: Key::Insert,
                };
            }

            ::crossterm::event::KeyCode::F(n) => {
                return InputEvent::KeyPress {
                    mods: translate_crossterm_key_modifier(ke.modifiers),
                    key: Key::F(n as usize),
                };
            }

            ::crossterm::event::KeyCode::Null => {
                return InputEvent::KeyPress {
                    mods: translate_crossterm_key_modifier(ke.modifiers),
                    key: Key::Unicode('\0'),
                };
            }

            ::crossterm::event::KeyCode::Esc => {
                return InputEvent::KeyPress {
                    mods: translate_crossterm_key_modifier(ke.modifiers),
                    key: Key::Escape,
                };
            }
        },

        ::crossterm::event::Event::Mouse(event) => match event {
            ::crossterm::event::MouseEvent::Down(button, col, row, mods) => {
                return InputEvent::ButtonPress(ButtonEvent {
                    mods: translate_crossterm_key_modifier(mods),
                    x: i32::from(col),
                    y: i32::from(row),
                    button: translate_crossterm_mouse_button(button),
                });
            }

            ::crossterm::event::MouseEvent::Up(button, col, row, mods) => {
                return InputEvent::ButtonRelease(ButtonEvent {
                    mods: translate_crossterm_key_modifier(mods),
                    x: i32::from(col),
                    y: i32::from(row),
                    button: translate_crossterm_mouse_button(button),
                });
            }

            ::crossterm::event::MouseEvent::ScrollUp(col, row, _mods) => {
                return InputEvent::WheelUp {
                    mods: KeyModifiers::new(),
                    x: i32::from(col),
                    y: i32::from(row),
                };
            }

            ::crossterm::event::MouseEvent::ScrollDown(col, row, _mods) => {
                return InputEvent::WheelDown {
                    mods: KeyModifiers::new(),
                    x: i32::from(col),
                    y: i32::from(row),
                };
            }

            ::crossterm::event::MouseEvent::Drag(_button, col, row, _mods) => {
                return InputEvent::PointerMotion(PointerEvent {
                    mods: KeyModifiers::new(),
                    x: i32::from(col),
                    y: i32::from(row),
                });
            }
        },

        ::crossterm::event::Event::Resize(_width, _height) => {
            // println!("New size {}x{}", width, height)
        }
    }

    return InputEvent::NoInputEvent;
}

fn send_input_events(accum: &Vec<InputEvent>, tx: &Sender<EventMessage>) {
    let mut v = Vec::<InputEvent>::new();

    // merge consecutive characters as "array" of chars
    let mut codepoints = Vec::<char>::new();
    for evt in accum {
        match evt {
            InputEvent::KeyPress {
                key: Key::Unicode(c),
                mods:
                    KeyModifiers {
                        ctrl: false,
                        alt: false,
                        shift: false,
                    },
            } => {
                codepoints.push(*c);
            }

            _ => {
                // flush previous codepoints
                if !codepoints.is_empty() {
                    v.push(InputEvent::KeyPress {
                        key: Key::UnicodeArray(codepoints),
                        mods: KeyModifiers {
                            ctrl: false,
                            alt: false,
                            shift: false,
                        },
                    });
                    codepoints = Vec::<char>::new();
                }

                // other events
                v.push(evt.clone());
            }
        }
    }

    // append
    if !codepoints.is_empty() {
        v.push(InputEvent::KeyPress {
            key: Key::UnicodeArray(codepoints),
            mods: KeyModifiers {
                ctrl: false,
                alt: false,
                shift: false,
            },
        });
    }

    // send
    if !v.is_empty() {
        let ev_count = v.len();
        let msg = EventMessage::new(
            0,
            Event::InputEvent {
                events: v,
                raw_data: None,
            },
        );
        crate::core::event::pending_input_event_inc(ev_count);
        tx.send(msg).unwrap_or(());
    }
}

fn get_input_events(tx: &Sender<EventMessage>) -> ::crossterm::Result<()> {
    let mut accum = Vec::<InputEvent>::with_capacity(4096);

    let mut start = Instant::now();
    let mut wait_ms = 1000;
    let mut flush_ms = 16;

    loop {
        if ::crossterm::event::poll(Duration::from_millis(wait_ms))? {
            // nr pending ?

            if let Ok(cross_evt) = ::crossterm::event::read() {
                // move to send input events ?
                dbg_println!("receive crossterm event {:?}\r", cross_evt);
                let evt = translate_crossterm_event(cross_evt);
                accum.push(evt);

                if accum.len() > 255 {
                    flush_ms = ::std::cmp::min(flush_ms + 500, 1000);
                    wait_ms = flush_ms;
                } else {
                    wait_ms = 1;
                }

                let el = start.elapsed();
                if el > Duration::from_millis(flush_ms) {
                    break;
                }
            } else {
                // dbg_println!("read error ?");
            }
        } else {
            // timeout
            if !accum.is_empty() {
                break;
            }
            start = Instant::now();
        }
    }

    if !accum.is_empty() {
        send_input_events(&accum, tx);
    }
    Ok(())
}
