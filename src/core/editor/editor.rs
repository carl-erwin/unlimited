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
use crate::core::disable_dbg_println;
use crate::core::enable_dbg_println;
use crate::core::get_dbg_println_flag;
use crate::core::toggle_dbg_println;

use crate::core::buffer;
use crate::core::buffer::Buffer;

use crate::core::buffer::BufferEvent;

use crate::core::config::Config;

use crate::core::event;
use crate::core::event::input_map::eval_input_event;
use crate::core::event::input_map::DefaultActionMode;

use crate::core::event::Event;
use crate::core::event::Event::Draw;
use crate::core::event::InputEvent;
use crate::core::event::Message;

use crate::core::event::Key;
use crate::core::event::KeyModifiers;

use crate::core::modes::Mode;

use crate::core::screen::Screen;
use crate::core::view;
use crate::core::view::FilterIo;
use crate::core::view::LayoutEnv;

use crate::core::view::LayoutDirection;

use crate::core::view::View;
use crate::core::view::ViewEvent;
use crate::core::view::ViewEventDestination;
use crate::core::view::ViewEventSource;

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

    buffer_list: Vec<
        struct BufferInfo {
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

    buffer_index: HashMap<String, buffer::Id>,  buffer::Id is the position in buffer_list
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
    layout_env: &mut LayoutEnv,
    input: &Vec<FilterIo>,
    output: &mut Vec<FilterIo>,
);

pub type RenderStageActionMap = HashMap<String, RenderStageFunction>;

pub struct Editor<'a> {
    pub config: Config,
    pub buffer_map: Arc<RwLock<HashMap<buffer::Id, Arc<RwLock<Buffer<'static>>>>>>,
    pub root_views: Vec<view::Id>,
    pub view_map: Arc<RwLock<HashMap<view::Id, Rc<RwLock<View<'a>>>>>>,
    pub modes: Rc<RefCell<HashMap<String, Rc<RefCell<Box<dyn Mode>>>>>>,
    pub dir_modes: Rc<RefCell<HashMap<String, Rc<RefCell<Box<dyn Mode>>>>>>,
    pub core_tx: Sender<Message<'a>>,
    pub ui_tx: Sender<Message<'a>>,
    pub worker_tx: Sender<Message<'a>>,
    pub indexer_tx: Sender<Message<'a>>,
}

impl<'a> Editor<'a> {
    ///
    pub fn new(
        config: Config,
        //
        core_tx: Sender<Message<'a>>,
        ui_tx: Sender<Message<'a>>,
        worker_tx: Sender<Message<'a>>,
        indexer_tx: Sender<Message<'a>>,
    ) -> Editor<'a> {
        Editor {
            config,
            buffer_map: Arc::new(RwLock::new(HashMap::new())),
            root_views: vec![],
            view_map: Arc::new(RwLock::new(HashMap::new())),
            modes: Rc::new(RefCell::new(HashMap::new())),
            dir_modes: Rc::new(RefCell::new(HashMap::new())),
            ui_tx,
            core_tx,
            worker_tx,
            indexer_tx,
        }
    }

    pub fn is_root_view(&self, id: view::Id) -> bool {
        self.root_views.iter().find(|&&x| x == id).is_some()
    }

    pub fn register_mode<'e>(&mut self, mode: Box<dyn Mode>) {
        let name = mode.name();
        self.modes
            .borrow_mut()
            .insert(name.to_owned(), Rc::new(RefCell::new(mode)));
    }

    pub fn register_directory_mode<'e>(&mut self, mode: Box<dyn Mode>) {
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

    // insert view into editor global map, no checks
    pub fn add_view(&mut self, id: view::Id, view: View<'a>) {
        assert_ne!(id, view::Id(0));
        self.view_map.write().insert(id, Rc::new(RwLock::new(view))); // move to View::new ?
    }

    pub fn buffer_by_id(&mut self, bid: buffer::Id) -> Arc<RwLock<Buffer<'static>>> {
        self.buffer_map.write().get(&bid).unwrap().clone()
    }
}

pub fn config_var_is_set(editor: &Editor<'static>, var_name: &str, default: bool) -> bool {
    if let Some(v) = editor.config.vars.get(var_name) {
        !(v == "0")
    } else {
        default
    }
}

pub fn config_var_get<'a>(editor: &'a Editor<'a>, var_name: &str) -> Option<&'a String> {
    editor.config.vars.get(var_name)
}

pub fn get_view_map(
    editor: &Editor<'static>,
) -> Arc<RwLock<HashMap<view::Id, Rc<RwLock<View<'static>>>>>> {
    let map = editor.view_map.clone();
    map
}

pub fn get_view_by_id(editor: &Editor<'static>, vid: view::Id) -> Rc<RwLock<View<'static>>> {
    editor.view_map.read().get(&vid).unwrap().clone()
}

pub fn remove_view_by_id(
    editor: &Editor<'static>,
    vid: view::Id,
) -> Option<Rc<RwLock<View<'static>>>> {
    let mut map = editor.view_map.write();
    map.remove(&vid)
}

pub fn check_view_by_id(
    editor: &Editor<'static>,
    vid: view::Id,
) -> Option<Rc<RwLock<View<'static>>>> {
    let map = editor.view_map.read();
    let rc = map.get(&vid)?;
    Some(rc.clone())
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

pub fn check_view_dimension(editor: &mut Editor<'static>, env: &EditorEnv) {
    dbg_println!("checking view dimension {:?}", env.root_view_id);

    let view = get_view_by_id(editor, env.root_view_id);
    let view = view.as_ref();
    let mut view = view.write();

    // resize ?
    {
        let screen = view.screen.read();
        if env.width == screen.width() && env.height == screen.height() {
            return;
        }
    }

    dbg_println!("resize view {}x{}", env.width, env.height);

    view.screen = Arc::new(RwLock::new(Box::new(Screen::new(env.width, env.height))));
    view.width = env.width; // remove view.width/height ?
    view.height = env.height;

    dbg_println!("resize OK");
}

pub fn update_view_and_send_draw_event(
    mut editor: &mut Editor<'static>,
    mut env: &mut EditorEnv<'static>,
) {
    // check size
    check_view_dimension(editor, env);

    let view_id = env.root_view_id;
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
    ui_tx: &Sender<Message>,
    view: &Rc<RwLock<View>>,
) {
    let view = view.read();

    let new_screen = Arc::clone(&view.screen);

    let msg = Message::new(
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

//
fn eval_stack_index(
    v: &mut View,
    stack_index: usize,
    default_action_mode: DefaultActionMode,
    trigger_pos: usize,
    trigger_pos_max: usize,
    mut in_node: &mut Option<Rc<InputEventRule>>,
) -> Option<String> {
    for ev_pos in trigger_pos..trigger_pos_max {
        let ev = &v.input_ctx.trigger[ev_pos];
        let mut out_node = None;
        let input_map = &v.input_ctx.input_map.borrow()[stack_index];
        let action_name = eval_input_event(
            &ev,
            &input_map.1,
            default_action_mode,
            &mut in_node,
            &mut out_node,
        );
        // stop a first match
        if let Some(action_name) = action_name {
            return Some(action_name);
        }
        // no match
        *in_node = out_node;
    }
    None
}

fn eval_input_stack_level(
    v: &mut View,
    default_action_mode: DefaultActionMode,
    mut trigger_pos: usize,
    trigger_pos_max: usize,
    mut stack_index: usize,
    in_node: &mut Option<Rc<InputEventRule>>,
) -> Option<String> {
    while stack_index > 0 {
        stack_index -= 1;
        let action_name = eval_stack_index(
            v,
            stack_index,
            default_action_mode,
            trigger_pos,
            trigger_pos_max,
            in_node,
        );
        // found action
        if action_name.is_some() {
            return action_name;
        }
        //
        if in_node.is_some() {
            v.input_ctx.stack_pos = Some(stack_index);
            return None;
        }
        // restart the whole sequence for next level
        if stack_index > 0 {
            trigger_pos = 0;
            *in_node = None;
            continue;
        }
        // last level
        if in_node.is_none() {
            v.input_ctx.stack_pos = None;
        } else {
            v.input_ctx.stack_pos = Some(stack_index);
        }
    }

    None
}

fn process_single_input_event<'a>(
    editor: &'a mut Editor<'static>,
    env: &'a mut EditorEnv<'static>,
    view_id: view::Id,
) -> bool {
    let mut view = get_view_by_id(editor, view_id);
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
        v.input_ctx.trigger.push((*ev).clone());
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

        if action_name.is_some() {
            dbg_println!("1st pass action_name '{:?}'", action_name);
        } else {
            dbg_println!("1st pass previous node {:?}", in_node);
        }

        // 2nd  pass with default/fallback action enabled
        let action_name2 = if action_name.is_none() && in_node.is_none() {
            dbg_println!("try default rules/replay all triggers");
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

            if name.is_some() {
                dbg_println!("2st pass action_name '{:?}'", name);
            } else {
                dbg_println!("2st pass previous node {:?}", in_node);
            }

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
            dbg_println!(" no action found, but sequence started -> return false");
        } else {
            dbg_println!("no action found -> return false");
        }

        return false;
    }

    let mut debug_action = false;

    let debug_flag = get_dbg_println_flag();

    // exec_input_action()
    let action = {
        let mut v = view.write();

        let action_name = action_name.unwrap();
        dbg_println!("found action : [{}]", action_name);

        let action_fn = v.input_ctx.action_map.get(&action_name).clone();
        if action_fn.is_none() {
            dbg_println!("no function pointer found for action : {}", action_name);
            v.input_ctx.trigger.clear();
            v.input_ctx.current_node = None;
            v.input_ctx.stack_pos = None;
            return false;
        }

        let var = format!("trace:{action_name}");
        if config_var_is_set(&editor, &var, false) {
            debug_action = true;
        }

        let f = action_fn.clone().unwrap();
        f.clone()
    };

    // return action ?
    if debug_action {
        enable_dbg_println();
    }

    let start = Instant::now();

    action(editor, env, &mut view);
    let end = Instant::now();

    if debug_flag == 0 {
        disable_dbg_println();
    }

    dbg_println!("time to run action {} µs", (end - start).as_micros());

    {
        let mut v = view.write();
        v.input_ctx.trigger.clear();
        v.input_ctx.current_node = None;
        v.input_ctx.stack_pos = None;
    }

    true
}

fn flush_ui_event(mut editor: &mut Editor, mut env: &mut EditorEnv, ui_tx: &Sender<Message>) {
    //
    let p_input = crate::core::event::pending_input_event_count();
    let p_rdr = crate::core::event::pending_render_event_count();

    //    dbg_println!("FLUSH: pending input  event  = {}\r", p_input);
    //    dbg_println!("FLUSH: pending render events = {}\r", p_rdr);

    // % last render time
    // TODO(ceg): receive FPS from ui in Event ?
    if (p_rdr <= 60) || p_input <= 60 {
        // hit
        let view = editor
            .view_map
            .read()
            .get(&env.root_view_id)
            .unwrap()
            .clone();
        send_draw_event(&mut editor, &mut env, ui_tx, &view);
        env.last_rdr_event = Instant::now();
    }
}

fn get_focused_view_id(
    mut editor: &mut Editor<'static>,
    mut env: &mut EditorEnv<'static>,
    vid: view::Id,
) -> view::Id {
    let vid = vid;
    let view = check_view_by_id(editor, vid);
    if view.is_none() {
        return env.root_view_id;
    }

    let view = view.unwrap();
    let v = view.read();

    // TODO(ceg): floating_children in priority ?

    if v.children.is_empty() {
        return vid;
    }

    if let Some(focused_view_id) = v.focus_to {
        return get_focused_view_id(&mut editor, &mut env, focused_view_id);
    }

    vid
}

pub fn set_focus_on_view_id(
    mut editor: &mut Editor<'static>,
    mut env: &mut EditorEnv<'static>,
    vid: view::Id,
) {
    let view = check_view_by_id(editor, vid);
    if view.is_none() {
        return;
    }
    let view = view.unwrap();
    let mut v = view.write();

    if v.ignore_focus == true {
        // require:  explicit focus grabbing
        return;
    }

    set_active_view(&mut editor, &mut env, &mut v);
}

pub fn set_active_view(
    editor: &mut Editor<'static>,
    env: &mut EditorEnv<'static>,
    view: &mut View<'static>,
) {
    // TODO(ceg): propagate focus up to root
    let vid = view.id;

    let prev_view_id = env.active_view.unwrap_or(view::Id(0));
    if prev_view_id == vid {
        return;
    }

    dbg_println!("set_active_view ---------");
    dbg_println!("set_active_view update vid {:?}", vid);
    dbg_println!("focus changed {:?} -> {:?}", prev_view_id, vid);

    let mut parent_id = view.parent_id;

    dbg_println!("focus changed parent_id {:?}", parent_id);

    if let Some(ctrl) = &view.controller {
        env.active_view = Some(ctrl.id);
        return;
    }

    env.active_view = Some(vid);

    loop {
        if let Some(pid) = parent_id {
            dbg_println!("focus changed : checking parent {:?}", pid);

            if let Some(pview) = check_view_by_id(editor, pid) {
                let mut pview = pview.write();
                pview.focus_to = Some(vid);
                parent_id = pview.parent_id;

                dbg_println!("set_active_view next parent_id {:?}", parent_id);
            } else {
                dbg_println!("focus changed : no parent found stop");

                break;
            }
        } else {
            break;
        }
    }

    dbg_println!("set focus on view {:?}", env.active_view);
}

// always compute ?
fn clip_locked_coordinates_xy(
    _editor: &mut Editor<'static>,
    env: &mut EditorEnv<'static>,
    _root_view_id: view::Id,
    _view_id: view::Id,
    x: &mut i32,
    y: &mut i32,
) -> view::Id {
    let id = env.focus_locked_on_view_id.unwrap();

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
    root_view_id: view::Id,
    vid: view::Id,
    mut x: &mut i32,
    mut y: &mut i32,
) -> view::Id {
    let mut id = root_view_id;

    if env.focus_locked_on_view_id.is_some() {
        return clip_locked_coordinates_xy(
            &mut editor,
            &mut env,
            root_view_id,
            vid,
            &mut x,
            &mut y,
        );
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
            if let Some(v) = check_view_by_id(editor, id) {
                let v = v.read();

                if v.children.is_empty() {
                    dbg_println!("CLIPPING        no more children");
                    dbg_println!("CLIPPING ----------------------------------- END");
                    return id;
                }

                for child in v.children.iter() {
                    let child_v = get_view_by_id(editor, child.id);
                    let child_v = child_v.write();

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
                    let child_v = get_view_by_id(editor, child.id);
                    let child_v = child_v.read();
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

fn clip_coordinates_and_get_view_id(
    mut editor: &mut Editor<'static>,
    mut env: &mut EditorEnv<'static>,
    ev: &InputEvent,
    root_view_id: view::Id,
    vid: view::Id,
) -> (view::Id, InputEvent) {
    let mut ev = ev.clone();

    let vid = match &mut ev {
        InputEvent::ButtonPress(event::ButtonEvent { x, y, .. }) => {
            let vid = clip_coordinates_xy(&mut editor, &mut env, root_view_id, vid, x, y);
            env.last_selected_view_id = vid;
            vid
        }
        InputEvent::ButtonRelease(event::ButtonEvent { x, y, .. }) => {
            clip_coordinates_xy(&mut editor, &mut env, root_view_id, vid, x, y)
        }
        InputEvent::PointerMotion(event::PointerEvent { x, y, .. }) => {
            clip_coordinates_xy(&mut editor, &mut env, root_view_id, vid, x, y)
        }
        InputEvent::WheelUp { x, y, .. } => {
            clip_coordinates_xy(&mut editor, &mut env, root_view_id, vid, x, y)
        }
        InputEvent::WheelDown { x, y, .. } => {
            clip_coordinates_xy(&mut editor, &mut env, root_view_id, vid, x, y)
        }
        InputEvent::KeyPress { .. } => env.active_view.unwrap_or(vid),
        _ => vid,
    };

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
) {
    use StagePosition::In;
    use StagePosition::Post;
    use StagePosition::Pre;

    run_stage(Pre, stage, &mut editor, &mut env, view_id);
    run_stage(In, stage, &mut editor, &mut env, view_id);
    run_stage(Post, stage, &mut editor, &mut env, view_id);
}

fn run_stage(
    pos: StagePosition,
    stage: Stage,
    mut editor: &mut Editor<'static>,
    mut env: &mut EditorEnv<'static>,
    view_id: view::Id,
) {
    let view = check_view_by_id(editor, view_id);
    if view.is_none() {
        return;
    }

    dbg_println!("run stage for view  {:?}", view_id);

    let start = Instant::now();

    let view = view.unwrap().clone();

    match stage {
        Stage::Input => {
            // TODO(ceg): run_stage_input
            match pos {
                StagePosition::Pre => {
                    env.process_input_start = Instant::now();
                    view::run_stage(&mut editor, &mut env, &view, pos, stage);
                }

                StagePosition::In => {
                    //  move block to ? view::run_stage(&mut editor, &mut env, &view, pos, stage);

                    // - need_rendering ? -
                    // move ev to env.current_event
                    env.refresh_ui = process_single_input_event(&mut editor, &mut env, view_id);
                }
                StagePosition::Post => {
                    view::run_stage(&mut editor, &mut env, &view, pos, stage);

                    env.process_input_end = Instant::now();

                    // root view changed ?
                    if env.root_view_id != env.prev_view_id {
                        env.refresh_ui = true;

                        dbg_println!(
                            "view change {:?} ->  {:?}",
                            env.prev_view_id,
                            env.root_view_id
                        );

                        check_view_dimension(editor, env);
                        {
                            // NB: resize previous view's screen to lower memory usage
                            if let Some(view) = check_view_by_id(editor, env.prev_view_id) {
                                view.write().screen.write().resize(1, 1);
                            }

                            // prepare next view input
                            let view = editor
                                .view_map
                                .read()
                                .get(&env.root_view_id)
                                .unwrap()
                                .clone();

                            view::run_stage(
                                &mut editor,
                                &mut env,
                                &view,
                                StagePosition::Pre,
                                Stage::Input,
                            );

                            let id = env.root_view_id;
                            run_stages(Stage::Compositing, &mut editor, &mut env, id);

                            // view changed -> call compositing stage
                            // TODO(ceg): unique root view
                            env.active_view = None;
                            env.pointer_over_view_id = view::Id(0);
                            env.last_selected_view_id = view::Id(0);
                            env.focus_locked_on_view_id = None;

                            env.prev_view_id = env.root_view_id;
                        }
                    }
                }
            }
        }

        //
        Stage::Compositing => {
            view::run_stage(&mut editor, &mut env, &view, pos, stage);
            if let (StagePosition::In, Stage::Compositing) = (pos, stage) {
                view::compute_root_view_layout(editor, env, &view);
            }
        }

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
}

/*
  TODO(ceg): filter event type

    find view under pointer -> pointed_view_id
    cmp with active_view:


*/
fn setup_focus_and_event(
    mut editor: &mut Editor<'static>,
    mut env: &mut EditorEnv<'static>,
    ev: &InputEvent,
    compose: &mut bool,
) -> view::Id {
    let root_view_id = env.root_view_id;

    let vid = get_focused_view_id(&mut editor, &mut env, root_view_id);

    dbg_println!(">> setup_focus_and_event FOCUS on {:?}", vid);

    dbg_println!(">> setup_focus_and_event ACTIVE VIEW {:?}", env.active_view);

    if root_view_id != vid {
        // only set, not cleared
        *compose = true;
    };

    let (vid, ev) = clip_coordinates_and_get_view_id(&mut editor, &mut env, ev, root_view_id, vid);

    set_focus_on_view_id(&mut editor, &mut env, vid);

    env.current_input_event = ev;
    env.prev_view_id = root_view_id;
    vid
}

fn check_pointer_over_change(
    mut editor: &mut Editor<'static>,
    mut env: &mut EditorEnv<'static>,
    prev_view_id: view::Id,
    new_view_id: view::Id,
) {
    if prev_view_id == new_view_id {
        return;
    }

    dbg_println!(
        "pointer_over changed {:?} -> {:?}",
        prev_view_id,
        new_view_id
    );

    {
        if let Some(prev_v) = check_view_by_id(editor, prev_view_id) {
            let prev_v = prev_v.clone();

            let mut prev_v = prev_v.write();
            let subscribers = { prev_v.subscribers.clone() };

            for cb in subscribers.iter() {
                let mode = cb.0.as_ref();

                if cb.1.id != prev_view_id || cb.2.id != prev_view_id {
                    continue;
                }

                mode.borrow().on_view_event(
                    &mut editor,
                    &mut env,
                    ViewEventSource { id: prev_view_id },
                    ViewEventDestination { id: prev_view_id },
                    &ViewEvent::Leave,
                    &mut prev_v,
                    None,
                );
            }
        }

        if let Some(new_v) = check_view_by_id(editor, new_view_id) {
            let new_v = new_v.clone();
            let mut new_v = new_v.write();
            let subscribers = { new_v.subscribers.clone() };

            for cb in subscribers.iter() {
                let mode = cb.0.as_ref();

                if cb.1.id != new_view_id || cb.2.id != new_view_id {
                    continue;
                }

                mode.borrow().on_view_event(
                    &mut editor,
                    &mut env,
                    ViewEventSource { id: new_view_id },
                    ViewEventDestination { id: new_view_id },
                    &ViewEvent::Enter,
                    &mut new_v,
                    None,
                );
            }
        }
    }

    env.pointer_over_view_id = new_view_id;
}

fn check_selection_change(
    mut editor: &mut Editor<'static>,
    mut env: &mut EditorEnv<'static>,
    prev_view_id: view::Id,
    new_view_id: view::Id,
) {
    if prev_view_id == new_view_id {
        return;
    }

    {
        if let Some(new_v) = check_view_by_id(editor, new_view_id) {
            let new_v = new_v.clone();
            let mut new_v = new_v.write();

            // TODO(ceg): use event mask
            if new_v.ignore_focus {
                dbg_println!("clicked changed ignored");
                env.last_selected_view_id = prev_view_id;
                return;
            }

            // notify prev
            if let Some(prev_v) = check_view_by_id(editor, prev_view_id) {
                let prev_v = prev_v.clone();
                let mut prev_v = prev_v.write();

                let subscribers = prev_v.subscribers.clone();
                for cb in subscribers {
                    let mode = cb.0.as_ref();

                    if cb.1.id != prev_view_id || cb.2.id != prev_view_id {
                        continue;
                    }

                    mode.borrow().on_view_event(
                        &mut editor,
                        &mut env,
                        ViewEventSource { id: prev_view_id },
                        ViewEventDestination { id: prev_view_id },
                        &ViewEvent::ViewDeselected,
                        &mut prev_v,
                        None,
                    );
                }
            }

            dbg_println!("clicked changed {:?} -> {:?}", prev_view_id, new_view_id);

            // notify new
            let subscribers = new_v.subscribers.clone();

            for cb in subscribers.iter() {
                let mode = cb.0.as_ref();

                if cb.1.id != new_view_id || cb.2.id != new_view_id {
                    continue;
                }

                mode.borrow().on_view_event(
                    &mut editor,
                    &mut env,
                    ViewEventSource { id: new_view_id },
                    ViewEventDestination { id: new_view_id },
                    &ViewEvent::ViewSelected,
                    &mut new_v,
                    None,
                );
            }
        }
    }

    env.last_selected_view_id = new_view_id;
}

// Loop over all input events
fn run_all_stages(
    mut editor: &mut Editor<'static>,
    mut env: &mut EditorEnv<'static>,
    events: &Vec<InputEvent>,
) {
    // Pre
    // self.flat_events
    env.pending_events = crate::core::event::pending_input_event_count();
    let flat_events = events;
    //let flat_events = flatten_input_events(&events);
    if flat_events.is_empty() {
        return;
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

        // select view that will receive the event
        dbg_println!(
            " before setup_focus_and_event -> active_view  env {:?}",
            env.active_view
        );

        let prev_root_index = env.root_view_index;

        let prev_active_view = env.active_view;

        let pointer_over_view_id = env.pointer_over_view_id;
        let last_selected_view_id = env.last_selected_view_id;

        let target_id = setup_focus_and_event(&mut editor, &mut env, &ev, &mut recompose);
        dbg_println!("setup_focus_and_event ->  Id {:?}", target_id);

        check_pointer_over_change(&mut editor, &mut env, pointer_over_view_id, target_id);

        let new_clicked = env.last_selected_view_id;
        check_selection_change(&mut editor, &mut env, last_selected_view_id, new_clicked);

        dbg_println!("pointer_over_view_id {:?}", pointer_over_view_id);
        dbg_println!("new_clicked {:?}", new_clicked);

        run_stages(Stage::Input, &mut editor, &mut env, target_id);

        // render intermediate screen
        assert_ne!(target_id, env.root_view_id);
        run_stages(Stage::Compositing, &mut editor, &mut env, target_id);

        // update active view (no root change)
        if prev_root_index == env.root_view_index {
            dbg_println!("update active view ?");

            if let Some(prev_active_view) = prev_active_view {
                if prev_active_view != target_id {
                    let vid = match &ev {
                        InputEvent::PointerMotion(event::PointerEvent { .. }) => prev_active_view,
                        InputEvent::WheelUp { .. } => prev_active_view,
                        InputEvent::WheelDown { .. } => prev_active_view,
                        InputEvent::KeyPress { .. } => prev_active_view,
                        _ => env.active_view.unwrap_or(target_id),
                    };

                    dbg_println!("set focus on {:?}", vid);
                    set_focus_on_view_id(&mut editor, &mut env, vid);
                }
            } else {
                set_focus_on_view_id(&mut editor, &mut env, target_id);
            }
        }
    }

    // must render root view once
    let id = env.root_view_id;
    run_stages(Stage::Compositing, &mut editor, &mut env, id);

    // send screen to ui
    run_stages(Stage::UpdateUi, &mut editor, &mut env, id);
}

fn process_input_events(
    mut editor: &mut Editor<'static>,
    mut env: &mut EditorEnv<'static>,
    _ui_tx: &Sender<Message>,
    events: &Vec<InputEvent>,
) {
    env.time_spent = [[0, 0, 0], [0, 0, 0], [0, 0, 0]];

    let start = Instant::now();
    run_all_stages(&mut editor, &mut env, &events);
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

fn process_buffer_event(
    editor: &mut Editor<'static>,
    env: &mut EditorEnv<'static>,
    event: &BufferEvent,
) {
    dbg_println!("{:?}", event);

    match event {
        //
        BufferEvent::BufferFullyIndexed { buffer_id } => {
            let mut view_ids = vec![];

            let map = get_view_map(editor);
            let map = map.read();
            for (view_id, v) in map.iter() {
                let view = v.write();
                if let Some(buffer) = view.buffer() {
                    let buffer = buffer.read();
                    if buffer.id == *buffer_id {
                        view_ids.push(view_id);
                    }
                }
            }

            for view_id in view_ids {
                let view = get_view_by_id(editor, *view_id);
                let mut view = view.write();
                if let Some(buffer) = view.buffer() {
                    let buffer = buffer.read();
                    if buffer.id == *buffer_id {
                        let modes = view.modes.clone();
                        for mode_name in modes {
                            let map = editor.modes.borrow_mut().clone();
                            if let Some(mode) = map.get(&mode_name) {
                                let mode = mode.borrow_mut();
                                mode.on_buffer_event(editor, env, event, &mut view);
                            }
                        }
                    }
                }
            }
        }

        _ => {
            // unexpected
            panic!("{:?}", event);
        }
    }
}

pub fn main_loop(
    mut editor: &mut Editor<'static>,
    mut env: &mut EditorEnv<'static>,
    core_rx: &Receiver<Message<'static>>,
    ui_tx: &Sender<Message<'static>>,
) {
    let mut seq: usize = 0;

    fn get_next_seq(seq: &mut usize) -> usize {
        *seq += 1;

        *seq
    }

    while !env.quit {
        if let Ok(msg) = core_rx.recv() {
            match msg.event {
                Event::UpdateView { width, height } => {
                    env.width = width;
                    env.height = height;

                    dbg_println!(
                        "UpdateView env.width {} env.height {}",
                        env.width,
                        env.height
                    );

                    env.pending_events = crate::core::event::pending_input_event_dec(1);
                    update_view_and_send_draw_event(&mut editor, &mut env);
                }

                Event::RefreshView => {
                    update_view_and_send_draw_event(&mut editor, &mut env);
                }

                Event::Input { events } => {
                    if !editor.view_map.read().is_empty() {
                        process_input_events(&mut editor, &mut env, &ui_tx, &events);
                    }
                }

                Event::Buffer { event } => {
                    process_buffer_event(&mut editor, &mut env, &event);
                }

                _ => {}
            }
        }
    }

    // stop indexer(s)
    {
        for (_id, d) in editor.buffer_map.as_ref().read().iter() {
            d.write().abort_indexing = true;
        }
    }

    // send ApplicationQuit to worker thread
    let msg = Message::new(0, Event::ApplicationQuit);
    editor.worker_tx.send(msg).unwrap_or(());

    // send ApplicationQuit to ui thread
    let msg = Message::new(get_next_seq(&mut seq), Event::ApplicationQuit);
    ui_tx.send(msg).unwrap_or(());

    // send ApplicationQuit to indexer thread
    let msg = Message::new(get_next_seq(&mut seq), Event::ApplicationQuit);
    editor.indexer_tx.send(msg).unwrap_or(());
}
