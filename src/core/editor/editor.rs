// std

use std::collections::HashMap;
use std::rc::Rc;

use std::cell::RefCell;
use std::sync::mpsc::Receiver;
use std::sync::mpsc::Sender;
use std::sync::Arc;

use parking_lot::RwLock;

use std::time::Instant;

// ext

// sibling
pub use super::*;

// crate

use crate::core::event;
use crate::core::event::input_map::eval_input_event;
use crate::core::event::input_map::DefaultActionMode;

use crate::core::event::Event;
use crate::core::event::Event::Draw;
use crate::core::event::EventMessage;
use crate::core::event::InputEvent;
use crate::core::event::Key;
use crate::core::event::KeyModifiers;

use crate::core::modes::Mode;

use crate::core::screen::Screen;
use crate::core::view;
use crate::core::view::FilterIo;
use crate::core::view::LayoutEnv;

use crate::core::view::LayoutDirection;

use crate::core::view::View;

/* TODO(ceg):

 InputStageActionMap is kept in EditorEnv
 Have a map per view
 and if eval fails, fallback to EditorEnv's
 It will allow per mode actions instantiate for each view
 transform into STACK of map ?

 TODO(ceg):
   check file metadata on every operations -> file changed ... reload ?

   add Timers -> for Blinking marks

   parse argument to extract line,column or offset {
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
    Pre = 0,
    In = 1,
    Post = 2,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Stage {
    Input = 0,
    Compositing = 1,
    UpdateUi = 2,
}

fn stage_to_index(stage: Stage) -> usize {
    stage as usize
}

// Option<Stage> ?
fn index_to_stage(index: usize) -> Stage {
    match index {
        0 => Stage::Input,
        1 => Stage::Compositing,
        2 => Stage::UpdateUi,
        _ => panic!("invalid stage index"),
    }
}

fn stage_pos_to_index(pos: StagePosition) -> usize {
    pos as usize
}

// Option<StagePosition> ?
fn _index_to_stage_pos(index: usize) -> StagePosition {
    match index {
        0 => StagePosition::Pre,
        1 => StagePosition::In,
        2 => StagePosition::Post,
        _ => panic!("invalid stage position index"),
    }
}

pub type InputStageFunction = fn(
    editor: &mut Editor<'static>,
    env: &mut EditorEnv<'static>,
    view: &Rc<RwLock<View<'static>>>,
);

pub type InputStageActionMap<'a> = HashMap<String, InputStageFunction>;

pub type StageFunction = fn(
    editor: &mut Editor<'static>,
    env: &mut EditorEnv<'static>,
    view: &Rc<RwLock<View<'static>>>,
    pos: StagePosition,
    stage: Stage,
);

//
pub type RenderStageFunction = fn(
    editor: &mut Editor,
    env: &mut EditorEnv,
    view: &View,
    env: &mut LayoutEnv,
    input: &Vec<FilterIo>,
    output: &mut Vec<FilterIo>,
);

pub type RenderStageActionMap = HashMap<String, RenderStageFunction>;

//
//
use crate::core::config::Config;

use crate::core::document;
use crate::core::document::Document;
use crate::core::document::DocumentBuilder;

pub struct Editor<'a> {
    pub config: Config,
    pub document_map: Arc<RwLock<HashMap<document::Id, Arc<RwLock<Document<'static>>>>>>,
    pub root_views: Vec<view::Id>,
    pub view_map: HashMap<view::Id, Rc<RwLock<View<'a>>>>,
    pub modes: Rc<RefCell<HashMap<String, Rc<RefCell<Box<dyn Mode>>>>>>,
    pub core_tx: Sender<EventMessage<'a>>,
    pub ui_tx: Sender<EventMessage<'a>>,
    pub worker_tx: Sender<EventMessage<'a>>,
    pub indexer_tx: Sender<EventMessage<'a>>,
}

impl<'a> Editor<'a> {
    ///
    pub fn new(
        config: Config,
        //
        core_tx: Sender<EventMessage<'a>>,
        ui_tx: Sender<EventMessage<'a>>,
        worker_tx: Sender<EventMessage<'a>>,
        indexer_tx: Sender<EventMessage<'a>>,
    ) -> Editor<'a> {
        Editor {
            config,
            document_map: Arc::new(RwLock::new(HashMap::new())),
            root_views: vec![],
            view_map: HashMap::new(),
            modes: Rc::new(RefCell::new(HashMap::new())),
            ui_tx,
            core_tx,
            worker_tx,
            indexer_tx,
        }
    }

    pub fn is_root_view(&self, id: view::Id) -> bool {
        self.root_views.iter().find(|&&x| x == id).is_some()
    }

    ///
    pub fn setup_default_buffers(&mut self) {
        let mut builder = DocumentBuilder::new();
        builder.document_name("debug-message").internal(true);

        let b = builder.finalize();

        let document_map = self.document_map.clone();
        let mut document_map = document_map.write();

        if let Some(b) = b {
            let id = document_map.len();
            document_map.insert(document::Id(id), b);
        }

        let mut builder = DocumentBuilder::new();
        builder.document_name("scratch").internal(true);

        let b = builder.finalize();

        if let Some(b) = b {
            let id = document_map.len();
            document_map.insert(document::Id(id), b);
        }
    }

    pub fn register_mode<'e>(&mut self, mode: Box<dyn Mode>) {
        let name = mode.name();
        self.modes
            .borrow_mut()
            .insert(name.to_owned(), Rc::new(RefCell::new(mode)));
    }

    pub fn get_mode<'e>(&mut self, name: &str) -> Option<Rc<RefCell<Box<dyn Mode>>>> {
        let h = self.modes.clone();
        let h = h.borrow();
        let m = h.get(&name.to_owned());
        if m.is_none() {
            return None;
        }
        Some(m.unwrap().clone())
    }
}

//////////////////////////////////////////////

// TODO(ceg): handle conflicting bindings
pub fn register_input_stage_action(
    map: &mut InputStageActionMap,
    s: &str,
    func: InputStageFunction,
) {
    map.insert(s.to_string(), func);
}

pub fn check_view_dimension(editor: &Editor, env: &EditorEnv) {
    dbg_println!("checking view dimension {:?}", env.view_id);

    let view = editor.view_map.get(&env.view_id);
    let view = view.unwrap();
    let view = view.as_ref();
    let mut view = view.write();

    // resize ?
    {
        let screen = view.screen.read();
        if env.width == screen.width() && env.height == screen.height() {
            return;
        }
    }

    view.screen = Arc::new(RwLock::new(Box::new(Screen::new(env.width, env.height))));
}

pub fn update_view_and_send_draw_event(
    mut editor: &mut Editor<'static>,
    mut env: &mut EditorEnv<'static>,
) {
    // check size
    check_view_dimension(editor, env);

    let view_id = env.view_id;
    run_stages(Stage::Compositing, &mut editor, &mut env, view_id);
    run_stages(Stage::UpdateUi, &mut editor, &mut env, view_id);

    if crate::core::bench_to_eof() {
        // WILL stop the editor and quit immediately
        env.quit = true;
    }
}

///////////////////////////////////////////////////////////////////////////////////////////////////

pub fn send_draw_event(
    _editor: &mut Editor,
    _env: &mut EditorEnv,
    ui_tx: &Sender<EventMessage>,
    view: &Rc<RwLock<View>>,
) {
    let view = view.read();

    let new_screen = Arc::clone(&view.screen);

    let msg = EventMessage::new(
        0, // get_next_seq(&mut seq), TODO
        Draw {
            screen: new_screen,
            time: Instant::now(),
        },
    );

    crate::core::event::pending_render_event_inc(1);
    ui_tx.send(msg).unwrap_or(());
}

use crate::core::event::InputEventRule;

fn eval_input_stack_level(
    v: &mut View,
    default_action_mode: DefaultActionMode,
    mut trigger_pos: usize,
    trigger_pos_max: usize,
    mut stack_index: usize,
    mut in_node: &mut Option<Rc<InputEventRule>>,
) -> Option<String> {
    let mut action_name = None;

    while stack_index > 0 {
        stack_index -= 1;

        dbg_println!("--------------------------------------------------------");
        dbg_println!("checking stack_index = {}", stack_index);

        for ev_pos in trigger_pos..trigger_pos_max {
            let ev = &v.input_ctx.trigger[ev_pos];
            dbg_println!("playing event[{}] = {:?}", ev_pos, ev);
            let mut out_node = None;
            let input_map = &v.input_ctx.input_map.borrow()[stack_index];
            action_name = eval_input_event(
                &ev,
                &input_map,
                default_action_mode,
                &mut in_node,
                &mut out_node,
            );

            if action_name.is_some() {
                // stop a first match
                dbg_println!("after play : found action {:?}", action_name);
                break;
            }
            dbg_println!("after play : in_node = {:?}", in_node);
            dbg_println!("after play : out_node = {:?}", out_node);

            if out_node.is_none() {
                // no match
                dbg_println!("no match");
            }
            dbg_println!("save out_node");
            *in_node = out_node;
        }

        if action_name.is_some() {
            dbg_println!(
                "found action {:?} at input stack level {}",
                action_name,
                stack_index
            );
            break;
        }

        if in_node.is_some() {
            dbg_println!("found sequence start at input stack index {}", stack_index);
            v.input_ctx.stack_pos = Some(stack_index);
            return None;
        }

        dbg_println!("no action at input stack index {}", stack_index);

        // restart the whole sequence for next level
        if stack_index > 0 {
            trigger_pos = 0;
            *in_node = None;
            dbg_println!("restart input at stack index {}", stack_index - 1);
        } else {
            dbg_println!(
                "no sequence found in stack  (default: {:?})",
                default_action_mode
            );
            if in_node.is_none() {
                v.input_ctx.stack_pos = None;
            } else {
                v.input_ctx.stack_pos = Some(stack_index);
            }
        }
    }

    action_name
}

fn process_single_input_event<'a>(
    editor: &'a mut Editor<'static>,
    env: &'a mut EditorEnv<'static>,
    view_id: view::Id,
) -> bool {
    let mut view = &editor.view_map.get(&view_id).unwrap().clone();
    {
        let v = view.read();
        dbg_println!("DISPATCH EVENT TO VID {:?}", view_id);
        assert_eq!(v.id, view_id);
    }

    let ev = &env.current_input_event; // Option ?
    if *ev == crate::core::event::InputEvent::NoInputEvent {
        // ignore no input event event :-)
        return false;
    }

    // record input sequence
    {
        let mut v = view.write();
        dbg_println!("eval input event input ev = {:?}", ev);
        dbg_println!("prev (accum) events = {:?}", v.input_ctx.trigger);
        v.input_ctx.trigger.push((*ev).clone());
        dbg_println!("cur (accum) events = {:?}", v.input_ctx.trigger);
    }

    // action_name = check_input_map_stack(editor, env, v);
    // {
    let action_name = {
        let mut v = view.write();

        let stack_pos = if let Some(stack_pos) = v.input_ctx.stack_pos {
            // current map
            dbg_println!("reuse stack level {}", stack_pos + 1);
            stack_pos + 1
        } else {
            // top
            let pos = v.input_ctx.input_map.as_ref().borrow().len();
            dbg_println!("start from stack top level {}", pos);
            pos
        };

        if stack_pos == 0 {
            v.input_ctx.trigger.clear();
            v.input_ctx.stack_pos = None;
            return false;
        }

        v.input_ctx.stack_pos = Some(stack_pos);

        let mut in_node = v.input_ctx.current_node.clone();
        dbg_println!("last node = {:?}", in_node);

        // TODO(ceg): function
        let trigger_pos = v.input_ctx.trigger.len() - 1;
        let trigger_pos_max = v.input_ctx.trigger.len();

        dbg_println!("trigger_pos     = {}", trigger_pos);
        dbg_println!("trigger_pos_max = {}", trigger_pos_max);

        // first pass (no default/fallback action)
        let action_name = eval_input_stack_level(
            &mut v,
            DefaultActionMode::IgnoreDefaultAction,
            trigger_pos,
            trigger_pos_max,
            stack_pos,
            &mut in_node,
        );

        if in_node.is_some() {
            v.input_ctx.current_node = in_node.clone(); // save last input node
            dbg_println!("save node {:?}", in_node);
        }

        dbg_println!(
            "1st pass action_name '{:?}' in_node {:?}",
            action_name,
            in_node
        );

        // 2nd  pass with default/fallback action enabled
        let action_name2 = if action_name.is_none() && in_node.is_none() {
            dbg_println!("try default rules/ replay all triggers");
            v.input_ctx.stack_pos = None;

            let name = eval_input_stack_level(
                &mut v,
                DefaultActionMode::RunDefaultAction,
                0, // NB: restart whole sequence
                trigger_pos_max,
                stack_pos,
                &mut in_node,
            );

            if in_node.is_some() {
                v.input_ctx.current_node = in_node.clone(); // save last input node
                dbg_println!("save node {:?}", in_node);
            }

            dbg_println!("2nd pass action_name '{:?}' in_node {:?}", name, in_node);

            name
        } else {
            action_name.clone()
        };

        if action_name2.is_none() && in_node.is_none() {
            v.input_ctx.trigger.clear();
            v.input_ctx.current_node = None;
            v.input_ctx.stack_pos = None;
            dbg_println!("clear input ctx");
        }

        action_name2
    };

    if action_name.is_none() {
        let v = view.read();
        if v.input_ctx.current_node.is_some() {
            dbg_println!(" no action found , but sequence started -> return false");
        } else {
            dbg_println!("no action found -> return false");
        }

        return false;
    }

    // exec_input_action()
    let action = {
        let mut v = view.write();

        let action_name = action_name.unwrap();
        dbg_println!("found action : [{}]", action_name);

        let action_fn = v.input_ctx.action_map.get(&action_name).clone();
        if action_fn.is_none() {
            dbg_println!("not function pointer found for action : {}", action_name);
            v.input_ctx.trigger.clear();
            v.input_ctx.current_node = None;
            v.input_ctx.stack_pos = None;
            return false;
        }
        let f = action_fn.clone().unwrap();
        f.clone()
    };

    // return action ?
    let start = Instant::now();
    action(editor, env, &mut view);
    let end = Instant::now();
    dbg_println!("time to run action {} µs", (end - start).as_micros());

    {
        let mut v = view.write();
        v.input_ctx.trigger.clear();
        v.input_ctx.current_node = None;
        v.input_ctx.stack_pos = None;
    }

    true
}

fn flush_ui_event(mut editor: &mut Editor, mut env: &mut EditorEnv, ui_tx: &Sender<EventMessage>) {
    //
    let p_input = crate::core::event::pending_input_event_count();
    let p_rdr = crate::core::event::pending_render_event_count();

    //    dbg_println!("FLUSH: pending input  event  = {}\r", p_input);
    //    dbg_println!("FLUSH: pending render events = {}\r", p_rdr);

    // % last render time
    // TODO(ceg): receive FPS from ui in Event ?
    if (p_rdr <= 60) || p_input <= 60 {
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

    let v = view.read();
    if v.children.is_empty() {
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
    let mut v = view.write();
    set_focus_on_view(&mut editor, &mut env, &mut v);
}

pub fn set_focus_on_view(
    editor: &mut Editor<'static>,
    env: &mut EditorEnv<'static>,
    view: &mut View<'static>,
) {
    // TODO(ceg): propagate focus up to root
    let vid = view.id;

    //    assert!(view.children.is_empty());

    let mut parent_id = view.parent_id;
    loop {
        if let Some(pid) = parent_id {
            dbg_println!("set_focus update parent_id {:?}", pid);
            if let Some(pview) = editor.view_map.get(&pid) {
                let mut pview = pview.write();
                pview.focus_to = Some(vid);
                parent_id = pview.parent_id;
                env.focus_on = vid; // Option ?
                dbg_println!("next  parent_id {:?}", parent_id);
            } else {
                break;
            }
        } else {
            break;
        }
    }
}

// always compute ?
fn clip_locked_coordinates_xy(
    _editor: &mut Editor<'static>,
    env: &mut EditorEnv<'static>,
    _root_vid: view::Id,
    _vid: view::Id,
    x: &mut i32,
    y: &mut i32,
) -> view::Id {
    let id = env.focus_locked_on.unwrap();

    dbg_println!(
        "CLIPPING LOCKED  {:?} ----------------------------------BEGIN",
        id
    );

    dbg_println!(
        "CLIPPING LOCKED ---------------------------------- X({}) Y({})",
        x,
        y
    );
    dbg_println!(
        "CLIPPING LOCKED ---------------------------------- GL X({}) GL Y({})",
        x,
        y
    );

    env.diff_x = *x - env.global_x.unwrap();
    env.diff_y = *y - env.global_y.unwrap();

    // update local coordinates
    // it is up to the mode to ignore negative values
    *x = env.local_x.unwrap() + env.diff_x;
    *y = env.local_y.unwrap() + env.diff_y;

    dbg_println!(
        "CLIPPING LOCKED ---------------------------------- DIFF X({}) Y({})",
        env.diff_x,
        env.diff_y
    );
    dbg_println!("CLIPPING LOCKED ----------------------------------END");

    id
}

// clips (x,y) to local view @ (x,y)
// returns the view's id at
fn clip_coordinates_xy(
    mut editor: &mut Editor<'static>,
    mut env: &mut EditorEnv<'static>,
    root_vid: view::Id,
    vid: view::Id,
    mut x: &mut i32,
    mut y: &mut i32,
) -> view::Id {
    let mut id = root_vid;

    if env.focus_locked_on.is_some() {
        return clip_locked_coordinates_xy(&mut editor, &mut env, root_vid, vid, &mut x, &mut y);
    }

    let root_x = *x;
    let root_y = *y;
    env.diff_x = 0;
    env.diff_y = 0;
    env.global_x = Some(root_x);
    env.global_y = Some(root_y);

    // check layout type
    dbg_println!("CLIPPING -----------------------------------BEGIN");
    dbg_println!("CLIPPING clipping orig coords ({},{})", *x, *y);
    dbg_println!("CLIPPING         select  {:?}", id);

    loop {
        'inner: loop {
            if let Some(v) = editor.view_map.get(&id) {
                let v = v.read();

                if v.children.is_empty() {
                    dbg_println!("CLIPPING        no more children");
                    dbg_println!("CLIPPING ----------------------------------- END");
                    return id;
                }

                for child in v.children.iter() {
                    let child_v = editor.view_map.get(&child).unwrap().write();
                    let screen = child_v.screen.read();

                    dbg_println!(
                    "CLIPPING dump child  {:?} dim [x({}), y({})][w({}) h({})] [x+w({}) y+h({})]",
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

                let mut last_id = view::Id(0);
                for (idx, child) in v.children.iter().enumerate() {
                    let child_v = editor.view_map.get(&child).unwrap().write();
                    let screen = child_v.screen.read();

                    last_id = child_v.id;

                    dbg_println!(
                    "CLIPPING checking child  {:?} dim [x({}), y({})][w({}) h({})] [x+w({}) y+h({})]",
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
                            *y -= child_v.y as i32;
                        } else {
                            *x -= child_v.x as i32;
                        }

                        // found
                        dbg_println!("CLIPPING         updated clipping coords ({},{})", *x, *y);
                        dbg_println!("CLIPPING         select  {:?}", child_v.id);

                        env.local_x = Some(*x);
                        env.local_y = Some(*y);

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
    dbg_println!("CLIPPING ev in: {:?}", ev);

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

    dbg_println!("CLIPPING ev out: {:?}", ev);

    (vid, ev)
}

///////////////////////////////////////////////////////////////////////////////////////////////////

fn _flatten_input_events(events: &Vec<InputEvent>) -> Vec<InputEvent> {
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
            Stage::Compositing => Stage::UpdateUi,
            Stage::UpdateUi => Stage::Input,
            // Stage::Restart ?
        };
    }

    let start = Instant::now();

    let view = view.unwrap().clone();

    match stage {
        Stage::Input => {
            // TODO(ceg): run_stage_input
            match pos {
                StagePosition::Pre => {
                    env.process_input_start = Instant::now();
                    env.skip_compositing = false;
                    view::run_stage(&mut editor, &mut env, &view, pos, stage);
                }

                StagePosition::In => {
                    //  move block to ? view::run_stage(&mut editor, &mut env, &view, pos, stage);

                    // - need_rendering ? -
                    // move ev to env.current_event
                    env.event_processed =
                        process_single_input_event(&mut editor, &mut env, view_id);
                }
                StagePosition::Post => {
                    view::run_stage(&mut editor, &mut env, &view, pos, stage);

                    env.process_input_end = Instant::now();

                    if env.view_id != env.prev_vid {
                        env.event_processed = true;

                        dbg_println!("view change {:?} ->  {:?}", env.prev_vid, env.view_id);

                        check_view_dimension(editor, env);
                        {
                            // NB: resize previous view's screen to lower memory usage
                            if let Some(view) = editor.view_map.get(&env.prev_vid) {
                                view.write().screen.write().resize(1, 1);
                            }

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
        Stage::UpdateUi => match pos {
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

    let end = Instant::now();
    let diff = (end - start).as_micros();
    env.time_spent[stage_to_index(stage)][stage_pos_to_index(pos)] += diff;

    if pos != StagePosition::Post {
        return stage;
    }

    match stage {
        Stage::Input => {
            if env.skip_compositing {
                dbg_println!("skip Stage::Compositing");
                Stage::UpdateUi
            } else {
                Stage::Compositing
            }
        }
        Stage::Compositing => Stage::UpdateUi,
        Stage::UpdateUi => Stage::Input,
        // Stage::Restart ?
    }
}

fn setup_focus_and_event(
    mut editor: &mut Editor<'static>,
    mut env: &mut EditorEnv<'static>,
    ev: &InputEvent,
    compose: &mut bool,
) -> view::Id {
    env.focus_on = view::Id(0);
    let root_vid = env.view_id;
    let vid = get_focused_vid(&mut editor, &mut env, root_vid);
    dbg_println!("FOCUS on  {:?}", vid);

    if root_vid != vid {
        // only set, not cleared
        *compose = true;
    };

    let (vid, ev) = clip_coordinates_and_get_vid(&mut editor, &mut env, ev, root_vid, vid);
    // - - TODO(ceg): if button press only: env.focus_on = Option<vid> ? -
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
    use StagePosition::Post;

    // Pre
    // self.flat_events
    env.pending_events = crate::core::event::pending_input_event_count();
    let flat_events = events;
    //let flat_events = flatten_input_events(&events);
    if flat_events.is_empty() {
        return Stage::Input;
    };

    // IN : move flat_events to en, StageTrait pre/in/post
    // run(&mut editor, &mut env) -> next (stage/pos)
    // and loop over
    // self.recompose = true
    let mut recompose = false;
    let _ui_tx = editor.ui_tx.clone();

    for ev in flat_events.iter() {
        if env.pending_events > 0 {
            env.pending_events = crate::core::event::pending_input_event_dec(1);
        }

        let id = setup_focus_and_event(&mut editor, &mut env, &ev, &mut recompose);
        run_stages(Stage::Input, &mut editor, &mut env, id);
        // if !env.skip_compositing
        {
            run_stages(Stage::Compositing, &mut editor, &mut env, id);
        }
        // flush_ui_event(editor, env, &ui_tx);
        //run_stages(Stage::UpdateUi, &mut editor, &mut env, id);
    }

    // MOVE TO POST ?
    if let Some(focus_vid) = env.focus_changed_to {
        set_focus_on_vid(editor, env, focus_vid);
        env.focus_changed_to = None;
        // Stage::Restart ?
    }

    // POST ?
    let id = env.view_id;
    run_stage(Post, Stage::Input, &mut editor, &mut env, id);

    if recompose {
        Stage::Compositing
    } else {
        Stage::UpdateUi
    }
}

fn process_input_events(
    mut editor: &mut Editor<'static>,
    mut env: &mut EditorEnv<'static>,
    _ui_tx: &Sender<EventMessage>,
    events: &Vec<InputEvent>,
) {
    let start = Instant::now();

    env.time_spent = [[0, 0, 0], [0, 0, 0], [0, 0, 0]];

    let mut stage = run_input_stage(&mut editor, &mut env, &events);

    while stage != Stage::Input {
        let id = env.view_id;
        stage = run_stages(stage, &mut editor, &mut env, id);
    }

    let end = Instant::now();

    for (idx, _f) in env.time_spent.iter().enumerate() {
        dbg_println!(
            "time spent in {:?} : {:4?} µs\r",
            index_to_stage(idx),
            env.time_spent[idx]
        );
    }

    dbg_println!(
        "input event : total process time {} µs\r",
        (end - start).as_micros()
    );
}

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
                Event::UpdateView { width, height } => {
                    env.width = width;
                    env.height = height;

                    env.pending_events = crate::core::event::pending_input_event_dec(1);
                    update_view_and_send_draw_event(&mut editor, &mut env);
                }

                Event::RefreshView => {
                    update_view_and_send_draw_event(&mut editor, &mut env);
                }

                Event::Input { events } => {
                    if !editor.view_map.is_empty() {
                        process_input_events(&mut editor, &mut env, &ui_tx, &events);
                    }
                }

                _ => {}
            }
        }
    }

    // stop indexer(s)
    {
        for (_id, d) in editor.document_map.as_ref().read().iter() {
            d.as_ref().write().abort_indexing = true;
        }
    }

    // send ApplicationQuitEvent to worker thread
    let msg = EventMessage::new(0, Event::ApplicationQuitEvent);
    editor.worker_tx.send(msg).unwrap_or(());

    // send ApplicationQuitEvent to ui thread
    let msg = EventMessage::new(get_next_seq(&mut seq), Event::ApplicationQuitEvent);
    ui_tx.send(msg).unwrap_or(());

    // send ApplicationQuitEvent to indexer thread
    let msg = EventMessage::new(get_next_seq(&mut seq), Event::ApplicationQuitEvent);
    editor.indexer_tx.send(msg).unwrap_or(());
}
