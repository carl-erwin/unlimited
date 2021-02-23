// Copyright (c) Carl-Erwin Griffith

//
use std::any::Any;
use std::cell::RefCell;
use std::rc::Rc;

use std::sync::Arc;
use std::sync::RwLock;

use std::time::Instant;

//

use crate::core::editor::Editor;

use crate::core::editor::EditorEnv;

use crate::core::document::Document;

use crate::core::screen::Screen;

use crate::core::mark::Mark;

// TODO: remove

use crate::core::codepointinfo;

use crate::core::event::InputEvent;

use crate::core::view::layout::{run_view_render_filters, run_view_render_filters_direct};

use std::collections::HashMap;

use crate::core::modes::text_mode;
use crate::core::modes::text_mode::*;
use crate::core::modes::Mode;
use crate::core::modes::TextMode;

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
    CenterArroundMainMarkIfOffScreen,
    CenterArround { offset: u64 },
    MoveMarksToNextLine,
    MoveMarksToPreviousLine,
    MoveMarkToNextLine { idx: usize },
    MoveMarkToPreviousLine { idx: usize },
    CheckMarks,
    SaveMarks,
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

/// The **View** represents a way to represent a given Document.<br/>
// TODO: find a way to have marks as plugin.<br/>
// in future version marks will be stored in buffer meta data.<br/>
pub struct View<'a> {
    pub id: Id,

    // TODO: Option<Arc<RwLock<Document<'a>>>> : shared access with indexer
    //
    //  access pattern: clone to "release" parent
    pub document: Option<Rc<RefCell<Document<'a>>>>, // if none and no children ... panic ?
    //
    pub screen: Arc<RwLock<Box<Screen>>>,

    //
    pub start_offset: u64, // where we want to start the rendering
    pub end_offset: u64,   // where the rendering stopped

    pub main_mode: &'static str,                    // mandatory by name
    pub modes: HashMap<&'static str, Box<dyn Any>>, // HUM ......
}

impl<'a> View<'a> {
    pub fn document(&self) -> Option<Rc<RefCell<Document<'a>>>> {
        let doc = self.document.clone();
        let doc = doc?;
        Some(doc)
    }

    /// Create a new View at a gin offset in the Document.<br/>
    pub fn new(
        mut env: &mut EditorEnv<'a>,
        id: Id,
        start_offset: u64,
        width: usize,
        height: usize,
        document: Option<Rc<RefCell<Document<'a>>>>,
    ) -> View<'a> {
        let screen = Arc::new(RwLock::new(Box::new(Screen::new(width, height))));

        // set default mode(s)
        let mut modes: HashMap<&str, Box<dyn Any>> = HashMap::new();
        let text_mode = Box::new(TextMode::new(&mut env));
        let mode_name = text_mode.name();

        modes.insert(mode_name, text_mode);

        View {
            id,
            document,
            screen,

            //
            start_offset,
            end_offset: start_offset, // will be recomputed later
            main_mode: mode_name,
            modes,
        }
    }

    pub fn get_mode<'v, M: 'static>(&'v self, name: &str) -> &'v M {
        let m = self.modes.get(name).unwrap();
        let m = m.downcast_ref::<M>().unwrap(); // will panic
        m
    }

    pub fn get_mode_mut<'v, M: 'static>(&'v mut self, name: &str) -> &'v mut M {
        let m = self.modes.get_mut(name).unwrap();
        let m = m.downcast_mut::<M>().unwrap(); // will panic
        m
    }

    pub fn check_invariants(&self) {
        self.screen.read().unwrap().check_invariants();

        let max_offset = self.document().as_ref().unwrap().borrow().size();

        let tm = self.get_mode::<TextMode>("text-mode");

        let marks = &tm.marks;
        for m in marks.iter() {
            if m.offset > max_offset as u64 {
                panic!("");
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

        // TODO: find abetter way to pas mode data arround, macro ?

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
            let doc = doc.as_ref().borrow();

            let tm = self.get_mode_mut::<TextMode>("text-mode");
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
            let doc = doc.as_ref().unwrap().borrow();
            let tm = self.get_mode_mut::<TextMode>("text-mode");
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
        // nothing to do :-( ?
        if nb_lines == 0 {
            return;
        }

        let max_offset = {
            let doc = self.document.as_ref().unwrap().borrow();
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
            let doc = doc.as_ref().borrow_mut();

            let tm = self.get_mode_mut::<TextMode>("text-mode");
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

        let main_mark = Mark::new(0); // fake main mark

        loop {
            run_view_render_filters_direct(
                env,
                &self,
                m.offset,
                max_offset,
                &mut screen,
                main_mark.clone(),
            );
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

    pub fn center_arround_offset(&mut self, env: &EditorEnv, offset: u64) {
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
    let doc = &view.as_ref().borrow();
    let doc = doc.document.as_ref().unwrap();
    let doc = doc.as_ref().borrow_mut();

    let mut v = Vec::<(u64, u64)>::new();

    let mut m = Mark::new(start_offset); // TODO: rename into screen_start

    // get start of the line @offset
    {
        let v = &view.as_ref().borrow();
        let tm = v.get_mode::<TextMode>("text-mode");

        let codec = tm.text_codec.as_ref();

        m.move_to_start_of_line(&doc, codec);
    }

    let max_offset = doc.size() as u64;

    // and build tmp screens until end_offset if found
    let screen_width = ::std::cmp::max(1, screen_width);
    let screen_height = ::std::cmp::max(4, screen_height);
    let mut screen = Screen::new(screen_width, screen_height);

    let main_mark = Mark::new(0);

    loop {
        run_view_render_filters(
            env,
            &view,
            m.offset,
            max_offset,
            &mut screen,
            main_mark.clone(),
        );
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

// TODO: post_eval stage(editor, env, view, action as member of mode);
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
                text_mode::center_arround_mark(editor, env, &trigger, &view);
            }
            Action::CenterArroundMainMarkIfOffScreen => {
                let trigger = Vec::new();
                // TODO: transform all cb to &trigger -> Option<&trigger>
                //        put trigger in env ?

                let center = {
                    let v = &mut view.as_ref().borrow();

                    let tm = v.get_mode::<TextMode>("text-mode");
                    let mid = tm.mark_index;
                    let marks = &tm.marks;
                    let offset = marks[mid].offset;
                    let screen = v.screen.read().unwrap();
                    !screen.contains_offset(offset)
                };
                if center {
                    text_mode::center_arround_mark(editor, env, &trigger, &view);
                }
            }
            Action::CenterArround { offset } => {
                // TODO:

                let trigger = Vec::new();
                env.center_offset = Some(*offset);
                text_mode::center_arround_mark(editor, env, &trigger, &view);
            }
            Action::MoveMarksToNextLine => {
                let trigger = Vec::new();
                text_mode::move_marks_to_next_line(editor, env, &trigger, &view);
            }
            Action::MoveMarksToPreviousLine => {}
            Action::MoveMarkToNextLine { idx } => {
                move_mark_to_next_line(env, view, *idx);
                env.cur_mark_index = None;
            }
            Action::MoveMarkToPreviousLine { idx: _usize } => {}

            Action::CheckMarks => {
                let v = &mut view.as_ref().borrow_mut();
                let tm = v.get_mode_mut::<TextMode>("text-mode");
                tm.marks.dedup();
                tm.mark_index = tm.marks.len().saturating_sub(1);
            }

            Action::SaveMarks => {
                let v = &mut view.as_ref().borrow_mut();
                let tm = v.get_mode_mut::<TextMode>("text-mode");

                //
                tm.marks.dedup();
                let marks_offsets: Vec<u64> = tm.marks.iter().map(|m| m.offset).collect();

                //
                let doc = v.document.as_ref().unwrap();
                let mut doc = doc.as_ref().borrow_mut();
                doc.tag(env.max_offset, marks_offsets);
            }

            Action::CancelSelection => {
                let v = &mut view.as_ref().borrow_mut();
                let tm = v.get_mode_mut::<TextMode>("text-mode");
                tm.select_point = None;
                env.draw_marks = true;
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

    let tm = v.get_mode::<TextMode>("text-mode");
    let main_mark = tm.marks[tm.mark_index].clone();

    run_view_render_filters_direct(env, &v, v.start_offset, max_offset, &mut screen, main_mark);

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
