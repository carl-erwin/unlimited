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
extern crate ncurses;

use self::ncurses::*;

use std::thread;
use std::time::Duration;
use std::time::Instant;

use std::collections::HashMap;
use std::sync::mpsc::Receiver;
use std::sync::mpsc::Sender;

//
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
        *seq += 1;
        *seq
    }

    // front-end init code {----
    // init termion
    /* If your locale env is unicode, you should use `setlocale`. */
    // let locale_conf = LcCategory::all;
    // setlocale(locale_conf, "zh_CN.UTF-8"); // if your locale is like mine(zh_CN.UTF-8).
    // Start ncurses
    initscr();
    curs_set(CURSOR_VISIBILITY::CURSOR_INVISIBLE);
    keypad(stdscr(), true); /* for F1, arrow etc ... */
    noecho();
    raw();
    //    halfdelay(1); // 20ms
    clear();

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

    while !ui_state.quit {
        // check terminal size
        let (width, height) = {
            let mut width: i32 = 0;
            let mut height: i32 = 0;
            getmaxyx(stdscr(), &mut height, &mut width);
            (width as u16, height as u16)
        };

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
                        clear();
                        ui_state.resize_flag = false;
                    }

                    draw_view(&mut last_screen, &mut screen);

                    let end = Instant::now();
                    _prev_screen_rdr_time = end.duration_since(start);
                    last_screen = screen;
                }

                _ => {}
            }
        } else {
            // TODO: handle timeout
        }

        // get_input_events(&core_tx);
    }

    /* Terminate ncurses. */
    endwin();
}

/*
    TODO:
    1 : be explicit
    2 : create editor internal result type Result<>
    3 : use idomatic    func()? style
*/
fn draw_view(last_screen: &mut Screen, mut screen: &mut Screen) {
    draw_screen(last_screen, &mut screen);
}

fn draw_screen(last_screen: &mut Screen, screen: &mut Screen) {
    clear();

    for li in 0..screen.height() {
        mv(1, li as i32);

        let line = screen.get_line(li).unwrap();

        for c in 0..line.width() {
            let cpi = line.get_cpi(c).unwrap();

            if cpi.is_selected {
                attron(A_REVERSE());
            }
            if cpi.cp < 128 as char {
                mvaddch(li as i32, c as i32, cpi.displayed_cp as u32);
            } else {
                //                u8 buff[8];
                //                utf8_put_cp(cpi->codepoint, buff);
                //                mvaddstr(li, c, (const char *)buff);
            }

            if cpi.is_selected {
                attroff(A_REVERSE());
            }
        }

        /*
        for _ in line.nb_cells..line.width {
            write!(stdout, " ").unwrap();
        }
        */
    }

    /* Update the screen. */
    refresh();
}

fn get_input_events(tx: &Sender<EventMessage>) {
    let mut v = Vec::<InputEvent>::new();

    let keycode = wgetch(stdscr()); /* Wait for user input */
    // FIXME: use timer to check core event
    if keycode == ERR {
        return;
    }

    let _name = keyname(keycode);

    match keycode {
        KEY_UP => {
            v.push(InputEvent::KeyPress {
                ctrl: false,
                alt: false,
                shift: false,
                key: Key::Up,
            });
        }

        KEY_DOWN => {
            v.push(InputEvent::KeyPress {
                ctrl: false,
                alt: false,
                shift: false,
                key: Key::Down,
            });
        }

        KEY_LEFT => {
            v.push(InputEvent::KeyPress {
                ctrl: false,
                alt: false,
                shift: false,
                key: Key::Left,
            });
        }
        KEY_RIGHT => {
            v.push(InputEvent::KeyPress {
                ctrl: false,
                alt: false,
                shift: false,
                key: Key::Right,
            });
        }

        KEY_PPAGE => {
            v.push(InputEvent::KeyPress {
                ctrl: false,
                alt: false,
                shift: false,
                key: Key::PageUp,
            });
        }

        KEY_NPAGE => {
            v.push(InputEvent::KeyPress {
                ctrl: false,
                alt: false,
                shift: false,
                key: Key::PageDown,
            });
        }

        0...26 => {
            let mut ctrl = false;
            let c;

            let k = unsafe { ::std::char::from_u32_unchecked(keycode as u32 + ('a' as u32) - 1) };
            match k {
                'j' => {
                    c = '\n';
                }

                'i' => {
                    c = '\t';
                }

                _ => {
                    c = unsafe { ::std::char::from_u32_unchecked(k as u32) };
                    ctrl = true;
                }
            }

            v.push(InputEvent::KeyPress {
                ctrl: ctrl,
                alt: false,
                shift: false,
                key: Key::Unicode(c),
            });
        }

        27...127 => {
            /* TODO */
            let k = unsafe { ::std::char::from_u32_unchecked(keycode as u32) };
            v.push(InputEvent::KeyPress {
                ctrl: false,
                alt: false,
                shift: false,
                key: Key::Unicode(k),
            });
        }

        _ => {}
    } // ! switch(keycode)

    if !v.is_empty() {
        let msg = EventMessage::new(
            0,
            Event::InputEvent {
                events: v,
                raw_data: None,
            },
        );
        tx.send(msg).unwrap_or(());
    }
}
