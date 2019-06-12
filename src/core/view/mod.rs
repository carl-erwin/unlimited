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
use crate::core::document::Document;
use crate::core::screen::Screen;

use crate::core::mark::Mark;

use crate::core::codec::text::utf8;
use crate::core::codepointinfo;

pub type Id = u64;

pub mod layout;

// TODO: add the main mark as a ref
#[derive(Debug)]
pub struct View<'a> {
    pub id: Id,
    pub start_offset: u64,
    pub end_offset: u64,
    pub document: Option<Rc<RefCell<Document<'a>>>>,
    pub screen: Box<Screen>,

    // TODO: in future version marks will be stored in buffer meta data
    pub moving_marks: Rc<RefCell<Vec<Mark>>>,
    pub fixed_marks: Rc<RefCell<Vec<Mark>>>,
    // use for cut and paste
    pub last_cut_log_index: Option<usize>,
}

impl<'a> View<'a> {
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
            end_offset: start_offset, // will be recomputed later
            document,
            screen,
            moving_marks,
            fixed_marks: Rc::new(RefCell::new(Vec::new())),
            last_cut_log_index: None,
        }
    }

    pub fn undo(&mut self) {
        let mut sync_view = false;

        // hack no multicursor for now
        {
            let mut doc = self.document.as_mut().unwrap().borrow_mut();
            if let Some(off) = doc.undo() {
                for m in &mut self.moving_marks.borrow_mut().iter_mut() {
                    m.offset = off;
                    break;
                }

                sync_view = !self.screen.contains_offset(off);
            }
        }

        if sync_view {
            self.center_arround_mark();
        }
    }

    pub fn redo(&mut self) {
        let mut sync_view = false;

        // hack no multicursor for now
        {
            let mut doc = self.document.as_mut().unwrap().borrow_mut();
            if let Some(off) = doc.redo() {
                for m in &mut self.moving_marks.borrow_mut().iter_mut() {
                    m.offset = off;
                    break;
                }

                sync_view = !self.screen.contains_offset(off);
            }
        }

        if sync_view {
            self.center_arround_mark();
        }
    }

    pub fn insert_codepoint_array(&mut self, array: &[char]) {
        let mut utf8 = Vec::with_capacity(array.len());
        for codepoint in array {
            let mut data: &mut [u8; 4] = &mut [0, 0, 0, 0];
            let data_size = utf8::encode(*codepoint as u32, &mut data);
            for i in 0..data_size {
                utf8.push(data[i]);
            }
        }

        let mut offset: u64 = 0;
        {
            let mut doc = self.document.as_mut().unwrap().borrow_mut();
            for m in &mut self.moving_marks.borrow_mut().iter_mut() {
                doc.insert(m.offset, utf8.len(), &utf8);
                m.offset += utf8.len() as u64;
                offset = m.offset;
                break;
            }
        }

        // move to upper layer
        if offset < self.screen.first_offset
            || offset > self.screen.last_offset
            || array.len() > self.screen.width() * self.screen.height()
        {
            self.center_arround_mark();
        }
    }

    pub fn insert_codepoint(&mut self, codepoint: char, nr_pending_events: usize) {
        let mut scroll_needed = false;
        let mut sync_view = false;

        {
            let mut data: &mut [u8; 4] = &mut [0, 0, 0, 0];
            let data_size = utf8::encode(codepoint as u32, &mut data);
            let mut doc = self.document.as_mut().unwrap().borrow_mut();
            for m in &mut self.moving_marks.borrow_mut().iter_mut() {
                // TODO: add main mark check
                let (_, _, y) = self.screen.find_cpi_by_offset(m.offset);
                if y == self.screen.height() - 1 && codepoint == '\n' {
                    scroll_needed = true;
                }

                doc.insert(m.offset, data_size, data);
                m.offset += data_size as u64;

                sync_view = !self.screen.contains_offset(m.offset);
            }
        }

        if nr_pending_events > 1 {
            return;
        }

        if scroll_needed {
            self.scroll_down(1);
        } else if sync_view {
            self.center_arround_mark();
        }
    }

    pub fn remove_codepoint(&mut self) {
        let mut doc = self.document.as_mut().unwrap().borrow_mut();
        for m in &mut self.moving_marks.borrow_mut().iter_mut() {
            let mut data = Vec::with_capacity(4);
            doc.buffer.read(m.offset, data.capacity(), &mut data);
            let (_, _, size) = utf8::get_codepoint(&data, 0);
            doc.remove(m.offset, size as usize, None);
        }
    }

    pub fn remove_until_end_of_word(&mut self) {
        let mut doc = self.document.as_mut().unwrap().borrow_mut();

        for m in &mut self.moving_marks.borrow_mut().iter_mut() {
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

    pub fn remove_previous_codepoint(&mut self) {
        let mut scroll_needed = false;

        {
            let mut doc = self.document.as_mut().unwrap().borrow_mut();
            for m in &mut self.moving_marks.borrow_mut().iter_mut() {
                if m.offset == 0 {
                    continue;
                }

                m.move_backward(&doc.buffer, utf8::get_previous_codepoint_start);

                let mut data = vec![];
                doc.buffer.read(m.offset, 4, &mut data);
                let (_, _, size) = utf8::get_codepoint(&data, 0);
                doc.remove(m.offset, size, None);

                if m.offset < self.start_offset {
                    scroll_needed = true;
                }
            }
        }

        if scroll_needed {
            self.scroll_up(0); // resync merged line
        }
    }

    // TODO: maintain main mark Option<(x,y)>
    pub fn move_marks_backward(&mut self) {
        let mut scroll_needed = false;

        {
            let doc = self.document.as_mut().unwrap().borrow_mut();

            for m in &mut self.moving_marks.borrow_mut().iter_mut() {
                // TODO: add main mark check
                if m.offset <= self.start_offset {
                    scroll_needed = true;
                }

                m.move_backward(&doc.buffer, utf8::get_previous_codepoint_start);
            }
        }

        if scroll_needed {
            self.scroll_up(1);
        }
    }

    pub fn move_marks_forward(&mut self) {
        let mut scroll_needed = false;

        {
            let doc = self.document.as_mut().unwrap().borrow_mut();
            for m in &mut self.moving_marks.borrow_mut().iter_mut() {
                // TODO: add main mark check
                if m.offset >= self.end_offset {
                    scroll_needed = true;
                }

                m.move_forward(&doc.buffer, utf8::get_next_codepoint_start);
            }
        }

        if scroll_needed {
            self.scroll_down(1);
        }
    }

    pub fn move_marks_to_beginning_of_line(&mut self) {
        let doc = self.document.as_mut().unwrap().borrow_mut();
        for m in &mut self.moving_marks.borrow_mut().iter_mut() {
            m.move_to_beginning_of_line(&doc.buffer, utf8::get_prev_codepoint);
        }
    }

    pub fn move_marks_to_end_of_line(&mut self) {
        let doc = self.document.as_mut().unwrap().borrow_mut();
        for m in &mut self.moving_marks.borrow_mut().iter_mut() {
            m.move_to_end_of_line(&doc.buffer, utf8::get_codepoint);
        }
    }

    pub fn move_marks_to_previous_line(&mut self) {
        let mut scroll_needed = false;
        let mut mark_moved = false;

        for m in &mut self.moving_marks.borrow_mut().iter_mut() {
            // if view.is_mark_on_screen(m) {
            // yes get coordinates
            let (_, x, y) = self.screen.find_cpi_by_offset(m.offset);
            if y > 0 {
                let new_y = y - 1;
                let l = self.screen.get_line(new_y).unwrap();
                let new_x = ::std::cmp::min(x, l.nb_cells - 1);
                let cpi = self.screen.get_cpinfo(new_x, new_y).unwrap();
                if cpi.metadata == false {
                    m.offset = cpi.offset;
                    mark_moved = true;
                }
            }

            if mark_moved == false {
                // mark was on first line or offscreen
                if self.screen.contains_offset(m.offset) {
                    scroll_needed = true;
                }

                // mark is offscren
                let screen_width = self.screen.width();
                let screen_height = self.screen.height();

                // sync_offset
                let end_offset = m.offset;
                let start_offset = if end_offset as usize > (screen_width * screen_height) {
                    let doc = self.document.as_ref().unwrap().borrow_mut();

                    let off = end_offset - (screen_width * screen_height) as u64;
                    let mut tmp = Mark::new(off);
                    tmp.move_to_beginning_of_line(&doc.buffer, utf8::get_prev_codepoint);
                    tmp.offset
                } else {
                    0
                };

                let lines = layout::get_lines_offsets(
                    &self,
                    start_offset,
                    end_offset,
                    screen_width,
                    screen_height,
                );

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
                let mut tmp_mark = Mark::new(line_start_off);

                // compute column
                let new_x = {
                    let doc = self.document.as_ref().unwrap().borrow_mut();
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

                let doc = self.document.as_ref().unwrap().borrow_mut();
                for _ in 0..new_x {
                    tmp_mark.move_forward(&doc.buffer, utf8::get_next_codepoint_start);
                }

                if tmp_mark.offset > line_end_off {
                    tmp_mark.offset = line_end_off;
                }

                if tmp_mark.offset < m.offset {
                    m.offset = tmp_mark.offset;
                }
            }
        }

        if scroll_needed {
            let n = self.screen.height() / 2;
            self.scroll_up(n);
        }
    }

    pub fn move_marks_to_next_line(&mut self) {
        let max_offset = {
            let doc = self.document.as_mut().unwrap().borrow_mut();
            doc.buffer.size as u64
        };

        let mut scroll_needed = false;

        for m in &mut self.moving_marks.borrow_mut().iter_mut() {
            if m.offset == max_offset {
                continue;
            }

            let mut is_offscreen = true;

            if self.screen.contains_offset(m.offset) {
                // yes get coordinates
                let (_, x, y) = self.screen.find_cpi_by_offset(m.offset);
                if y < self.screen.height() - 1 {
                    is_offscreen = false;

                    let new_y = y + 1;
                    let l = self.screen.get_line(new_y).unwrap();
                    if l.nb_cells > 0 {
                        let new_x = ::std::cmp::min(x, l.nb_cells - 1);
                        let cpi = self.screen.get_cpinfo(new_x, new_y).unwrap();
                        if cpi.metadata == false {
                            m.offset = cpi.offset;
                        }
                    }
                } else {
                    scroll_needed = true;
                }
            }

            if is_offscreen {
                // mark is offscren
                let screen_width = self.screen.width();
                let screen_height = self.screen.height();

                // get start_of_line(m.offset) -> u64
                let start_offset = {
                    let doc = self.document.as_ref().unwrap().borrow_mut();
                    let mut tmp = Mark::new(m.offset);
                    tmp.move_to_beginning_of_line(&doc.buffer, utf8::get_prev_codepoint);
                    tmp.offset
                };

                // a codepoint can use 4 bytes the virtual end is
                // + 1 full line away
                let end_offset = ::std::cmp::min(m.offset + (4 * screen_width) as u64, max_offset);

                // get lines start, end offset
                let lines = layout::get_lines_offsets(
                    &self,
                    start_offset,
                    end_offset,
                    screen_width,
                    screen_height,
                );

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
                    let doc = self.document.as_ref().unwrap().borrow_mut();
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
                let doc = self.document.as_ref().unwrap().borrow_mut();
                for _ in 0..new_x {
                    tmp_mark.move_forward(&doc.buffer, utf8::get_next_codepoint_start);
                }

                if tmp_mark.offset > line_end_off {
                    tmp_mark.offset = line_end_off;
                }

                m.offset = tmp_mark.offset;
            }

            if m.offset > self.end_offset {
                scroll_needed = true;
            }
        }

        // only if main mark
        if scroll_needed {
            let n = self.screen.height() / 2;
            self.scroll_down(n);
        }
    }

    pub fn move_mark_to_screen_start(&mut self) {
        for m in &mut self.moving_marks.borrow_mut().iter_mut() {
            // TODO: add main mark check
            if m.offset < self.start_offset || m.offset > self.end_offset {
                m.offset = self.start_offset;
            }
        }
    }

    pub fn move_mark_to_screen_end(&mut self) {
        for m in &mut self.moving_marks.borrow_mut().iter_mut() {
            // TODO: add main mark check
            if m.offset < self.start_offset || m.offset > self.end_offset {
                m.offset = self.end_offset;
            }
        }
    }

    pub fn scroll_to_previous_screen(&mut self) {
        let nb = ::std::cmp::max(self.screen.height() - 1, 1);
        self.scroll_up(nb);
        self.move_mark_to_screen_end();
    }

    pub fn scroll_up(&mut self, nb_lines: usize) {
        if self.start_offset == 0 {
            return;
        }

        let width = self.screen.width();
        let height = self.screen.height();

        // the offset to find is the first screen codepoint
        let offset_to_find = self.start_offset;

        // go to N previous physical lines ... here N is height
        // rewind width*height chars
        let mut m = Mark::new(self.start_offset);
        if m.offset > (width * height) as u64 {
            m.offset -= (width * height) as u64
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
        let lines = layout::get_lines_offsets(&self, m.offset, offset_to_find, width, height);

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

    pub fn move_mark_to_beginning_of_file(&mut self) {
        for m in &mut self.moving_marks.borrow_mut().iter_mut() {
            m.offset = 0;
            break;
        }

        self.start_offset = 0;
    }

    pub fn center_arround_mark(&mut self) {
        for m in &mut self.moving_marks.borrow_mut().iter_mut() {
            self.start_offset = m.offset;
            break;
        }

        let h = self.screen.height() / 2;
        self.scroll_up(h);
    }

    pub fn move_mark_to_end_of_file(&mut self) {
        let size = {
            let doc = self.document.as_mut().unwrap().borrow_mut();
            doc.buffer.size
        };

        for m in &mut self.moving_marks.borrow_mut().iter_mut() {
            m.offset = size as u64;
            break;
        }

        self.start_offset = size as u64;
        let h = self.screen.height() / 2;
        self.scroll_up(h);
    }

    pub fn scroll_to_next_screen(&mut self) {
        let nb = ::std::cmp::max(self.screen.height() - 1, 1);
        self.scroll_down(nb);
        self.move_mark_to_screen_start();
    }

    pub fn scroll_down(&mut self, nb_lines: usize) {
        if nb_lines == 0 {
            return;
        }

        let max_offset = {
            let doc = self.document.as_mut().unwrap().borrow_mut();
            doc.buffer.size as u64
        };

        if nb_lines >= self.screen.height() {
            // will be slower than just reading the current screen

            let screen_width = self.screen.width();
            let screen_height = self.screen.height() + 32;

            let start_offset = self.start_offset;
            let end_offset = ::std::cmp::min(
                self.start_offset + (4 * nb_lines * screen_width) as u64,
                max_offset,
            );

            let lines = layout::get_lines_offsets(
                &self,
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
        } else {
            if self.screen.contains_offset(max_offset) {
                return;
            }

            // just read the current screen

            // get last used line , if contains eof return
            if let (Some(l), _) = self.screen.get_used_line_clipped(nb_lines) {
                if let Some(cpi) = l.get_first_cpi() {
                    // set first offset of last used line as next screen start
                    self.start_offset = cpi.offset;
                }
            }
        }
    }

    pub fn save_document(&mut self) -> bool {
        let doc = self.document.as_mut().unwrap().borrow_mut();
        match doc.sync_to_disk() {
            Err(_) => false,
            Ok(_) => true,
        }
    }

    pub fn cut_to_end_of_line(&mut self) -> bool {
        {
            let mut doc = self.document.as_mut().unwrap().borrow_mut();

            for m in &mut self.moving_marks.borrow_mut().iter_mut() {
                let mut end = m.clone();
                end.move_to_end_of_line(&doc.buffer, utf8::get_codepoint);
                doc.remove(m.offset, (end.offset - m.offset) as usize, None);
                break;
            }

            // save buffer log idx
            assert!(doc.buffer_log.pos > 0);
            self.last_cut_log_index = Some(doc.buffer_log.pos - 1);
        }
        true
    }

    pub fn paste(&mut self) -> bool {
        if let Some(idx) = self.last_cut_log_index {
            let mut doc = self.document.as_mut().unwrap().borrow_mut();
            let tr = doc.buffer_log.data[idx].clone();

            for m in &mut self.moving_marks.borrow_mut().iter_mut() {
                let mut end = m.clone();
                end.move_to_end_of_line(&doc.buffer, utf8::get_codepoint);
                doc.insert(m.offset, tr.data.len(), tr.data.as_slice());
                m.offset += tr.data.len() as u64;
                break;
            }

            true
        } else {
            false
        }
    }

    pub fn button_press(&mut self, button: u32, x: i32, y: i32) {
        match button {
            0 => {}
            _ => {
                return;
            }
        }

        // move cursor to (x,y)
        let (x, y) = (x as usize, y as usize);

        let max_offset = self.screen.doc_max_offset;

        // check click past eof
        let force_eof = if let (Some(_), _, last_y) = self.screen.get_last_cpinfo() {
            y > last_y
        } else {
            false
        };

        let skip_h = self.screen.skip_height.unwrap_or(0);
        if y <= skip_h {
            return;
        }
        let y = y - skip_h;

        let skip_w = self.screen.skip_width.unwrap_or(0);
        if x <= skip_w {
            return;
        }
        let x = x - skip_w;

        if let (Some(cpi), _, _) = self.screen.get_used_cpinfo_clipped(x, y) {
            if cpi.metadata == false {
                for m in &mut self.moving_marks.borrow_mut().iter_mut() {
                    m.offset = if force_eof { max_offset } else { cpi.offset };
                    // we only move one mark
                    break; // TODO: add main mark ref
                }
            }
        }
    }

    pub fn button_release(&mut self, button: u32, _x: i32, _y: i32) {
        let button = if button == 0xff {
            // TODO: return last pressed button
            0xff
        } else {
            button
        };

        match button {
            _ => {}
        }
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
