// Copyright (c) Carl-Erwin Griffith

use core::panic;
//
use std::any::Any;
use std::cell::RefCell;
use std::rc::Rc;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Arc;
use std::sync::RwLock;
use std::time::Instant;

//
use crate::core::event::input_map::build_input_event_map;
use crate::core::event::input_map::DEFAULT_INPUT_MAP;

use crate::core::document::Document;

use crate::core::editor::Editor;
use crate::core::editor::EditorEnv;
use crate::core::editor::Stage;
use crate::core::editor::StageFunction;
use crate::core::editor::StagePosition;

use crate::core::screen::Screen;

use crate::core::view::layout::{run_compositing_stage, run_compositing_stage_direct};

use std::collections::HashMap;

use super::layout;

use crate::core::editor::InputStageActionMap;
use crate::core::event::InputEvent;
use crate::core::event::InputEventMap;
use crate::core::event::InputEventRule;

///////////////////////////////////////////////////////////////////////////////////////////////////
pub type Id = usize;

// let ptr : InputStageFunction = cancel_input(editor: &mut Editor, env: &mut EditorEnv, trigger: &Vec<input_event>,  view: &Rc<RefCell<View>>)

// TODO: add modes
// a view can be configured to have a "main mode" "interpreter/presenter"
// like "text-mode", hex-mode
// the mode is responsible to manage the view
// by default the first view wil be in text mode
//
// reorg
// buffer
// doc list
// doc -> [list of view]
// view -> main mode + list of sub mode  (recursive) ?
// notify all view when doc change
//
// any view(doc)
// we should be able to view a document we different views

// TODO: "virtual" scene graph
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
// store this in parent and reuse in resize
#[derive(Debug, PartialEq, Clone)]
pub enum LayoutDirection {
    NotSet,
    Vertical,
    Horizontal,
}
// store this in parent and reuse in resize
#[derive(Debug, Clone)]
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

// MOVE TO Layout code
pub fn compute_layout_sizes(start: usize, ops: &Vec<LayoutOperation>) -> Vec<usize> {
    let mut sizes = vec![];

    dbg_println!("start = {}", start);

    if start == 0 {
        return sizes;
    }

    let mut remain = start;

    for op in ops {
        if remain == 0 {
            sizes.push(0);
            continue;
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

// trait ?
// collection of functions, at each pass
// layout
// see process_input_event and augment the signatrue

// pre()
// process
// post()

// TODO: add ?
//        doc,
//        view

// TODO:
// add struct to map view["mode(n)"] -> data
// add struct to map doc["mode(n)"]  -> data: ex: line index

static VIEW_ID: AtomicUsize = AtomicUsize::new(1);

pub struct InputContext {
    pub action_map: InputStageActionMap<'static>, // ref to current focused widget ?
    pub input_map: Rc<RefCell<Vec<InputEventMap>>>,
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

/// The **View** represents a way to represent a given Document.<br/>
// TODO: find a way to have marks as plugin.<br/>
// in future version marks will be stored in buffer meta data.<br/>
// TODO editor.env.current.view_id = view.id
// can zoom ?
pub struct View<'a> {
    pub id: Id,
    pub destroyable: bool,
    pub parent_id: Option<Id>,
    pub focus_to: Option<Id>, // child id
    //pub root_view_index: Option<usize>, // ?
    pub document: Option<Arc<RwLock<Document<'static>>>>, // if none and no children ... panic ?
    pub mode_ctx: HashMap<String, Box<dyn Any>>,
    //
    pub screen: Arc<RwLock<Box<Screen>>>,

    //
    pub start_offset: u64, // where we want to start the rendering
    pub end_offset: u64,   // where the rendering stopped

    // Input
    pub input_ctx: InputContext,

    // layout
    //
    pub x: usize,
    pub y: usize,
    /// layout ops index in parent_id.layout_ops
    pub layout_index: Option<usize>,

    pub layout_direction: LayoutDirection,
    pub layout_ops: Vec<LayoutOperation>,
    // TODO: keep them here or use view.id -> editor.view(view.id)
    pub children: Vec<Id>,

    //
    pub stage_actions: Vec<(String, StageFunction)>,

    //
    pub compose_filters: RefCell<Vec<Box<dyn layout::Filter<'a>>>>,
    pub compose_priority: usize,
}

impl<'a> View<'a> {
    pub fn document(&self) -> Option<Arc<RwLock<Document<'static>>>> {
        let doc = self.document.clone();
        let doc = doc?;
        Some(doc)
    }

    /// Create a new View at a gin offset in the Document.<br/>
    pub fn new(
        editor: &mut Editor<'static>,
        env: &mut EditorEnv<'static>,
        parent_id: Option<Id>,
        x: usize, // relative to parent, i32 allow negative moves?
        y: usize, // relative to parent, i32 allow negative moves?
        width: usize,
        height: usize,
        document: Option<Arc<RwLock<Document<'static>>>>,
        modes: &Vec<String>, // TODO: add core mode fr save/quit/quit/abort/split{V,H}
        start_offset: u64,
    ) -> View<'static> {
        let screen = Arc::new(RwLock::new(Box::new(Screen::new(width, height))));

        let id = VIEW_ID.fetch_add(1, Ordering::SeqCst);
        let mode_ctx = HashMap::new();
        let input_ctx = InputContext::new();

        let mut v = View {
            parent_id,
            destroyable: true,
            focus_to: None,
            id,
            document,
            screen,
            //
            input_ctx,
            //
            start_offset,
            end_offset: start_offset, // will be recomputed later
            mode_ctx,
            //
            x,
            y,
            layout_index: None,
            layout_direction: LayoutDirection::NotSet,
            layout_ops: vec![],
            children: vec![],
            //
            stage_actions: vec![],

            compose_filters: RefCell::new(vec![]),
            compose_priority: 0, //  greater first
        };

        // setup modes/input map/etc..
        for mode_name in modes.iter() {
            if mode_name.len() == 0 {
                continue;
            }

            let mode = {
                let mode = editor.get_mode(mode_name).clone();
                if mode.is_none() {
                    panic!("cannot find mode {}", mode_name);
                }
                Rc::clone(mode.unwrap())
            };

            // move to mode.configure()
            // merge all actions
            // move to mode

            let action_map = mode.build_action_map();
            for (name, fnptr) in action_map {
                v.input_ctx.action_map.insert(name.clone(), fnptr);
            }

            if mode_name != "core" {
                dbg_println!("DEFAULT_INPUT_MAP\n{}", DEFAULT_INPUT_MAP);
                // TODO: user define
                // let input_map = mode.build_input_map(); TODO
                {
                    let input_map = build_input_event_map(DEFAULT_INPUT_MAP).unwrap();
                    let mut input_map_stack = v.input_ctx.input_map.as_ref().borrow_mut();
                    input_map_stack.push(input_map);
                }
            }

            // TODO: merge modes input maps / (conflicts ?)
            // parser ctx build from current v.input_ctx.input_map
            // let map = v.input_ctx.input_map.clone();
            // let map = map.borrow_mut();
            // for i in input_map.borrow_mut().iter() {}

            // create view's mode context
            // allocate per view ModeCtx shared between the stages
            {
                let ctx = mode.alloc_ctx();
                v.set_mode_ctx(mode.name(), ctx);

                dbg_println!("mode[{}] configure VID {}", mode.name(), v.id);

                mode.configure_view(editor, env, &mut v);
            }
        }

        v
    }

    pub fn dimension(&self) -> (usize, usize) {
        let screen = self.screen.read().unwrap();
        (screen.width(), screen.height())
    }

    pub fn set_mode_ctx(&mut self, name: &str, ctx: Box<dyn Any>) -> bool {
        let res = self.mode_ctx.insert(name.to_owned(), ctx);
        assert!(res.is_none());
        true
    }

    pub fn check_mode_ctx<T: 'static>(&self, name: &str) -> bool {
        let ret = self.mode_ctx.get(&name.to_owned());
        ret.is_some()
    }

    pub fn mode_ctx_mut<T: 'static>(&mut self, name: &str) -> &mut T {
        match self.mode_ctx.get_mut(&name.to_owned()) {
            Some(box_any) => {
                let any = box_any.as_mut();
                match any.downcast_mut::<T>() {
                    Some(m) => {
                        return m;
                    }
                    None => panic!("internal error: wrong type registered"),
                }
            }

            None => panic!("not configured properly"),
        }
    }

    pub fn mode_ctx<T: 'static>(&self, name: &str) -> &T {
        match self.mode_ctx.get(&name.to_owned()) {
            Some(box_any) => {
                let any = box_any.as_ref();
                match any.downcast_ref::<T>() {
                    Some(m) => {
                        return m;
                    }
                    None => panic!("internal error: wrong type registered"),
                }
            }

            None => panic!("mode {}, not configured properly", name),
        }
    }

    pub fn get_view_at_mouse_position(&mut self, _x: i32, _y: i32) -> Option<&'a View<'a>> {
        None
    }

    pub fn check_invariants(&self) {
        self.screen.read().unwrap().check_invariants();

        let _max_offset = self.document().as_ref().unwrap().read().unwrap().size();

        // TODO: mode check invariants
    }
} // impl View

///
pub fn run_stage(
    editor: &mut Editor<'static>,
    env: &mut EditorEnv<'static>,
    view: &Rc<RefCell<View<'static>>>,
    pos: StagePosition,
    stage: Stage,
) {
    // TODO: Rc ?
    // exec order ?, path ?
    let actions = view.borrow().stage_actions.clone();

    // disable for composition ?
    for a in actions {
        a.1(editor, env, view, pos, stage);
    }

    match (pos, stage) {
        (StagePosition::In, Stage::Compositing) => {
            compute_view_layout(editor, env, &view); // can be merged with stage_actions ?
        }
        _ => {}
    }
}

pub fn compute_view_layout(
    editor: &mut Editor,
    env: &mut EditorEnv,
    view: &Rc<RefCell<View>>,
) -> Option<()> {
    let mut v = view.borrow_mut();

    let doc = v.document()?;
    let max_offset = { doc.as_ref().read().unwrap().size() as u64 };

    // TODO: reuse v.screen
    let dimension = v.screen.read().unwrap().dimension();
    dbg_println!("DIMENSION {:?}", dimension);
    let mut screen = Box::new(Screen::with_dimension(v.screen.read().unwrap().dimension()));
    run_compositing_stage_direct(editor, env, &v, v.start_offset, max_offset, &mut screen);
    if let Some(last_offset) = screen.last_offset {
        v.end_offset = last_offset;
    }
    v.screen = Arc::new(RwLock::new(screen)); // move v.screen to view double buffer  v.screen_get() v.screen_swap(new: move)
    v.check_invariants();
    Some(())
}

// TODO: text-mode
// scroll bar: bg color (35, 34, 89)
// scroll bar: cursor color (192, 192, 192)
pub fn update_view(
    _editor: &mut Editor,
    _env: &mut EditorEnv,
    _view: &Rc<RefCell<View>>,
) -> Option<()> {
    let _start = Instant::now();

    Some(())
}

#[test]
fn test_view() {}
