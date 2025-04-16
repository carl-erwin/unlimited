use core::panic;
//
use parking_lot::RwLock;
use std::any::Any;
use std::cell::RefCell;
use std::rc::Rc;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Arc;

use std::collections::HashSet;

use bitflags::bitflags;

use crate::core::buffer;
use crate::core::buffer::Buffer;

use crate::core::editor::Editor;
use crate::core::editor::EditorEnv;
use crate::core::editor::Stage;
use crate::core::editor::StageFunction;
use crate::core::editor::StagePosition;

use crate::core::editor::add_view_tag;
use crate::core::editor::get_view_by_id;
use crate::core::editor::get_view_ids_by_tags;

use crate::core::editor::register_editor_event_watcher;

use crate::core::screen::Screen;

pub mod layout;

pub use self::layout::*;

use std::collections::HashMap;

use crate::core::editor::InputStageActionMap;
use crate::core::event::InputEvent;
use crate::core::event::InputEventMap;
use crate::core::event::InputEventRule;

use crate::core::modes::Mode;

///////////////////////////////////////////////////////////////////////////////////////////////////
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Id(pub usize);

pub type Tags = HashSet<String>;

// TODO(ceg):
//
// reorg
// buffer
// buffer list
// buffer -> [list of view]
// view -> main mode + list of sub mode  (recursive) ?
// notify all view when buffer change
//
// any view(buffer)
// we should be able to view a buffer with different views

// TODO(ceg): "virtual" scene graph
// add recursive View definition:
// we want a split-able view, with move-able borders/origin point
// a view is:
// a "parent" screen + a sorted "by depth ('z')" list of "child" view
// the depth attribute will be used to route the user input events (x,y,z)
// we need the "focused" view
// we "siblings" concepts/query
//  *) add arbitrary child with constraints fixed (x,y/w,h), attached left/right / % of parent,
//  *) split vertically
//  *) split horizontally
//  *) detect coordinate conflicts
//  *) move "borders"
//  *) move "created" sub views
//  json description ? for save/restore
// main view+screen
// +------------------------------------------------------------------------------------------+
// | +---------------------------------------------------------------------------------------+|
// | |                                                                                       ||
// | +---------------------------------------------------------------------------------------+|
// | +--------------+                                                                      |[]|
// | |              |                                                                      |[]|
// | |              |                                                                      |  |
// | |              |                                                                      |  |
// | |              |                                                                      |  |
// | |              |                                                                      |  |
// | |              |                                                                      |  |
// | |              |                                                                      |  |
// | |              |                                                                      |  |
// | |              |                                                                      |  |
// | |              |                                                                      |  |
// | |              |                                                                      |  |
// | |              |                                                                      |  |
// | |              |                                                                      |  |
// | |              |                                                                      |  |
// | |              |                                                                      |  |
// | |              |                                                                      |  |
// | |              |                                                                      |  |
// | |              |                                                                      |  |
// | |              |                                                                      |  |
// | |              |                                                                      |  |
// | |              |                                                                      |  |
// | |              |                                                                      |  |
// | |              |                                                                      |  |
// | |              |                                                                      |  |
// | +--------------+                                                                      |  |
// +------------------------------------------------------------------------------------------+

// MOVE TO Layout code

pub type Position = (usize, usize);
pub type Dimension = (usize, usize);

// store this in parent and reuse in resize
#[derive(Debug, PartialEq, Clone, Copy)]
pub enum LayoutDirection {
    NotSet,
    Vertical,
    Horizontal,
}
// store this in parent and reuse in resize
#[derive(Debug, Clone, Copy)]
pub enum LayoutSize {
    // Child at View::{.x, .y}
    Floating,

    // We want a fixed size of sz cells vertically/horizontally in the parent
    // used = size
    // remain = remain - sz
    Fixed { size: usize },

    // We want a fixed percentage of sz cells vertically/horizontally
    // used = (parent.sz/100) * sz
    // remain = parent.sz - used
    Percent { p: f32 },

    // We want a fixed percentage of sz cells vertically/horizontally
    // used = (remain/100 * sz)
    // (remain <- remain - (remain/100 * sz))
    RemainPercent { p: f32 },

    // We want a fixed percentage of sz cells vertically/horizontally
    // used = (remain - minus)
    // remain = remain - used
    RemainMinus { minus: usize },
}

// MOVE TO Layout code
pub fn compute_layout_sizes(start: usize, ops: &Vec<LayoutSize>) -> Vec<usize> {
    let mut sizes = vec![];

    dbg_println!("compute_layout_sizes --------------------");

    dbg_println!("compute_layout_sizes start = {}", start);

    if start == 0 {
        return sizes;
    }

    let mut remain = start;

    for op in ops {
        if remain == 0 {
            sizes.push(0);
            continue;
        }

        dbg_println!("compute_layout_sizes op = {:?}", op);

        match op {
            LayoutSize::Floating => {}

            LayoutSize::Fixed { size } => {
                remain = remain.saturating_sub(*size);

                dbg_println!("compute_layout_sizes: give = {size}");

                sizes.push(*size);
            }

            LayoutSize::Percent { p } => {
                let used = (*p * start as f32) / 100.0;
                let used = used as usize;
                remain = remain.saturating_sub(used);

                dbg_println!("compute_layout_sizes: give = {used}");

                sizes.push(used);
            }

            LayoutSize::RemainPercent { p } => {
                let used = (*p * remain as f32) / 100.0;
                let used = used as usize;
                remain = remain.saturating_sub(used);

                dbg_println!("compute_layout_sizes: give = {used}");

                sizes.push(used);
            }

            // We want a fixed percentage of sz cells vertically/horizontally
            // used = minus
            // (remain <- remain - minus))
            LayoutSize::RemainMinus { minus } => {
                let used = remain.saturating_sub(*minus);

                remain = remain.saturating_sub(used);

                dbg_println!("compute_layout_sizes: give = {used}");

                sizes.push(used);
            }
        }

        dbg_println!("compute_layout_sizes: remain = {remain}");
    }

    sizes
}

// trait ?
// collection of functions, at each pass
// layout
// see process_input_event and augment the signature

// pre()
// process
// post()

// TODO(ceg): add ?
//        buffer,
//        view

// TODO(ceg):
// add struct to map view["mode(n)"] -> data
// add struct to map doc["mode(n)"]  -> data: ex: line index

static VIEW_ID: AtomicUsize = AtomicUsize::new(1);

#[derive(Default)]
pub struct InputContext {
    pub action_map: InputStageActionMap<'static>, // ref to current focused widget ?
    pub input_map: Rc<RefCell<Vec<(&'static str, InputEventMap)>>>, // mode name
    pub stack_pos: Option<usize>,
    pub current_node: Option<Rc<InputEventRule>>,
    pub trigger: Vec<InputEvent>,
}

impl InputContext {
    pub fn new() -> Self {
        InputContext {
            action_map: HashMap::new(),
            input_map: Rc::new(RefCell::new(Vec::new())),
            stack_pos: None,
            current_node: None,
            trigger: vec![],
        }
    }
}

#[derive(Debug, Clone, Copy)]
pub struct ViewEventSource {
    pub id: Id,
}

#[derive(Debug, Clone, Copy)]
pub struct ViewEventDestination {
    pub id: Id,
}

#[derive(Debug, Clone, Copy)]
pub enum ViewEvent {
    Subscribe,
    PreLayoutSizing,
    PreComposition, // same as PostLayoutSizing,
    PostComposition,
    OffsetsChange { start_offset: u64, end_offset: u64 }, // ContentChanged
    Enter,
    Leave,
    ViewSelected,
    ViewDeselected,
}

// marks | selections Refresh_event(editor, env, ViewEventSource { view_id }, ViewEventSource { view_id }, view_event)
// cb signature  fn cb_on_buffer_event(editor, env, BufferEventSource { buffer_id }, buffer_event)

// register siblings view
// text <--> scrollbar
// cb signature  fn cb_on_view_event(editor, env, ViewEventSource { view_id }, ViewEventSource { view_id }, view_event)
// cb signature  fn cb_on_buffer_event(editor, env, BufferEventSource { buffer_id }, buffer_event)

//  enum ViewEvent {
//    ViewOffsetsChange { start_offset, end_offset }
//    marks | selections
//    destroy ?
// }
//
// the notification should be done in post input, before composition
//

#[derive(Debug, Clone, Copy)]
pub struct ChildView {
    pub id: Id,
    pub layout_op: LayoutSize,
}

pub struct ControllerView {
    pub id: Id,
    pub mode_name: &'static str,
}

pub type SubscriberInfo = (
    Rc<RefCell<Box<dyn Mode>>>,
    ViewEventSource,
    ViewEventDestination,
);

// The EventMask allows the dispatch of corresponding event(s)
// The root view should be configured with EventMask::All
bitflags! {
    #[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
    pub struct EventMask: u32 {
        const None          = 0x0;
        const All           = 0xffffffff;

        const ButtonPress   = 0b00000001;
        const ButtonRelease = 0b00000010;
        const KeyPress      = 0b00000100;
        const KeyRelease    = 0b00001000;
        const EnterView     = 0b00010000;
        const LeaveView     = 0b00100000;
        const Motion        = 0b01000000;
    }
}

/// The **View** is a way to present a given Buffer.<br/>
// TODO(ceg): find a way to have marks as plugin.<br/>
// in future version marks will be stored in buffer meta data.<br/>
// TODO editor.env.current.view_id = view.id
// can zoom ?
//
// TODO(ceg): add view.subscribe_to_view_id
// TODO(ceg): add view.publish_to_view_id
//
// follow mode fills
// TODO(ceg): add view.prev_content_view_id
// TODO(ceg): add view.next_content_view_id
pub struct View<'a> {
    pub id: Id,
    pub json_attr: Option<String>,

    pub destroyable: bool,
    pub is_group: bool,
    pub is_leader: bool,

    pub is_splittable: bool, // This flags marks a view that can be cloned when doing split views

    pub ignore_focus: bool, // never set focus on this view

    pub event_mask: EventMask,

    pub parent_id: Option<Id>,
    pub transfer_focus_to: Option<Id>, // child id TODO(ceg): redirect input ?
    pub status_view_id: Option<Id>, // TODO(ceg): remove this ?  or per view see env.status_view_id

    pub controller: Option<ControllerView>, // REMOVE this

    pub controlled_view: Option<Id>,

    /*
      any view that can display some text,
      TODO(ceg): use special buffer for this
      split text-mode into
      text-display-mode: scrolling ops etc
      text-edit-mode   : marks , selection etc...
      if edit is on display marks
      disable buffer log for this special buffer

      maybe allow to change compose filter of status_view_id ?
      for custom status display ?
    */
    pub buffer_id: buffer::Id,
    pub buffer: Option<Arc<RwLock<Buffer<'static>>>>, // if none and no children ... panic ?

    pub tags: Tags,

    pub modes: Vec<String>, // TODO: add Arc<dyn Modes>

    pub mode_ctx: HashMap<&'static str, Box<dyn Any>>,
    //
    pub screen: Arc<RwLock<Box<Screen>>>,

    //
    pub start_offset: u64, // where we want to start the rendering
    pub end_offset: u64,   // where the rendering stopped

    // Input
    pub input_ctx: InputContext,

    // layout
    // position in root view
    pub global_x: Option<usize>,
    pub global_y: Option<usize>,
    // position in parent
    pub x: usize,
    pub y: usize,
    // dimension in parent
    pub width: usize,
    pub height: usize,

    /// layout ops index in parent_id.layout_ops[]
    pub layout_index: Option<usize>,

    pub layout_direction: LayoutDirection,

    pub layout_size: LayoutSize,

    pub children: Vec<ChildView>,

    pub floating_children: Vec<ChildView>,

    //
    pub stage_actions: Vec<(String, StageFunction)>,

    //
    pub compose_content_filters: Rc<RefCell<Vec<Box<dyn ContentFilter<'a>>>>>,
    pub compose_screen_overlay_filters: Rc<RefCell<Vec<Box<dyn ScreenOverlayFilter<'a>>>>>,
    pub compose_priority: usize,

    // can be moved to layout engine
    pub filter_in: Rc<RefCell<Vec<FilterIo>>>,
    pub filter_out: Rc<RefCell<Vec<FilterIo>>>,

    pub subscribers: Vec<SubscriberInfo>,
}

/// Use this function if the mode needs to watch a given view
///
pub fn register_view_subscriber(
    editor: &mut Editor<'static>,
    env: &mut EditorEnv<'static>,
    mode: Rc<RefCell<Box<dyn Mode>>>, // TODO: use type
    src: ViewEventSource,
    dst: ViewEventDestination,
) -> Option<()> {
    dbg_println!(
        "register_view_subscriber: mode[{}] src {:?} dst {:?}",
        mode.borrow().name(),
        src,
        dst
    );

    let src_view = get_view_by_id(editor, src.id);
    let ctx = (mode.clone(), src, dst);

    let mut src_view = src_view.write();

    src_view.subscribers.push(ctx);

    mode.borrow().on_view_event(
        editor,
        env,
        src,
        dst,
        &ViewEvent::Subscribe,
        &mut src_view,
        None,
    );

    Some(())
}

// TODO(ceg): we must find a better way to store subscribers and avoid recursive locking
// register_view_subscriber call rc(src_view).write().subscribers.push(...)
// NB: self registering by hand to avoid dead lock
// see register_view_subscriber
// move subscriptions to editor , to avoid interior mut
// editor.subscription[view.id] ?
pub fn view_self_subscribe(
    _editor: &mut Editor<'static>,
    _env: &mut EditorEnv<'static>,
    mode: Rc<RefCell<Box<dyn Mode>>>, // TODO: use type
    view: &mut View<'static>,
) -> Option<()> {
    let src = ViewEventSource { id: view.id };
    let dst = ViewEventDestination { id: view.id };
    let ctx = (mode, src, dst);
    view.subscribers.push(ctx);

    Some(())
}

impl<'a> View<'a> {
    pub fn buffer(&self) -> Option<Arc<RwLock<Buffer<'static>>>> {
        self.buffer.clone()
    }

    // Setup view modes. (respect vector order)
    fn setup_modes(
        editor: &mut Editor<'static>,
        env: &mut EditorEnv<'static>,
        view: &mut View<'static>,
        modes: &[String],
    ) {
        // setup modes/input map/etc..
        view.modes.clear();

        for mode_name in modes.iter() {
            if mode_name.is_empty() {
                // TODO(ceg): log error
                continue;
            }

            view.modes.push(mode_name.to_owned());

            let mode = editor.get_mode(mode_name);
            if mode.is_none() {
                panic!("cannot find mode {}", mode_name);
            }
            let mode_rc = mode.unwrap();
            let mut m = mode_rc.borrow_mut();

            // TODO(ceg): add doc
            let action_map = m.build_action_map();

            view.register_action_map(action_map);

            // create per view mode context
            // allocate per view ModeCtx shared between the stages
            {
                let ctx = m.alloc_ctx(editor);
                view.set_mode_ctx(m.name(), ctx);
                dbg_println!("mode[{}] configure  {:?}", m.name(), view.id);
                m.configure_view(editor, env, view);
                if m.watch_editor_event() {
                    register_editor_event_watcher(editor, m.name(), mode_rc.clone(), view.id);
                }
                view_self_subscribe(editor, env, mode_rc.clone(), view);
            }
        }
    }

    // no conflict checks
    pub fn register_action_map(&mut self, action_map: InputStageActionMap<'static>) {
        for (name, cb) in action_map {
            self.input_ctx.action_map.insert(name.clone(), cb);
        }
    }

    /// Create a new View at a given offset of the Buffer.<br/>
    pub fn new(
        editor: &mut Editor<'static>,
        env: &mut EditorEnv<'static>,
        parent_id: Option<Id>,
        x_y: Position,
        w_h: Dimension,
        buffer: Option<Arc<RwLock<Buffer<'static>>>>,
        tags: &Vec<String>,
        modes: &Vec<String>, // TODO(ceg): add core mode for save/quit/quit/abort/split{V,H}
        start_offset: u64,
        layout_direction: LayoutDirection,
        layout_size: LayoutSize,
    ) -> View<'static> {
        let screen = Arc::new(RwLock::new(Box::new(Screen::new(w_h.0, w_h.1))));

        let id = VIEW_ID.fetch_add(1, Ordering::SeqCst);
        let mode_ctx = HashMap::new();
        let input_ctx = InputContext::new();

        let buffer_id = match buffer {
            Some(ref arc) => arc.as_ref().read().id,
            _ => buffer::Id(0),
        };

        dbg_println!("CREATE new VIEW {id}, modes {modes:?}");

        if let Some(parent_id) = parent_id {
            if parent_id == Id(0) {
                panic!();
            }
        }

        for tag in tags {
            dbg_println!("tag {} -> view {:?}", tag, Id(id));
            add_view_tag(editor, &tag, Id(id));
        }

        let tags = {
            let mut hset = HashSet::new();
            for t in tags {
                hset.insert(t.clone());
            }
            hset
        };

        let mut v = View {
            parent_id,
            json_attr: None,
            destroyable: false,
            is_leader: false,
            is_group: false,
            is_splittable: false,
            ignore_focus: true,

            event_mask: EventMask::None,

            transfer_focus_to: None,
            status_view_id: None,
            controller: None,
            controlled_view: None,
            id: Id(id),
            buffer_id,
            buffer,
            screen,
            //
            input_ctx,
            //
            start_offset,
            end_offset: start_offset, // will be recomputed later
            tags,
            modes: modes.clone(), // use this to clone the view
            mode_ctx,
            //
            global_x: None,
            global_y: None,
            x: x_y.0,
            y: x_y.1,
            width: w_h.0,
            height: w_h.1,
            //
            layout_index: None,
            layout_direction,
            layout_size,

            children: vec![],
            floating_children: vec![],

            //
            stage_actions: vec![],

            compose_content_filters: Rc::new(RefCell::new(vec![])),
            compose_screen_overlay_filters: Rc::new(RefCell::new(vec![])),

            compose_priority: 0, //  greater first

            // here ?
            filter_in: Rc::new(RefCell::new(vec![])),
            filter_out: Rc::new(RefCell::new(vec![])),

            //
            subscribers: vec![], // list of other views to notify when this view changes
        };

        View::setup_modes(editor, env, &mut v, &modes);

        v
    }

    pub fn dimension(&self) -> (usize, usize) {
        self.screen.read().dimension()
    }

    pub fn set_mode_ctx(&mut self, name: &'static str, ctx: Box<dyn Any>) -> bool {
        let res = self.mode_ctx.insert(name, ctx);
        dbg_println!("set_mode_ctx name {}", name);
        assert!(res.is_none());
        true
    }

    pub fn check_mode_ctx<T: 'static>(&self, name: &'static str) -> bool {
        let ret = self.mode_ctx.get(&name);
        ret.is_some()
    }

    pub fn mode_ctx_mut<T: 'static>(&mut self, name: &'static str) -> &mut T {
        match self.mode_ctx.get_mut(&name) {
            Some(box_any) => {
                let any = box_any.as_mut();
                match any.downcast_mut::<T>() {
                    Some(m) => m,
                    None => {
                        panic!("internal error: wrong type registered : mode name '{}' | view tags {:?}", name, self.tags)
                    }
                }
            }

            None => panic!("not configured properly: mode name {}", name),
        }
    }

    pub fn mode_ctx<T: 'static>(&self, name: &'static str) -> &T {
        match self.mode_ctx.get(&name) {
            Some(box_any) => {
                let any = box_any.as_ref();
                match any.downcast_ref::<T>() {
                    Some(m) => m,
                    None => panic!("internal error: wrong type registered"),
                }
            }

            None => panic!("mode {}, not configured properly", name),
        }
    }

    pub fn check_invariants(&self) {
        self.screen.read().check_invariants();
        let _max_offset = self.buffer().unwrap().read().size();
        // TODO(ceg): mode check invariants
    }
} // impl View

//
pub fn get_command_view_id(editor: &mut Editor<'static>, _env: &EditorEnv<'static>) -> Option<Id> {
    let v = get_view_ids_by_tags(&editor, "command-line")?;
    if v.len() == 1 {
        return Some(v[0]);
    }
    return None;
}

//
pub fn get_view_by_tag(
    editor: &mut Editor<'static>,
    _env: &EditorEnv<'static>,
    tag: &str,
) -> Option<Id> {
    let v = get_view_ids_by_tags(&editor, tag)?;
    if v.len() == 1 {
        return Some(v[0]);
    }
    return None;
}

pub fn get_status_line_view_id(
    editor: &mut Editor<'static>,
    _env: &EditorEnv<'static>,
) -> Option<Id> {
    let v = get_view_ids_by_tags(&editor, "status-line")?;
    if v.len() == 1 {
        return Some(v[0]);
    }
    return None;
}

/// TODO(ceg): rename
pub fn run_stage(
    editor: &mut Editor<'static>,
    env: &mut EditorEnv<'static>,
    view: &Rc<RwLock<View<'static>>>,
    pos: StagePosition,
    stage: Stage,
) {
    // TODO(ceg): Rc ?
    // exec order ?, path ?
    let actions = view.read().stage_actions.clone();

    // disable for composition ?
    dbg_println!("running {} {:?} {:?} actions", actions.len(), pos, stage);
    for a in actions {
        a.1(editor, env, view, pos, stage);
    }
}

pub fn compute_root_view_layout(
    editor: &mut Editor<'static>,
    env: &mut EditorEnv<'static>,
    view: &Rc<RwLock<View<'static>>>,
) -> Option<()> {
    let (dimension, start_offset, max_offset) = {
        let mut v = view.write();

        // root view always at (0, 0)
        v.global_x = Some(0);
        v.global_y = Some(0);

        let buffer = v.buffer()?;
        let max_offset = { buffer.read().size() as u64 };
        let dimension = v.screen.read().dimension();
        let start_offset = v.start_offset;
        (dimension, start_offset, max_offset)
    };

    // TODO(ceg): screen cache/allocator
    let mut screen = Screen::with_dimension(dimension);

    run_compositing_stage_direct(
        editor,
        env,
        view,
        start_offset,
        max_offset,
        &mut screen,
        LayoutPass::ScreenContentAndOverlay,
    );

    let mut v = view.write();
    if let Some(last_offset) = screen.last_offset {
        v.end_offset = last_offset;
    }
    v.screen = Arc::new(RwLock::new(Box::new(screen)));
    v.width = dimension.0;
    v.height = dimension.1;
    v.check_invariants();

    Some(())
}
