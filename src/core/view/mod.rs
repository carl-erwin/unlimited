// Copyright (c) Carl-Erwin Griffith

/* TODO

  add function center_screen_arround_offset(data, view_modes, offset, screen_description)
  where screen_description {
   width,
   height,
   option<screen_cache>
  }

  this function will called to refresh the view screen when
  the user modifies the buffer
*/

//
use std::cell::RefCell;
use std::rc::Rc;
use std::sync::Arc;
use std::sync::RwLock;

use std::time::Instant;

//
use crate::core::editor::Editor;
use crate::core::server::EditorEnv; // TODO: editor

use crate::dbg_println;

use crate::core::document::Document;

use crate::core::screen::Screen;

use crate::core::mark::Mark;

use crate::core::codec::text::utf8;
use crate::core::codec::text::SyncDirection; // TODO: remove
use crate::core::codec::text::TextCodec;

use crate::core::codepointinfo;

use crate::core::event::ButtonEvent;
use crate::core::event::InputEvent;
use crate::core::event::KeyModifiers;
use crate::core::event::PointerEvent;

use crate::core::view::layout::{run_view_layout_filters, run_view_layout_filters_direct};

pub type Id = u64;

pub mod layout;

// TODO: move to editor
pub type ModeFunction = fn(
    editor: &mut Editor,
    env: &mut EditorEnv,
    trigger: &Vec<InputEvent>,
    view: &Rc<RefCell<View>>,
) -> (); // () for now

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

#[derive(Debug, Clone)]
pub enum Action {
    ScrollUp { n: usize },
    ScrollDown { n: usize },
    CenterArroundMainMark,
    CenterArroundMainMarkIfOffscreen,
    CenterArround { offset: u64 },
    MoveMarksToNextLine,
    MoveMarksToPreviousLine,
    MoveMarkToNextLine { idx: usize },
    MoveMarkToPreviousLine { idx: usize },
    CheckMarks,
}

// TODO:
// add struct to map view["mode(n)"] -> data
// add struct to map doc["mode(n)"]  -> data: ex: line index

/// The **View** represents a way to represent a given Document.<br/>
// TODO: find a way to have marks as plugin.<br/>
// in future version marks will be stored in buffer meta data.<br/>
pub struct View<'a> {
    pub id: Id,

    // TODO: add struct to map view["mode(n)"] -> mode(n).data
    // reorder fields
    pub start_offset: u64, // where we want to start the rendering
    pub end_offset: u64,   // where the rendering stopped

    pub center_on_mark_move: bool,
    pub scroll_on_mark_move: bool,

    pub document: Option<Rc<RefCell<Document<'a>>>>, // if none and no children ... panic ?

    pub text_codec: Box<dyn TextCodec>, // Option ? move to mode

    pub screen: Arc<RwLock<Box<Screen>>>,

    pub moving_marks: Arc<RwLock<Vec<Mark>>>, // move to mode ?
    pub mark_index: usize,

    pub select_point: Option<Mark>,

    // use for cut and paste
    pub last_cut_log_index: Option<usize>,
}

impl<'a> View<'a> {
    pub fn document(&self) -> Option<Rc<RefCell<Document<'a>>>> {
        let doc = self.document.clone();
        let doc = doc?;
        Some(doc)
    }

    /// Create a new View at a gin offset in the Document.<br/>
    pub fn new(
        id: Id,
        start_offset: u64,
        width: usize,
        height: usize,
        document: Option<Rc<RefCell<Document>>>,
    ) -> View {
        let screen = Arc::new(RwLock::new(Box::new(Screen::new(width, height))));

        // TODO: in future version will be stored in buffer meta data
        let moving_marks = Arc::new(RwLock::new(vec![Mark { offset: 0 }]));

        View {
            id,
            start_offset,
            end_offset: start_offset,   // will be recomputed later
            center_on_mark_move: false, // add movement enums and pass it to center fn
            scroll_on_mark_move: true,
            document,
            text_codec: Box::new(utf8::Utf8Codec::new()),
            screen,
            moving_marks,
            mark_index: 0,
            select_point: None,
            last_cut_log_index: None,
        }
    }

    pub fn check_invariants(&self) {
        self.screen.read().unwrap().check_invariants();
    }

    /* TODO: use nb_lines
     to compute previous screen height
     new_h = screen.wheight + (nb_lines * screen.width * max_codec_encode_size)
    */
    pub fn scroll_up(&mut self, env: &EditorEnv, nb_lines: usize) {
        if self.start_offset == 0 || nb_lines == 0 {
            return;
        }

        // TODO: DUMB version
        // NEW: first try to check nb_lines in the same area
        // repeat mark moves
        // we can read backward self.screen.read().unwrap().width() chars
        // if we find '\n' or \r we stop
        // and take the next char offset -> self.start_offset
        if nb_lines == 1 {
            let doc = self.document.as_mut().unwrap().borrow_mut();

            let mut tmp = Mark::new(self.start_offset);
            for _ in 0..nb_lines {
                if tmp.offset == 0 {
                    break;
                }
                tmp.offset -= 1;
                tmp.move_to_start_of_line(&doc, self.text_codec.as_ref());
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
        if m.offset > diff {
            m.offset -= diff;
        } else {
            m.offset = 0;
        }

        // get start of line
        {
            let doc = self.document.as_mut().unwrap().borrow_mut();
            m.move_to_start_of_line(&doc, self.text_codec.as_ref());
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
        // nothing to do :-( ?
        if nb_lines == 0 {
            return;
        }

        let max_offset = {
            let doc = self.document.as_mut().unwrap().borrow_mut();
            doc.size() as u64
        };

        // avoid useless scroll
        if self.screen.read().unwrap().contains_offset(max_offset) {
            return;
        }

        if nb_lines >= self.screen.read().unwrap().height() {
            // slower : call layout builder to build  nb_lines - screen.height()
            self.scroll_down_offscreen(env, max_offset, nb_lines);
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

    fn scroll_down_offscreen(&mut self, env: &EditorEnv, max_offset: u64, nb_lines: usize) {
        // will be slower than just reading the current screen

        let screen_width = self.screen.read().unwrap().width();
        let screen_height = self.screen.read().unwrap().height() + 32;

        let start_offset = self.start_offset;
        let end_offset = ::std::cmp::min(
            self.start_offset + (4 * nb_lines * screen_width) as u64,
            max_offset,
        );

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
    /// using the run_view_layout_filters function until end_offset is reached.<br/>
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
            let doc = doc.as_ref().borrow_mut();
            // get start of the line @offset
            m.move_to_start_of_line(&doc, self.text_codec.as_ref());
            doc.size() as u64
        };

        // and build tmp screens until end_offset if found
        let screen_width = ::std::cmp::max(1, screen_width);
        let screen_height = ::std::cmp::max(4, screen_height);
        let mut screen = Screen::new(screen_width, screen_height);

        loop {
            run_view_layout_filters_direct(env, &self, m.offset, max_offset, &mut screen);
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

    fn center_arround_offset(&mut self, env: &EditorEnv, offset: u64) {
        // TODO use env.center_offset
        self.start_offset = offset;
        let h = self.screen.read().unwrap().height() / 2;
        self.scroll_up(env, h);
    }
} // View

/// This function computes start/end of lines between start_offset end_offset.<br/>
/// It (will) run the configured filters/plugins.<br/>
/// using the run_view_layout_filters function until end_offset is reached.<br/>
pub fn get_lines_offsets(
    env: &EditorEnv,
    view: &Rc<RefCell<View>>,
    start_offset: u64,
    end_offset: u64,
    screen_width: usize,
    screen_height: usize,
) -> Vec<(u64, u64)> {
    let doc = &view.as_ref().borrow();
    let doc = doc.document.as_ref().unwrap();
    let doc = doc.as_ref().borrow_mut();

    let mut v = Vec::<(u64, u64)>::new();

    let mut m = Mark::new(start_offset); // TODO: rename into screen_start

    // get start of the line @offset
    {
        let v = &view.as_ref().borrow();
        let codec = v.text_codec.as_ref();
        m.move_to_start_of_line(&doc, codec);
    }

    let max_offset = doc.size() as u64;

    // and build tmp screens until end_offset if found
    let screen_width = ::std::cmp::max(1, screen_width);
    let screen_height = ::std::cmp::max(4, screen_height);
    let mut screen = Screen::new(screen_width, screen_height);

    loop {
        run_view_layout_filters(env, &view, m.offset, max_offset, &mut screen);
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

pub fn run_view_action(
    editor: &mut Editor,
    env: &mut EditorEnv,
    view: &Rc<RefCell<View>>,
    actions: &Vec<Action>,
) {
    for a in actions.iter() {
        match a {
            Action::ScrollUp { n } => {
                let v = &mut view.as_ref().borrow_mut();
                v.scroll_up(env, *n);
            }
            Action::ScrollDown { n } => {
                let v = &mut view.as_ref().borrow_mut();
                v.scroll_down(env, *n);
            }
            Action::CenterArroundMainMark => {
                let trigger = Vec::new();
                center_arround_mark(editor, env, &trigger, &view);
            }
            Action::CenterArroundMainMarkIfOffscreen => {
                let trigger = Vec::new();
                // TODO: transform all cb to &trigger -> Option<&trigger>
                //        put trigger in env ?

                let center = {
                    let v = &mut view.as_ref().borrow();
                    let mid = v.mark_index;
                    let marks = v.moving_marks.read().unwrap();
                    let offset = marks[mid].offset;
                    let screen = v.screen.read().unwrap();
                    !screen.contains_offset(offset)
                };
                if center {
                    center_arround_mark(editor, env, &trigger, &view);
                }
            }
            Action::CenterArround { offset } => {
                // TODO:

                let trigger = Vec::new();
                env.center_offset = Some(*offset);
                center_arround_mark(editor, env, &trigger, &view);
            }
            Action::MoveMarksToNextLine => {
                let trigger = Vec::new();
                move_marks_to_next_line(editor, env, &trigger, &view);
            }
            Action::MoveMarksToPreviousLine => {}
            Action::MoveMarkToNextLine { idx } => {
                move_mark_to_next_line(env, view, *idx);
                env.cur_mark_index = None;
            }
            Action::MoveMarkToPreviousLine { idx: _usize } => {}

            Action::CheckMarks => {
                let v = &mut view.as_ref().borrow_mut();

                // TODO: function v.update_marks() ->
                let nr_marks = {
                    let mut marks = v.moving_marks.write().unwrap();
                    marks.dedup();
                    marks.len()
                };
                v.mark_index = if nr_marks != 0 { nr_marks - 1 } else { 0 };
            }
        }
    }
}

pub fn refresh_view_marks(_editor: &mut Editor, _env: &mut EditorEnv, view: &Rc<RefCell<View>>) {
    let v = view.as_ref().borrow_mut();

    // TODO: marks_filter
    // set_render_marks
    // brute force for now

    let marks = v.moving_marks.read().unwrap();

    for m in marks.iter() {
        //dbg_println!(" checking m.offset {}", m.offset);

        if m.offset < v.start_offset {
            continue;
        }

        // the marks sorted
        if m.offset > v.end_offset {
            break;
        }

        let mut screen = v.screen.write().unwrap();
        for l in 0..screen.height() {
            let line = screen.get_mut_line(l).unwrap();

            for c in 0..line.nb_cells {
                let cpi = line.get_mut_cpi(c).unwrap();

                if let Some(offset) = cpi.offset {
                    if offset == m.offset {
                        cpi.is_selected = !cpi.metadata;
                    }
                }
            }
        }
    }
}

pub fn compute_view_layout(
    _editor: &mut Editor,
    env: &mut EditorEnv,
    view: &Rc<RefCell<View>>,
) -> Option<()> {
    let mut v = view.as_ref().borrow_mut();

    let doc = v.document()?;

    let max_offset = { doc.borrow().size() as u64 };

    // TODO: reuse v.screen
    let mut screen = Box::new(Screen::with_dimension(v.screen.read().unwrap().dimension()));

    run_view_layout_filters_direct(env, &v, v.start_offset, max_offset, &mut screen);

    // TODO: from env ?
    v.end_offset = screen.last_offset.unwrap();
    v.screen = Arc::new(RwLock::new(screen)); // move v.screen to view double buffer  v.screen_get() v.screen_swap(new: move)
    v.check_invariants();

    Some(())
}

// TODO: test-mode
// scroll bar: bg color (35, 34, 89)
// scroll bar: cursor color (192, 192, 192)
pub fn update_view(
    editor: &mut Editor,
    env: &mut EditorEnv,
    view: &Rc<RefCell<View>>,
) -> Option<()> {
    let _start = Instant::now();

    // refresh some env vars
    {
        let v = &mut view.as_ref().borrow();
        env.max_offset = v.document()?.borrow().size() as u64;
    }

    // pre layout action
    {
        let actions = env.view_pre_render.clone();
        env.view_pre_render.clear();
        run_view_action(editor, env, view, &actions);
    }

    compute_view_layout(editor, env, view);

    // post layout action
    if false {
        let actions = env.view_post_render.clone();
        env.view_post_render.clear();
        run_view_action(editor, env, view, &actions);
    }

    // let t0 = Instant::now();
    // refresh_view_marks(editor, env, view);
    // let t1 = Instant::now();
    // dbg_println!("refresh_view_marks : {} ms", (t1 - t0).as_millis());

    let _end = Instant::now();
    // env.time_to_build_screen = end.duration_since(start);

    Some(())
}

///
use crate::core::event::Key;

// CEG
pub fn scroll_up(
    _editor: &mut Editor,
    env: &mut EditorEnv,
    _trigger: &Vec<InputEvent>,
    _view: &Rc<RefCell<View>>,
) {
    // TODO: 3 is from mode configuration
    // env["default-scroll-size"] -> int
    env.view_pre_render.push(Action::ScrollUp { n: 3 });
}

pub fn scroll_down(
    _editor: &mut Editor,
    env: &mut EditorEnv,
    _trigger: &Vec<InputEvent>,
    _view: &Rc<RefCell<View>>,
) {
    // TODO: 3 is from mode configuration
    // env["default-scroll-size"] -> int
    env.view_pre_render.push(Action::ScrollDown { n: 3 });
}

// TODO: rename into insert_input_event
/// Insert an array of unicode code points using hardcoded utf8 codec.<br/>
pub fn insert_codepoint_array(
    _editor: &mut Editor,
    env: &mut EditorEnv,
    trigger: &Vec<InputEvent>,
    view: &Rc<RefCell<View>>,
) {
    let array = match trigger[0] {
        InputEvent::KeyPress {
            mods:
                KeyModifiers {
                    ctrl: false,
                    alt: false,
                    shift: false,
                },
            key: Key::UnicodeArray(ref v),
        } => v,

        _ => {
            return;
        }
    };

    let center = {
        let v = view.as_ref().borrow();
        let mut doc = v.document.as_ref().unwrap().borrow_mut();

        let codec = v.text_codec.as_ref();
        let mut utf8 = Vec::with_capacity(array.len());

        for codepoint in array {
            let mut data: &mut [u8] = &mut [0, 0, 0, 0];
            let data_size = codec.encode(*codepoint as u32, &mut data);
            for d in data.iter().take(data_size) {
                utf8.push(*d);
            }
        }

        let mut offset: u64 = 0;
        let mut grow: u64 = 0;

        let marks_offsets: Vec<u64> = v
            .moving_marks
            .read()
            .unwrap()
            .iter()
            .map(|m| m.offset)
            .collect();

        doc.tag(env.max_offset, marks_offsets);

        for m in v.moving_marks.write().unwrap().iter_mut() {
            m.offset += grow;
            doc.insert(m.offset, utf8.len(), &utf8);
            m.offset += utf8.len() as u64;

            offset = m.offset; // TODO: remove this merge

            grow += utf8.len() as u64;
        }

        env.max_offset = doc.size() as u64;
        //
        let marks_offsets: Vec<u64> = v
            .moving_marks
            .read()
            .unwrap()
            .iter()
            .map(|m| m.offset)
            .collect();
        doc.tag(env.max_offset, marks_offsets);

        // mark offscreen ?
        let screen = v.screen.read().unwrap();
        screen.contains_offset(offset) == false || array.len() > screen.width() * screen.height()
    };

    if center {
        env.view_pre_render.push(Action::CenterArroundMainMark);
    };
}

pub fn remove_previous_codepoint(
    _editor: &mut Editor,
    env: &mut EditorEnv,
    _trigger: &Vec<InputEvent>,
    view: &Rc<RefCell<View>>,
) {
    let v = &mut view.as_ref().borrow_mut();
    {
        let doc = v.document.clone(); // TODO: use Option<clone> to release imut boorow of v
        let doc = doc.as_ref().clone().unwrap();
        let mut doc = doc.as_ref().borrow_mut();
        let codec = v.text_codec.as_ref();

        if doc.size() == 0 {
            return;
        }

        let mut marks = v.moving_marks.write().unwrap();

        let marks_offsets: Vec<u64> = marks.iter().map(|m| m.offset).collect();

        doc.tag(env.max_offset, marks_offsets);

        let mut shrink = 0;
        for m in marks.iter_mut() {
            if m.offset == 0 {
                continue;
            }

            dbg_println!("before shrink m.offset= {}", m.offset);
            m.offset -= shrink;
            dbg_println!("after shrink m.offset= {}", m.offset);
            if m.offset == 0 {
                continue;
            }

            m.move_backward(&doc, codec);
            dbg_println!("after move.backward m.offset= {}", m.offset);

            let mut data = vec![];
            doc.read(m.offset, 4, &mut data);
            let (_, _, size) = codec.decode(SyncDirection::Forward, &data, 0);
            dbg_println!("read {} bytes", size);

            let nr_removed = doc.remove(m.offset, size, None);
            dbg_println!("nr_removed {} bytes", nr_removed);

            dbg_println!(
                "shrink({}) + nr_rm({}) = {}",
                shrink,
                nr_removed,
                shrink + nr_removed as u64
            );
            shrink += nr_removed as u64;

            if m.offset < v.start_offset {
                env.view_pre_render.push(Action::ScrollUp { n: 1 });
            }
        }

        env.max_offset = doc.size() as u64;
        env.view_pre_render.push(Action::CheckMarks);

        let marks_offsets = marks.iter().map(|m| m.offset).collect();
        doc.tag(env.max_offset, marks_offsets);
    }
}

/// Undo the previous write operation and sync the screen around the main mark.<br/>
pub fn undo(
    _editor: &mut Editor,
    env: &mut EditorEnv,
    _trigger: &Vec<InputEvent>,
    view: &Rc<RefCell<View>>,
) {
    // hack no multicursor for now
    // TODO: add transaction
    // undo/redo . use self.buffer_log.pos as tag
    // destroy all marks
    // collect/recreate marks @ undo result
    {
        let v = &mut view.as_ref().borrow_mut();
        let doc = v.document.as_ref().unwrap();
        let mut doc = doc.as_ref().borrow_mut();
        let mut marks = v.moving_marks.write().unwrap();

        doc.undo_until_tag();
        doc.undo_until_tag();
        if let Some(marks_offsets) = doc.get_tag_offsets() {
            //dbg_println!("restore marks {:?}", marks_offsets);
            marks.clear();
            for offset in marks_offsets {
                marks.push(Mark { offset });
            }
        }
    }

    {
        let v = &mut view.as_ref().borrow_mut();
        v.mark_index = 0;
    }

    env.view_pre_render
        .push(Action::CenterArroundMainMarkIfOffscreen);
}

/// Redo the previous write operation and sync the screen around the main mark.<br/>
pub fn redo(
    _editor: &mut Editor,
    env: &mut EditorEnv,
    _trigger: &Vec<InputEvent>,
    view: &Rc<RefCell<View>>,
) {
    let v = &mut view.as_ref().borrow_mut();
    v.mark_index = 0;

    let doc = v.document.as_ref().unwrap();
    let mut doc = doc.as_ref().borrow_mut();
    let mut marks = v.moving_marks.write().unwrap();

    doc.redo_until_tag();
    doc.redo_until_tag();
    if let Some(marks_offsets) = doc.get_tag_offsets() {
        //dbg_println!("restore marks {:?}", marks_offsets);
        marks.clear();
        for offset in marks_offsets {
            marks.push(Mark { offset });
        }
    }

    env.view_pre_render
        .push(Action::CenterArroundMainMarkIfOffscreen);
}

/// Remove the current utf8 encoded code point.<br/>
pub fn remove_codepoint(
    _editor: &mut Editor,
    env: &mut EditorEnv,
    _trigger: &Vec<InputEvent>,
    view: &Rc<RefCell<View>>,
) {
    let v = &mut view.as_ref().borrow_mut();

    let doc = v.document.as_ref().unwrap();
    let mut doc = doc.as_ref().borrow_mut();
    let codec = v.text_codec.as_ref();

    if doc.size() == 0 {
        return;
    }

    let mut marks = v.moving_marks.write().unwrap();

    let marks_offsets: Vec<u64> = marks.iter().map(|m| m.offset).collect();
    doc.tag(env.max_offset, marks_offsets);

    let mut shrink = 0;

    for m in marks.iter_mut() {
        if m.offset >= shrink {
            m.offset -= shrink;
        }

        let mut data = Vec::with_capacity(4);
        doc.read(m.offset, data.capacity(), &mut data);
        let (_, _, size) = codec.decode(SyncDirection::Forward, &data, 0);

        let nr_removed = doc.remove(m.offset, size as usize, None);
        shrink += nr_removed as u64;
    }

    env.max_offset = doc.size() as u64;

    marks.dedup(); // here ?

    let marks_offsets: Vec<u64> = marks.iter().map(|m| m.offset).collect();
    doc.tag(env.max_offset, marks_offsets);
}

/// Skip blanks (if any) and remove until end of the word.
/// TODO: handle ',' | ';' | '(' | ')' | '{' | '}'
pub fn remove_until_end_of_word(
    _editor: &mut Editor,
    env: &mut EditorEnv,
    _trigger: &Vec<InputEvent>,
    view: &Rc<RefCell<View>>,
) {
    let v = &mut view.as_ref().borrow_mut();

    let doc = v.document.as_ref().unwrap();
    let mut doc = doc.as_ref().borrow_mut();
    let codec = v.text_codec.as_ref();

    let size = doc.size() as u64;

    if size == 0 {
        return;
    }

    let mut marks = v.moving_marks.write().unwrap();

    let marks_offsets: Vec<u64> = marks.iter().map(|m| m.offset).collect();
    doc.tag(size, marks_offsets);

    let mut shrink: u64 = 0;

    for m in marks.iter_mut() {
        if m.offset >= shrink {
            m.offset -= shrink;
        }

        let start = m.clone();

        let mut data = Vec::with_capacity(4);

        // skip blanks until any char or end-of-line
        loop {
            data.clear();
            doc.read(m.offset, data.capacity(), &mut data);
            let (cp, _, size) = codec.decode(SyncDirection::Forward, &data, 0);

            if size == 0 {
                break;
            }

            match cp {
                ' ' | '\t' => {
                    m.offset += size as u64;
                    continue;
                }

                _ => break,
            }
        }

        // skip until blank or end-of-line
        loop {
            data.clear();
            doc.read(m.offset, data.capacity(), &mut data);
            let (cp, _, size) = codec.decode(SyncDirection::Forward, &data, 0);

            if size == 0 {
                break;
            }

            match cp {
                ' ' | '\t' | '\r' | '\n' => {
                    break;
                }

                _ => {
                    m.offset += size as u64;
                    continue;
                }
            }
        }

        // remove [start, m[
        let nr_removed = doc.remove(start.offset, (m.offset - start.offset) as usize, None);

        shrink += nr_removed as u64;

        m.offset = start.offset;
    }

    let marks_offsets: Vec<u64> = marks.iter().map(|m| m.offset).collect();

    env.max_offset = doc.size() as u64;
    doc.tag(env.max_offset, marks_offsets);
}

// TODO: maintain main mark Option<(x,y)>
pub fn move_marks_backward(
    _editor: &mut Editor,
    env: &mut EditorEnv,
    _trigger: &Vec<InputEvent>,
    view: &Rc<RefCell<View>>,
) {
    let v = &mut view.as_ref().borrow_mut();
    let doc = v.document.clone();
    let doc = doc.as_ref().unwrap().borrow();
    let codec = v.text_codec.as_ref();
    let midx = v.mark_index;

    let mut marks = v.moving_marks.write().unwrap();

    for (idx, m) in marks.iter_mut().enumerate() {
        if idx == midx && m.offset <= v.start_offset {
            env.view_pre_render.push(Action::ScrollUp { n: 1 });
        }

        m.move_backward(&doc, codec);
    }

    env.view_pre_render.push(Action::CheckMarks);

    if v.center_on_mark_move {
        env.view_pre_render.push(Action::CenterArroundMainMark);
    }
}

pub fn move_marks_forward(
    _editor: &mut Editor,
    env: &mut EditorEnv,
    _trigger: &Vec<InputEvent>,
    view: &Rc<RefCell<View>>,
) {
    {
        let v = &mut view.as_ref().borrow_mut();
        let doc = v.document.clone();
        let doc = doc.as_ref().unwrap().borrow();
        let codec = v.text_codec.as_ref();
        let midx = v.mark_index;

        let nr_marks = {
            let mut marks = v.moving_marks.write().unwrap();

            for (idx, m) in marks.iter_mut().enumerate() {
                // TODO: add main mark check
                if idx == midx && m.offset >= v.end_offset {
                    env.view_pre_render.push(Action::ScrollDown { n: 1 });
                }

                m.move_forward(&doc, codec);
            }

            // update main mark index
            marks.len()
        };

        // TODO:  env.view_pre_render.push(Action::SelectLastMark);
        v.mark_index = if nr_marks > 0 { nr_marks - 1 } else { 0 }; // TODO: dedup ?
    }

    //      move this check at post render to reschedule render ?
    //      if v.center_on_mark_move {
    //           env.view_pre_render.push(Action::CenterArroundMainMark);
    //      }

    env.view_pre_render.push(Action::CheckMarks);
}

pub fn move_marks_to_start_of_line(
    _editor: &mut Editor,
    env: &mut EditorEnv,
    _trigger: &Vec<InputEvent>,
    view: &Rc<RefCell<View>>,
) {
    let v = &mut view.as_ref().borrow();

    let doc = v.document.as_ref().unwrap().borrow();
    let codec = v.text_codec.as_ref();
    let midx = v.mark_index;
    let screen = v.screen.read().unwrap();
    let mut marks = v.moving_marks.write().unwrap();

    for (idx, m) in marks.iter_mut().enumerate() {
        m.move_to_start_of_line(&doc, codec);

        if idx == midx && screen.contains_offset(m.offset) == false {
            env.view_pre_render.push(Action::CenterArroundMainMark);
        }
    }

    env.view_pre_render.push(Action::CheckMarks);
}

pub fn move_marks_to_end_of_line(
    _editor: &mut Editor,
    env: &mut EditorEnv,
    _trigger: &Vec<InputEvent>,
    view: &Rc<RefCell<View>>,
) {
    let v = &view.as_ref().borrow();
    let doc = v.document.as_ref().unwrap().borrow();
    let codec = v.text_codec.as_ref();
    let midx = v.mark_index;
    let screen = v.screen.read().unwrap();
    let mut marks = v.moving_marks.write().unwrap();

    for (idx, m) in marks.iter_mut().enumerate() {
        m.move_to_end_of_line(&doc, codec);

        if idx == midx && screen.contains_offset(m.offset) == false {
            env.view_pre_render.push(Action::CenterArroundMainMark);
        }
    }

    env.view_pre_render.push(Action::CheckMarks);
}

fn move_mark_to_previous_line(
    _editor: &mut Editor,
    env: &mut EditorEnv,
    _trigger: &Vec<InputEvent>,
    view: &Rc<RefCell<View>>,
    midx: usize,
) {
    let mut mark_moved = false;

    let m_offset = {
        let v = &mut view.as_ref().borrow_mut();
        let mut marks = v.moving_marks.write().unwrap();
        let mut m = &mut marks[midx];

        let screen = v.screen.read().unwrap();
        // TODO: if v.is_mark_on_screen(m) -> (bool, x, y) + (prev/new offset)?
        match screen.find_cpi_by_offset(m.offset) {
            // offscreen
            (None, _, _) => {}
            // mark on first line
            (Some(_), _, y) if y == 0 => {}

            // onscreen
            (Some(_), x, y) if y > 0 => {
                // TODO: refactor code to support screen cell metadata
                let new_y = y - 1; // select previous line
                let l = screen.get_line(new_y).unwrap();
                // previous line is filled ?
                if l.nb_cells > 0 {
                    let new_x = ::std::cmp::min(x, l.nb_cells - 1);
                    let cpi = screen.get_cpinfo(new_x, new_y).unwrap();
                    if !cpi.metadata {
                        m.offset = cpi.offset.unwrap();
                        mark_moved = true;
                    }
                } else {
                    // ???
                }
            }

            // impossible
            _ => {}
        }

        m.offset
    };

    // offscreen
    if !mark_moved {
        // mark is offscreen

        let end_offset = m_offset;
        let (start_offset, screen_width, screen_height) = {
            let v = &mut view.as_ref().borrow_mut();

            let start_offset = {
                let doc = v.document.as_ref().unwrap();
                let doc = doc.as_ref().borrow();
                let codec = v.text_codec.as_ref();

                // todo: set marks codecs
                let mut tmp = Mark { offset: m_offset };

                // goto start of current line (mar is on first line of screen)
                tmp.move_to_start_of_line(&doc, codec);
                // goto end of previous line
                tmp.move_backward(&doc, codec);
                // goto start of previous line
                tmp.move_to_start_of_line(&doc, codec);
                tmp.offset

                /*
                if m.offset - tmp.offset > (screen.width * screen.height)
                {
                   long line mode
                }
                else {

                }

                */
            };

            let width = v.screen.read().unwrap().width();

            let add_height = if width > 0 {
                (m_offset - start_offset) as usize / width
            } else {
                1
            };
            let height = v.screen.read().unwrap().height() + (add_height * 4); // 4 is utf8 max encode size

            (start_offset, width, height)
        };

        // TODO: loop until m.offset is on screen

        let lines = {
            let v = &mut view.as_ref().borrow_mut();
            v.get_lines_offsets_direct(env, start_offset, end_offset, screen_width, screen_height)
        };

        // find "previous" line index
        let index = match lines
            .iter()
            .position(|e| e.0 <= end_offset && end_offset <= e.1)
        {
            None | Some(0) => return, // error
            Some(i) => i - 1,
        };

        let line_start_off = lines[index].0;
        let line_end_off = lines[index].1;

        let mut tmp_mark = Mark::new(line_start_off);

        // compute column
        let new_x = {
            let v = &mut view.as_ref().borrow();
            let doc = v.document.as_ref().unwrap();
            let doc = doc.as_ref().borrow();
            let codec = v.text_codec.as_ref();

            let mut s = Mark::new(lines[index + 1].0);
            let e = Mark::new(lines[index + 1].1);
            let mut count = 0;
            while s.offset != e.offset {
                if s.offset == m_offset {
                    break;
                }

                s.move_forward(&doc, codec);
                count += 1;
            }
            count
        };

        {
            let v = &mut view.as_ref().borrow();
            let doc = v.document.as_ref().unwrap();
            let doc = doc.as_ref().borrow();
            let codec = v.text_codec.as_ref();

            for _ in 0..new_x {
                tmp_mark.move_forward(&doc, codec);
            }

            if tmp_mark.offset > line_end_off {
                tmp_mark.offset = line_end_off;
            }
        }
        // TODO: add some post processing after screen moves
        // this will avoid custom code in pageup/down
        // if m.offset < screen.start -> m.offset = start_offset
        // if m.offset > screen.end -> m.offset = screen.line[last_index].start_offset

        // resync mark to "new" first line offset
        if tmp_mark.offset < m_offset {
            let v = &mut view.as_ref().borrow_mut();
            let mut marks = v.moving_marks.write().unwrap();
            let mut m = &mut marks[midx];
            m.offset = tmp_mark.offset;
        }
    }
}

pub fn move_marks_to_previous_line(
    editor: &mut Editor,
    env: &mut EditorEnv,
    trigger: &Vec<InputEvent>,
    view: &Rc<RefCell<View>>,
) {
    // TODO: maintain env.mark_index_max ?
    let idx_max = {
        let v = view.as_ref().borrow_mut();
        let marks = v.moving_marks.write().unwrap();
        marks.len() - 1
    };

    for idx in 0..=idx_max {
        let prev_offset = {
            let v = view.as_ref().borrow();
            let marks = v.moving_marks.write().unwrap();
            marks[idx].offset
        };
        move_mark_to_previous_line(editor, env, trigger, view, idx);

        // TODO: move this to pre/post render
        if idx == 0 {
            // env.view_pre_render.push(Action::UpdateViewOnMainMarkMove { moveType: ToPreviousLine, before: prev_offset, after: new_offset });
            let new_offset = {
                let v = view.as_ref().borrow();
                let marks = v.moving_marks.write().unwrap();
                marks[idx].offset
            };

            if new_offset != prev_offset {
                let mut v = view.as_ref().borrow_mut();
                v.mark_index = 0; // reset main mark

                let screen = v.screen.read().unwrap();
                let was_on_screen = screen.contains_offset(prev_offset);
                let is_on_screen = screen.contains_offset(new_offset);
                if was_on_screen && !is_on_screen {
                    env.view_pre_render.push(Action::ScrollUp { n: 1 });
                } else if !is_on_screen {
                    env.view_pre_render.push(Action::CenterArroundMainMark);
                }
            }
        }
    }

    env.view_pre_render.push(Action::CheckMarks);
}

pub fn move_on_screen_mark_to_next_line(
    m: &mut Mark,
    screen: &Screen,
) -> (bool, Option<(u64, u64)>, Option<Action>) {
    if !screen.contains_offset(m.offset) {
        return (false, None, None);
    }

    // yes get coordinates
    let (_, x, y) = screen.find_cpi_by_offset(m.offset);
    let screen_height = screen.height();

    //dbg_println!("m.offset screen (X({}), Y({}))", x, y);
    //dbg_println!("screen_height {}", screen_height);

    // not last line -> on screen move ?
    if y < screen_height - 1 {
        let new_y = y + 1;
        let l = screen.get_line(new_y).unwrap();
        let old_offset = m.offset;
        if l.nb_cells > 0 {
            let new_x = ::std::cmp::min(x, l.nb_cells - 1);
            let cpi = screen.get_cpinfo(new_x, new_y).unwrap();
            m.offset = cpi.offset.unwrap();
        } else {
            // l.nb_cells == 0, the line is empty do nothing
        }

        (true, Some((old_offset, m.offset)), None)
    } else {
        assert_eq!(y, screen_height - 1);
        (false, None, Some(Action::ScrollDown { n: 1 }))
    }
}

// remove multiple borrows
pub fn move_mark_to_next_line(
    env: &mut EditorEnv,
    view: &Rc<RefCell<View>>,
    mark_idx: usize,
) -> Option<(u64, u64)> {
    // TODO: m.on_buffer_end() ?
    let max_offset = env.max_offset;

    // offscreen ?
    let mut m_offset = 0;
    let mut old_offset = 0;

    {
        let v = view.as_ref().borrow_mut();
        let screen = v.screen.read().unwrap();

        let mut marks = v.moving_marks.write().unwrap();
        let mut m = &mut marks[mark_idx];
        m_offset = m.offset;
        old_offset = m.offset;

        if m.offset == max_offset {
            return None;
        }

        let (ok, offsets, action) = move_on_screen_mark_to_next_line(&mut m, &screen);
        if let Some(action) = action {
            // Add stage RenderStage :: PreRender PostRender
            // will be removed when the "scroll" update is implemented
            // ADD screen cache ?
            // screen[first mark -> last mark ] ? Ram usage ?
            // updated on resize -> slow
            env.view_pre_render.push(action);
        }

        if ok == true {
            return offsets;
        }
    }

    if true {
        // mark is offscreen
        let (screen_width, screen_height) = {
            let view = view.as_ref().borrow_mut();
            let screen = view.screen.read().unwrap();
            (screen.width(), screen.height())
        };

        // get start_of_line(m.offset) -> u64
        let start_offset = {
            let v = &view.as_ref().borrow();
            let doc = v.document.as_ref().unwrap();
            let doc = doc.as_ref().borrow();
            let codec = v.text_codec.as_ref();
            //
            let marks = v.moving_marks.read().unwrap();
            let m = &marks[mark_idx];
            let mut tmp = Mark::new(m.offset);
            tmp.move_to_start_of_line(&doc, codec);
            tmp.offset
        };

        // a codepoint can use 4 bytes the virtual end is
        // + 1 full line away
        let end_offset = ::std::cmp::min(m_offset + (4 * screen_width) as u64, max_offset);

        // get lines start, end offset
        // NB: run full layout code for one screen line ( folding etc ... )

        // TODO: return Vec<Box<screen>> ? update contenet
        // TODO: add perf view screen cache ? sorted by screens.start_offset
        // with same width/heigh as v.screen
        let lines = {
            let mut view = view.as_ref().borrow_mut();
            view.get_lines_offsets_direct(
                env,
                start_offset,
                end_offset,
                screen_width,
                screen_height,
            )
        };

        dbg_println!("GET {} lines ", lines.len());

        // find the cursor index
        let index = match lines
            .iter()
            .position(|e| e.0 <= m_offset && m_offset <= e.1)
        {
            None => return None,
            Some(i) => {
                if i == lines.len() - 1 {
                    return None;
                } else {
                    i
                }
            }
        };

        // compute column
        let new_x = {
            let v = &view.as_ref().borrow();
            let doc = v.document.as_ref().unwrap();
            let doc = doc.as_ref().borrow();
            let codec = v.text_codec.as_ref();

            // TODO: use codec.read(doc, n=width) until e.offset is reached
            let mut s = Mark::new(lines[index].0);
            let e = Mark::new(lines[index].1);
            let mut count = 0;
            while s.offset < e.offset {
                if s.offset == m_offset {
                    break;
                }

                s.move_forward(&doc, codec);
                count += 1;
            }
            count
        };

        // get next line start/end offsets
        let next_index = index + 1;
        let line_start_off = lines[next_index].0;
        let line_end_off = lines[next_index].1;

        let mut tmp_mark = Mark::new(line_start_off);

        let v = &view.as_ref().borrow();
        let doc = v.document.as_ref().unwrap();
        let doc = doc.as_ref().borrow();
        let codec = v.text_codec.as_ref();
        // TODO: codec.skip_n(doc, 0..new_x)
        for _ in 0..new_x {
            tmp_mark.move_forward(&doc, codec); // TODO: pass n as arg
        }

        if tmp_mark.offset > line_end_off {
            tmp_mark.offset = line_end_off;
        }

        m_offset = tmp_mark.offset;
    }

    {
        let v = view.as_ref().borrow_mut();
        let _screen = v.screen.read().unwrap();

        let mut marks = v.moving_marks.write().unwrap();
        let m = &mut marks[mark_idx];
        m.offset = m_offset;
    }

    Some((old_offset, m_offset))
}

pub fn move_marks_to_next_line(
    _editor: &mut Editor,
    env: &mut EditorEnv,
    _trigger: &Vec<InputEvent>,
    view: &Rc<RefCell<View>>,
) {
    //
    let v = view.as_ref().borrow_mut();

    let mut marks = v.moving_marks.write().unwrap();

    let idx_max = marks.len();
    assert!(idx_max > 0);
    let mut idx_start = 0;

    // allocate temporary screen
    let (width, height, screen_first_offset) = {
        let screen = v.screen.as_ref().read().unwrap();
        let screen_first_offset = screen.first_offset.unwrap();

        let width = screen.width();

        /*
         * NB: 1 is for the first lne of the next screen
         *  1 + (max - min) / screen_width   more lines
         */

        let min_offset = marks[0].offset;
        let max_offset = marks[idx_max - 1].offset;
        let add_lines = (max_offset - min_offset) as usize / width;
        let height = screen.height() + 1 + add_lines;

        dbg_println!("new virtual screen : {} x {}", width, height);

        (width, height, screen_first_offset)
    };

    let mut screen = Screen::new(width, height);
    screen.is_off_screen = true;

    // use current screen
    let mut m = Mark::new(screen_first_offset);
    if m.offset > marks[idx_start].offset {
        m.offset = marks[idx_start].offset;
    }

    let max_offset = {
        let doc = v.document.clone();
        let doc = doc.as_ref().unwrap();
        let doc = doc.as_ref().borrow_mut();

        m.move_to_start_of_line(&doc, v.text_codec.as_ref());

        doc.size() as u64
    };

    dbg_println!("max_offset {}", max_offset);

    while idx_start < idx_max {
        dbg_println!(" idx_start {} < idx_max {}", idx_start, idx_max);

        // update screen with configure filters
        screen.clear();

        dbg_println!("compute layout from offset {}", m.offset);

        run_view_layout_filters_direct(env, &v, m.offset, max_offset, &mut screen);

        dbg_println!("screen first offset {:?}", screen.first_offset);
        dbg_println!("screen first offset {:?}", screen.last_offset);

        assert_ne!(0, screen.push_count());

        // TODO: pass doc &doc to avoid double borrow
        // env.doc ?
        // env.view ? to avoid too many args

        //
        let last_line = screen.get_last_used_line();
        if last_line.is_none() {
            dbg_println!("no last line");
            panic!();
            break;
        }
        let last_line = last_line.unwrap();

        // idx_start not on screen  ? ...
        if !screen.contains_offset(marks[idx_start].offset) {
            dbg_println!("offset not found on screen go to next screen");

            // go to next screen
            // using the firt offset of the last line
            if let Some(cpi) = last_line.get_first_cpi() {
                m.offset = cpi.offset.unwrap(); // update next screen start offset
                continue;
            }
            panic!();
        }

        // idx_start is on screen
        let mut idx_end = idx_start + 1;
        let cpi = last_line.get_first_cpi().unwrap();
        while idx_end < idx_max {
            if marks[idx_end].offset >= cpi.offset.unwrap() {
                break;
            }
            idx_end += 1;
        }

        dbg_println!("update marks[{}..{} / {}]", idx_start, idx_end, idx_max);

        for i in idx_start..idx_end {
            let ret = move_on_screen_mark_to_next_line(&mut marks[i], &screen);
            assert!(ret.0, true);
        }

        idx_start = idx_end; // next mark index

        m.offset = cpi.offset.unwrap(); // update next screen start
    }

    // check main mark
    {
        let screen = v.screen.as_ref().read().unwrap();
        let idx = v.mark_index;
        if !screen.contains_offset(marks[idx].offset) {
            env.view_pre_render.push(Action::ScrollDown { n: 1 });
        }

        env.view_pre_render.push(Action::CheckMarks);
    }
}

pub fn clone_and_move_mark_to_previous_line(
    editor: &mut Editor,
    env: &mut EditorEnv,
    trigger: &Vec<InputEvent>,
    view: &Rc<RefCell<View>>,
) {
    let prev_off = {
        let v = view.as_ref().borrow();
        let marks = v.moving_marks.read().unwrap();
        let m = &marks[0];
        m.offset
    };

    dbg_println!(" clone move up: prev_offset {}", prev_off);

    move_mark_to_previous_line(editor, env, trigger, view, 0); // TODO return (idx, prev_off, new_off)

    let m_offset = {
        let v = view.as_ref().borrow();
        let marks = v.moving_marks.read().unwrap();
        let m = &marks[0];
        m.offset
    };

    if m_offset != prev_off {
        let mut v = view.as_ref().borrow_mut();
        v.mark_index = 0;
        let mut marks = v.moving_marks.write().unwrap();

        // insert mark @ m_offset + pa
        marks.insert(0, Mark { offset: m_offset });
        marks[1].offset = prev_off;
        // env.sort mark sync direction
        // update view.mark_index

        let screen = v.screen.read().unwrap();
        let was_on_screen = screen.contains_offset(prev_off);
        let is_on_screen = screen.contains_offset(m_offset);
        if was_on_screen && !is_on_screen {
            env.view_pre_render.push(Action::ScrollUp { n: 1 });
        } else if !is_on_screen {
            env.view_pre_render.push(Action::CenterArroundMainMark);
        }
    }
}

pub fn clone_and_move_mark_to_next_line(
    _editor: &mut Editor,
    env: &mut EditorEnv,
    _trigger: &Vec<InputEvent>,
    view: &Rc<RefCell<View>>,
) {
    // refresh mark index
    let mark_len = {
        let mut v = view.as_ref().borrow_mut();

        let mark_len = {
            let mut marks = v.moving_marks.write().unwrap();
            let midx = marks.len() - 1;
            let offset = marks[midx].offset;
            // duplicated last mark + select
            marks.push(Mark { offset });
            marks.len()
        };

        v.mark_index = mark_len - 1;
        env.cur_mark_index = Some(v.mark_index);

        // doc
        let doc = v.document.as_ref().unwrap();
        let doc = doc.as_ref().borrow();
        env.max_offset = doc.size() as u64;

        mark_len
    };

    let offsets = move_mark_to_next_line(env, view, mark_len - 1); // TODO return offset (old, new)
    let offsets = offsets.unwrap();

    dbg_println!(" clone move down: offsets {:?}", offsets);

    let mut v = view.as_ref().borrow_mut();

    // no move ?
    if offsets.0 == offsets.1 {
        // destroy duplicated mark
        v.mark_index = {
            let mut marks = v.moving_marks.write().unwrap();
            marks.pop();
            marks.len() - 1
        };

        let screen = v.screen.read().unwrap();
        let was_on_screen = screen.contains_offset(offsets.0);
        if !was_on_screen {
            env.view_pre_render.push(Action::CenterArroundMainMark);
        }
        return;
    }

    dbg_println!(" clone move down: new_offset {}", offsets.1);
    // env.sort mark sync direction
    // update view.mark_index

    let screen = v.screen.read().unwrap();
    let was_on_screen = screen.contains_offset(offsets.0);
    let is_on_screen = screen.contains_offset(offsets.1);
    dbg_println!(
        " was_on_screen {} , is_on_screen  {}",
        was_on_screen,
        is_on_screen
    );

    if was_on_screen && !is_on_screen {
        env.view_pre_render.push(Action::ScrollDown { n: 1 });
    } else if !is_on_screen {
        env.view_pre_render.push(Action::CenterArroundMainMark);
    }
}

pub fn move_mark_to_screen_start(
    _editor: &mut Editor,
    _env: &mut EditorEnv,
    _trigger: &Vec<InputEvent>,
    view: &Rc<RefCell<View>>,
) {
    let v = view.as_ref().borrow();
    let mut marks = v.moving_marks.write().unwrap();

    for m in marks.iter_mut() {
        // TODO: add main mark check
        if m.offset < v.start_offset || m.offset > v.end_offset {
            m.offset = v.start_offset;
        }
    }
}

pub fn move_mark_to_screen_end(
    _editor: &mut Editor,
    _env: &mut EditorEnv,
    _trigger: &Vec<InputEvent>,
    view: &Rc<RefCell<View>>,
) {
    let v = view.as_ref().borrow();
    let mut marks = v.moving_marks.write().unwrap();

    for m in marks.iter_mut() {
        // TODO: add main mark check
        if m.offset < v.start_offset || m.offset > v.end_offset {
            m.offset = v.end_offset;
        }
    }
}

pub fn scroll_to_previous_screen(
    editor: &mut Editor,
    env: &mut EditorEnv,
    trigger: &Vec<InputEvent>,
    view: &Rc<RefCell<View>>,
) {
    {
        let mut v = view.as_ref().borrow_mut();
        let nb = ::std::cmp::max(v.screen.read().unwrap().height() - 1, 1);
        v.scroll_up(env, nb);
    }

    // TODO: add hints to trigger mar moves
    move_mark_to_screen_end(editor, env, trigger, &view);
}

pub fn move_mark_to_start_of_file(
    _editor: &mut Editor,
    _env: &mut EditorEnv,
    _trigger: &Vec<InputEvent>,
    view: &Rc<RefCell<View>>,
) {
    let mut v = view.as_ref().borrow_mut();
    v.start_offset = 0;
    v.mark_index = 0;

    let mut moving_marks = v.moving_marks.write().unwrap();
    moving_marks.clear();
    moving_marks.push(Mark { offset: 0 });
}

// TODO: view.center_arrout_offset()
pub fn center_arround_mark(
    _editor: &mut Editor,
    env: &mut EditorEnv,
    _trigger: &Vec<InputEvent>,
    view: &Rc<RefCell<View>>,
) {
    let mut v = view.as_ref().borrow_mut();

    let offset = {
        let marks = v.moving_marks.read().unwrap();
        let mi = v.mark_index;
        marks[mi].offset
    };

    v.center_arround_offset(env, offset);
}

pub fn center_arround_offset(
    _editor: &mut Editor,
    env: &mut EditorEnv,
    _trigger: &Vec<InputEvent>,
    view: &Rc<RefCell<View>>,
) {
    if let Some(center_offset) = env.center_offset {
        let mut v = view.as_ref().borrow_mut();
        let offset = {
            let doc = v.document.as_ref().unwrap();
            let doc = doc.as_ref().borrow();
            ::std::cmp::min(doc.size() as u64, center_offset)
        };

        v.center_arround_offset(env, offset);
    }
}

pub fn move_mark_to_end_of_file(
    _editor: &mut Editor,
    env: &mut EditorEnv,
    _trigger: &Vec<InputEvent>,
    view: &Rc<RefCell<View>>,
) {
    let mut v = view.as_ref().borrow_mut();

    let offset = {
        let doc = v.document.as_ref().unwrap();
        let doc = doc.as_ref().borrow();
        doc.size() as u64
    };

    v.start_offset = offset;
    v.mark_index = 0;

    let mut marks = v.moving_marks.write().unwrap();
    marks.clear();
    marks.push(Mark { offset });

    //
    let n = v.screen.read().unwrap().height() / 2;
    env.view_pre_render.push(Action::ScrollUp { n })
}

pub fn scroll_to_next_screen(
    _editor: &mut Editor,
    env: &mut EditorEnv,
    _trigger: &Vec<InputEvent>,
    view: &Rc<RefCell<View>>,
) {
    let v = view.as_ref().borrow_mut();
    let n = ::std::cmp::max(v.screen.read().unwrap().height() - 1, 1);
    env.view_pre_render.push(Action::ScrollDown { n });
}

/*
    TODO: with multi marks:
      add per mark cut/paste buffer
      and reuse it when pasting
      check behavior when the marks offset cross each other
      the buffer log is not aware of cut/paste/multicursor
*/
pub fn cut_to_end_of_line(
    _editor: &mut Editor,
    _env: &mut EditorEnv,
    _trigger: &Vec<InputEvent>,
    view: &Rc<RefCell<View>>,
) {
    let v = &mut view.as_ref().borrow_mut();

    let pos = {
        let doc = v.document.as_ref().unwrap();
        let mut doc = doc.as_ref().borrow_mut();
        let codec = v.text_codec.as_ref();

        for m in v.moving_marks.read().unwrap().iter() {
            let mut end = m.clone();
            end.move_to_end_of_line(&doc, codec);
            doc.remove(m.offset, (end.offset - m.offset) as usize, None);
            break;
        }

        doc.buffer_log.pos
    };

    // save buffer log idx
    assert!(pos > 0);
    v.last_cut_log_index = Some(pos - 1);
}

pub fn paste(
    _editor: &mut Editor,
    _env: &mut EditorEnv,
    _trigger: &Vec<InputEvent>,
    view: &Rc<RefCell<View>>,
) {
    let v = &mut view.as_ref().borrow();

    if let Some(idx) = v.last_cut_log_index {
        let doc = v.document.as_ref().unwrap();
        let mut doc = doc.as_ref().borrow_mut();
        let codec = v.text_codec.as_ref();

        let tr = doc.buffer_log.data[idx].clone();
        let mut marks = v.moving_marks.write().unwrap();
        for m in marks.iter_mut() {
            let mut end = m.clone();
            end.move_to_end_of_line(&doc, codec);
            if let Some(ref data) = tr.data {
                doc.insert(m.offset, data.len(), data.as_slice());
                m.offset += data.len() as u64;
            }
        }

    // true
    } else {
        // false
    }
}

pub fn move_to_token_start(
    _editor: &mut Editor,
    env: &mut EditorEnv,
    _trigger: &Vec<InputEvent>,
    view: &Rc<RefCell<View>>,
) {
    // TODO: factorize macrk action
    // mark.apply(fn); where fn=m.move_to_token_end(&doc, codec);
    //

    {
        let v = &mut view.as_ref().borrow();
        let doc = v.document.as_ref().unwrap();
        let doc = doc.as_ref().borrow();
        let codec = v.text_codec.as_ref();
        let midx = v.mark_index;

        let mut marks = v.moving_marks.write().unwrap();

        for (idx, m) in marks.iter_mut().enumerate() {
            m.move_to_token_start(&doc, codec);

            // main mark ?
            if idx == midx {
                if !v.screen.read().unwrap().contains_offset(m.offset) {
                    // TODO: push to post action queue
                    // {SYNC_VIEW, CLEAR_VIEW, SCROLL_N }
                    //
                    env.view_pre_render.push(Action::CenterArroundMainMark);
                }
            }
        }
    }
}

pub fn move_to_token_end(
    _editor: &mut Editor,
    env: &mut EditorEnv,
    _trigger: &Vec<InputEvent>,
    view: &Rc<RefCell<View>>,
) {
    let mut sync = false;

    {
        let v = &mut view.as_ref().borrow();
        let doc = v.document.as_ref().unwrap();
        let doc = doc.as_ref().borrow();
        let codec = v.text_codec.as_ref();

        let mut marks = v.moving_marks.write().unwrap();

        for m in marks.iter_mut() {
            m.move_to_token_end(&doc, codec);

            // main mark ?
            if !v.screen.read().unwrap().contains_offset(m.offset) {
                // TODO: push to post action queue
                // {SYNC_VIEW, CLEAR_VIEW, SCROLL_N }
                //
                sync = true;
            }
        }
    }

    if sync {
        env.view_pre_render.push(Action::CenterArroundMainMark);
    }
}

pub fn set_selection_point_at_mark(
    _editor: &mut Editor,
    env: &mut EditorEnv,
    _trigger: &Vec<InputEvent>,
    view: &Rc<RefCell<View>>,
) {
    let mut sync = false;

    {
        let v = &mut view.as_ref().borrow_mut();
        let offset = {
            let mut marks = v.moving_marks.read().unwrap();
            let m = &marks[v.mark_index];
            m.offset
        };
        // update selection point
        v.select_point = Some(Mark { offset });
    }

    if sync
    /* alwayd center ? */
    {
        env.view_pre_render.push(Action::CenterArroundMainMark);
    }
}

pub fn button_press(
    _editor: &mut Editor,
    _env: &mut EditorEnv,
    trigger: &Vec<InputEvent>,
    view: &Rc<RefCell<View>>,
) {
    let v = &mut view.as_ref().borrow_mut();

    let (button, x, y) = match trigger[0] {
        InputEvent::ButtonPress(ref button_event) => match button_event {
            ButtonEvent {
                mods:
                    KeyModifiers {
                        ctrl: _,
                        alt: _,
                        shift: _,
                    },
                x,
                y,
                button,
            } => (*button, *x, *y),
        },

        _ => {
            return;
        }
    };

    match button {
        0 => {}
        _ => {
            return;
        }
    }

    // TODO: new function clip screen.coordinates(x,y) -> (x, y)
    /*
      (0,0) --------------------- max_width
                 clip.x
             -------------------
        |    | | | | | | | | | | |
        |    | |   clip.width  | |
      clip.y | | |_|_|_|_|_|_| h |
        |    | | | | | | | | | e |
        |    | | | | | | | | | i |
        |    | | | | | | | | | g |
        |    | | |_|_|_|_|_|_| h |
        |    | | | | | | | | | t |
        |    | | | | | | | | | | |
        |    -------------------
    max_height

    */

    let screen = v.screen.clone();
    let screen = screen.read().unwrap();

    // move cursor to (x,y)
    let (mut x, mut y) = (x as usize, y as usize);

    // 0 <= x < screen.width()
    if x < screen.clip_rect().x {
        x = 0;
    } else if x >= screen.clip_rect().x + screen.clip_rect().width {
        x = screen.clip_rect().width - 1;
    } else {
        x -= screen.clip_rect().x;
    }

    // 0 <= y < screen.height()
    if y < screen.clip_rect().y {
        y = 0;
    } else if y > screen.clip_rect().y + screen.clip_rect().height {
        y = screen.clip_rect().height - 1;
    } else {
        y -= screen.clip_rect().y;
    }

    //
    let _max_offset = screen.doc_max_offset;

    let last_li = screen.get_last_used_line_index();
    if y >= last_li {
        if last_li >= screen.height() {
            y = screen.height() - 1;
        } else {
            y = last_li;
        }
    }

    if let Some(l) = screen.get_line(y) {
        if l.nb_cells > 0 && x > l.nb_cells {
            x = l.nb_cells - 1;
        } else if l.nb_cells == 0 {
            x = 0;
        }
    } else {
    }

    // check from right to left until some codepoint is found
    let mut i = x + 1;
    while i > 0 {
        if let Some(cpi) = screen.get_used_cpinfo(x, y) {
            // clear selection point
            v.select_point = None;

            // reset main mark
            v.mark_index = 0;
            let mut marks = v.moving_marks.write().unwrap();
            marks.clear();
            marks.push(Mark {
                offset: cpi.offset.unwrap(),
            });
        }

        i -= 1;
    }

    // s // to internal view.as_ref().borrow_mut().state.s
}

pub fn button_release(
    _editor: &mut Editor,
    _env: &mut EditorEnv,
    trigger: &Vec<InputEvent>,
    _view: &Rc<RefCell<View>>,
) {
    match trigger[0] {
        InputEvent::ButtonPress(ref button_event) => match button_event {
            ButtonEvent {
                mods:
                    KeyModifiers {
                        ctrl: _,
                        alt: _,
                        shift: _,
                    },
                x: _,
                y: _,
                button,
            } => {
                let button = if *button == 0xff {
                    // TODO: return last pressed button
                    0xff
                } else {
                    *button
                };

                match button {
                    _ => {}
                }
            }
        },

        _ => {}
    }
}

// crossterm
pub fn pointer_motion(
    _editor: &mut Editor,
    _env: &mut EditorEnv,
    trigger: &Vec<InputEvent>,
    view: &Rc<RefCell<View>>,
) {
    let v = &mut view.as_ref().borrow_mut();
    let screen = v.screen.clone();
    let screen = screen.read().unwrap();

    assert_eq!(v.mark_index, 0);
    assert_eq!(v.moving_marks.read().unwrap().len(), 1);

    // TODO: match events
    match &trigger[0] {
        InputEvent::PointerMotion(PointerEvent { mods: _, x, y }) => {
            // TODO: change screen (x,y) to i32 ? and filter in functions ?
            let x = {
                if *x < 0 {
                    0
                } else {
                    *x as usize
                }
            };
            let y = {
                if *y < 0 {
                    0
                } else {
                    *y as usize
                }
            };

            if let Some(cpi) = screen.get_cpinfo(x, y) {
                {
                    // update selection point
                    v.select_point = Some(Mark {
                        offset: cpi.offset.unwrap(),
                    });

                    dbg_println!(
                        "@{:?} : pointer motion x({}) y({}) | select offset({})",
                        Instant::now(),
                        x,
                        y,
                        cpi.offset.unwrap()
                    );
                }
            }
        }

        _ => {}
    }
}

pub fn select_next_view(
    editor: &mut Editor,
    env: &mut EditorEnv,
    _trigger: &Vec<InputEvent>,
    _view: &Rc<RefCell<View>>,
) {
    env.view_id = std::cmp::min(env.view_id + 1, editor.view_map.len() - 1);
}

pub fn select_previous_view(
    _editor: &mut Editor,
    env: &mut EditorEnv,
    _trigger: &Vec<InputEvent>,
    _view: &Rc<RefCell<View>>,
) {
    env.view_id = env.view_id.saturating_sub(1);
}

//////////////////////////////////
// TODO: screen_putstr_with_attr metadat etc ...
// return array of built &cpi ? to allow attr changes pass ?
pub fn screen_putstr(mut screen: &mut Screen, s: &str) -> bool {
    for c in s.chars() {
        let ok = screen_putchar(&mut screen, c, 0xffff_ffff_ffff_ffff, false);
        if !ok {
            return false;
        }
    }

    true
}

pub fn screen_putchar(screen: &mut Screen, c: char, offset: u64, is_selected: bool) -> bool {
    let (ok, _) = screen.push(layout::filter_codepoint(
        c,
        Some(offset),
        is_selected,
        codepointinfo::CodepointInfo::default_color(),
        codepointinfo::CodepointInfo::default_bg_color(),
        true,
    ));
    ok
}

#[test]
fn test_view() {}
