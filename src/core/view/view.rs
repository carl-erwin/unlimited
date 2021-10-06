use core::panic;
//
use parking_lot::RwLock;
use std::any::Any;
use std::cell::RefCell;
use std::rc::Rc;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Arc;

use crate::core::document::Document;

use crate::core::editor::Editor;
use crate::core::editor::EditorEnv;
use crate::core::editor::Stage;
use crate::core::editor::StageFunction;
use crate::core::editor::StagePosition;

use crate::core::screen::Screen;

use super::ContentFilter;
use super::FilterIo;
use super::LayoutPass;
use super::ScreenOverlayFilter;

use crate::core::view::run_compositing_stage_direct;

use std::collections::HashMap;

use crate::core::editor::InputStageActionMap;
use crate::core::event::InputEvent;
use crate::core::event::InputEventMap;
use crate::core::event::InputEventRule;

use crate::core::modes::Mode;

///////////////////////////////////////////////////////////////////////////////////////////////////
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Id(pub usize);

// TODO(ceg):
//
// reorg
// buffer
// doc list
// doc -> [list of view]
// view -> main mode + list of sub mode  (recursive) ?
// notify all view when doc change
//
// any view(doc)
// we should be able to view a document with different views

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
// store this in parent and reuse in resize
#[derive(Debug, PartialEq, Clone, Copy)]
pub enum LayoutDirection {
    NotSet,
    Vertical,
    Horizontal,
}
// store this in parent and reuse in resize
#[derive(Debug, Clone, Copy)]
pub enum LayoutOperation {
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
                let used = (*p * start as f32) / 100.0;
                let used = used as usize;
                remain = remain.saturating_sub(used);
                sizes.push(used);
            }

            LayoutOperation::RemainPercent { p } => {
                let used = (*p * remain as f32) / 100.0;
                let used = used as usize;
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

// TODO(ceg): add ?
//        doc,
//        view

// TODO(ceg):
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
    PreComposition,
    PostComposition,
    OffsetsChange { start_offset: u64, end_offset: u64 },
}

// marks | selectionsRefreshew_event(editor, env, ViewEventSource { view_id }, ViewEventSource { view_id }, view_event)
// cb signature  fn cb_on_document_event(editor, env, DocumentEventSource { doc_id }, doc_event)

// register siblings view
// text <--> scrollbar
// cb signature  fn cb_on_view_event(editor, env, ViewEventSource { view_id }, ViewEventSource { view_id }, view_event)
// cb signature  fn cb_on_document_event(editor, env, DocumentEventSource { doc_id }, doc_event)

//  enum ViewEvent {
//    ViewOffsetsChange { start_offset, end_offset }
//    marks | selections
//    destroy ?
// }
//
// the notification should be done in post input, before composition
//

/// The **View** is a way to present a given Document.<br/>
// TODO(ceg): find a way to have marks as plugin.<br/>
// in future version marks will be stored in buffer meta data.<br/>
// TODO editor.env.current.view_id = view.id
// can zoom ?
//
// TODO(ceg): add view.subscribe_to_vid
// TODO(ceg): add view.publish_to_vid
//
// follow mode fills
// TODO(ceg): add view.prev_content_vid
// TODO(ceg): add view.next_content_vid
pub struct View<'a> {
    pub id: Id,
    pub destroyable: bool,
    pub is_group_leader: bool, // This flags marks a view that can be cloned when doing split views

    pub parent_id: Option<Id>,
    pub focus_to: Option<Id>,       // child id TODO(ceg): redirect input ?
    pub status_view_id: Option<Id>, // TODO(ceg): remove this ?  or per view see env.status_view_id
    /*
      any view that can display some text,
      TODO(ceg): use special document for this
      split text-mode into
      text-display-mode: scrolling ops etc
      text-edit-mode   : marks , selection etc...
      if edit is on display marks
      disable buffer log for this special document

      maybe allow to change compose filter of status_view_id ?
      for custom status display ?
    */
    //pub root_view_index: Option<usize>, // ?
    pub document: Option<Arc<RwLock<Document<'static>>>>, // if none and no children ... panic ?

    pub modes: Vec<String>,

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
    pub width: usize,
    pub height: usize,

    /// layout ops index in parent_id.layout_ops[]
    pub layout_index: Option<usize>,

    pub layout_direction: LayoutDirection,
    pub layout_ops: Vec<LayoutOperation>,
    pub children: Vec<Id>,
    pub main_child: Option<usize>, // index in children

    //
    pub stage_actions: Vec<(String, StageFunction)>,

    //
    pub compose_content_filters: Rc<RefCell<Vec<Box<dyn ContentFilter<'a>>>>>,
    pub compose_screen_overlay_filters: Rc<RefCell<Vec<Box<dyn ScreenOverlayFilter<'a>>>>>,
    pub compose_priority: usize,

    // can be moved to layout engine
    pub filter_in: Rc<RefCell<Vec<FilterIo>>>,
    pub filter_out: Rc<RefCell<Vec<FilterIo>>>,

    pub subscribers: Vec<(
        Rc<RefCell<Box<dyn Mode>>>,
        ViewEventSource,
        ViewEventDestination,
    )>,
}

/// Use this function if the mode needs to watch a given view
///
pub fn register_view_subscriber(
    mut editor: &mut Editor<'static>,
    mut env: &mut EditorEnv<'static>,
    mode: Rc<RefCell<Box<dyn Mode>>>, // TODO: use type
    src: ViewEventSource,
    dst: ViewEventDestination,
) -> Option<()> {
    mode.borrow()
        .on_view_event(&mut editor, &mut env, src, dst, &ViewEvent::Subscribe, None);

    let ctx = (mode, src, dst);
    let src_view = editor.view_map.get(&src.id)?;

    src_view.write().subscribers.push(ctx);

    Some(())
}

impl<'a> View<'a> {
    pub fn document(&self) -> Option<Arc<RwLock<Document<'static>>>> {
        self.document.clone()
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
        modes: &Vec<String>, // TODO(ceg): add core mode fr save/quit/quit/abort/split{V,H}
        start_offset: u64,
    ) -> View<'static> {
        let screen = Arc::new(RwLock::new(Box::new(Screen::new(width, height))));

        let id = VIEW_ID.fetch_add(1, Ordering::SeqCst);
        let mode_ctx = HashMap::new();
        let input_ctx = InputContext::new();

        let mut v = View {
            parent_id,
            destroyable: true,
            is_group_leader: false,
            focus_to: None,
            status_view_id: None,
            id: Id(id),
            document,
            screen,
            //
            input_ctx,
            //
            start_offset,
            end_offset: start_offset, // will be recomputed later
            modes: modes.clone(),     // use this to clone the view
            mode_ctx,
            //
            x,
            y,
            width,
            height,
            //
            layout_index: None,
            layout_direction: LayoutDirection::NotSet,
            layout_ops: vec![],
            children: vec![],
            main_child: None,
            //
            stage_actions: vec![],

            compose_content_filters: Rc::new(RefCell::new(vec![])),
            compose_screen_overlay_filters: Rc::new(RefCell::new(vec![])),

            compose_priority: 0, //  greater first

            // here ?
            filter_in: Rc::new(RefCell::new(vec![])),
            filter_out: Rc::new(RefCell::new(vec![])),

            //
            subscribers: vec![], // list of other views to notify when the current view changes
        };

        // setup modes/input map/etc..
        for mode_name in modes.iter() {
            if mode_name.is_empty() {
                // TODO(ceg): log error
                continue;
            }

            let mode = editor.get_mode(mode_name);
            if mode.is_none() {
                panic!("cannot find mode {}", mode_name);
            }
            let mut mode = mode.as_ref().unwrap().borrow_mut();

            // TODO(ceg): add doc
            let action_map = mode.build_action_map();
            for (name, fnptr) in action_map {
                v.input_ctx.action_map.insert(name.clone(), fnptr);
            }

            // create view's mode context
            // allocate per view ModeCtx shared between the stages
            {
                let ctx = mode.alloc_ctx();
                v.set_mode_ctx(mode.name(), ctx);
                dbg_println!("mode[{}] configure  {:?}", mode.name(), v.id);
                mode.configure_view(editor, env, &mut v);
            }
        }

        v
    }

    pub fn dimension(&self) -> (usize, usize) {
        self.screen.read().dimension()
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
                    None => panic!("internal error: wrong type registered : mode name {}", name),
                }
            }

            None => panic!("not configured properly: mode name {}", name),
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

    pub fn check_invariants(&self) {
        self.screen.read().check_invariants();
        let _max_offset = self.document().unwrap().read().size();
        // TODO(ceg): mode check invariants
    }
} // impl View

//
pub fn get_status_view(
    editor: &Editor<'static>,
    env: &EditorEnv<'static>,
    view: &Rc<RwLock<View<'static>>>,
) -> Option<Id> {
    if env.status_view_id.is_some() {
        return env.status_view_id;
    }

    let view = view.read();

    if view.status_view_id.is_some() {
        return view.status_view_id;
    }

    let v = view;
    while let Some(pvid) = v.parent_id {
        let pv = editor.view_map.get(&pvid).unwrap();
        let pv = pv.read();
        if pv.status_view_id.is_some() {
            return pv.status_view_id;
        }
    }

    None
}

///
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
    editor: &mut Editor<'static>,
    env: &mut EditorEnv<'static>,
    view: &Rc<RwLock<View<'static>>>,
) -> Option<()> {
    let (dimension, max_offset) = {
        let v = view.read();
        let doc = v.document()?;
        let max_offset = { doc.read().size() as u64 };
        let dimension = v.screen.read().dimension();
        (dimension, max_offset)
    };

    let screen = {
        let start_offset = {
            let v = view.read();
            v.start_offset
        };

        // TODO(ceg): screen cache/allocator
        let mut screen = Screen::with_dimension(dimension);

        run_compositing_stage_direct(
            editor,
            env,
            &view,
            start_offset,
            max_offset,
            &mut screen,
            LayoutPass::ContentAndScreenOverlay,
        );

        screen
    };

    {
        let mut v = view.write();
        if let Some(last_offset) = screen.last_offset {
            v.end_offset = last_offset;
        }
        v.screen = Arc::new(RwLock::new(Box::new(screen)));
        v.check_invariants();
    }

    Some(())
}
