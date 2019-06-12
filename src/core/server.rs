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

use std::sync::mpsc::Receiver;
use std::sync::mpsc::Sender;

use std::cell::RefCell;
use std::rc::Rc;
use std::time::Instant;

use crate::core::document;
use crate::core::editor::Editor;
use crate::core::event::Event;
use crate::core::event::Event::*;
use crate::core::event::EventMessage;
use crate::core::event::InputEvent;
use crate::core::event::Key;

use crate::core::view::layout::build_screen_layout;
use crate::core::view::{Id, View};

use crate::core::screen::Screen;

pub struct CoreState {
    keys: Vec<InputEvent>,
    pending_events: usize,
    quit: bool,
    status: String,
}

impl CoreState {
    fn new() -> Self {
        CoreState {
            keys: Vec::new(),
            pending_events: 0,
            quit: false,
            status: String::new(),
        }
    }
}

pub fn build_layout_and_send_event(
    editor: &mut Editor,
    mut core_state: &mut CoreState,
    ui_tx: &Sender<EventMessage>,
    doc_id: u64,
    view_id: u64,
) {
    let mut view = editor.view_map[view_id as usize].1.as_ref().borrow_mut();

    let start = Instant::now();
    fill_screen(&mut core_state, &mut view);
    let end = Instant::now();

    let mut new_screen = view.screen.clone();
    new_screen.time_to_build = end.duration_since(start);
    new_screen.input_size = core_state.pending_events;

    let msg = EventMessage::new(
        0, // get_next_seq(&mut seq), TODO
        BuildLayoutEvent {
            view_id: view_id as u64,
            doc_id,
            screen: new_screen,
        },
    );
    ui_tx.send(msg).unwrap_or(());
}

pub fn start(
    mut editor: &mut Editor,
    core_rx: &Receiver<EventMessage>,
    ui_tx: &Sender<EventMessage>,
) {
    let mut core_state = CoreState::new();

    let mut seq: usize = 0;

    fn get_next_seq(seq: &mut usize) -> usize {
        *seq = *seq + 1;
        *seq
    }

    while !core_state.quit {
        match core_rx.recv() {
            Ok(evt) => {
                match evt.event {
                    Event::ApplicationQuitEvent => {
                        break;
                    }

                    Event::RequestDocumentList => {
                        let mut list: Vec<(document::Id, String)> = vec![];
                        for e in &editor.document_map {
                            let name = &e.1.as_ref().borrow().name;
                            list.push((*e.0, name.to_string()));
                        }
                        let msg =
                            EventMessage::new(get_next_seq(&mut seq), Event::DocumentList { list });
                        ui_tx.send(msg).unwrap_or(());
                    }

                    Event::CreateView {
                        width,
                        height,
                        doc_id,
                    } => {
                        let vid = editor.view_map.len();
                        let doc = editor.document_map.get(&doc_id);
                        if let Some(doc) = doc {
                            let view =
                                View::new(vid as u64, 0 as u64, width, height, Some(doc.clone()));

                            editor.view_map.push((view.id, Rc::new(RefCell::new(view))));

                            let msg = EventMessage::new(
                                get_next_seq(&mut seq),
                                Event::ViewCreated {
                                    width,
                                    height,
                                    doc_id,
                                    view_id: vid as Id,
                                },
                            );
                            ui_tx.send(msg).unwrap_or(());
                        }
                    }

                    /*
                        <- createView : w, h , doc::id
                        -> viewCreate : view id, w, h, doc::id
                    */
                    /*
                        <- destroyView : w, h , doc::id
                        -> viewDestroyed : view id, w, h, doc::id
                    */
                    Event::RequestLayoutEvent {
                        view_id,
                        doc_id,
                        width,
                        height,
                    } => {
                        let view_id = view_id as usize;
                        if view_id < editor.view_map.len() {
                            {
                                let mut view = editor.view_map[view_id].1.as_ref().borrow_mut();

                                // resize ?
                                if width != view.screen.width || height != view.screen.height {
                                    view.screen = Box::new(Screen::new(width, height));
                                }
                            }

                            build_layout_and_send_event(
                                &mut editor,
                                &mut core_state,
                                ui_tx,
                                doc_id,
                                view_id as u64,
                            );
                        }

                        // is there a view/screen ?
                        // with the correct size ?
                        // alloc/resize screen
                    }

                    Event::InputEvent { events } => {
                        if !editor.view_map.is_empty() {
                            {
                                let view_id = 0 as usize;
                                let mut view = editor.view_map[view_id].1.as_ref().borrow_mut();
                                core_state.pending_events = events.len();
                                for ev in &events {
                                    process_input_events(&mut core_state, &mut view, &ui_tx, &ev);
                                    core_state.pending_events -= 1;
                                }
                                core_state.pending_events = events.len();
                            }

                            // TODO: is view changed only
                            if true {
                                build_layout_and_send_event(
                                    &mut editor,
                                    &mut core_state,
                                    ui_tx,
                                    0,
                                    0 as u64,
                                );
                            }
                        }
                    }

                    _ => {}
                }
            }
            _ => {}
        }
    }

    let msg = EventMessage::new(get_next_seq(&mut seq), Event::ApplicationQuitEvent);
    ui_tx.send(msg).unwrap_or(());
}

fn fill_screen(_core_state: &mut CoreState, view: &mut View) {
    if let Some(ref _buf) = view.document {
        let mut screen = &mut view.screen;

        screen.clear();

        let mut data = vec![];
        let max_size = (screen.width * screen.height * 4) as usize;
        let doc = view.document.as_ref().unwrap().borrow_mut();
        doc.read(view.start_offset, max_size, &mut data);

        let max_offset = doc.buffer.size as u64;

        view.end_offset = build_screen_layout(&data, view.start_offset, max_offset, &mut screen);

        // core_state.last_offset = view.end_offset;

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
                            // core_state.mark_offset = m.offset;
                        }
                    }
                }
            }
        }
    }
}

fn process_input_events(
    core_state: &mut CoreState,
    view: &mut View,
    _ui_tx: &Sender<EventMessage>,
    ev: &InputEvent,
) {
    if *ev == crate::core::event::InputEvent::NoInputEvent {
        // ignore no input event event :-)
        return;
    }

    core_state.keys.push(ev.clone());

    let mut clear_keys = true;
    match *ev {
        InputEvent::KeyPress {
            ctrl: true,
            alt: false,
            shift: false,
            key: Key::Unicode('q'),
        } => {
            core_state.quit = true;
        }

        InputEvent::KeyPress {
            ctrl: true,
            alt: false,
            shift: false,
            key: Key::Unicode('u'),
        } => {
            view.undo();
        }

        InputEvent::KeyPress {
            ctrl: true,
            alt: false,
            shift: false,
            key: Key::Unicode('r'),
        } => {
            view.redo();
        }

        InputEvent::KeyPress {
            ctrl: true,
            alt: false,
            shift: false,
            key: Key::Unicode('x'),
        } => {
            clear_keys = false;
        }

        // ctrl+a
        InputEvent::KeyPress {
            ctrl: true,
            alt: false,
            shift: false,
            key: Key::Unicode('a'),
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
            key: Key::Unicode('e'),
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
            key: Key::Unicode('d'),
        } => {
            view.remove_codepoint();
        }

        // ctrl+s
        InputEvent::KeyPress {
            ctrl: true,
            alt: false,
            shift: false,
            key: Key::Unicode('s'),
        } => {
            view.save_document();
        }

        // ctrl+k
        InputEvent::KeyPress {
            ctrl: true,
            alt: false,
            shift: false,
            key: Key::Unicode('k'),
        } => {
            view.cut_to_end_of_line();
        }

        // ctrl+y
        InputEvent::KeyPress {
            ctrl: true,
            alt: false,
            shift: false,
            key: Key::Unicode('y'),
        } => {
            view.paste();
        }

        // ctrl+l
        InputEvent::KeyPress {
            ctrl: true,
            alt: false,
            shift: false,
            key: Key::Unicode('l'),
        } => {
            view.center_arround_mark();
        }

        // ctrl+?
        InputEvent::KeyPress {
            ctrl: true,
            alt: false,
            shift: false,
            key: Key::Unicode(c),
        } => {
            core_state.status = format!("ctrl+<{}>", c);
        }

        // left
        InputEvent::KeyPress {
            ctrl: false,
            alt: false,
            shift: false,
            key: Key::Left,
        } => {
            view.move_marks_backward();

            core_state.status = "<left>".to_owned();
        }

        // up
        InputEvent::KeyPress {
            ctrl: false,
            alt: false,
            shift: false,
            key: Key::Up,
        } => {
            view.move_marks_to_previous_line();

            core_state.status = "<up>".to_owned();
        }

        // down
        InputEvent::KeyPress {
            ctrl: false,
            alt: false,
            shift: false,
            key: Key::Down,
        } => {
            view.move_marks_to_next_line();

            core_state.status = "<down>".to_owned();
        }

        // right
        InputEvent::KeyPress {
            ctrl: false,
            alt: false,
            shift: false,
            key: Key::Right,
        } => {
            view.move_marks_forward();

            core_state.status = "<right>".to_owned();
        }

        // page_up
        InputEvent::KeyPress {
            ctrl: false,
            alt: false,
            shift: false,
            key: Key::PageUp,
        } => {
            view.scroll_to_previous_screen();
            core_state.status = "<page_up>".to_owned();
        }

        // page_down
        InputEvent::KeyPress {
            ctrl: false,
            alt: false,
            shift: false,
            key: Key::PageDown,
        } => {
            view.scroll_to_next_screen();
            core_state.status = "<page_down>".to_owned();
        }

        // alt+< goto beginning of file
        InputEvent::KeyPress {
            ctrl: false,
            alt: true,
            shift: false,
            key: Key::Unicode('<'),
        } => {
            view.move_mark_to_beginning_of_file();
            core_state.status = "<move to beginning of file>".to_owned();
        }

        // alt+> goto end of file
        InputEvent::KeyPress {
            ctrl: false,
            alt: true,
            shift: false,
            key: Key::Unicode('>'),
        } => {
            view.move_mark_to_end_of_file();
            core_state.status = "<move to end of file>".to_owned();
        }

        // delete
        InputEvent::KeyPress {
            ctrl: false,
            alt: false,
            shift: false,
            key: Key::Delete,
        } => {
            view.remove_codepoint();
            core_state.status = "<del>".to_owned();
        }

        // backspace
        InputEvent::KeyPress {
            ctrl: false,
            alt: false,
            shift: false,
            key: Key::BackSpace,
        } => {
            view.remove_previous_codepoint();
            core_state.status = "<backspace>".to_owned();
        }

        // insert text
        InputEvent::KeyPress {
            ctrl: false,
            alt: false,
            shift: false,
            key: Key::Unicode(cp),
        } => {
            view.insert_codepoint(cp, core_state.pending_events);
            core_state.status = format!("<insert [0x{:x}]>", cp as u32);
        }

        // insert text block
        InputEvent::KeyPress {
            ctrl: false,
            alt: false,
            shift: false,
            key: Key::UnicodeArray(ref v),
        } => {
            view.insert_codepoint_array(&v);
        }

        // alt+d
        InputEvent::KeyPress {
            ctrl: false,
            alt: true,
            shift: false,
            key: Key::Unicode('d'),
        } => {
            view.remove_until_end_of_word();
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
                core_state.status = format!("<click({},@({},{}))]>", button, x, y - 2);
            }
            3 => {
                view.scroll_up(3);
                core_state.status = format!("<click({},@({},{}))]>", button, x, y - 2);
            }
            4 => {
                view.scroll_down(3);
                core_state.status = format!("<click({},@({},{}))]>", button, x, y - 2);
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
            core_state.status = format!("<unclick({},@({},{}))]>", button, x, y);
        }

        _ => {}
    }

    if clear_keys {
        core_state.keys.clear();
    }
}
