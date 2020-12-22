// Copyright (c) Carl-Erwin Griffith

use std::sync::mpsc::Receiver;
use std::sync::mpsc::Sender;

use std::cell::RefCell;
use std::collections::HashMap;
use std::rc::Rc;
use std::sync::Arc;
use std::sync::RwLock;

use std::time::Duration;
use std::time::Instant;

use crate::core::editor::Editor;
use crate::core::event::Event;
use crate::core::event::Event::DrawEvent;

use crate::core::event::EventMessage;
use crate::core::event::InputEvent;

use crate::core::view::View;

use crate::core::view;
use crate::core::view::update_view;

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
       { "in": [{ "key": "alt+shift+Up" }],                  "action": "text-mode:clone-and-move-mark-to-previous-line" },

       { "in": [{ "key": "Down" }],                            "action": "text-mode:move-marks-to-next-line" },
       { "in": [{ "key": "alt+shift+Down" }],                  "action": "text-mode:clone-and-move-mark-to-next-line" },

       { "in": [{ "key": "PageUp"   }],                        "action": "text-mode:page-up" },
       { "in": [{ "key": "PageDown" }],                        "action": "text-mode:page-down" },
       
       { "in": [{ "key": "ctrl+a" }],                          "action": "text-mode:move-marks-to-start-of-line" },
       { "in": [{ "key": "ctrl+e" }],                          "action": "text-mode:move-marks-to-end-of-line" },
       { "in": [{ "key": "Home" }],                            "action": "text-mode:move-marks-to-start-of-line" },
       { "in": [{ "key": "End" }],                             "action": "text-mode:move-marks-to-end-of-line" },


       { "in": [{ "key": "alt+<" }],                           "action": "text-mode:move-marks-to-start-of-file" },
       { "in": [{ "key": "alt+>" }],                           "action": "text-mode:move-marks-to-end-of-file" },

       { "in": [{ "key": "ctrl+Home" }],                       "action": "text-mode:move-marks-to-start-of-file" },
       { "in": [{ "key": "ctrl+End" }],                        "action": "text-mode:move-marks-to-end-of-file" },


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

       { "in": [{ "key": "ctrl+Left"  }],                      "action": "text-mode:move-to-token-start" },
       { "in": [{ "key": "ctrl+Right" }],                      "action": "text-mode:move-to-token-end" },

       { "in": [{ "key": "ctrl+Up"    }],                      "action": "text-mode:scroll-up" },
       { "in": [{ "key": "ctrl+Down"  }],                      "action": "text-mode:scroll-down" },

       { "in": [{ "wheel": "Up"       }],                      "action": "text-mode:scroll-up" },
       { "in": [{ "wheel": "Down"     }],                      "action": "text-mode:scroll-down" },


       { "in": [{ "key": "ctrl+alt+Left"     }],               "action": "text-mode:move-mark-backward-word" },
       { "in": [{ "key": "ctrl+alt+Right"     }],              "action": "text-mode:move-mark-one-forward" },
       
       { "in": [{ "button-press":  "0"   }],                   "action": "text-mode:move-mark-to-clicked-area" },
       { "in": [{ "button-release": "0"  }],                   "action": "text-mode:ignore" },

       { "in": [{ "pointer-motion": "" }],                   "action": "text-mode:pointer-motion" },

       { "in": [{ "key": "ctrl+x" }, { "key": "Left" } ],      "action": "select-previous-view" },
       { "in": [{ "key": "ctrl+x" }, { "key": "Right" } ],     "action": "select-next-view" },

       { "in": [{ "key": "F2" } ],                             "action": "select-previous-view" },
       { "in": [{ "key": "F3" } ],                             "action": "select-next-view" },


       { "in": [{ "key": "ctrl+s" }],                          "action": "save-document" },

       { "in": [{ "key": "Esc"      }],                        "action": "editor:cancel" },

       { "in": [{ "key": "ctrl+q"   }],                        "action": "application:quit" },
       { "in": [{ "key": "ctrl+x" }, { "key": "ctrl+c" } ],    "action": "application:quit" },
       { "in": [{ "key": "ctrl+x" }, { "key": "ctrl+q" } ],    "action": "application:quit-abort" },

       { "in": [{ "system": "SIGTERM" } ],                     "action": "application:quit" },

       { "default": [],                                        "action": "text-mode:self-insert" }
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

    pub pending_events: usize,

    //
    pub width: usize,
    pub height: usize,
    pub view_id: usize, // doc id in view

    // ADD view env ? TODO: refresh env after input_proessing

    // move ths to update_action
    // reset on each event handling
    pub view_pre_render: Vec<view::Action>,
    pub view_post_render: Vec<view::Action>,
    pub center_offset: Option<u64>,
    pub cur_mark_index: Option<usize>,
    pub max_offset: u64,
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
            pending_events: 0,
            width: 0,
            height: 0,
            view_id: 0,
            view_pre_render: Vec::new(),
            view_post_render: Vec::new(),
            center_offset: None,
            cur_mark_index: None,
            max_offset: 0,
        }
    }
}

pub fn check_view_dimension(editor: &Editor, env: &EditorEnv) {
    let mut view = editor.view_map[env.view_id].1.as_ref().borrow_mut();
    // resize ?
    {
        let screen = view.screen.read().unwrap();
        if env.width == screen.width() && env.height == screen.height() {
            return;
        }
    }

    view.screen = Arc::new(RwLock::new(Box::new(Screen::new(env.width, env.height))));
}

pub fn update_view_and_send_draw_event(
    mut editor: &mut Editor,
    mut env: &mut EditorEnv,
    ui_tx: &Sender<EventMessage>,
) {
    // check size
    check_view_dimension(editor, env);

    let view = editor.view_map[env.view_id].1.clone();

    update_view(&mut editor, &mut env, &view);
    send_draw_event(&mut editor, ui_tx, &view);
}

pub fn send_draw_event(
    _editor: &mut Editor,
    ui_tx: &Sender<EventMessage>,
    view: &Rc<RefCell<View>>,
) {
    let view = view.as_ref().borrow();
    let new_screen = Arc::clone(&view.screen);

    let marks = Arc::clone(&view.moving_marks);

    let msg = EventMessage::new(
        0, // get_next_seq(&mut seq), TODO
        DrawEvent {
            screen: new_screen,
            marks,
            time: Instant::now(),
        },
    );

    crate::core::event::pending_render_event_inc(1);
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

                Event::UpdateViewEvent { width, height } => {
                    env.width = width;
                    env.height = height;
                    update_view_and_send_draw_event(&mut editor, &mut env, ui_tx);
                }

                Event::InputEvent { events, raw_data } => {
                    if !editor.view_map.is_empty() {
                        process_input_events(&mut editor, &mut env, &ui_tx, &events, &raw_data);
                    }
                }

                _ => {}
            }
        }
    }

    let msg = EventMessage::new(get_next_seq(&mut seq), Event::ApplicationQuitEvent);
    ui_tx.send(msg).unwrap_or(());
}

fn process_input_event(
    editor: &mut Editor,
    mut env: &mut EditorEnv,
    view_id: usize,
    ev: &InputEvent,
) -> bool {
    let mut view = &editor.view_map[view_id].1.clone();

    if *ev == crate::core::event::InputEvent::NoInputEvent {
        // ignore no input event event :-)
        env.status = "no input event".to_string();
        return false;
    }

    let action = eval_input_event(
        &ev,
        &env.input_map,
        &mut env.current_node, // TODO: EvalEnv
        &mut env.next_node,    // TODO: EvalEnv
    );

    let trigger = vec![(*ev).clone()];

    if let Some(action) = action {
        env.current_node = None;
        env.next_node = None;

        let start = Instant::now();
        dbg_println!("found action {} : input ev = {:?}", action, ev);

        match action.as_str() {
            _ => {
                if let Some(action) = env.action_map.get(&action) {
                    action(editor, env, &trigger, &mut view);
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
    ui_tx: &Sender<EventMessage>,
    events: &Vec<InputEvent>,
    _raw_data: &Option<Vec<u8>>, // TODO: remove
) {
    env.pending_events = crate::core::event::pending_input_event_count();

    let start = Instant::now();
    for ev in events {
        let vid = env.view_id;
        let mut event_processed = process_input_event(&mut editor, &mut env, vid, ev);

        // to check_focus_change()
        if vid != env.view_id {
            dbg_println!("view change {} ->  {}", vid, env.view_id);
            check_view_dimension(editor, env);
            event_processed = true;

            // NB: resize previous view's screen to lower memory usage
            let view = editor.view_map[vid].1.clone();
            let v = view.as_ref().borrow_mut();
            v.screen.write().unwrap().resize(1,1);
        }

        if event_processed {
            let start = Instant::now();
            let view = editor.view_map[env.view_id].1.clone();
            update_view(&mut editor, &mut env, &view);
            let end = Instant::now();
            dbg_println!("EVAL: update view time {}\r", (end - start).as_millis());
        }

        if env.pending_events > 0 {
            env.pending_events = crate::core::event::pending_input_event_dec(1);
        }
    }

    let end = Instant::now();
    dbg_println!("EVAL: input process time {}\r", (end - start).as_millis());

    //
    let p_input = crate::core::event::pending_input_event_count();
    let p_rdr = crate::core::event::pending_render_event_count();

    dbg_println!("EVAL: pending input event = {}\r", p_input);
    dbg_println!("EVAL: pending render events = {}\r", p_rdr);

    // % last render time
    // TODO: receive FPS form ui in Event ?
    if (p_input <= 60) || editor.last_rdr_event.elapsed() > Duration::from_millis(1000 / 10) {
        // hit
        let view = &editor.view_map[env.view_id].1.clone();
        send_draw_event(&mut editor, ui_tx, &view);
        editor.last_rdr_event = Instant::now();
    }
}

pub fn application_quit(
    _editor: &mut Editor,
    env: &mut EditorEnv,
    _trigger: &Vec<InputEvent>,
    view: &Rc<RefCell<View>>,
) {
    env.status = "<quit>".to_string();

    let doc = &view.as_ref().borrow();
    let doc = doc.document.as_ref().unwrap();
    if doc.borrow().changed {
        env.status = "<quit> : modified buffer exits. type F4 to quit without saving".to_string();
    } else {
        env.quit = true;
    }
}

pub fn application_quit_abort(
    _editor: &mut Editor,
    env: &mut EditorEnv,
    _trigger: &Vec<InputEvent>,
    _view: &Rc<RefCell<View>>,
) {
    env.quit = true;
}

pub fn save_document(
    _editor: &mut Editor,
    _env: &mut EditorEnv,
    _trigger: &Vec<InputEvent>,
    view: &Rc<RefCell<View>>,
) {
    let v = view.as_ref().borrow_mut();
    let doc = v.document.as_ref().unwrap();
    let mut doc = doc.as_ref().borrow_mut();

    let _ = doc.sync_to_disk().is_ok(); // ->  operation ok
}

fn register_action(map: &mut ActionMap, s: &str, func: view::ModeFunction) {
    map.insert(s.to_string(), func);
}

fn build_action_map() -> ActionMap {
    let mut map: ActionMap = HashMap::new(); // text-mode action map

    register_action(&mut map, "application:quit", application_quit);

    register_action(&mut map, "application:quit-abort", application_quit_abort);

    register_action(&mut map, "save-document", save_document);

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

    register_action(
        &mut map,
        "text-mode:move-to-token-start",
        view::move_to_token_start,
    );

    register_action(
        &mut map,
        "text-mode:move-to-token-end",
        view::move_to_token_end,
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
        "text-mode:move-marks-to-start-of-line",
        view::move_marks_to_start_of_line,
    );
    register_action(
        &mut map,
        "text-mode:move-marks-to-end-of-line",
        view::move_marks_to_end_of_line,
    );

    register_action(
        &mut map,
        "text-mode:move-marks-to-start-of-file",
        view::move_mark_to_start_of_file,
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

    register_action(&mut map, "select-next-view", view::select_next_view);

    register_action(&mut map, "select-previous-view", view::select_previous_view);

    register_action(
        &mut map,
        "text-mode:clone-and-move-mark-to-previous-line",
        view::clone_and_move_mark_to_previous_line,
    );
    register_action(
        &mut map,
        "text-mode:clone-and-move-mark-to-next-line",
        view::clone_and_move_mark_to_next_line,
    );

    register_action(&mut map, "text-mode:pointer-motion", view::pointer_motion);

    map
}
