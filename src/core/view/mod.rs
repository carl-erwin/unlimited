// Copyright (c) Carl-Erwin Griffith
//
// Permission is hereby granted, free of charge, to any
// person obtaining a copy of this software and associated
// documentation files (the "Software"), to deal in the
// Software without restriction, including without
// limitation the rights to use, copy, modify, merge,
// publish, distribute, sublicense, and/or sell copies of
// the Software, and to permit persons to whom the Software
// is furnished to do so, subject to the following
// conditions:
//
// The above copyright notice and this permission notice
// shall be included in all copies or substantial portions
// of the Software.
//
// THE SOFTWARE IS PROVIDED "AS IS", WITHOUT WARRANTY OF
// ANY KIND, EXPRESS OR IMPLIED, INCLUDING BUT NOT LIMITED
// TO THE WARRANTIES OF MERCHANTABILITY, FITNESS FOR A
// PARTICULAR PURPOSE AND NONINFRINGEMENT. IN NO EVENT
// SHALL THE AUTHORS OR COPYRIGHT HOLDERS BE LIABLE FOR ANY
// CLAIM, DAMAGES OR OTHER LIABILITY, WHETHER IN AN ACTION
// OF CONTRACT, TORT OR OTHERWISE, ARISING FROM, OUT OF OR
// IN CONNECTION WITH THE SOFTWARE OR THE USE OR OTHER

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

//
use crate::core::server::EditorEnv;
use crate::dbg_println;

use crate::core::document::Document;

use crate::core::screen::Screen;

use crate::core::mark::Mark;

use crate::core::codec::text::utf8;
use crate::core::codepointinfo;

use crate::core::event::ButtonEvent;
use crate::core::event::InputEvent;
use crate::core::event::KeyModifiers;

use crate::core::view::layout::{run_view_layout_filters, run_view_layout_filters_direct};

pub type Id = u64;

pub mod layout;

pub type ModeFunction = fn(trigger: &Vec<InputEvent>, view: &Rc<RefCell<View>>) -> (); // () for now

// let ptr : ModeFunction = cancel_input(trigger: &Vec<input_event>, doc: &mut Doc, view: &mut View)

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

// TODO:
// add struct to map view["mode(n)"] -> data
// add struct to map doc["mode(n)"]  -> data: ex: line index

/// The **View** represents a way to represent a given Document.<br/>
// TODO: find a way to have marks as plugin.<br/>
// in future version marks will be stored in buffer meta data.<br/>
#[derive(Debug)]
pub struct View<'a> {
    pub id: Id,

    // TODO: add struct to map view["mode(n)"] -> mode(n).data
    // reorder fields
    pub start_offset: u64,
    pub end_offset: u64,
    pub center_on_cursor_move: bool,

    pub document: Option<Rc<RefCell<Document<'a>>>>,
    pub screen: Box<Screen>,

    pub moving_marks: Rc<RefCell<Vec<Mark>>>,
    pub fixed_marks: Rc<RefCell<Vec<Mark>>>,
    // use for cut and paste
    pub last_cut_log_index: Option<usize>,
}

impl<'a> View<'a> {
    /// Create a new View at a gin offset in the Document.<br/>
    pub fn new(
        id: Id,
        start_offset: u64,
        width: usize,
        height: usize,
        document: Option<Rc<RefCell<Document>>>,
    ) -> View {
        let screen = Box::new(Screen::new(width, height));

        // TODO: in future version will be stored in buffer meta data
        let moving_marks = Rc::new(RefCell::new(vec![Mark { offset: 0 }]));

        View {
            id,
            start_offset,
            end_offset: start_offset,     // will be recomputed later
            center_on_cursor_move: false, // add movement enums and pass it to center fn
            document,
            screen,
            moving_marks,
            fixed_marks: Rc::new(RefCell::new(Vec::new())),
            last_cut_log_index: None,
        }
    }

    pub fn check_invariants(&self) {
        self.screen.check_invariants();
    }

    /* TODO: use nb_lines
     to compute previous screen height
     new_h = screen.wheight + (nb_lines * screen.width * max_codec_encode_size)
    */
    pub fn scroll_up(&mut self, nb_lines: usize) {
        if self.start_offset == 0 || nb_lines == 0 {
            return;
        }

        // DUMB: version
        // NEW: first try to check nb_lines in the same area
        // repeat mark moves
        if false {
            let mut tmp = Mark::new(self.start_offset);

            for _ in 0..nb_lines {
                if tmp.offset == 0 {
                    break;
                }
                tmp.offset -= 1;
                let doc = self.document.as_mut().unwrap().borrow_mut();
                tmp.move_to_beginning_of_line(&doc.buffer, utf8::get_prev_codepoint);
            }

            self.start_offset = tmp.offset;

            // TODO: render screen here
            // if not aligned full rebuild etc...
            // diff tmp stat > s.width s.height
            return;
        }

        ////
        let width = self.screen.width();
        let height = self.screen.height() + nb_lines;

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
            m.move_to_beginning_of_line(&doc.buffer, utf8::get_prev_codepoint);
        }

        // build tmp screens until first offset of the original screen if found
        // build_screen from this offset
        // the window MUST cover to screen => height * 2
        // TODO: always in last index ?
        let lines = self.get_lines_offsets_direct(m.offset, offset_to_find, width, height);

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

    pub fn scroll_down(&mut self, nb_lines: usize) {
        // nothing to do :-( ?
        if nb_lines == 0 {
            return;
        }

        let max_offset = {
            let doc = self.document.as_mut().unwrap().borrow_mut();
            doc.buffer.size as u64
        };

        // avoid useless scroll
        if self.screen.contains_offset(max_offset) {
            return;
        }

        if nb_lines >= self.screen.height() {
            // slower : call layout builder to build  nb_lines - screen.height()
            self.scroll_down_offscreen(max_offset, nb_lines);
            return;
        }

        // just read the current screen
        if let (Some(l), _) = self.screen.get_used_line_clipped(nb_lines) {
            if let Some(cpi) = l.get_first_cpi() {
                // set first offset of screen.line[nb_lines] as next screen start
                self.start_offset = cpi.offset;
            }
        } else {
            panic!();
        }
    }

    fn scroll_down_offscreen(&mut self, max_offset: u64, nb_lines: usize) {
        // will be slower than just reading the current screen

        let screen_width = self.screen.width();
        let screen_height = self.screen.height() + 32;

        let start_offset = self.start_offset;
        let end_offset = ::std::cmp::min(
            self.start_offset + (4 * nb_lines * screen_width) as u64,
            max_offset,
        );

        let lines =
            self.get_lines_offsets_direct(start_offset, end_offset, screen_width, screen_height);

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
            // get beginning of the line @offset
            m.move_to_beginning_of_line(&doc.buffer, utf8::get_prev_codepoint);
            doc.buffer.size as u64
        };

        // and build tmp screens until end_offset if found
        let screen_width = ::std::cmp::max(1, screen_width);
        let screen_height = ::std::cmp::max(4, screen_height);
        let mut screen = Screen::new(screen_width, screen_height);

        loop {
            let _ = run_view_layout_filters_direct(&self, m.offset, max_offset, &mut screen);

            if screen.nb_push == 0 {
                return v;
            }

            // push lines offsets
            // FIXME: find a better way to iterate over the used lines
            for i in 0..screen.current_line_index {
                if !v.is_empty() && i == 0 {
                    // do not push line range twice
                    continue;
                }

                let s = screen.line[i].get_first_cpi().unwrap().offset;
                let e = screen.line[i].get_last_cpi().unwrap().offset;

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
                    .offset;

                let e = screen.line[screen.current_line_index]
                    .get_last_cpi()
                    .unwrap()
                    .offset;
                v.push((s, e));
                return v;
            }

            // TODO: activate only in debug builds
            if 0 == 1 {
                match screen.find_cpi_by_offset(m.offset) {
                    (Some(cpi), x, y) => {
                        assert_eq!(x, 0);
                        assert_eq!(y, 0);
                        assert_eq!(cpi.offset, m.offset);
                    }
                    _ => panic!("implementation error"),
                }
            }

            if let Some(l) = screen.get_last_used_line() {
                if let Some(cpi) = l.get_first_cpi() {
                    m.offset = cpi.offset; // update next screen start
                }
            }

            screen.clear(); // prepare next screen
        }
    }

    pub fn center_arround_offset(&mut self, offset: u64) {
        self.start_offset = offset;
        let h = self.screen.height() / 2;
        self.scroll_up(h);
    }
} // View

/// This function computes start/end of lines between start_offset end_offset.<br/>
/// It (will) run the configured filters/plugins.<br/>
/// using the run_view_layout_filters function until end_offset is reached.<br/>
pub fn get_lines_offsets(
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

    // get beginning of the line @offset
    m.move_to_beginning_of_line(&doc.buffer, utf8::get_prev_codepoint);

    let max_offset = doc.buffer.size as u64;

    // and build tmp screens until end_offset if found
    let screen_width = ::std::cmp::max(1, screen_width);
    let screen_height = ::std::cmp::max(4, screen_height);
    let mut screen = Screen::new(screen_width, screen_height);

    loop {
        let _ = run_view_layout_filters(&view, m.offset, max_offset, &mut screen);
        if screen.nb_push == 0 {
            return v;
        }

        // push lines offsets
        // FIXME: find a better way to iterate over the used lines
        for i in 0..screen.current_line_index {
            if !v.is_empty() && i == 0 {
                // do not push line range twice
                continue;
            }

            let s = screen.line[i].get_first_cpi().unwrap().offset;
            let e = screen.line[i].get_last_cpi().unwrap().offset;

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
                .offset;

            let e = screen.line[screen.current_line_index]
                .get_last_cpi()
                .unwrap()
                .offset;
            v.push((s, e));
            return v;
        }

        // TODO: activate only in debug builds
        if 0 == 1 {
            match screen.find_cpi_by_offset(m.offset) {
                (Some(cpi), x, y) => {
                    assert_eq!(x, 0);
                    assert_eq!(y, 0);
                    assert_eq!(cpi.offset, m.offset);
                }
                _ => panic!("implementation error"),
            }
        }

        if let Some(l) = screen.get_last_used_line() {
            if let Some(cpi) = l.get_first_cpi() {
                m.offset = cpi.offset; // update next screen start
            }
        }

        screen.clear(); // prepare next screen
    }
}

// TODO: test-mode
// scroll bar: bg color (35, 34, 89)
// scroll bar: cursor color (192, 192, 192)
pub fn update_view(view: &Rc<RefCell<View>>, _env: &mut EditorEnv) {
    let mut v = view.as_ref().borrow_mut();

    let doc = v.document.clone();
    let doc = doc.as_ref();

    // TODO: transform read as filter pass
    if let Some(ref doc) = doc {
        // 1st pass raw_data_filter
        let max_offset = { doc.borrow().buffer.size as u64 };

        let mut screen = Box::new(Screen::new(v.screen.width(), v.screen.height()));

        let end_offset =
            run_view_layout_filters_direct(&v, v.start_offset, max_offset, &mut screen);

        // TODO: from env ?
        v.end_offset = end_offset;
        v.screen = screen; // move v.screen to view double buffer  v.screen_get() v.screen_swap(new: move)
        v.check_invariants();

        // TODO: marks_filter
        // set_render_marks
        // brute force for now
        let marks = v.moving_marks.clone(); // do not hold v
        for m in marks.borrow().iter() {
            // TODO: screen.find_line_by_offset(m.offset) -> Option<&mut Line>
            if m.offset < v.start_offset || m.offset > v.end_offset {
                continue;
            }

            for l in 0..v.screen.height() {
                let line = v.screen.get_mut_line(l).unwrap();

                if line.metadata {
                    // continue;
                }

                for c in 0..line.nb_cells {
                    let cpi = line.get_mut_cpi(c).unwrap();
                    cpi.is_selected = cpi.offset == m.offset && !cpi.metadata;
                }
            }
        }
    }
}

///
use crate::core::event::Key;

// CEG
pub fn scroll_up(_trigger: &Vec<InputEvent>, view: &Rc<RefCell<View>>) {
    // TODO: 3 is from mode configuration
    // env["default-scroll-size"] -> int

    view.as_ref().borrow_mut().scroll_up(3);
}

pub fn scroll_down(_trigger: &Vec<InputEvent>, view: &Rc<RefCell<View>>) {
    // TODO: 3 is from mode configuration
    // env["default-scroll-size"] -> int

    view.as_ref().borrow_mut().scroll_down(3);
}

// TODO: rename into insert_input_event
/// Insert an array of unicode code points using hardcoded utf8 codec.<br/>
pub fn insert_codepoint_array(trigger: &Vec<InputEvent>, view: &Rc<RefCell<View>>) {
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

    let mut utf8 = Vec::with_capacity(array.len());
    for codepoint in array {
        let mut data: &mut [u8; 4] = &mut [0, 0, 0, 0];
        let data_size = utf8::encode(*codepoint as u32, &mut data);
        for d in data.iter().take(data_size) {
            utf8.push(*d);
        }
    }

    let mut offset: u64 = 0;

    {
        let v = view.as_ref().borrow_mut();
        let mut doc = v.document.as_ref().unwrap().borrow_mut();

        for m in v.moving_marks.borrow_mut().iter_mut() {
            doc.insert(m.offset, utf8.len(), &utf8);
            m.offset += utf8.len() as u64;
            offset = m.offset;
            break;
        }
    }

    let center = {
        let v = view.as_ref().borrow();
        // move to upper layer
        offset < v.screen.first_offset
            || offset > v.screen.last_offset
            || array.len() > v.screen.width() * v.screen.height()
    };

    if center {
        center_arround_mark(&trigger, &view);
    }
}

/// Undo the previous write operation and sync the screen around the main mark.<br/>
pub fn undo(trigger: &Vec<InputEvent>, view: &Rc<RefCell<View>>) {
    let mut sync_view = false;

    // hack no multicursor for now
    {
        let v = &mut view.as_ref().borrow_mut();

        let doc = v.document.as_ref().unwrap();
        let mut doc = doc.as_ref().borrow_mut();

        if let Some(off) = doc.undo() {
            for m in v.moving_marks.borrow_mut().iter_mut() {
                m.offset = off;
                break;
            }

            sync_view = !v.screen.contains_offset(off);
        }
    }

    if sync_view {
        center_arround_mark(&trigger, &view);
    }
}

/// Redo the previous write operation and sync the screen around the main mark.<br/>
pub fn redo(trigger: &Vec<InputEvent>, view: &Rc<RefCell<View>>) {
    let mut sync_view = false;

    // hack no multicursor for now
    {
        let v = &mut view.as_ref().borrow_mut();

        let doc = v.document.as_ref().unwrap();
        let mut doc = doc.as_ref().borrow_mut();

        if let Some(off) = doc.redo() {
            for m in v.moving_marks.borrow_mut().iter_mut() {
                m.offset = off;
                break;
            }

            sync_view = !v.screen.contains_offset(off);
        }
    }

    if sync_view {
        center_arround_mark(&trigger, &view);
    }
}

/// Remove the current utf8 encoded code point.<br/>
pub fn remove_codepoint(_trigger: &Vec<InputEvent>, view: &Rc<RefCell<View>>) {
    let v = &mut view.as_ref().borrow_mut();

    let doc = v.document.as_ref().unwrap();
    let mut doc = doc.as_ref().borrow_mut();

    for m in v.moving_marks.borrow_mut().iter_mut() {
        let mut data = Vec::with_capacity(4);
        doc.buffer.read(m.offset, data.capacity(), &mut data);
        let (_, _, size) = utf8::get_codepoint(&data, 0);
        doc.remove(m.offset, size as usize, None);
    }
}

/// Skip blanks (if any) and remove until end of the word.
/// TODO: handle ',' | ';' | '(' | ')' | '{' | '}'
pub fn remove_until_end_of_word(_trigger: &Vec<InputEvent>, view: &Rc<RefCell<View>>) {
    let v = &mut view.as_ref().borrow_mut();

    let doc = v.document.as_ref().unwrap();
    let mut doc = doc.as_ref().borrow_mut();

    for m in v.moving_marks.borrow_mut().iter_mut() {
        let start = m.clone();

        let mut data = Vec::with_capacity(4);

        // skip blanks until any char or end-of-line
        loop {
            data.clear();
            doc.buffer.read(m.offset, data.capacity(), &mut data);
            let (cp, _, size) = utf8::get_codepoint(&data, 0);

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
            doc.buffer.read(m.offset, data.capacity(), &mut data);
            let (cp, _, size) = utf8::get_codepoint(&data, 0);

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
        doc.remove(start.offset, (m.offset - start.offset) as usize, None);

        m.offset = start.offset;
        break; // no multicursor
    }
}

// TODO: maintain main mark Option<(x,y)>
pub fn move_marks_backward(trigger: &Vec<InputEvent>, view: &Rc<RefCell<View>>) {
    let mut scroll_needed = false;

    {
        let v = &view.as_ref().borrow();
        let doc = v.document.as_ref().unwrap().borrow();

        for m in v.moving_marks.borrow_mut().iter_mut() {
            // TODO: add main mark check
            if m.offset <= v.start_offset {
                scroll_needed = true;
            }

            m.move_backward(&doc.buffer, utf8::get_previous_codepoint_start);
        }
    }

    if scroll_needed {
        view.as_ref().borrow_mut().scroll_up(1);
    }

    if view.as_ref().borrow_mut().center_on_cursor_move {
        center_arround_mark(&trigger, &view);
    }
}

pub fn move_marks_forward(trigger: &Vec<InputEvent>, view: &Rc<RefCell<View>>) {
    let mut scroll_needed = false;

    {
        let v = &view.as_ref().borrow();
        let doc = v.document.as_ref().unwrap().borrow();

        for m in v.moving_marks.borrow_mut().iter_mut() {
            // TODO: add main mark check
            if m.offset >= view.as_ref().borrow().end_offset {
                scroll_needed = true;
            }

            m.move_forward(&doc.buffer, utf8::get_next_codepoint_start);
        }
    }

    if scroll_needed {
        view.as_ref().borrow_mut().scroll_down(1);
    }

    if view.as_ref().borrow_mut().center_on_cursor_move {
        center_arround_mark(&trigger, &view);
    }
}

pub fn move_marks_to_beginning_of_line(_trigger: &Vec<InputEvent>, view: &Rc<RefCell<View>>) {
    let v = &view.as_ref().borrow_mut();

    let doc = v.document.as_ref().unwrap().borrow();

    for m in v.moving_marks.borrow_mut().iter_mut() {
        m.move_to_beginning_of_line(&doc.buffer, utf8::get_prev_codepoint);
    }
}

pub fn move_marks_to_end_of_line(_trigger: &Vec<InputEvent>, view: &Rc<RefCell<View>>) {
    let v = view.as_ref().borrow();
    let doc = v.document.as_ref().unwrap().borrow();
    for m in v.moving_marks.borrow_mut().iter_mut() {
        m.move_to_end_of_line(&doc.buffer, utf8::get_codepoint);
    }
}

pub fn move_marks_to_previous_line(trigger: &Vec<InputEvent>, view: &Rc<RefCell<View>>) {
    let mut mark_moved = false;
    let mut center_on_cursor_move = false;

    {
        let moving_marks = {
            let v = &mut view.as_ref().borrow_mut();
            center_on_cursor_move = v.center_on_cursor_move;
            v.moving_marks.clone()
        };

        for m in moving_marks.borrow_mut().iter_mut() {
            {
                let v = &mut view.as_ref().borrow_mut();

                // TODO: if v.is_mark_on_screen(m) -> (bool, x, y) ?
                match v.screen.find_cpi_by_offset(m.offset) {
                    // offscreen
                    (None, _, _) => {}
                    // mark on first line
                    (Some(_), _, y) if y == 0 => {
                        // previous line is offscreen
                    }

                    // onscreen
                    (Some(_), x, y) if y > 0 => {
                        // TODO: refactor code to support screen cell metadata
                        let new_y = y - 1; // select previous line
                        let l = v.screen.get_line(new_y).unwrap();
                        // previous line is filled ?
                        if l.nb_cells > 0 {
                            let new_x = ::std::cmp::min(x, l.nb_cells - 1);
                            let cpi = v.screen.get_cpinfo(new_x, new_y).unwrap();
                            if !cpi.metadata {
                                m.offset = cpi.offset;
                                mark_moved = true;
                            }
                        } else {
                            // ???
                        }
                    }

                    // impossible
                    _ => {}
                }
            }

            // offscreen
            if !mark_moved {
                // mark is offscreen

                let end_offset = m.offset;
                let (start_offset, screen_width, screen_height) = {
                    let v = &mut view.as_ref().borrow_mut();

                    let start_offset = {
                        let doc = v.document.as_ref().unwrap();
                        let doc = doc.as_ref().borrow();

                        // todo: set marks codecs
                        let mut tmp = m.clone();

                        // goto beginning of current line (mar is on first line of screen)
                        tmp.move_to_beginning_of_line(&doc.buffer, utf8::get_prev_codepoint);
                        // goto end of previous line
                        tmp.move_backward(&doc.buffer, utf8::get_previous_codepoint_start);
                        // goto beginning of previous line
                        tmp.move_to_beginning_of_line(&doc.buffer, utf8::get_prev_codepoint);
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

                    let width = v.screen.width();

                    let add_height = if width > 0 {
                        (m.offset - start_offset) as usize / width
                    } else {
                        1
                    };
                    let height = v.screen.height() + (add_height * 4); // 4 is utf8 max encode size

                    (start_offset, width, height)
                };

                // TODO: loop until m.offset is on screen

                let lines = {
                    let mut view = view.as_ref().borrow_mut();
                    view.get_lines_offsets_direct(
                        start_offset,
                        end_offset,
                        screen_width,
                        screen_height,
                    )
                };

                // find "previous" line index
                let index = match lines
                    .iter()
                    .position(|e| e.0 <= end_offset && end_offset <= e.1)
                {
                    None | Some(0) => continue,
                    Some(i) => i - 1,
                };

                let line_start_off = lines[index].0;
                let line_end_off = lines[index].1;

                // update screen start
                {
                    let v = &mut view.as_ref().borrow_mut();
                    v.start_offset = line_start_off;
                }

                let mut tmp_mark = Mark::new(line_start_off);

                // compute column
                let new_x = {
                    let doc = &view.as_ref().borrow();
                    let doc = doc.document.as_ref().unwrap();
                    let doc = doc.as_ref().borrow();

                    let mut s = Mark::new(lines[index + 1].0);
                    let e = Mark::new(lines[index + 1].1);
                    let mut count = 0;
                    while s.offset != e.offset {
                        if s.offset == m.offset {
                            break;
                        }

                        s.move_forward(&doc.buffer, utf8::get_next_codepoint_start);
                        count += 1;
                    }
                    count
                };

                {
                    let doc = &view.as_ref().borrow();
                    let doc = doc.document.as_ref().unwrap();
                    let doc = doc.as_ref().borrow();

                    for _ in 0..new_x {
                        tmp_mark.move_forward(&doc.buffer, utf8::get_next_codepoint_start);
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
                if tmp_mark.offset < m.offset {
                    m.offset = tmp_mark.offset;
                }
            }
        }
    }

    // post action
    if center_on_cursor_move {
        center_arround_mark(&trigger, &view);
    }
}

// remove multiple borrows
pub fn move_marks_to_next_line(_trigger: &Vec<InputEvent>, view: &Rc<RefCell<View>>) {
    let max_offset = {
        let doc = &view.as_ref().borrow();
        let doc = doc.document.as_ref().unwrap();
        let doc = doc.as_ref().borrow();
        doc.buffer.size as u64
    };

    let mut scroll_needed = false;

    let mut is_offscreen = true;
    let mut was_on_screen = false;

    let moving_marks = view.as_ref().borrow_mut().moving_marks.clone();

    for m in moving_marks.borrow_mut().iter_mut() {
        // TODO: m.on_buffer_end() ?
        if m.offset == max_offset {
            if !view.as_ref().borrow_mut().screen.contains_offset(m.offset) {
                scroll_needed = true;
            }
            continue;
        }

        is_offscreen = true;
        was_on_screen = false;

        if view.as_ref().borrow_mut().screen.contains_offset(m.offset) {
            was_on_screen = true;

            // yes get coordinates
            let (_, x, y) = view
                .as_ref()
                .borrow_mut()
                .screen
                .find_cpi_by_offset(m.offset);

            if y < view.as_ref().borrow_mut().screen.height() - 1 {
                is_offscreen = false;

                let new_y = y + 1;
                let v = view.as_ref().borrow_mut();
                let l = v.screen.get_line(new_y).unwrap();
                if l.nb_cells > 0 {
                    let new_x = ::std::cmp::min(x, l.nb_cells - 1);
                    let cpi = v.screen.get_cpinfo(new_x, new_y).unwrap();
                    if !cpi.metadata {
                        m.offset = cpi.offset;
                    }
                }
            } else {
                scroll_needed = true;
            }
        }

        if is_offscreen {
            // mark is offscreen
            let screen_width = view.as_ref().borrow_mut().screen.width();
            let screen_height = view.as_ref().borrow_mut().screen.height();

            // get start_of_line(m.offset) -> u64
            let start_offset = {
                let doc = &view.as_ref().borrow();
                let doc = doc.document.as_ref().unwrap();
                let doc = doc.as_ref().borrow();

                let mut tmp = Mark::new(m.offset);
                tmp.move_to_beginning_of_line(&doc.buffer, utf8::get_prev_codepoint);
                tmp.offset
            };

            // a codepoint can use 4 bytes the virtual end is
            // + 1 full line away
            let end_offset = ::std::cmp::min(m.offset + (4 * screen_width) as u64, max_offset);

            // get lines start, end offset
            let lines = {
                let mut view = view.as_ref().borrow_mut();
                view.get_lines_offsets_direct(start_offset, end_offset, screen_width, screen_height)
            };

            // find the cursor index
            let index = match lines
                .iter()
                .position(|e| e.0 <= m.offset && m.offset <= e.1)
            {
                None => continue,
                Some(i) => {
                    if i == lines.len() - 1 {
                        continue;
                    } else {
                        i
                    }
                }
            };

            // compute column
            let new_x = {
                let doc = &view.as_ref().borrow();
                let doc = doc.document.as_ref().unwrap();
                let doc = doc.as_ref().borrow();

                let mut s = Mark::new(lines[index].0);
                let e = Mark::new(lines[index].1);
                let mut count = 0;
                while s.offset < e.offset {
                    if s.offset == m.offset {
                        break;
                    }

                    s.move_forward(&doc.buffer, utf8::get_next_codepoint_start);
                    count += 1;
                }
                count
            };

            // get next line start/end offsets
            let next_index = index + 1;
            let line_start_off = lines[next_index].0;
            let line_end_off = lines[next_index].1;

            let mut tmp_mark = Mark::new(line_start_off);

            let doc = &view.as_ref().borrow();
            let doc = doc.document.as_ref().unwrap();
            let doc = doc.as_ref().borrow();
            for _ in 0..new_x {
                tmp_mark.move_forward(&doc.buffer, utf8::get_next_codepoint_start);
            }

            if tmp_mark.offset > line_end_off {
                tmp_mark.offset = line_end_off;
            }

            m.offset = tmp_mark.offset;
        }

        if m.offset >= view.as_ref().borrow_mut().end_offset {
            scroll_needed = true;
        }
    }

    if is_offscreen && was_on_screen || scroll_needed {
        view.as_ref().borrow_mut().scroll_down(1);
    }

    // if view.as_ref().borrow_mut().center_on_cursor_move {
    //     center_arround_mark(&trigger, &view);
    // }
}

pub fn move_mark_to_screen_start(_trigger: &Vec<InputEvent>, view: &Rc<RefCell<View>>) {
    let v = view.as_ref().borrow();

    for m in v.moving_marks.borrow_mut().iter_mut() {
        // TODO: add main mark check
        if m.offset < v.start_offset || m.offset > v.end_offset {
            m.offset = v.start_offset;
        }
    }
}

pub fn move_mark_to_screen_end(_trigger: &Vec<InputEvent>, view: &Rc<RefCell<View>>) {
    let v = view.as_ref().borrow();
    for m in v.moving_marks.borrow_mut().iter_mut() {
        // TODO: add main mark check
        if m.offset < v.start_offset || m.offset > v.end_offset {
            m.offset = v.end_offset;
        }
    }
}

pub fn scroll_to_previous_screen(trigger: &Vec<InputEvent>, view: &Rc<RefCell<View>>) {
    {
        let mut v = view.as_ref().borrow_mut();
        let nb = ::std::cmp::max(v.screen.height() - 1, 1);
        v.scroll_up(nb);
    }

    // TODO: add hints to trigger mar moves
    move_mark_to_screen_end(&trigger, &view);
}

pub fn move_mark_to_beginning_of_file(_trigger: &Vec<InputEvent>, view: &Rc<RefCell<View>>) {
    let mut v = view.as_ref().borrow_mut();
    for m in v.moving_marks.borrow_mut().iter_mut() {
        m.offset = 0;
        break;
    }

    v.start_offset = 0;
}

// TODO: view.center_arrout_offset()
pub fn center_arround_mark(_trigger: &Vec<InputEvent>, view: &Rc<RefCell<View>>) {
    let mut v = view.as_ref().borrow_mut();

    let marks = v.moving_marks.clone();
    for m in marks.borrow_mut().iter_mut() {
        v.start_offset = m.offset;
        break;
    }

    let h = v.screen.height() / 2;
    v.scroll_up(h);
}

pub fn move_mark_to_end_of_file(_trigger: &Vec<InputEvent>, view: &Rc<RefCell<View>>) {
    let mut v = view.as_ref().borrow_mut();

    let size = {
        let doc = v.document.as_ref().unwrap();
        let doc = doc.as_ref().borrow();
        doc.buffer.size
    };

    for m in v.moving_marks.borrow_mut().iter_mut() {
        m.offset = size as u64;
        break;
    }

    v.start_offset = size as u64;
    let h = v.screen.height() / 2;
    v.scroll_up(h);
}

pub fn scroll_to_next_screen(trigger: &Vec<InputEvent>, view: &Rc<RefCell<View>>) {
    {
        let mut v = view.as_ref().borrow_mut();
        let nb = ::std::cmp::max(v.screen.height() - 1, 1);
        v.scroll_down(nb);
    }
    move_mark_to_screen_start(&trigger, view);
}

pub fn save_document(_trigger: &Vec<InputEvent>, view: &Rc<RefCell<View>>) {
    let v = view.as_ref().borrow_mut();
    let doc = v.document.as_ref().unwrap();
    let mut doc = doc.as_ref().borrow_mut();

    let _ = doc.sync_to_disk().is_ok(); // ->  operation ok
}

pub fn cut_to_end_of_line(_trigger: &Vec<InputEvent>, view: &Rc<RefCell<View>>) {
    let v = &mut view.as_ref().borrow_mut();

    let pos = {
        let doc = v.document.as_ref().unwrap();
        let mut doc = doc.as_ref().borrow_mut();

        for m in v.moving_marks.borrow().iter() {
            let mut end = m.clone();
            end.move_to_end_of_line(&doc.buffer, utf8::get_codepoint);
            doc.remove(m.offset, (end.offset - m.offset) as usize, None);
            break;
        }

        doc.buffer_log.pos
    };

    // save buffer log idx
    assert!(pos > 0);
    v.last_cut_log_index = Some(pos - 1);
}

pub fn paste(_trigger: &Vec<InputEvent>, view: &Rc<RefCell<View>>) {
    let v = &mut view.as_ref().borrow();

    if let Some(idx) = v.last_cut_log_index {
        let doc = v.document.as_ref().unwrap();
        let mut doc = doc.as_ref().borrow_mut();

        let tr = doc.buffer_log.data[idx].clone();

        for m in v.moving_marks.borrow_mut().iter_mut() {
            let mut end = m.clone();
            end.move_to_end_of_line(&doc.buffer, utf8::get_codepoint);
            doc.insert(m.offset, tr.data.len(), tr.data.as_slice());
            m.offset += tr.data.len() as u64;
            break;
        }

    // true
    } else {
        // false
    }
}

pub fn move_to_prev_token_start(_trigger: &Vec<InputEvent>, view: &Rc<RefCell<View>>) {
    {
        // TODO: factorize macrk action
        // mark.apply(fn); where fn=m.move_to_next_token_end(&doc.buffer, utf8::get_codepoint);
        //
        let v = &mut view.as_ref().borrow();
        let doc = v.document.as_ref().unwrap();
        let doc = doc.as_ref().borrow();

        for m in v.moving_marks.borrow_mut().iter_mut() {
            let _end = m.clone();
            m.move_to_next_token_end(&doc.buffer, utf8::get_codepoint);
            break;
        }
    }
}

pub fn move_to_next_token_end(_trigger: &Vec<InputEvent>, view: &Rc<RefCell<View>>) {
    {
        let v = &mut view.as_ref().borrow();
        let doc = v.document.as_ref().unwrap();
        let doc = doc.as_ref().borrow();

        for m in v.moving_marks.borrow_mut().iter_mut() {
            //            let mut end = m.clone();
            m.move_to_next_token_end(&doc.buffer, utf8::get_codepoint);
            break;
        }
    }
}

pub fn button_press(trigger: &Vec<InputEvent>, view: &Rc<RefCell<View>>) {
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

    let mut s = String::new();

    s.push_str(&format!("clip : {:?}", v.screen.clip_rect()));

    match button {
        0 => {}
        _ => {
            return;
        }
    }

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

    // move cursor to (x,y)
    let (mut x, mut y) = (x as usize, y as usize);

    // 0 <= x < screen.width()
    if x < v.screen.clip_rect().x {
        x = 0;
    } else if x >= v.screen.clip_rect().x + v.screen.clip_rect().width {
        x = v.screen.clip_rect().width - 1;
    } else {
        x -= v.screen.clip_rect().x;
    }

    // 0 <= y < screen.height()
    if y < v.screen.clip_rect().y {
        y = 0;
        s.push_str(", y case 1");
    } else if y > v.screen.clip_rect().y + v.screen.clip_rect().height {
        y = v.screen.clip_rect().height - 1;
        s.push_str(", y case 2");
    } else {
        y -= v.screen.clip_rect().y;
        s.push_str(", y case 3");
    }

    //
    let _max_offset = v.screen.doc_max_offset;

    let last_li = v.screen.get_last_used_line_index();
    if y >= last_li {
        if last_li >= v.screen.height() {
            y = v.screen.height() - 1;
        } else {
            y = last_li;
        }
        s.push_str(", y >= last_li");
    }

    if let Some(l) = v.screen.get_line(y) {
        s.push_str(&format!(", get line ok , x:{}, nbcells:{}", x, l.nb_cells));

        if l.nb_cells > 0 && x > l.nb_cells {
            x = l.nb_cells - 1;
        } else if l.nb_cells == 0 {
            x = 0;
        }
    } else {
        s.push_str(", get line failed");
    }

    s.push_str(&format!(", new (x:{},y:{})", x, y));

    if let Some(cpi) = v.screen.get_used_cpinfo(x, y) {
        if !cpi.metadata {
            for m in v.moving_marks.borrow_mut().iter_mut() {
                m.offset = cpi.offset;
                // we only move one mark
                break; // TODO: add main mark ref
            }
        }
    }

    // s // to internal view.as_ref().borrow_mut().state.s
}

pub fn remove_previous_codepoint(_trigger: &Vec<InputEvent>, view: &Rc<RefCell<View>>) {
    let mut scroll_needed = false;

    let v = &mut view.as_ref().borrow_mut();
    {
        let doc = v.document.clone(); // TODO: use Option<clone> to release imut boorow of v
        let doc = doc.as_ref().clone().unwrap();
        let mut doc = doc.as_ref().borrow_mut();

        for m in v.moving_marks.borrow_mut().iter_mut() {
            if m.offset == 0 {
                continue;
            }

            m.move_backward(&doc.buffer, utf8::get_previous_codepoint_start);

            let mut data = vec![];
            doc.buffer.read(m.offset, 4, &mut data);
            let (_, _, size) = utf8::get_codepoint(&data, 0);
            doc.remove(m.offset, size, None);

            if m.offset < v.start_offset {
                scroll_needed = true;
            }
        }

        if scroll_needed {
            v.scroll_up(0); // resync merged line
        }
    }
}

/// Insert a single unicode code point using hardcoded utf8 codec.<br/>
pub fn insert_codepoint(trigger: &Vec<InputEvent>, view: &Rc<RefCell<View>>) {
    let codepoint = match trigger[0] {
        InputEvent::KeyPress {
            mods:
                KeyModifiers {
                    ctrl: false,
                    alt: false,
                    shift: false,
                },
            key: Key::Unicode(cp),
        } => cp,
        _ => {
            return;
        }
    };

    let mut scroll_needed = false;
    let mut sync_view = false;

    {
        let mut data: &mut [u8; 4] = &mut [0, 0, 0, 0];
        let data_size = utf8::encode(codepoint as u32, &mut data);

        let v = &mut view.as_ref().borrow(); // TODO: we can remove borrow_mut()

        let doc = v.document.clone(); // TODO: use Option<clone> to release imut boorow of v
        let doc = doc.as_ref().clone().unwrap();
        let mut doc = doc.as_ref().borrow_mut();

        for m in v.moving_marks.borrow_mut().iter_mut() {
            // TODO: add main mark check
            let (_, _, y) = v.screen.find_cpi_by_offset(m.offset);
            if y == v.screen.height() - 1 && codepoint == '\n' {
                scroll_needed = true;
            }

            doc.insert(m.offset, data_size, data);
            m.offset += data_size as u64;

            sync_view = !v.screen.contains_offset(m.offset);
        }
    }

    // replace trigger/view args by env:hints
    // if env::nr_pending_events > 1 {
    //    return;
    // }

    if scroll_needed {
        // build args
        // Option<> args = 1
        scroll_down(&trigger, view);
    } else if sync_view {
        center_arround_mark(&trigger, view);
    }
}

pub fn button_release(trigger: &Vec<InputEvent>, _view: &Rc<RefCell<View>>) {
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

// TODO: screen_putstr_with_attr metadat etc ...
// return array of built &cpi ? to allow attr changes pass ?
pub fn screen_putstr(mut screen: &mut Screen, s: &str) -> bool {
    for c in s.chars() {
        let ok = screen_putchar(&mut screen, c, 0xffff_ffff_ffff_ffff);
        if !ok {
            return false;
        }
    }

    true
}

pub fn screen_putchar(screen: &mut Screen, c: char, offset: u64) -> bool {
    let (ok, _) = screen.push(layout::filter_codepoint(
        c,
        offset,
        codepointinfo::CodepointInfo::default_color(),
    ));
    ok
}

#[test]
fn test_view() {}
