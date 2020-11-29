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
use std::collections::HashMap;
use std::rc::Rc;
use std::time::Duration;
use std::time::Instant;

use crate::core::document;
use crate::core::editor::Editor;
use crate::core::event::Event;
use crate::core::event::Event::BuildLayoutEvent;

use crate::core::event::EventMessage;
use crate::core::event::InputEvent;

use crate::core::view::{Id, View};

use crate::core::view;
use crate::core::view::update_view;

use crate::core::codepointinfo::CodepointInfo;
use crate::core::screen::Screen;

use crate::core::event::InputEventMap;
use crate::core::event::InputEventRule;

use crate::core::event::input_map::build_input_event_map;
use crate::core::event::input_map::eval_input_event;

type ActionMap = HashMap<String, view::ModeFunction>;

static DEFAULT_INPUT_MAP: &str = r#"[{
    "events": [
       { "in": [{ "key": "Left"     }],                        "action": "text-mode:move-marks-backward" },
       { "in": [{ "key": "Right"    }],                        "action": "text-mode:move-marks-forward" },
       { "in": [{ "key": "Up"       }],                        "action": "text-mode:move-marks-to-previous-line" },
       { "in": [{ "key": "Down"     }],                        "action": "text-mode:move-marks-to-next-line" },
       { "in": [{ "key": "PageUp"   }],                        "action": "text-mode:page-up" },
       { "in": [{ "key": "PageDown" }],                        "action": "text-mode:page-down" },
       
       { "in": [{ "key": "ctrl+a" }],                          "action": "text-mode:move-marks-to-beginning-of-line" },
       { "in": [{ "key": "ctrl+e" }],                          "action": "text-mode:move-marks-to-end-of-line" },
       { "in": [{ "key": "Home" }],                            "action": "text-mode:move-marks-to-beginning-of-line" },
       { "in": [{ "key": "End" }],                             "action": "text-mode:move-marks-to-end-of-line" },


       { "in": [{ "key": "alt+<" }],                           "action": "text-mode:move-marks-to-beginning-of-file" },
       { "in": [{ "key": "alt+>" }],                           "action": "text-mode:move-marks-to-end-of-file" },

       { "in": [{ "key": "ctrl+Home" }],                      "action": "text-mode:move-marks-to-beginning-of-file" },
       { "in": [{ "key": "ctrl+End" }],                       "action": "text-mode:move-marks-to-end-of-file" },


       { "in": [{ "key": "ctrl+u" }],                          "action": "text-mode:undo" },
       { "in": [{ "key": "ctrl+r" }],                          "action": "text-mode:redo" },
       { "in": [{ "key": "ctrl+d" }],                          "action": "text-mode:remove-codepoint" },
       { "in": [{ "key": "Delete" }],                          "action": "text-mode:remove-codepoint" },
       { "in": [{ "key": "BackSpace" }],                       "action": "text-mode:remove-previous-codepoint" },

       { "in": [{ "key": "alt+d" }],                           "action": "text-mode:remove-until-end-of-word" },
       { "in": [{ "key": "ctrl+Delete" }],                     "action": "text-mode:remove-until-end-of-word" },

       { "in": [{ "key": "ctrl+k" }],                          "action": "text-mode:cut-to-end-of-line" },
       { "in": [{ "key": "ctrl+y" }],                          "action": "text-mode:paste" },

       { "in": [{ "key": "ctrl+l" }],                          "action": "text-mode:center-arround-mark" },

       { "in": [{ "key": "ctrl+Left"  }],                      "action": "text-mode:move-to-previous-token-beginning" },
       { "in": [{ "key": "ctrl+Right" }],                      "action": "text-mode:move-to-next-token-end" },

       { "in": [{ "key": "ctrl+Up"    }],                      "action": "text-mode:scroll-up" },
       { "in": [{ "key": "ctrl+Down"  }],                      "action": "text-mode:scroll-down" },

       { "in": [{ "wheel": "Up"       }],                      "action": "text-mode:scroll-up" },
       { "in": [{ "wheel": "Down"     }],                      "action": "text-mode:scroll-down" },


       { "in": [{ "key": "ctrl+alt+Left"     }],               "action": "text-mode:move-mark-backward-word" },
       { "in": [{ "key": "ctrl+alt+Right"     }],              "action": "text-mode:move-mark-one-forward" },
       
       { "in": [{ "button-press":  "0"   }],                   "action": "text-mode:move-mark-to-clicked-area" },
       { "in": [{ "button-release": "0"  }],                   "action": "text-mode:ignore" },


       { "in": [{ "key": "ctrl+s" }],                           "action": "save-document" },

       { "in": [{ "key": "Esc"      }],                        "action": "editor:cancel" },

       { "in": [{ "key": "ctrl+q"   }],                        "action": "application:quit" },
       { "in": [{ "key": "ctrl+x" }, { "key": "ctrl+c" } ],    "action": "application:quit" },
       { "in": [{ "key": "F4" } ],                             "action": "application:quit-abort" },

       { "in": [{ "system": "SIGTERM" } ],                      "action": "application:quit" },

       { "default": [],                                         "action": "text-mode:self-insert" }
     ]
}]"#;

// env.repeat_action_n , api to set repeat
// ctrl+:  -> minor mode to read repeat count
// esc -> reset repeat count
// kbr macro recording
pub struct EditorEnv {
    quit: bool,
    status: String, // TODO: move to test-mode

    action_map: ActionMap,

    input_map: Rc<RefCell<InputEventMap>>,

    current_node: Option<Rc<InputEventRule>>,
    next_node: Option<Rc<InputEventRule>>,
}

impl EditorEnv {
    fn new() -> Self {
        let input_map = if let Ok(map) = build_input_event_map(DEFAULT_INPUT_MAP) {
            map
        } else {
            Rc::new(RefCell::new(HashMap::new()))
        };

        EditorEnv {
            quit: false,
            status: String::new(),
            action_map: build_action_map(),
            input_map,
            current_node: None,
            next_node: None,
        }
    }
}

pub fn build_layout(editor: &mut Editor, mut env: &mut EditorEnv, view_id: u64) {
    let view = editor.view_map[view_id as usize].1.clone();

    let start = Instant::now();
    update_view(&view, &mut env);
    let end = Instant::now();
    view.as_ref().borrow_mut().screen.time_to_build = end.duration_since(start);
}

pub fn build_layout_and_send_event(
    mut editor: &mut Editor,
    mut env: &mut EditorEnv,
    ui_tx: &Sender<EventMessage>,
    doc_id: u64,
    view: Rc<RefCell<View>>,
) {
    let view_id = {
        let view = view.borrow();
        view.id
    };

    // prepare filter
    // setup filter ctx
    // push filter vec
    // s/fill_scren/run_filter/

    // build_layout
    let start = Instant::now();
    build_layout(&mut editor, &mut env, view_id);

    // clone view's screen to send
    let view = view.borrow();

    let mut new_screen = view.screen.clone(); // Rc() ? Cow ?
    let end = Instant::now();
    new_screen.time_to_build = end.duration_since(start);

    // and send it
    let msg = EventMessage::new(
        0, // get_next_seq(&mut seq), TODO
        BuildLayoutEvent {
            view_id: view.id as u64,
            doc_id,
            screen: new_screen,
        },
    );
    ui_tx.send(msg).unwrap_or(());
}

pub fn send_build_layout_event(
    editor: &mut Editor,
    ui_tx: &Sender<EventMessage>,
    doc_id: u64,
    view_id: u64,
) {
    let view = editor.view_map[view_id as usize].1.as_ref().borrow_mut();
    let new_screen = view.screen.clone();

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

pub fn run(
    mut editor: &mut Editor,
    core_rx: &Receiver<EventMessage>,
    ui_tx: &Sender<EventMessage>,
) {
    let mut env = EditorEnv::new();

    let mut seq: usize = 0;

    fn get_next_seq(seq: &mut usize) -> usize {
        *seq += 1;
        *seq
    }

    while !env.quit {
        if let Ok(evt) = core_rx.recv() {
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
                            if width != view.screen.width() || height != view.screen.height() {
                                view.screen = Box::new(Screen::new(width, height));
                            }
                        }

                        let view = editor.view_map[view_id].1.clone();

                        build_layout_and_send_event(&mut editor, &mut env, ui_tx, doc_id, view);
                    }

                    // is there a view/screen ?
                    // with the correct size ?
                    // alloc/resize screen
                }

                Event::InputEvent { events, raw_data } => {
                    if !editor.view_map.is_empty() {
                        let view_id = 0 as usize;

                        process_input_events(
                            &mut editor,
                            &mut env,
                            view_id,
                            &ui_tx,
                            &events,
                            &raw_data,
                        );
                    }
                }

                _ => {}
            }
        }
    }

    let msg = EventMessage::new(get_next_seq(&mut seq), Event::ApplicationQuitEvent);
    ui_tx.send(msg).unwrap_or(());
}

fn _print_clipped_line(screen: &mut Screen, color: (u8, u8, u8), s: &str) {
    let mut nb_push = 0;
    for c in s.chars().take(screen.width()) {
        let mut cpi = CodepointInfo::new();
        cpi.metadata = true;
        cpi.is_selected = true;
        cpi.cp = c;
        cpi.displayed_cp = c;
        cpi.color = color;
        screen.push(cpi);
        nb_push += 1;
    }
    // fill line
    for _ in nb_push..screen.width() {
        let mut cpi = CodepointInfo::new();
        cpi.metadata = true;
        cpi.is_selected = true;

        cpi.cp = ' ';
        cpi.displayed_cp = ' ';
        cpi.color = color;
        screen.push(cpi);
    }
}

fn process_input_event(
    editor: &mut Editor,
    mut env: &mut EditorEnv,
    view_id: usize,
    ev: &InputEvent,
) -> bool {
    let mut view = &editor.view_map[view_id].1;

    if *ev == crate::core::event::InputEvent::NoInputEvent {
        // ignore no input event event :-)
        env.status = "no input event".to_string();
        return false;
    }

    let action = eval_input_event(
        &ev,
        &env.input_map,
        &mut env.current_node, // TODO: EvalCtx
        &mut env.next_node,    // TODO: EvalCtx
    );

    let trigger = vec![(*ev).clone()];

    if let Some(action) = action {
        let start = Instant::now();
        dbg_println!("found action {} : input ev = {:?}", action, ev);

        match action.as_str() {
            "application:quit" => {
                env.status = "<quit>".to_string();

                let doc = &view.as_ref().borrow();
                let doc = doc.document.as_ref().unwrap();
                if doc.borrow().changed {
                    env.status = "<quit> : modified buffer exits. type F4 to quit without saving"
                        .to_string();
                } else {
                    env.quit = true;
                }
            }

            "application:quit-abort" => {
                env.quit = true;
            }

            "save-document" => {
                view::save_document(&trigger, view);
                env.status = "<save>".to_string();
            }

            _ => {
                // TODO: pattern match type of action base on domain or augment mode callbacks cb(e,c,d,v, trigger, env? {k,v}*)
                // applicatoin:

                // else
                if let Some(action) = env.action_map.get(&action) {
                    action(&trigger, &mut view);
                }
            }
        }

        let end = Instant::now();

        dbg_println!("time to run action {}", (end - start).as_millis());
    } else {
        // TODO: move to caller ?
        // add eval_ctx::new to mask impl of node swapping
        std::mem::swap(&mut env.current_node, &mut env.next_node);
    }

    true
}

fn process_input_events(
    mut editor: &mut Editor,
    mut env: &mut EditorEnv,
    view_id: usize,
    ui_tx: &Sender<EventMessage>,
    events: &Vec<InputEvent>,
    _raw_data: &Option<Vec<u8>>, // TODO: remove
) {
    let p = crate::core::event::pending_input_event_count();

    for ev in events {
        let event_processed = process_input_event(&mut editor, &mut env, view_id, ev);

        if event_processed {
            let start = Instant::now();
            build_layout(&mut editor, &mut env, view_id as u64);
            let end = Instant::now();
            dbg_println!("time to build layout = {} ms\r", (end - start).as_millis());
        }

        if p > 0 {
            crate::core::event::pending_input_event_dec(1);
        }
    }

    let p = crate::core::event::pending_input_event_count();
    dbg_println!("pending input event = {}\r", p);

    // % last render time
    if p > 1 && editor.last_rdr_event.elapsed() < Duration::from_millis(1000 / 5) {
        return;
    }

    // hit
    crate::core::event::pending_render_event_inc(1);
    send_build_layout_event(&mut editor, ui_tx, 0, 0 as u64);
    editor.last_rdr_event = Instant::now();
}

fn register_action(map: &mut ActionMap, s: &str, func: view::ModeFunction) {
    map.insert(s.to_string(), func);
}

fn build_action_map() -> ActionMap {
    let mut map: ActionMap = HashMap::new(); // text-mode action map

    // TODO: text-mode
    register_action(
        &mut map,
        "text-mode:self-insert",
        view::insert_codepoint_array,
    );
    register_action(
        &mut map,
        "text-mode:move-marks-backward",
        view::move_marks_backward,
    );
    register_action(
        &mut map,
        "text-mode:move-marks-forward",
        view::move_marks_forward,
    );
    register_action(
        &mut map,
        "text-mode:move-marks-to-next-line",
        view::move_marks_to_next_line,
    );
    register_action(
        &mut map,
        "text-mode:move-marks-to-previous-line",
        view::move_marks_to_previous_line,
    );
    /*
        register_action(
            &mut map,
            "text-mode:move-to-previous-token-beginning",
            view::move_to_next_token_end,
        );
    */

    register_action(
        &mut map,
        "text-mode:move-to-next-token-end",
        view::move_to_next_token_end,
    );

    register_action(
        &mut map,
        "text-mode:page-up",
        view::scroll_to_previous_screen,
    );
    register_action(&mut map, "text-mode:page-down", view::scroll_to_next_screen);

    register_action(&mut map, "text-mode:scroll-up", view::scroll_up);
    register_action(&mut map, "text-mode:scroll-down", view::scroll_down);

    register_action(
        &mut map,
        "text-mode:move-marks-to-beginning-of-line",
        view::move_marks_to_beginning_of_line,
    );
    register_action(
        &mut map,
        "text-mode:move-marks-to-end-of-line",
        view::move_marks_to_end_of_line,
    );

    register_action(
        &mut map,
        "text-mode:move-marks-to-beginning-of-file",
        view::move_mark_to_beginning_of_file,
    );
    register_action(
        &mut map,
        "text-mode:move-marks-to-end-of-file",
        view::move_mark_to_end_of_file,
    );

    register_action(&mut map, "text-mode:undo", view::undo);
    register_action(&mut map, "text-mode:redo", view::redo);
    register_action(
        &mut map,
        "text-mode:remove-codepoint",
        view::remove_codepoint,
    );
    register_action(
        &mut map,
        "text-mode:remove-previous-codepoint",
        view::remove_previous_codepoint,
    );

    register_action(&mut map, "text-mode:button-press", view::button_press);
    register_action(&mut map, "text-mode:button-release", view::button_release);
    register_action(
        &mut map,
        "text-mode:move-mark-to-clicked-area",
        view::button_press,
    );

    register_action(
        &mut map,
        "text-mode:center-arround-mark",
        view::center_arround_mark,
    );
    register_action(
        &mut map,
        "text-mode:cut-to-end-of-line",
        view::cut_to_end_of_line,
    );

    register_action(&mut map, "text-mode:paste", view::paste);
    register_action(
        &mut map,
        "text-mode:remove-until-end-of-word",
        view::remove_until_end_of_word,
    );
    register_action(&mut map, "save-document", view::save_document);
    register_action(
        &mut map,
        "scroll-to-next-screen",
        view::scroll_to_next_screen,
    );
    register_action(
        &mut map,
        "scroll-to-previous-screen",
        view::scroll_to_previous_screen,
    );

    map
}
