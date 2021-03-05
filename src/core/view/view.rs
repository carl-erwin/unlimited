// Copyright (c) Carl-Erwin Griffith

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

use crate::core::codepointinfo;
use crate::core::document::Document;

use crate::core::editor::Editor;
use crate::core::editor::EditorEnv;
use crate::core::editor::Stage;
use crate::core::editor::StagePosition;

use crate::core::mark::Mark;
use crate::core::screen::Screen;

use crate::core::view::layout::{run_compositing_stage, run_compositing_stage_direct};

use std::collections::HashMap;

use crate::core::modes::text_mode::*;

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

#[derive(Debug, Clone, Copy)]
pub enum Action {
    ScrollUp { n: usize },
    ScrollDown { n: usize },
    CenterAroundMainMark,
    CenterAroundMainMarkIfOffScreen,
    CenterAround { offset: u64 },
    MoveMarksToNextLine,
    MoveMarksToPreviousLine,
    MoveMarkToNextLine { idx: usize },
    MoveMarkToPreviousLine { idx: usize },
    ResetMarks,
    CheckMarks,
    DedupAndSaveMarks,
    CancelSelection,
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
    pub input_map: Rc<RefCell<InputEventMap>>,
    pub current_node: Option<Rc<InputEventRule>>,
    pub next_node: Option<Rc<InputEventRule>>,
    pub trigger: Vec<InputEvent>,
}

impl InputContext {
    pub fn new() -> Self {
        InputContext {
            action_map: HashMap::new(),
            input_map: Rc::new(RefCell::new(HashMap::new())),
            current_node: None,
            next_node: None,
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
    pub parent_id: Option<Id>,
    pub focus_to: Option<Id>, // child id

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

    // move this to corresponding pre/pos stages
    // reset on each event handling
    pub pre_compose_action: Vec<Action>,
    pub post_compose_action: Vec<Action>,
    //
    pub compose_filters: RefCell<Vec<Box<dyn layout::Filter<'a>>>>,
}

impl<'a> View<'a> {
    pub fn document(&self) -> Option<Arc<RwLock<Document<'static>>>> {
        let doc = self.document.clone();
        let doc = doc?;
        Some(doc)
    }

    /// Create a new View at a gin offset in the Document.<br/>
    pub fn new(
        mut editor: &mut Editor<'static>,
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
            pre_compose_action: vec![],
            post_compose_action: vec![],
            compose_filters: RefCell::new(vec![]),
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
                v.input_ctx.action_map.insert(name.clone(), fnptr.clone());
            }

            // TODO: user define
            // let input_map = mode.build_input_map(); TODO
            let input_map = build_input_event_map(DEFAULT_INPUT_MAP).unwrap();
            v.input_ctx.input_map = input_map;

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

    pub fn dimension(&mut self) -> (usize, usize) {
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

        let max_offset = self.document().as_ref().unwrap().read().unwrap().size();

        // TODO: move to TEXT MODE
        if !self.check_mode_ctx::<TextModeContext>("text-mode") {
            return;
        }

        let tm = self.mode_ctx::<TextModeContext>("text-mode");
        let marks = &tm.marks;
        for m in marks.iter() {
            if m.offset > max_offset as u64 {
                panic!("m.offset {} > max_offset {}", m.offset, max_offset);
            }
        }
    }
} // impl View

///
pub fn run_stage(
    mut editor: &mut Editor<'static>,
    mut env: &mut EditorEnv<'static>,
    view: &Rc<RefCell<View>>,
    pos: StagePosition,
    stage: Stage,
) {
    match (stage, pos) {
        (Stage::Input, StagePosition::Pre) => {
            // move to v.run_stage() : for register modes
            // TODO: mode_configure ->  v.register_run_stage_action(cb)
            crate::core::modes::text_mode::run_text_mode_actions(
                &mut editor,
                &mut env,
                &view,
                stage,
                pos,
            );
        }

        (Stage::Input, StagePosition::Post) => {
            // refresh view offset
            {
                let mut v = view.borrow_mut();
                let max_offset = v.document().unwrap().read().unwrap().size() as u64;
                v.start_offset = std::cmp::min(v.start_offset, max_offset);
            }
            // save marks
            crate::core::modes::text_mode::run_text_mode_actions(
                &mut editor,
                &mut env,
                &view,
                stage,
                pos,
            );
        }

        (Stage::Compositing, StagePosition::Pre) => {
            // TODO: save marks HERE After All input processing
            // check doc.revision

            // move to v.run_stage() : for register modes
            crate::core::modes::text_mode::run_text_mode_actions(
                &mut editor,
                &mut env,
                &view,
                stage,
                pos,
            );
        }

        (Stage::Compositing, StagePosition::In) => {
            compute_view_layout(editor, env, &view);
        }

        (Stage::Compositing, StagePosition::Post) => {
            crate::core::modes::text_mode::run_text_mode_actions(
                &mut editor,
                &mut env,
                &view,
                stage,
                pos,
            );
        }

        _ => {}
    }
}

// OLD FUNCTION
// illustrates how to compute consecutive screen between [start_offset, end_offset]
// and return screen lines
pub fn get_lines_offsets(
    editor: &Editor,
    env: &EditorEnv,
    view: &Rc<RefCell<View>>,
    start_offset: u64,
    end_offset: u64,
    screen_width: usize,
    screen_height: usize,
) -> Vec<(u64, u64)> {
    let doc = view.borrow();
    let doc = doc.document.as_ref().unwrap();
    let doc = doc.as_ref().write().unwrap();

    let mut v = Vec::<(u64, u64)>::new();

    let mut m = Mark::new(start_offset); // TODO: rename into screen_start_offset

    let max_offset = doc.size() as u64;

    // and build tmp screens until end_offset if found
    let screen_width = ::std::cmp::max(1, screen_width);
    let screen_height = ::std::cmp::max(4, screen_height);
    let mut screen = Screen::new(screen_width, screen_height);
    screen.is_off_screen = true;

    loop {
        run_compositing_stage(editor, env, &view, m.offset, max_offset, &mut screen);
        if screen.push_count == 0 {
            return v;
        }
        // push lines offsets
        // FIXME: find a better way to iterate over the used lines
        for i in 0..screen.current_line_index {
            if !v.is_empty() && i == 0 {
                // do not push line range twice
                continue;
            }
            let s = screen.line[i].get_first_cpi().unwrap().offset.unwrap();
            let e = screen.line[i].get_last_cpi().unwrap().offset.unwrap();
            v.push((s, e));

            if s >= end_offset || e == max_offset {
                return v;
            }
        }

        // eof reached ?
        // FIXME: the api is not yet READY
        // we must find a way to cover all filled lines
        if screen.current_line_index < screen.height() {
            let s = screen.line[screen.current_line_index]
                .get_first_cpi()
                .unwrap()
                .offset
                .unwrap();

            let e = screen.line[screen.current_line_index]
                .get_last_cpi()
                .unwrap()
                .offset
                .unwrap();
            v.push((s, e));
            return v;
        }

        // TODO: activate only in debug builds
        if 0 == 1 {
            match screen.find_cpi_by_offset(m.offset) {
                (Some(cpi), x, y) => {
                    assert_eq!(x, 0);
                    assert_eq!(y, 0);
                    assert_eq!(cpi.offset.unwrap(), m.offset);
                }
                _ => panic!("implementation error"),
            }
        }

        if let Some(l) = screen.get_last_used_line() {
            if let Some(cpi) = l.get_first_cpi() {
                m.offset = cpi.offset.unwrap(); // update next screen start
            }
        }

        screen.clear(); // prepare next screen
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

pub fn screen_putchar(
    screen: &mut Screen,
    c: char,
    offset: u64,
    size: usize,
    is_selected: bool,
) -> bool {
    let (ok, _) = screen.push(layout::filter_codepoint(
        None,
        None,
        c,
        Some(offset),
        size,
        is_selected,
        codepointinfo::CodepointInfo::default_color(),
        codepointinfo::CodepointInfo::default_bg_color(),
        true,
    ));
    ok
}

//////////////////////////////////
// TODO: screen_putstr_with_attr metadata etc ...
// return array of built &cpi ? to allow attr changes pass ?
pub fn screen_putstr(mut screen: &mut Screen, s: &str) -> bool {
    for c in s.chars() {
        let ok = screen_putchar(&mut screen, c, 0xffff_ffff_ffff_ffff, 0, false);
        if !ok {
            return false;
        }
    }

    true
}

#[test]
fn test_view() {}
