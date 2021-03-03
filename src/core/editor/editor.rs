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
use crate::core::event;
use crate::core::event::input_map::eval_input_event;
use crate::core::event::Event;
use crate::core::event::Event::DrawEvent;
use crate::core::event::EventMessage;
use crate::core::event::InputEvent;
use crate::core::event::Key;
use crate::core::event::KeyModifiers;
use crate::core::mark::Mark;

use crate::core::modes::text_mode::TextModeContext; // TODO remove this impl details
use crate::core::modes::Mode;

use crate::core::screen::Screen;
use crate::core::view;
use crate::core::view::layout::FilterIoData;
use crate::core::view::layout::LayoutEnv;
use crate::core::view::update_view;
use crate::core::view::LayoutDirection;
use crate::core::view::LayoutOperation;
use crate::core::view::View;
// local

pub type ModeFunction = fn(
    editor: &mut Editor<'static>,
    env: &mut EditorEnv,
    view: &Rc<RefCell<View<'static, 'static>>>,
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
    pub view_map: HashMap<view::Id, Rc<RefCell<View<'a, 'a>>>>,
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
            view_map: HashMap::new(),
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
    dbg_println!("checking view dimension {}", env.view_id);

    let mut view = editor
        .view_map
        .get(&env.view_id)
        .as_ref()
        .unwrap()
        .borrow_mut();
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

    let view = editor.view_map.get(&env.view_id).unwrap().clone();

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
    let view = view.borrow();
    let tm = view.mode_ctx::<TextModeContext>("text-mode");

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
    view_id: view::Id,
    ev: &InputEvent,
) -> bool {
    let mut view = &editor.view_map.get(&view_id).unwrap().clone();

    {
        let v = view.borrow_mut();

        dbg_println!("DISPATCH EVENT TO VID {}", view_id);
        assert_eq!(v.id, view_id);

        if v.children.len() > 0 {
            // check focus: must add view(x.y)
        }
    }

    //
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
        let view = editor.view_map.get(&env.view_id).unwrap().clone();
        send_draw_event(&mut editor, &mut env, ui_tx, &view);
        env.last_rdr_event = Instant::now();
    }
}

fn get_focused_vid(
    mut editor: &mut Editor<'static>,
    mut env: &mut EditorEnv<'static>,
    vid: view::Id,
) -> view::Id {
    let vid = vid;
    let view = &editor.view_map.get(&vid).unwrap().clone();
    let v = view.borrow();
    if v.children.len() == 0 {
        return vid;
    }

    if let Some(focused_vid) = v.focus_to {
        return get_focused_vid(&mut editor, &mut env, focused_vid);
    }

    vid
}

pub fn set_focus_on_vid(editor: &mut Editor<'static>, env: &mut EditorEnv<'static>, vid: view::Id) {
    let view = &editor.view_map.get(&env.view_id).unwrap().clone();
    let mut v = view.borrow_mut();
    v.focus_to = Some(vid);
}

// clips (x,y) to local view @ (x,y)
// returns the view's id at
fn clip_coordinates_xy(
    editor: &mut Editor<'static>,
    _env: &mut EditorEnv<'static>,
    root_vid: view::Id,
    _vid: view::Id,
    x: &mut i32,
    y: &mut i32,
) -> view::Id {
    let mut id = root_vid;

    // check layout type
    dbg_println!("CLIPPING -----------------------------------BEGIN");
    dbg_println!("CLIPPING clipping orig coords ({},{})", *x, *y);
    dbg_println!("CLIPPING         select vid {}", id);

    loop {
        'inner: loop {
            if let Some(v) = editor.view_map.get(&id) {
                let v = v.borrow();

                if v.children.len() == 0 {
                    dbg_println!("CLIPPING        no more children");
                    dbg_println!("CLIPPING ----------------------------------- END");
                    return id;
                }

                for child in v.children.iter() {
                    let child_v = child.borrow();
                    let screen = child_v.screen.read().unwrap();

                    dbg_println!(
                    "CLIPPING dump child vid {} dim [x({}), y({})][w({}) h({})] [x+w({}) y+h({})]",
                    child_v.id,
                    child_v.x,
                    child_v.y,
                    screen.width(),
                    screen.height(),
                    child_v.x + screen.width(),
                    child_v.y + screen.height()
                );
                }

                dbg_println!("CLIPPING");

                let is_layout_vertical = v.layout_direction == LayoutDirection::Vertical;

                let mut last_id = 0;
                for (idx, child) in v.children.iter().enumerate() {
                    let child_v = child.borrow();
                    let screen = child_v.screen.read().unwrap();

                    last_id = child_v.id;

                    dbg_println!(
                    "CLIPPING checking child vid {} dim [x({}), y({})][w({}) h({})] [x+w({}) y+h({})]",
                    child_v.id,
                    child_v.x,
                    child_v.y,
                    screen.width(),
                    screen.height(),
                    child_v.x+screen.width(),
                    child_v.y+screen.height());

                    if *x >= child_v.x as i32
                        && *x < (child_v.x + screen.width()) as i32
                        && *y >= child_v.y as i32
                        && *y < (child_v.y + screen.height()) as i32
                    {
                        if is_layout_vertical {
                            *x -= child_v.x as i32;
                        } else {
                            *y -= child_v.y as i32;
                        }

                        // found
                        dbg_println!("CLIPPING         updated clipping coords ({},{})", *x, *y);
                        dbg_println!("CLIPPING         select vid {}", child_v.id);

                        id = child_v.id;
                        break 'inner;
                    } else {
                        dbg_println!("CLIPPING        not found @ idx {}", idx);
                    }
                }

                // take last id if not found
                id = last_id;
            }
        } // 'inner
    } // 'outer
}

fn clip_coordinates_and_get_vid(
    mut editor: &mut Editor<'static>,
    mut env: &mut EditorEnv<'static>,
    ev: &InputEvent,
    root_vid: view::Id,
    vid: view::Id,
) -> (view::Id, InputEvent) {
    let mut ev = ev.clone();
    let vid = match &mut ev {
        InputEvent::ButtonPress(event::ButtonEvent { x, y, .. }) => {
            clip_coordinates_xy(&mut editor, &mut env, root_vid, vid, x, y)
        }
        InputEvent::ButtonRelease(event::ButtonEvent { x, y, .. }) => {
            clip_coordinates_xy(&mut editor, &mut env, root_vid, vid, x, y)
        }
        InputEvent::PointerMotion(event::PointerEvent { x, y, .. }) => {
            clip_coordinates_xy(&mut editor, &mut env, root_vid, vid, x, y)
        }
        InputEvent::WheelUp { x, y, .. } => {
            clip_coordinates_xy(&mut editor, &mut env, root_vid, vid, x, y)
        }
        InputEvent::WheelDown { x, y, .. } => {
            clip_coordinates_xy(&mut editor, &mut env, root_vid, vid, x, y)
        }
        _ => vid,
    };

    (vid, ev)
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
        // pre_eval_input_stage(&mut editor, &mut env, vid, ev);

        // select root
        let root_vid = env.view_id;

        // get child focus
        let vid = get_focused_vid(&mut editor, &mut env, root_vid);
        dbg_println!("FOCUS on vid {}", vid);

        // clip event coordinates
        let (vid, ev) = clip_coordinates_and_get_vid(&mut editor, &mut env, ev, root_vid, vid);

        // TODO: if button press only: env.focus_on = Option<vid> ?
        set_focus_on_vid(&mut editor, &mut env, vid);

        // need_rendering ?
        env.event_processed = process_input_event(&mut editor, &mut env, vid, &ev);

        // post_eval_stage(&mut editor, &mut env, vid, ev);
        // {
        // to check_focus_change()
        if root_vid != env.view_id {
            dbg_println!("view change {} ->  {}", root_vid, env.view_id);
            check_view_dimension(editor, env);
            env.event_processed = true;

            // NB: resize previous view's screen to lower memory usage
            let view = editor.view_map.get(&root_vid).unwrap().clone();
            let v = view.borrow_mut();
            v.screen.write().unwrap().resize(1, 1);
        }
        // }

        // pre_render_stage(&mut editor, &mut env, vid, ev);

        if env.event_processed {
            let start = Instant::now();
            let view = editor.view_map.get(&env.view_id).unwrap().clone();
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
    let v = &view.borrow();
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
    let v = view.borrow_mut();

    let doc_id = {
        let doc = v.document.as_ref().unwrap();
        {
            // - needed ? already syncing ? -
            let doc = doc.as_ref().read().unwrap();
            if !doc.changed || doc.is_syncing {
                // TODO: ensure all over places are checking this flag, all doc....write()
                // better, some permissions mechanism ?
                // doc.access_permissions = r-
                // doc.access_permissions = -w
                // doc.access_permissions = rw
                return;
            }
        }

        // - set sync flag -
        {
            let mut doc = doc.as_ref().write().unwrap();
            let doc_id = doc.id;
            doc.is_syncing = true;
            doc_id
        }
    };

    // - send sync job to worker -
    //
    // NB: We must take the doc clone from Editor not View
    // because of lifetime(editor) > lifetime(view)
    // and view.doc is a clone from editor.document_map,
    // doing this let us avoid the use manual lifetime annotations ('static)
    // and errors like "data from `view` flows into `editor`"
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

pub fn split_with_direction(
    editor: &mut Editor<'static>,
    env: &mut EditorEnv,
    v: &'static mut View,
    width: usize,
    height: usize,
    dir: view::LayoutDirection,
    doc: Option<Arc<RwLock<Document>>>,
) {
    let sizes = if dir == LayoutDirection::Vertical {
        view::compute_layout_sizes(width, &v.layout_ops) // options ? for ret size == 0
    } else {
        view::compute_layout_sizes(height, &v.layout_ops) // options ? for ret size == 0
    };

    dbg_println!("split {:?} = SIZE {:?}", dir, sizes);
    for s in &sizes {
        if *s == 0 {
            return;
        }
    }

    let doc = {
        if v.document.is_none() {
            None
        } else {
            let doc_id = v.document.as_ref().unwrap();
            let doc_id = doc_id.read().unwrap().id;
            if let Some(_doc) = editor.document_map.get(&doc_id) {
                let doc = editor.document_map.get(&doc_id).unwrap().clone();
                Some(Arc::clone(&doc))
            } else {
                None
            }
        }
    };

    let mut x = v.x;
    let mut y = v.y;
    for (idx, size) in sizes.iter().enumerate() {
        // vertically
        let mut view = match dir {
            LayoutDirection::Vertical => {
                view::View::new(Some(v.id), v.start_offset, *size, height, doc.clone())
            }
            LayoutDirection::Horizontal => {
                view::View::new(Some(v.id), v.start_offset, width, *size, doc.clone())
            }

            _ => {
                return;
            }
        };

        // horizontally
        // create child modes
        view.x = x;
        view.y = y;

        for m in v.mode_ctx.iter() {
            let name = m.0;
            let mode = editor.modes.get(name.as_str()).unwrap();
            let ctx = mode.alloc_ctx();
            dbg_println!("view.id = {}", view.id);
            view.set_mode_ctx(mode.name(), ctx);
        }

        if idx == 0 {
            // TODO: propagate focus up to root
            v.focus_to = Some(view.id);

            let mut parent_id = v.parent_id;
            loop {
                if let Some(pid) = parent_id {
                    if let Some(pview) = editor.view_map.get(&pid) {
                        let mut pview = pview.borrow_mut();
                        pview.focus_to = Some(view.id);
                        parent_id = pview.parent_id;
                    } else {
                        break;
                    }
                } else {
                    break;
                }
            }
        }

        let id = view.id;
        let rc = Rc::new(RefCell::new(view));
        v.children.push(Rc::clone(&rc));
        editor.view_map.insert(id, Rc::clone(&rc));

        let mut view = match dir {
            LayoutDirection::Vertical => {
                x += *size;
            }
            LayoutDirection::Horizontal => {
                y += *size;
            }
            _ => {
                return;
            }
        };
    }
}

pub fn split_vertically(
    editor: &mut Editor<'static>,
    _env: &mut EditorEnv,
    view: &Rc<RefCell<View<'static, 'static>>>,
) {
    let mut v = view.borrow_mut();

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
    //
    if width <= 4 {
        return;
    }

    // compute_split(size, first_half, first_second);
    let (_left_w, _right_w) = View::compute_split(width);

    // TODO: store
    v.layout_direction = LayoutDirection::Vertical;
    v.layout_ops = vec![
        LayoutOperation::Percent { p: 50 },
        LayoutOperation::Percent { p: 50 },
    ];

    let sizes = view::compute_layout_sizes(width, &v.layout_ops); // options ? for ret size == 0

    dbg_println!("splitV = SIZE {:?}", sizes);
    for s in &sizes {
        if *s == 0 {
            return;
        }
    }

    let doc = {
        if v.document.is_none() {
            None
        } else {
            let doc_id = v.document.as_ref().unwrap();
            let doc_id = doc_id.read().unwrap().id;
            if let Some(_doc) = editor.document_map.get(&doc_id) {
                let doc = editor.document_map.get(&doc_id).unwrap().clone();
                Some(Arc::clone(&doc))
            } else {
                None
            }
        }
    };

    let mut x = v.x;
    let y = v.y;
    for (idx, size) in sizes.iter().enumerate() {
        let mut view = view::View::new(Some(v.id), v.start_offset, *size, height, doc.clone());
        // create child modes
        view.x = x;
        view.y = y;

        for m in v.mode_ctx.iter() {
            let name = m.0;
            let mode = editor.modes.get(name.as_str()).unwrap();
            let ctx = mode.alloc_ctx();
            dbg_println!("view.id = {}", view.id);
            view.set_mode_ctx(mode.name(), ctx);
        }

        if idx == 0 {
            // TODO: propagate focus up to root
            v.focus_to = Some(view.id);

            let mut parent_id = v.parent_id;
            loop {
                if let Some(pid) = parent_id {
                    if let Some(pview) = editor.view_map.get(&pid) {
                        let mut pview = pview.borrow_mut();
                        pview.focus_to = Some(view.id);
                        parent_id = pview.parent_id;
                    } else {
                        break;
                    }
                } else {
                    break;
                }
            }
        }

        let id = view.id;
        let rc = Rc::new(RefCell::new(view));
        v.children.push(Rc::clone(&rc));
        editor.view_map.insert(id, Rc::clone(&rc));

        x += *size;
    }
}

pub fn split_horizontally(
    editor: &mut Editor<'static>,
    _env: &mut EditorEnv,
    view: &Rc<RefCell<View<'static, 'static>>>,
) {
    let mut v = view.borrow_mut();

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

    if height <= 4 {
        return;
    }

    // compute_split(size, first_half, first_second);
    let (_top_h, _bottom_h) = View::compute_split(height);

    // TODO: store
    v.layout_direction = LayoutDirection::Horizontal;
    v.layout_ops = vec![
        LayoutOperation::Percent { p: 50 },
        LayoutOperation::Percent { p: 50 },
    ];

    let sizes = view::compute_layout_sizes(height, &v.layout_ops);

    dbg_println!("splitH = SIZE {:?}", sizes);
    for s in &sizes {
        if *s == 0 {
            return;
        }
    }

    let doc = {
        if v.document.is_none() {
            None
        } else {
            let doc_id = v.document.as_ref().unwrap();
            let doc_id = doc_id.read().unwrap().id;
            if let Some(_doc) = editor.document_map.get(&doc_id) {
                let doc = editor.document_map.get(&doc_id).unwrap().clone();
                Some(Arc::clone(&doc))
            } else {
                None
            }
        }
    };

    let x = v.x;
    let mut y = v.y;
    for (idx, size) in sizes.iter().enumerate() {
        let mut view = view::View::new(Some(v.id), v.start_offset, width, *size, doc.clone());
        // create child modes
        view.x = x;
        view.y = y;

        for m in v.mode_ctx.iter() {
            let name = m.0;
            let mode = editor.modes.get(name.as_str()).unwrap();
            let ctx = mode.alloc_ctx();
            dbg_println!("view.id = {}", view.id);
            view.set_mode_ctx(mode.name(), ctx);
        }

        if idx == 0 {
            // TODO: propagate focus up to root
            v.focus_to = Some(view.id);

            let mut parent_id = v.parent_id;
            loop {
                if let Some(pid) = parent_id {
                    if let Some(pview) = editor.view_map.get(&pid) {
                        let mut pview = pview.borrow_mut();
                        pview.focus_to = Some(view.id);
                        parent_id = pview.parent_id;
                    } else {
                        break;
                    }
                } else {
                    break;
                }
            }
        }

        let id = view.id;
        let rc = Rc::new(RefCell::new(view));
        v.children.push(Rc::clone(&rc));
        editor.view_map.insert(id, Rc::clone(&rc));

        y += *size;
    }
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
