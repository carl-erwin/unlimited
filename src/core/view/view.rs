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
use crate::core::codepointinfo;
use crate::core::document::Document;
use crate::core::editor::Editor;
use crate::core::editor::EditorEnv;

use crate::core::mark::Mark;
use crate::core::screen::Screen;

use crate::core::view::layout::{run_view_render_filters, run_view_render_filters_direct};

use std::collections::HashMap;

use crate::core::modes::text_mode::*;

use super::layout;

pub type Id = usize;

// let ptr : ModeFunction = cancel_input(editor: &mut Editor, env: &mut EditorEnv, trigger: &Vec<input_event>,  view: &Rc<RefCell<View>>)

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
    SaveCurrentMarks,
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

/// The **View** represents a way to represent a given Document.<br/>
// TODO: find a way to have marks as plugin.<br/>
// in future version marks will be stored in buffer meta data.<br/>
// TODO editor.env.current.view_id = view.id
// can zoom ?
pub struct View<'v, 'a> {
    pub id: Id,
    pub parent_id: Option<Id>,
    pub focus_to: Option<Id>, // child id

    pub document: Option<Arc<RwLock<Document<'a>>>>, // if none and no children ... panic ?
    pub mode_ctx: HashMap<String, Box<dyn Any>>,
    //
    pub screen: Arc<RwLock<Box<Screen>>>,

    //
    pub start_offset: u64, // where we want to start the rendering
    pub end_offset: u64,   // where the rendering stopped

    // layout
    //
    pub x: usize,
    pub y: usize,

    pub layout_direction: LayoutDirection,
    pub layout_ops: Vec<LayoutOperation>,
    // TODO: keep them here or use view.id -> editor.view(view.id)
    pub children: Vec<Rc<RefCell<View<'v, 'a>>>>,

    // move this to corresponding pre/pos stages
    // reset on each event handling
    pub pre_render_action: Vec<Action>,
    pub post_render_action: Vec<Action>,
}

impl<'v, 'a> View<'v, 'a> {
    pub fn document(&self) -> Option<Arc<RwLock<Document<'a>>>> {
        let doc = self.document.clone();
        let doc = doc?;
        Some(doc)
    }

    /// Create a new View at a gin offset in the Document.<br/>
    pub fn new(
        parent_id: Option<Id>,
        start_offset: u64,
        width: usize,
        height: usize,
        document: Option<Arc<RwLock<Document<'a>>>>,
    ) -> View<'v, 'a> {
        let screen = Arc::new(RwLock::new(Box::new(Screen::new(width, height))));

        let id = VIEW_ID.fetch_add(1, Ordering::SeqCst);

        let mode_ctx = HashMap::new();

        View {
            parent_id,
            focus_to: None,
            id,
            document,
            screen,
            //
            start_offset,
            end_offset: start_offset, // will be recomputed later
            mode_ctx,
            //
            x: 0,
            y: 0,
            layout_direction: LayoutDirection::NotSet,
            layout_ops: vec![],
            children: vec![],
            pre_render_action: vec![],
            post_render_action: vec![],
        }
    }

    pub fn set_mode_ctx(&mut self, name: &str, ctx: Box<dyn Any>) -> bool {
        let res = self.mode_ctx.insert(name.to_owned(), ctx);
        assert!(res.is_none());
        true
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

            None => panic!("not configured properly"),
        }
    }

    pub fn get_view_at_mouse_position(&mut self, _x: i32, _y: i32) -> Option<&'a View<'a, 'a>> {
        None
    }

    pub fn compute_split(size: usize) -> (usize, usize) {
        let half = size / 2;
        let (first_half, second_halt) = if half + half < size {
            (half + 1, half)
        } else {
            (half, half)
        };
        (first_half, second_halt)
    }

    pub fn split_horizontally(&mut self, _top: usize, _bottom: usize) {}

    pub fn check_invariants(&self) {
        self.screen.read().unwrap().check_invariants();

        let max_offset = self.document().as_ref().unwrap().read().unwrap().size();

        let tm = self.mode_ctx::<TextModeContext>("text-mode");

        let marks = &tm.marks;
        for m in marks.iter() {
            if m.offset > max_offset as u64 {
                panic!("m.offset {} > max_offset {}", m.offset, max_offset);
            }
        }
    }

    /* TODO: use nb_lines
     to compute previous screen height
     new_h = screen.wheight + (nb_lines * screen.width * max_codec_encode_size)
    */
    pub fn scroll_up(&mut self, env: &EditorEnv, nb_lines: usize) {
        if self.start_offset == 0 || nb_lines == 0 {
            return;
        }

        // TODO: find abetter way to pas mode data around, macro ?

        // TODO: DUMB version
        // NEW: first try to check nb_lines in the same area
        // repeat mark moves
        // we can read backward self.screen.read().unwrap().width() chars
        // if we find '\n' or \r we stop
        // and take the next char offset -> self.start_offset
        if nb_lines == 1 {
            let start_offset = self.start_offset;
            let doc = self.document.clone();
            let doc = doc.as_ref().unwrap();
            let doc = doc.as_ref().read().unwrap();

            let tm = self.mode_ctx_mut::<TextModeContext>("text-mode");
            let codec = tm.text_codec.as_ref();

            let mut tmp = Mark::new(start_offset);
            for _ in 0..nb_lines {
                if tmp.offset == 0 {
                    break;
                }
                tmp.offset -= 1;
                tmp.move_to_start_of_line(&doc, codec);
            }

            self.start_offset = tmp.offset;

            // TODO: render screen here
            // if not aligned full rebuild etc...
            // diff tmp stat > s.width s.height
            return;
        }

        ////
        let width = self.screen.read().unwrap().width();
        let height = self.screen.read().unwrap().height() + nb_lines;

        // the offset to find is the first screen codepoint
        let offset_to_find = self.start_offset;

        // go to N previous physical lines ... here N is height
        // rewind width*height chars
        let mut m = Mark::new(self.start_offset);
        let diff = (nb_lines * width * 4) as u64; // if ascci only 4 -> 1

        m.offset = m.offset.saturating_sub(diff);

        // get start of line
        {
            let doc = self.document.clone();
            let doc = doc.as_ref().unwrap().read().unwrap();
            let tm = self.mode_ctx_mut::<TextModeContext>("text-mode");
            let codec = tm.text_codec.as_ref();
            m.move_to_start_of_line(&doc, codec);
        }

        // build tmp screens until first offset of the original screen if found
        // build_screen from this offset
        // the window MUST cover to screen => height * 2
        // TODO: always in last index ?
        let lines = self.get_lines_offsets_direct(env, m.offset, offset_to_find, width, height);

        // find line index
        let index = match lines
            .iter()
            .position(|e| e.0 <= offset_to_find && offset_to_find <= e.1)
        {
            None => 0,
            Some(i) => {
                if i >= nb_lines {
                    ::std::cmp::min(lines.len() - 1, i - nb_lines)
                } else {
                    0
                }
            }
        };

        self.start_offset = lines[index].0;
    }

    pub fn scroll_down(&mut self, env: &EditorEnv, nb_lines: usize) {
        dbg_println!("SCROLL DOWN VID = {}", self.id);

        // nothing to do :-( ?
        if nb_lines == 0 {
            return;
        }

        let max_offset = {
            let doc = self.document.as_ref().unwrap().read().unwrap();
            doc.size() as u64
        };

        // avoid useless scroll
        if self.screen.read().unwrap().has_eof() {
            return;
        }

        if nb_lines >= self.screen.read().unwrap().height() {
            // slower : call layout builder to build  nb_lines - screen.height()
            self.scroll_down_off_screen(env, max_offset, nb_lines);
            return;
        }

        // just read the current screen
        if let (Some(l), _) = self.screen.write().unwrap().get_used_line_clipped(nb_lines) {
            if let Some(cpi) = l.get_first_cpi() {
                // set first offset of screen.line[nb_lines] as next screen start
                if let Some(offset) = cpi.offset {
                    self.start_offset = offset;
                }
            }
        } else {
            panic!();
        }
    }

    fn scroll_down_off_screen(&mut self, env: &EditorEnv, max_offset: u64, nb_lines: usize) {
        // will be slower than just reading the current screen

        let screen_width = self.screen.read().unwrap().width();
        let screen_height = self.screen.read().unwrap().height() + 32;

        let start_offset = self.start_offset;
        let end_offset = ::std::cmp::min(
            self.start_offset + (4 * nb_lines * screen_width) as u64,
            max_offset,
        );

        // will call all layout filters
        let lines = self.get_lines_offsets_direct(
            env,
            start_offset,
            end_offset,
            screen_width,
            screen_height,
        );

        // find line index and take lines[(index + nb_lines)].0 as new start of view
        let index = match lines
            .iter()
            .position(|e| e.0 <= start_offset && start_offset <= e.1)
        {
            None => 0,
            Some(i) => ::std::cmp::min(lines.len() - 1, i + nb_lines),
        };

        self.start_offset = lines[index].0;
    }

    /// This function computes start/end of lines between start_offset end_offset.<br/>
    /// It (will) run the configured filters/plugins.<br/>
    /// using the run_view_render_filters function until end_offset is reached.<br/>
    pub fn get_lines_offsets_direct(
        &mut self,
        env: &EditorEnv,
        start_offset: u64,
        end_offset: u64,
        screen_width: usize,
        screen_height: usize,
    ) -> Vec<(u64, u64)> {
        let mut v = Vec::<(u64, u64)>::new();
        let mut m = Mark::new(start_offset); // TODO: rename into screen_start

        let max_offset = {
            let doc = self.document.clone();
            let doc = doc.as_ref().unwrap();
            let doc = doc.as_ref().read().unwrap();

            let tm = self.mode_ctx_mut::<TextModeContext>("text-mode");
            let codec = tm.text_codec.as_ref();

            // get start of the line @offset
            m.move_to_start_of_line(&doc, codec);
            doc.size() as u64
        };

        // and build tmp screens until end_offset if found
        let screen_width = ::std::cmp::max(1, screen_width);
        let screen_height = ::std::cmp::max(4, screen_height);
        let mut screen = Screen::new(screen_width, screen_height);
        screen.is_off_screen = true;

        loop {
            run_view_render_filters_direct(env, &self, m.offset, max_offset, &mut screen);
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

    pub fn center_around_offset(&mut self, env: &EditorEnv, offset: u64) {
        // TODO use env.center_offset
        self.start_offset = offset;
        let h = self.screen.read().unwrap().height() / 2;
        self.scroll_up(env, h);
    }
} // View

/// This function computes start/end of lines between start_offset end_offset.<br/>
/// It (will) run the configured filters/plugins.<br/>
/// using the run_view_render_filters function until end_offset is reached.<br/>
pub fn get_lines_offsets(
    env: &EditorEnv,
    view: &Rc<RefCell<View>>,
    start_offset: u64,
    end_offset: u64,
    screen_width: usize,
    screen_height: usize,
) -> Vec<(u64, u64)> {
    let doc = &view.borrow();
    let doc = doc.document.as_ref().unwrap();
    let doc = doc.as_ref().write().unwrap();

    let mut v = Vec::<(u64, u64)>::new();

    let mut m = Mark::new(start_offset); // TODO: rename into screen_start

    // get start of the line @offset
    {
        let v = &view.borrow();
        let tm = v.mode_ctx::<TextModeContext>("text-mode");

        let codec = tm.text_codec.as_ref();

        m.move_to_start_of_line(&doc, codec);
    }

    let max_offset = doc.size() as u64;

    // and build tmp screens until end_offset if found
    let screen_width = ::std::cmp::max(1, screen_width);
    let screen_height = ::std::cmp::max(4, screen_height);
    let mut screen = Screen::new(screen_width, screen_height);
    screen.is_off_screen = true;

    loop {
        run_view_render_filters(env, &view, m.offset, max_offset, &mut screen);
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
    _editor: &mut Editor,
    env: &mut EditorEnv,
    view: &Rc<RefCell<View>>,
) -> Option<()> {
    let mut v = view.borrow_mut();

    let doc = v.document()?;

    let max_offset = { doc.as_ref().read().unwrap().size() as u64 };

    // TODO: reuse v.screen
    let mut screen = Box::new(Screen::with_dimension(v.screen.read().unwrap().dimension()));

    run_view_render_filters_direct(env, &v, v.start_offset, max_offset, &mut screen);

    // TODO: from env ?
    if let Some(last_offset) = screen.last_offset {
        v.end_offset = last_offset;
    }
    v.screen = Arc::new(RwLock::new(screen)); // move v.screen to view double buffer  v.screen_get() v.screen_swap(new: move)
    v.check_invariants();

    Some(())
}

// TODO: test-mode
// scroll bar: bg color (35, 34, 89)
// scroll bar: cursor color (192, 192, 192)
pub fn update_view(
    mut editor: &mut Editor,
    mut env: &mut EditorEnv,
    view: &Rc<RefCell<View>>,
) -> Option<()> {
    let _start = Instant::now();

    let nb_child = {
        let v = view.borrow_mut();
        if v.children.len() > 0 {
            for child in v.children.iter() {
                dbg_println!(" REC call to : update view depth {}", v.children.len());
                update_view(&mut editor, &mut env, &child);
            }
        }
        v.children.len()
    };

    {
        let v = view.borrow_mut();
        dbg_println!("update view {} nb_child {}", v.id, nb_child);
    }

    // refresh_env_variables(editor, env, view);
    {
        let mut v = view.borrow_mut();
        env.max_offset = v.document()?.read().unwrap().size() as u64;
        if v.start_offset > env.max_offset {
            v.start_offset = env.max_offset;
        }
    }

    // pre layout action == post input
    {
        run_text_mode_actions(editor, env, view, Stage::PreRender);
    }

    // already recursive

    compute_view_layout(editor, env, view);

    // post layout action
    if false {
        run_text_mode_actions(editor, env, view, Stage::PostRender);
    }

    let _end = Instant::now();
    // env.time_to_build_screen = end.duration_since(start);

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
