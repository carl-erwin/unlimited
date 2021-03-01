// Copyright (c) Carl-Erwin Griffith

// std
use std::cell::RefCell;
use std::collections::HashMap;
use std::rc::Rc;
use std::sync::mpsc::Receiver;
use std::sync::mpsc::Sender;
use std::sync::Arc;
use std::sync::RwLock;
use std::time::Duration;
use std::time::Instant;

// ext

// sibling
pub use super::*;

// crate
use crate::core::codepointinfo::CodepointInfo;
use crate::core::event::input_map::eval_input_event;
use crate::core::event::Event;
use crate::core::event::Event::DrawEvent;
use crate::core::event::EventMessage;
use crate::core::event::InputEvent;
use crate::core::event::Key;
use crate::core::event::KeyModifiers;

use crate::core::mark::Mark;

use crate::core::modes::text_mode::TextModeData; // TODO remove this impl details
use crate::core::modes::Mode;

use crate::core::screen::Screen;
use crate::core::view;
use crate::core::view::layout::FilterIoData;
use crate::core::view::layout::LayoutEnv;
use crate::core::view::update_view;
use crate::core::view::View;

// local

pub type ModeFunction = fn(
    editor: &mut Editor<'static>,
    env: &mut EditorEnv,
    view: &Rc<RefCell<View<'_, 'static>>>,
) -> ();

// ActionMap is kept in EditorEnv
// TODO:
// Have a map per view
// and if eval fails, fallback to EditorEnv's
// It will allow per mode actions instanciate for each view
// transform into STACK of map ?
pub type ActionMap<'a> = HashMap<String, ModeFunction>;

//
pub type RenderFunction = fn(
    editor: &mut Editor,
    env: &mut EditorEnv,
    view: &View,
    env: &mut LayoutEnv,
    input: &Vec<FilterIoData>,
    output: &mut Vec<FilterIoData>,
) -> ();

pub type RenderMap = HashMap<String, RenderFunction>;

//
//
use crate::core::config::Config;

use crate::core::document;
use crate::core::document::Document;
use crate::core::document::DocumentBuilder;

//
pub type Id = u64;

/*

 Hierarchy Reminder

    core
        editor
            config
            document_map<doc_id, Rc<Document>>
            view_map<view_id, Rc<View>>
            list<mode>

        editor_env
            input_map
            Option<&view>


   TODO: add Timers -> for Blinkin marks
*/

/* TODO:

   parse argument to extract line,colinfo,offset {
    file@1246
    file:10
    file:10,5
    +l file
    +l,c file
    @offset file
  }

    document_list: Vec<
        struct DocumentInfo {
            FileType: { directory(full_path), regular(full_path), internal("*debug-message*), char_device(full_path), block_device(full_path) }
            basename,: String,
            Title,: String,
            id,
            start_line    : Option<usize>
            start_column  : Option<usize>
            start_offset  : Option<usize
        }

  TODO:
    keep user arguments order, push new files,
    this list is never cleared
    before insertion the real path(*ln) is checked to avoid double open

    ioctl mode for block devices ?

    document_index: HashMap<String, document::Id>,  document::Id is the position in document_list

*/
pub struct Editor<'a> {
    pub config: Config,
    pub document_map: HashMap<document::Id, Arc<RwLock<Document<'a>>>>,
    pub view_map: Vec<(view::Id, Rc<RefCell<View<'a, 'a>>>)>,
    pub modes: HashMap<String, Box<dyn Mode>>,
    pub core_tx: Sender<EventMessage<'a>>,
    pub ui_tx: Sender<EventMessage<'a>>,
    pub worker_tx: Sender<EventMessage<'a>>,
}

impl<'a> Editor<'a> {
    ///
    pub fn new(
        config: Config,
        core_tx: Sender<EventMessage<'a>>,
        ui_tx: Sender<EventMessage<'a>>,
        worker_tx: Sender<EventMessage<'a>>,
    ) -> Editor<'a> {
        Editor {
            config,
            document_map: HashMap::new(),
            view_map: Vec::new(),
            modes: HashMap::new(),
            ui_tx,
            core_tx,
            worker_tx,
        }
    }

    ///
    pub fn setup_default_buffers(&mut self) {
        let mut builder = DocumentBuilder::new();
        builder
            .document_name("debug-message")
            .file_name("/dev/null")
            .internal(true);

        let b = builder.finalize();

        if let Some(b) = b {
            let id = self.document_map.len() as u64;
            self.document_map.insert(id, b);
        }

        let mut builder = DocumentBuilder::new();
        builder
            .document_name("scratch")
            .file_name("/dev/null")
            .internal(true);

        let b = builder.finalize();

        if let Some(b) = b {
            let id = self.document_map.len() as u64;
            self.document_map.insert(id, b);
        }
    }
    pub fn register_mode<'e>(&mut self, mode: Box<dyn Mode>) {
        let name = mode.name();
        self.modes.insert(name.to_owned(), mode);
    }
}

//////////////////////////////////////////////

// TODO: handle conflicting bindings
pub fn register_action(map: &mut ActionMap, s: &str, func: ModeFunction) {
    map.insert(s.to_string(), func);
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
    editor: &mut Editor,
    mut env: &mut EditorEnv,
    ui_tx: &Sender<EventMessage>,
) {
    // check size
    check_view_dimension(editor, env);

    let view = editor.view_map[env.view_id].1.clone();

    update_view(editor, &mut env, &view);
    send_draw_event(editor, &mut env, ui_tx, &view);
}

// move to core: and later transform into RenderFilter
// SLOW
// we should iterate over the screen
// find the first mark
pub fn refresh_screen_marks(screen: &mut Screen, marks: &Vec<Mark>, set: bool) {
    if !set {
        screen_apply(screen, |_, _, cpi| {
            cpi.is_mark = false;
            true // continue
        });
        return;
    }

    let (first_offset, last_offset) = match (screen.first_offset, screen.last_offset) {
        (Some(first_offset), Some(last_offset)) => (first_offset, last_offset),
        _ => {
            return;
        }
    };

    for m in marks.iter() {
        match screen.find_cpi_by_offset(m.offset) {
            (Some(&_cpi), x, y) => {
                screen.get_mut_cpinfo(x, y).unwrap().is_mark = true;
            }
            _ => {}
        }
    }

    if true {
        return;
    }

    // incremental mark rendering
    // draw marks
    let mut mark_offset: u64 = 0xFFFFFFFFFFFFFFFF; // replace by max u64
    let mut fetch_mark = true;
    let mut mark_it = marks.iter();
    screen_apply(screen, |_, _, cpi| {
        if let Some(cpi_offset) = cpi.offset {
            if fetch_mark {
                // get 1st  mark >= current cpi_offset
                loop {
                    let m = mark_it.next();
                    if m.is_none() {
                        return false;
                    }

                    let m = m.unwrap();
                    if m.offset < first_offset {
                        continue;
                    }

                    if m.offset > last_offset {
                        return false;
                    }

                    if m.offset >= cpi_offset {
                        mark_offset = m.offset;
                        break;
                    }
                }
                fetch_mark = false;
            }

            if cpi_offset == mark_offset {
                cpi.is_mark = !cpi.metadata;
            } else {
                //
                if mark_offset < cpi_offset {
                    fetch_mark = true;
                }
            }
        }

        true
    });
}

// move to screen module , rename walk/map ?
fn screen_apply<F: FnMut(usize, usize, &mut CodepointInfo) -> bool>(
    screen: &mut Screen,
    mut on_cpi: F,
) {
    for l in 0..screen.height() {
        if let Some(line) = screen.get_mut_line(l) {
            for c in 0..line.nb_cells {
                if let Some(cpi) = line.get_mut_cpi(c) {
                    if on_cpi(c, l, cpi) == false {
                        return;
                    }
                }
            }
        }
    }
}

pub fn send_draw_event(
    _editor: &mut Editor,
    env: &mut EditorEnv,
    ui_tx: &Sender<EventMessage>,
    view: &Rc<RefCell<View>>,
) {
    let view = view.as_ref().borrow();
    let tm = view.mode_ctx::<TextModeData>("text-mode");

    // TODO: REMOVE THIS:
    // add mark filter before screen
    // render marks here for now
    refresh_screen_marks(
        &mut view.screen.write().as_mut().unwrap(),
        &tm.marks,
        env.draw_marks,
    );

    let new_screen = Arc::clone(&view.screen);

    let msg = EventMessage::new(
        0, // get_next_seq(&mut seq), TODO
        DrawEvent {
            screen: new_screen,
            time: Instant::now(),
        },
    );

    crate::core::event::pending_render_event_inc(1);
    ui_tx.send(msg).unwrap_or(());
}

fn process_input_event<'a>(
    editor: &'a mut Editor<'static>,
    mut env: &'a mut EditorEnv<'static>,
    view_id: usize,
    ev: &InputEvent,
) -> bool {
    let mut view = &editor.view_map[view_id].1.clone();

    if *ev == crate::core::event::InputEvent::NoInputEvent {
        // ignore no input event event :-)
        return false;
    }

    dbg_println!("prev (accum) events = {:?}", env.trigger);

    dbg_println!("eval input event input ev = {:?}", ev);

    // TODO: track whole input seq // not tested
    env.trigger.push((*ev).clone());

    let action = eval_input_event(
        &ev,
        &env.input_map,
        &mut env.current_node, // TODO: EvalEnv
        &mut env.next_node,    // TODO: EvalEnv
    );

    if let Some(action) = action {
        env.current_node = None;
        env.next_node = None;

        let start = Instant::now();
        dbg_println!("found action {} : input ev = {:?}", action, ev);

        match action.as_str() {
            _ => {
                if let Some(action) = env.action_map.get(&action) {
                    action(editor, env, &mut view);
                } else {
                    // clear ?
                }
                env.trigger.clear();
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

fn send_ui_event(
    mut editor: &mut Editor,
    mut env: &mut EditorEnv,
    ui_tx: &Sender<EventMessage>,
    _events: &Vec<InputEvent>,
) {
    dbg_println!(
        "EVAL: input process time {}\r",
        (env.process_input_end - env.process_input_start).as_millis()
    );

    //
    let p_input = crate::core::event::pending_input_event_count();
    let p_rdr = crate::core::event::pending_render_event_count();

    dbg_println!("EVAL: pending input event = {}\r", p_input);
    dbg_println!("EVAL: pending render events = {}\r", p_rdr);

    // % last render time
    // TODO: receive FPS form ui in Event ?
    if (p_input <= 60) || env.last_rdr_event.elapsed() > Duration::from_millis(1000 / 10) {
        // hit
        let view = &editor.view_map[env.view_id].1.clone();
        send_draw_event(&mut editor, &mut env, ui_tx, &view);
        env.last_rdr_event = Instant::now();
    }
}

/*
pre_process_input_event(&mut editor, &mut env, &ui_tx, &events);
input_process(&mut editor, &mut env, &ui_tx, &events);
pre_input_process(&mut editor, &mut env, &ui_tx, &events);
process_input_events(&mut editor, &mut env, &ui_tx, &events);

*/
fn process_input_events(
    mut editor: &mut Editor<'static>,
    mut env: &mut EditorEnv<'static>,
    ui_tx: &Sender<EventMessage>,
    events: &Vec<InputEvent>,
) {
    env.pending_events = crate::core::event::pending_input_event_count();

    let mut flat_events = vec![];
    // transform UnicodeArray of 1 element to single element
    for ev in events {
        match ev {
            InputEvent::KeyPress {
                key: Key::UnicodeArray(ref codepoints),
                mods:
                    KeyModifiers {
                        ctrl: false,
                        alt: false,
                        shift: false,
                    },
            } => {
                for c in codepoints {
                    flat_events.push(InputEvent::KeyPress {
                        key: Key::Unicode(*c),
                        mods: KeyModifiers {
                            ctrl: false,
                            alt: false,
                            shift: false,
                        },
                    });
                }
            }

            _ => {
                flat_events.push(ev.clone());
            }
        }
    }

    env.process_input_start = Instant::now();
    for ev in &flat_events {
        let vid = env.view_id;

        // pre_eval_input_stage(&mut editor, &mut env, vid, ev);

        // need_rendering ?
        env.event_processed = process_input_event(&mut editor, &mut env, vid, ev);

        // post_eval_stage(&mut editor, &mut env, vid, ev);
        // {
        // to check_focus_change()
        if vid != env.view_id {
            dbg_println!("view change {} ->  {}", vid, env.view_id);
            check_view_dimension(editor, env);
            env.event_processed = true;

            // NB: resize previous view's screen to lower memory usage
            let view = editor.view_map[vid].1.clone();
            let v = view.as_ref().borrow_mut();
            v.screen.write().unwrap().resize(1, 1);
        }
        // }

        // pre_render_stage(&mut editor, &mut env, vid, ev);

        if env.event_processed {
            let start = Instant::now();
            let view = editor.view_map[env.view_id].1.clone();
            // render_view(&mut editor, &mut env, &view);
            update_view(&mut editor, &mut env, &view);
            let end = Instant::now();
            dbg_println!("EVAL: update view time {}\r", (end - start).as_millis());
        }
        if env.pending_events > 0 {
            env.pending_events = crate::core::event::pending_input_event_dec(1);
        }

        //
    }
    env.process_input_end = Instant::now();

    send_ui_event(editor, env, ui_tx, events);
}

pub fn application_quit(_editor: &mut Editor, env: &mut EditorEnv, view: &Rc<RefCell<View>>) {
    let v = &view.as_ref().borrow();
    let doc = v.document.as_ref().unwrap();
    let doc = doc.as_ref().read().unwrap();

    if !doc.changed {
        env.quit = true;
    }
}

pub fn application_quit_abort(
    _editor: &mut Editor,
    env: &mut EditorEnv,

    _view: &Rc<RefCell<View>>,
) {
    env.quit = true;
}

pub fn save_document(editor: &mut Editor, _env: &mut EditorEnv, view: &Rc<RefCell<View>>) {
    let v = view.as_ref().borrow_mut();

    let doc_id = {
        let doc = v.document.as_ref().unwrap();
        {
            let doc = doc.as_ref().read().unwrap();
            if doc.is_syncing {
                // sync is pending
                // TODO: lock all doc....write()
                return;
            }
        }

        {
            let mut doc = doc.as_ref().write().unwrap();
            let doc_id = doc.id;
            doc.is_syncing = true;
            doc_id
        }
    };

    // NB: We must take the doc clone from Editor not View
    // because of lifetime(editor) > lifetime(view)
    // and view.doc is a clone from editor.document_map,
    // or else error  "data from `view` flows into `editor`"
    if let Some(doc) = editor.document_map.get(&doc_id) {
        let msg = EventMessage {
            seq: 0,
            event: Event::SyncTask {
                doc: Arc::clone(doc),
            },
        };
        editor.worker_tx.send(msg).unwrap_or(());
    }
}

// TODO: CoreMode ? quit/force-quit
pub fn build_core_action_map<'a>() -> ActionMap<'a> {
    let mut map: ActionMap = HashMap::new();

    // core
    register_action(&mut map, "application:quit", application_quit);
    register_action(&mut map, "application:quit-abort", application_quit_abort);

    register_action(&mut map, "save-document", save_document); // core ?

    register_action(&mut map, "split-vertically", split_vertically);
    register_action(&mut map, "split-horizontally", split_horizontally);

    map
}

// store this in parent and reuse in resize
pub enum LayoutOperation {
    // We want a fixed size of sz cells vertically/horizontally in the parent
    // used = size
    // remain = remain - sz
    Fixed { size: usize },

    // We want a fixed percentage of sz cells vertically/horizontally
    // used = (parent.sz/100) * sz
    // remain = parent.sz - used
    Percent { p: usize },

    // We want a fixed percentage of sz cells vertically/horizontally
    // used = (remain/100 * sz)
    // (remain <- remain - (remain/100 * sz))
    RemainPercent { p: usize },

    // We want a fixed percentage of sz cells vertically/horizontally
    // used = (remain - minus)
    // remain = remain - used
    RemainMinus { minus: usize },
}

fn compute_layout_sizes(start: usize, ops: &Vec<LayoutOperation>) -> Vec<usize> {
    let mut sizes = vec![];

    dbg_println!("start = {}", start);

    if start == 0 {
        return sizes;
    }

    let mut remain = start;

    for op in ops {
        if remain == 0 {
            break;
        }

        match op {
            LayoutOperation::Fixed { size } => {
                remain = remain.saturating_sub(*size);
                sizes.push(*size);
            }

            LayoutOperation::Percent { p } => {
                let used = (*p * start) / 100;
                remain = remain.saturating_sub(used);
                sizes.push(used);
            }

            LayoutOperation::RemainPercent { p } => {
                let used = (*p * remain) / 100;
                remain = remain.saturating_sub(used);
                sizes.push(used);
            }

            // We want a fixed percentage of sz cells vertically/horizontally
            // used = minus
            // (remain <- remain - minus))
            LayoutOperation::RemainMinus { minus } => {
                let used = remain.saturating_sub(*minus);
                remain = remain.saturating_sub(used);
                sizes.push(used);
            }
        }
    }

    sizes
}

pub fn split_vertically(
    editor: &mut Editor<'static>,
    _env: &mut EditorEnv,
    view: &Rc<RefCell<View<'_, 'static>>>,
) {
    let mut v = view.as_ref().borrow_mut();

    // check if already split
    if v.children.len() != 0 {
        return;
    }

    // compute left and right size as current View / 2
    // get screen

    let (width, height) = {
        let screen = v.screen.read().unwrap();
        (screen.width(), screen.height())
    };
    if width == 0 || height == 0 {
        return;
    }

    // compute_split(size, first_half, first_second);
    let (_left_w, _right_w) = View::compute_split(width);

    let ops = vec![
        LayoutOperation::Percent { p: 50 },
        LayoutOperation::Percent { p: 50 },
    ];

    let sizes = compute_layout_sizes(width, &ops);

    dbg_println!("splitV = SIZE {:?}", sizes);

    for size in sizes {
        let doc_id = v.document.as_ref().unwrap().read().unwrap().id;

        if let Some(doc) = editor.document_map.get(&doc_id) {
            let doc = editor.document_map.get(&doc_id).unwrap().clone();
            let screen = Arc::new(RwLock::new(Box::new(Screen::new(size, height))));
            let view = view::View::new(v.start_offset, size, height, Some(Arc::clone(&doc)));
            v.children.push(Rc::new(RefCell::new(view)));
        }
    }
}

pub fn split_horizontally(editor: &mut Editor, _env: &mut EditorEnv, view: &Rc<RefCell<View>>) {
    let v = view.as_ref().borrow_mut();

    // check if already split
    if v.children.len() != 0 {
        return;
    }

    // compute left and right size as current View / 2
    // get screen

    let (width, height) = {
        let screen = v.screen.read().unwrap();
        (screen.width(), screen.height())
    };
    if width == 0 || height == 0 {
        return;
    }

    // compute_split(size, first_half, first_second);
    let (_left_w, _right_w) = View::compute_split(width);

    let ops = vec![
        LayoutOperation::Fixed { size: 1 },        // TITLE
        LayoutOperation::RemainMinus { minus: 3 }, // BODY
        LayoutOperation::Fixed { size: 3 },        // STATUS/CMD
    ];

    let sizes = compute_layout_sizes(height, &ops);

    dbg_println!("splitV = SIZE {:?}", sizes);
}

// TODO: put in main_loop.rs
pub fn main_loop(
    mut editor: &mut Editor<'static>,
    mut env: &mut EditorEnv<'static>,
    core_rx: &Receiver<EventMessage<'static>>,
    ui_tx: &Sender<EventMessage<'static>>,
) {
    let mut seq: usize = 0;

    fn get_next_seq(seq: &mut usize) -> usize {
        *seq += 1;
        *seq
    }

    while !env.quit {
        if let Ok(evt) = core_rx.recv() {
            match evt.event {
                Event::UpdateViewEvent { width, height } => {
                    env.width = width;
                    env.height = height;
                    update_view_and_send_draw_event(&mut editor, &mut env, ui_tx);
                }

                Event::InputEvents { events } => {
                    if !editor.view_map.is_empty() {
                        env.draw_marks = true;
                        process_input_events(&mut editor, &mut env, &ui_tx, &events);
                    }
                }

                _ => {}
            }
        }
    }

    // send ApplicationQuitEvent to worker thread
    let msg = EventMessage::new(0, Event::ApplicationQuitEvent);
    editor.worker_tx.send(msg).unwrap_or(());

    // send ApplicationQuitEvent to ui thread
    let msg = EventMessage::new(get_next_seq(&mut seq), Event::ApplicationQuitEvent);
    ui_tx.send(msg).unwrap_or(());
}
