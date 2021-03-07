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

use crate::core::event;
use crate::core::event::input_map::eval_input_event;
use crate::core::event::Event;
use crate::core::event::Event::DrawEvent;
use crate::core::event::EventMessage;
use crate::core::event::InputEvent;
use crate::core::event::Key;
use crate::core::event::KeyModifiers;

use crate::core::modes::Mode;

use crate::core::screen::Screen;
use crate::core::view;
use crate::core::view::layout::FilterIoData;
use crate::core::view::layout::LayoutEnv;

use crate::core::view::LayoutDirection;

use crate::core::view::View;

/* TODO:

 InputStageActionMap is kept in EditorEnv
 Have a map per view
 and if eval fails, fallback to EditorEnv's
 It will allow per mode actions instanciate for each view
 transform into STACK of map ?

 TODO:
   check file metadata on every operations -> file changed ... reload ?

   add Timers -> for Blinking marks

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

    keep user arguments order, push new files,
    this list is never cleared
    before insertion the real path(*ln) is checked to avoid double open

    ioctl mode for block devices ?

    document_index: HashMap<String, document::Id>,  document::Id is the position in document_list
*/
// local
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum StagePosition {
    Pre,
    In,
    Post,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Stage {
    Input,
    Compositing,
    Render,
}

pub type InputStageFunction = fn(
    editor: &mut Editor<'static>,
    env: &mut EditorEnv<'static>,
    view: &Rc<RefCell<View<'static>>>,
);

pub type InputStageActionMap<'a> = HashMap<String, InputStageFunction>;

//
pub type RenderStageFunction = fn(
    editor: &mut Editor,
    env: &mut EditorEnv,
    view: &View,
    env: &mut LayoutEnv,
    input: &Vec<FilterIoData>,
    output: &mut Vec<FilterIoData>,
) -> ();

pub type RenderStageActionMap = HashMap<String, RenderStageFunction>;

//
//
use crate::core::config::Config;

use crate::core::document;
use crate::core::document::Document;
use crate::core::document::DocumentBuilder;

//
pub type Id = u64;

pub struct Editor<'a> {
    pub config: Config,
    pub document_map: HashMap<document::Id, Arc<RwLock<Document<'a>>>>,
    pub root_views: Vec<view::Id>,
    pub view_map: HashMap<view::Id, Rc<RefCell<View<'a>>>>,
    pub modes: HashMap<String, Rc<Box<dyn Mode>>>,
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
            root_views: vec![],
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
        self.modes.insert(name.to_owned(), Rc::new(mode));
    }
    pub fn get_mode<'e>(&mut self, name: &str) -> Option<&Rc<Box<dyn Mode>>> {
        self.modes.get(&name.to_owned())
    }
}

//////////////////////////////////////////////

// TODO: handle conflicting bindings
pub fn register_input_stage_action(
    map: &mut InputStageActionMap,
    s: &str,
    func: InputStageFunction,
) {
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
    view::compute_view_layout(editor, env, &view);
    send_draw_event(editor, &mut env, ui_tx, &view);
}

///////////////////////////////////////////////////////////////////////////////////////////////////

pub fn send_draw_event(
    _editor: &mut Editor,
    _env: &mut EditorEnv,
    ui_tx: &Sender<EventMessage>,
    view: &Rc<RefCell<View>>,
) {
    let view = view.borrow();

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

fn process_single_input_event<'a>(
    editor: &'a mut Editor<'static>,
    env: &'a mut EditorEnv<'static>,
    view_id: view::Id,
) -> bool {
    let mut view = &editor.view_map.get(&view_id).unwrap().clone();

    {
        let v = view.borrow();
        dbg_println!("DISPATCH EVENT TO VID {}", view_id);
        assert_eq!(v.id, view_id);
    }

    let ev = &env.current_input_event; // Option ?

    //
    if *ev == crate::core::event::InputEvent::NoInputEvent {
        // ignore no input event event :-)
        return false;
    }

    {
        let v = view.borrow();
        dbg_println!("prev (accum) events = {:?}", v.input_ctx.trigger);
        dbg_println!("eval input event input ev = {:?}", ev);
    }
    // TODO: track whole input seq // not tested

    let action_name = {
        let mut v = view.borrow_mut();
        v.input_ctx.trigger.push((*ev).clone());

        let mut in_node = v.input_ctx.current_node.clone();
        let mut out_node = v.input_ctx.next_node.clone();
        let action_name =
            eval_input_event(&ev, &v.input_ctx.input_map, &mut in_node, &mut out_node);
        // TODO: return out_node
        // swap for next call
        v.input_ctx.current_node = out_node;
        v.input_ctx.next_node = None;

        action_name.clone()
    };

    let action = {
        let mut v = view.borrow_mut();

        if action_name.is_none() {
            v.input_ctx.trigger.clear();
            return false;
        }
        let action_name = action_name.unwrap();
        dbg_println!("found action : [{}]", action_name);

        v.input_ctx.current_node = None;
        v.input_ctx.next_node = None;

        let action_fn = v.input_ctx.action_map.get(&action_name).clone();
        if action_fn.is_none() {
            dbg_println!("not function pointer found for action : {}", action_name);
            v.input_ctx.trigger.clear();
            return false;
        }
        let f = action_fn.clone().unwrap();
        f.clone()
    };

    // return action ?
    let start = Instant::now();
    action(editor, env, &mut view);
    let end = Instant::now();
    dbg_println!("time to run action {}", (end - start).as_millis());

    {
        let mut v = view.borrow_mut();
        v.input_ctx.trigger.clear();
    }

    true
}

fn flush_ui_event(mut editor: &mut Editor, mut env: &mut EditorEnv, ui_tx: &Sender<EventMessage>) {
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
    let view = editor.view_map.get(&vid);
    if view.is_none() {
        return env.view_id;
    }

    let view = view.unwrap().clone();

    let v = view.borrow();
    if v.children.len() == 0 {
        return vid;
    }

    if let Some(focused_vid) = v.focus_to {
        return get_focused_vid(&mut editor, &mut env, focused_vid);
    }

    vid
}

pub fn set_focus_on_vid(
    mut editor: &mut Editor<'static>,
    mut env: &mut EditorEnv<'static>,
    vid: view::Id,
) {
    let view = editor.view_map.get(&vid);
    if view.is_none() {
        return;
    }
    let view = Rc::clone(view.unwrap());
    let mut v = view.borrow_mut();
    set_focus_on_view(&mut editor, &mut env, &mut v);
}

pub fn set_focus_on_view(
    editor: &mut Editor<'static>,
    _env: &mut EditorEnv<'static>,
    view: &mut View<'static>,
) {
    // TODO: propagate focus up to root
    let vid = view.id;

    //    assert!(view.children.is_empty());

    let mut parent_id = view.parent_id;
    loop {
        if let Some(pid) = parent_id {
            dbg_println!("set_focus update parent_id {}", pid);
            if let Some(pview) = editor.view_map.get(&pid) {
                let mut pview = pview.borrow_mut();
                pview.focus_to = Some(vid);
                parent_id = pview.parent_id;
                dbg_println!("next  parent_id {:?}", parent_id);
            } else {
                break;
            }
        } else {
            break;
        }
    }
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
                    let child_v = editor.view_map.get(&child).unwrap().borrow_mut();
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
                    let child_v = editor.view_map.get(&child).unwrap().borrow_mut();
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

///////////////////////////////////////////////////////////////////////////////////////////////////

fn flatten_input_events(events: &Vec<InputEvent>) -> Vec<InputEvent> {
    let mut flat_events = vec![];
    // transform UnicodeArray(vec<char>) -> vec.len() * Unicode(char)
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

    flat_events
}

fn run_stages(
    stage: Stage,
    mut editor: &mut Editor<'static>,
    mut env: &mut EditorEnv<'static>,
    view_id: view::Id,
) -> Stage {
    use StagePosition::In;
    use StagePosition::Post;
    use StagePosition::Pre;

    run_stage(Pre, stage, &mut editor, &mut env, view_id);
    run_stage(In, stage, &mut editor, &mut env, view_id);
    //
    run_stage(Post, stage, &mut editor, &mut env, view_id)
}

fn run_stage(
    pos: StagePosition,
    stage: Stage,
    mut editor: &mut Editor<'static>,
    mut env: &mut EditorEnv<'static>,
    view_id: view::Id,
) -> Stage {
    let view = editor.view_map.get(&view_id);
    if view.is_none() {
        return match stage {
            Stage::Input => Stage::Compositing,
            Stage::Compositing => Stage::Render,
            Stage::Render => Stage::Input,
            // Stage::Restart ?
        };
    }

    let view = view.unwrap().clone();

    dbg_println!("render_stage VID {} : {:?} {:?}", view_id, pos, stage);

    match stage {
        Stage::Input => {
            // TODO: run_stage_input
            match pos {
                StagePosition::Pre => {
                    env.process_input_start = Instant::now();
                    view::run_stage(&mut editor, &mut env, &view, pos, stage);
                }
                StagePosition::In => {
                    // - need_rendering ? -
                    // move ev to env.current_event
                    env.event_processed =
                        process_single_input_event(&mut editor, &mut env, view_id);
                    if env.pending_events > 0 {
                        env.pending_events = crate::core::event::pending_input_event_dec(1);
                    }
                }
                StagePosition::Post => {
                    view::run_stage(&mut editor, &mut env, &view, pos, stage);

                    env.process_input_end = Instant::now();

                    if env.view_id != env.prev_vid {
                        env.event_processed = true;

                        dbg_println!("view change {} ->  {}", env.prev_vid, env.view_id);

                        check_view_dimension(editor, env);
                        {
                            // NB: resize previous view's screen to lower memory usage
                            let view = editor.view_map.get(&env.prev_vid).unwrap().clone();
                            view.borrow_mut().screen.write().unwrap().resize(1, 1);

                            // prepare next view input
                            let view = editor.view_map.get(&env.view_id).unwrap().clone();
                            view::run_stage(
                                &mut editor,
                                &mut env,
                                &view,
                                StagePosition::Pre,
                                Stage::Input,
                            );

                            // view changed
                            let id = env.view_id;
                            run_stages(Stage::Compositing, &mut editor, &mut env, id);
                        }
                    }
                }
            }
        }

        //
        Stage::Compositing => match pos {
            StagePosition::Pre => view::run_stage(&mut editor, &mut env, &view, pos, stage),
            StagePosition::In => view::run_stage(&mut editor, &mut env, &view, pos, stage),
            StagePosition::Post => view::run_stage(&mut editor, &mut env, &view, pos, stage),
        },

        //
        Stage::Render => match pos {
            StagePosition::Pre => {
                view::run_stage(&mut editor, &mut env, &view, pos, stage);
            }

            StagePosition::In => {
                view::run_stage(&mut editor, &mut env, &view, pos, stage);
                //
                let ui_tx = editor.ui_tx.clone();
                flush_ui_event(editor, env, &ui_tx);
            }

            StagePosition::Post => {
                view::run_stage(&mut editor, &mut env, &view, pos, stage);
            }
        },
    }

    if pos != StagePosition::Post {
        return stage;
    }

    match stage {
        Stage::Input => Stage::Compositing,
        Stage::Compositing => Stage::Render,
        Stage::Render => Stage::Input,
        // Stage::Restart ?
    }
}

fn setup_focus(
    mut editor: &mut Editor<'static>,
    mut env: &mut EditorEnv<'static>,
    ev: &InputEvent,
    compose: &mut bool,
) -> view::Id {
    let root_vid = env.view_id;
    let vid = get_focused_vid(&mut editor, &mut env, root_vid);
    dbg_println!("FOCUS on vid {}", vid);

    if root_vid != vid {
        // only set, not cleared
        *compose = true;
    };

    let (vid, ev) = clip_coordinates_and_get_vid(&mut editor, &mut env, ev, root_vid, vid);
    // - - TODO: if button press only: env.focus_on = Option<vid> ? -
    set_focus_on_vid(&mut editor, &mut env, vid);

    env.current_input_event = ev;
    env.prev_vid = root_vid;
    vid
}

// Loop over all input events
fn run_input_stage(
    mut editor: &mut Editor<'static>,
    mut env: &mut EditorEnv<'static>,
    events: &Vec<InputEvent>,
) -> Stage {
    use StagePosition::In;
    use StagePosition::Post;
    use StagePosition::Pre;

    // Pre
    // self.flat_events
    env.pending_events = crate::core::event::pending_input_event_count();
    let flat_events = events;
    //let flat_events = flatten_input_events(&events);
    if flat_events.len() == 0 {
        return Stage::Input;
    };

    // IN : move flat_events to en, StageTrait pre/in/post
    // run(&mut editor, &mut env) -> next (stage/pos)
    // and loop over
    // self.recompose = true
    let mut recompose = false;
    for ev in flat_events.iter() {
        let id = setup_focus(&mut editor, &mut env, &ev, &mut recompose);
        run_stages(Stage::Input, &mut editor, &mut env, id);
        run_stages(Stage::Compositing, &mut editor, &mut env, id);
    }

    // MOVE TO POST ?
    if let Some(focus_vid) = env.focus_changed_to {
        set_focus_on_vid(editor, env, focus_vid);
        env.focus_changed_to = None;
        // Stage::Restart ?
    }

    // POST
    let id = env.view_id;
    run_stage(Post, Stage::Input, &mut editor, &mut env, id);

    if recompose {
        Stage::Compositing
    } else {
        Stage::Render
    }
}

fn process_input_events(
    mut editor: &mut Editor<'static>,
    mut env: &mut EditorEnv<'static>,
    _ui_tx: &Sender<EventMessage>,
    events: &Vec<InputEvent>,
) {
    // TODO: move event to env ?
    let mut stage = run_input_stage(&mut editor, &mut env, &events);
    while stage != Stage::Input {
        let id = env.view_id;
        stage = run_stages(stage, &mut editor, &mut env, id);
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
