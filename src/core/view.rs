//
use std::rc::Rc;
use std::cell::RefCell;

//
use core::document::Document;
use core::screen::Screen;

use core::mark::Mark;
use core::codepointinfo::CodepointInfo;


use core::codec::text::utf8;


pub type Id = u64;


// TODO: add the main mark as a ref
#[derive(Debug)]
pub struct View {
    pub id: Id,
    pub start_offset: u64,
    pub end_offset: u64,
    pub document: Option<Rc<RefCell<Document>>>,
    pub screen: Box<Screen>,

    // TODO: in future version marks will be stored in buffer meta data
    pub moving_marks: Rc<RefCell<Vec<Mark>>>,
    pub fixed_marks: Rc<RefCell<Vec<Mark>>>,
}


impl View {
    pub fn new(id: Id,
               start_offset: u64,
               width: usize,
               height: usize,
               document: Option<Rc<RefCell<Document>>>)
               -> View {

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
        }
    }






    pub fn insert_codepoint(&mut self, codepoint: char) {

        let mut data: &mut [u8; 4] = &mut [0, 0, 0, 0];

        let data_size = utf8::encode(codepoint as u32, &mut data);
        let mut doc = self.document.as_mut().unwrap().borrow_mut();
        for m in &mut self.moving_marks.borrow_mut().iter_mut() {
            doc.buffer.write(m.offset, data_size, data);
            m.offset += data_size as u64;
        }
    }


    pub fn remove_codepoint(&mut self) {

        let mut doc = self.document.as_mut().unwrap().borrow_mut();
        for m in &mut self.moving_marks.borrow_mut().iter_mut() {
            let (_, _, size) = utf8::get_codepoint(&doc.buffer.data, m.offset);
            doc.buffer.remove(m.offset, size, None);
        }
    }


    pub fn remove_previous_codepoint(&mut self) {

        let mut doc = self.document.as_mut().unwrap().borrow_mut();
        for m in &mut self.moving_marks.borrow_mut().iter_mut() {

            if m.offset == 0 {
                continue;
            }

            m.move_backward(&doc.buffer, utf8::get_previous_codepoint_start);
            let (_, _, size) = utf8::get_codepoint(&doc.buffer.data, m.offset);
            doc.buffer.remove(m.offset, size, None);
        }
    }


    pub fn move_marks_backward(&mut self) {
        let doc = self.document.as_mut().unwrap().borrow_mut();

        for m in &mut self.moving_marks.borrow_mut().iter_mut() {
            m.move_backward(&doc.buffer, utf8::get_previous_codepoint_start);
        }
    }


    pub fn move_marks_forward(&mut self) {

        let doc = self.document.as_mut().unwrap().borrow_mut();
        for m in &mut self.moving_marks.borrow_mut().iter_mut() {
            m.move_forward(&doc.buffer, utf8::get_next_codepoint_start);
        }
    }


    pub fn move_marks_to_beginning_of_line(&mut self) {

        let doc = self.document.as_mut().unwrap().borrow_mut();
        for mut m in &mut self.moving_marks.borrow_mut().iter_mut() {
            m.move_to_beginning_of_line(&doc.buffer, utf8::get_prev_codepoint);
        }
    }


    pub fn move_marks_to_end_of_line(&mut self) {

        let doc = self.document.as_mut().unwrap().borrow_mut();
        for mut m in &mut self.moving_marks.borrow_mut().iter_mut() {
            m.move_to_end_of_line(&doc.buffer, utf8::get_codepoint);
        }
    }


    pub fn move_marks_to_previous_line(&mut self) {

        let mut scroll_needed = false;

        for m in &mut self.moving_marks.borrow_mut().iter_mut() {
            // if view.is_mark_on_screen(m) {
            // yes get coordinates
            let (_, x, y) = self.screen.find_cpi_by_offset(m.offset);
            if y > 0 {
                let new_y = y - 1;
                let l = self.screen.get_line(new_y).unwrap();
                let new_x = ::std::cmp::min(x, l.nb_cells - 1);
                let cpi = self.screen.get_cpinfo(new_x, new_y).unwrap();
                m.offset = cpi.offset;
            } else {

                scroll_needed = true;

                // mark is offscren
                let screen_width = self.screen.width;
                let screen_height = self.screen.height;

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

                let lines =
                    self.get_lines_offsets(start_offset, end_offset, screen_width, screen_height);

                // find "previous" line index
                let index = match lines
                          .iter()
                          .position(|e| e.0 <= end_offset && end_offset <= e.1) {
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

        if scroll_needed == true {
            self.scroll_up(1);
        }

    }


    pub fn move_marks_to_next_line(&mut self) {

        let max_offset = {
            let doc = self.document.as_mut().unwrap().borrow_mut();
            doc.buffer.data.len() as u64
        };

        let screen = self.screen.clone(); // TODO: use cache

        let mut scroll_needed = false;

        for m in &mut self.moving_marks.borrow_mut().iter_mut() {

            if m.offset == max_offset {
                continue;
            }

            let mut is_offscreen = true;
            if screen.contains_offset(m.offset) {
                // yes get coordinates
                let (_, x, y) = screen.find_cpi_by_offset(m.offset);
                if y < screen.height - 1 {

                    is_offscreen = false;

                    let new_y = y + 1;
                    let l = screen.get_line(new_y).unwrap();
                    if l.nb_cells > 0 {
                        let new_x = ::std::cmp::min(x, l.nb_cells - 1);
                        let cpi = screen.get_cpinfo(new_x, new_y).unwrap();
                        m.offset = cpi.offset;
                    }
                }
            }

            if is_offscreen == true {

                // mark is offscren
                let screen_width = self.screen.width;
                let screen_height = self.screen.height;

                // get start_of_line(m.offset) -> u64
                let start_offset = {
                    let doc = self.document.as_ref().unwrap().borrow_mut();
                    let mut tmp = Mark::new(m.offset);
                    tmp.move_to_beginning_of_line(&doc.buffer, utf8::get_prev_codepoint);
                    tmp.offset
                };

                let end_offset = ::std::cmp::min(m.offset + (4 * screen_width) as u64, max_offset);

                // get lines start, end offset
                let lines =
                    self.get_lines_offsets(start_offset, end_offset, screen_width, screen_height);

                // find "next" line index
                let index = match lines
                          .iter()
                          .position(|e| e.0 <= m.offset && m.offset <= e.1) {
                    None => continue,
                    Some(i) => {
                        if i == lines.len() - 1 {
                            continue;
                        } else {
                            i + 1
                        }
                    }
                };

                // compute column
                let new_x = {
                    let doc = self.document.as_ref().unwrap().borrow_mut();
                    let mut s = Mark::new(lines[index - 1].0);
                    let e = Mark::new(lines[index - 1].1);
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

                // get line start
                let line_start_off = lines[index].0;
                let line_end_off = lines[index].1;
                let mut tmp_mark = Mark::new(line_start_off);

                let doc = self.document.as_ref().unwrap().borrow_mut();
                for _ in 0..new_x {
                    tmp_mark.move_forward(&doc.buffer, utf8::get_next_codepoint_start);
                }

                if tmp_mark.offset > line_end_off {
                    tmp_mark.offset = line_end_off;
                }

                m.offset = tmp_mark.offset;

                scroll_needed = true;
            }
        }

        if scroll_needed == true {
            self.scroll_down(1);
        }
    }

    pub fn scroll_to_previous_screen(&mut self) {

        if self.start_offset == 0 {
            return;
        }

        let width = self.screen.width;
        let height = self.screen.height;

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
        let lines = self.get_lines_offsets(m.offset, offset_to_find, width, height);
        let index = {
            if lines.len() > 0 {
                lines.len() - 1
            } else {
                panic!("");
            }
        };

        let index = if index >= height {
            index - height + 1
        } else {
            0
        };

        self.start_offset = lines[index].0;

        // TEST
        if 0 == 1 {
            let doc = self.document.as_mut().unwrap().borrow_mut();
            let data = &doc.buffer.data;
            let len = data.len();
            let max_offset = len as u64;

            let mut screen = Screen::new(width, height);
            let _ = decode_slice_to_screen(&data[0 as usize..len],
                                           self.start_offset,
                                           max_offset,
                                           &mut screen);
            match screen.find_cpi_by_offset(offset_to_find) {
                (Some(_), x, y) => {
                    assert_eq!(x, 0);
                    assert_eq!(y, screen.current_line_index - 1);
                }
                _ => panic!("implementation error"),
            }
        }
    }

    pub fn scroll_up(&mut self, nb_lines: usize) {

        if self.start_offset == 0 {
            return;
        }

        let width = self.screen.width;
        let height = self.screen.height;

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
        let lines = self.get_lines_offsets(m.offset, offset_to_find, width, height);

        // find line index
        let index = match lines
                  .iter()
                  .position(|e| e.0 <= offset_to_find && offset_to_find <= e.1) {
            None => 0,
            Some(i) => ::std::cmp::min(lines.len() - 1, i - nb_lines),
        };

        self.start_offset = lines[index].0;
    }


    pub fn scroll_to_next_screen(&mut self) {

        let nb = self.screen.height - 1;
        return self.scroll_down(nb);
    }

    pub fn scroll_down(&mut self, nb_lines: usize) {

        if nb_lines == 0 {
            return;
        }

        if nb_lines >= self.screen.height {
            return;
        }

        let max_offset = {
            let doc = self.document.as_mut().unwrap().borrow_mut();
            doc.buffer.data.len() as u64
        };

        if self.screen.contains_offset(max_offset) {
            return;
        }

        // get last used line , if contains eof return
        match self.screen.get_used_line_clipped(nb_lines) {
            (Some(l), _) => {
                if let Some(cpi) = l.get_first_cpi() {
                    // set first offset of last used line as next screen start
                    self.start_offset = cpi.offset;
                }
            }

            _ => {}
        }
    }


    fn get_lines_offsets(&self,
                         start_offset: u64,
                         end_offset: u64,
                         screen_width: usize,
                         screen_height: usize)
                         -> Vec<(u64, u64)> {

        let mut v = Vec::<(u64, u64)>::new();

        let mut m = Mark::new(start_offset);

        let doc = self.document.as_ref().unwrap().borrow_mut();

        // get beginning of the line @offset
        m.move_to_beginning_of_line(&doc.buffer, utf8::get_prev_codepoint);

        // and build tmp screens until end_offset if found
        let mut screen = Screen::new(screen_width, screen_height);

        // fill screen
        let data = &doc.buffer.data;
        let len = data.len();
        let max_offset = len as u64;
        loop {
            let _ =
                decode_slice_to_screen(&data[0 as usize..len], m.offset, max_offset, &mut screen);

            // push lines offsets
            // FIXME: find a better way to iterate over the used lines
            for i in 0..screen.current_line_index {

                let s = screen.line[i].get_first_cpi().unwrap().offset;
                let e = screen.line[i].get_last_cpi().unwrap().offset;

                if v.len() != 0 && i == 0 {
                    // do not push line range twice
                    continue;
                }

                v.push((s, e));

                if s >= end_offset || e == max_offset {
                    return v;
                }
            }

            // eof reached ?
            // FIXME: the api is not yet READY
            // we must find a way to cover all fill lines
            if screen.current_line_index < screen.height {
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


    pub fn button_press(&mut self, button: u32, x: i32, y: i32) {

        match button {
            0 => {}
            _ => {
                return;
            }
        }

        // move cursor to (x,y)
        let (x, y) = (x as usize, y as usize);
        let (cpi, _, _) = self.screen.get_used_cpinfo_clipped(x, y);

        match cpi {
            Some(cpi) => {
                for m in &mut self.moving_marks.borrow_mut().iter_mut() {
                    m.offset = cpi.offset;
                    // we only move one mark
                    break; // TODO: add main mark ref
                }

            }
            _ => {}
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



//////////////////////////////////

pub fn decode_slice_to_screen(data: &[u8],
                              base_offset: u64,
                              max_offset: u64,
                              mut screen: &mut Screen)
                              -> u64 {

    let max_cpi = screen.width * screen.height;
    let (vec, last_offset) = decode_slice_to_vec(data, base_offset, max_offset, max_cpi);

    let mut prev_cp = ' ';
    for cpi in &vec {

        let (ok, _) = match (prev_cp, cpi.cp) {
            // TODO: handle \r\n
            /*
                ('\r', '\n') => {
                    prev_cp = ' ';
                    (true, 0 as usize)
                }
            */
            _ => {
                prev_cp = cpi.cp;
                screen.push(cpi.clone())
            }
        };
        if ok == false {
            break;
        }

    }

    last_offset
}



fn decode_slice_to_vec(data: &[u8],
                       base_offset: u64,
                       max_offset: u64,
                       max_cpi: usize)
                       -> (Vec<CodepointInfo>, u64) {

    let mut vec = Vec::with_capacity(max_cpi);

    let mut off: u64 = base_offset;
    let last_off = data.len() as u64;

    while off != last_off {

        let (cp, _, size) = utf8::get_codepoint(data, off);
        vec.push(filter_codepoint(cp, off));
        off += size as u64;
        if vec.len() == max_cpi {
            break;
        }
    }

    // eof handling
    if last_off == max_offset {
        vec.push(CodepointInfo {
                     cp: ' ',
                     displayed_cp: '$',
                     offset: last_off,
                     is_selected: !false,
                 });
    }

    (vec, off)
}



//
pub fn filter_codepoint(c: char, offset: u64) -> CodepointInfo {

    let displayed_cp = match c {
        '\r' | '\n' | '\t' => ' ',
        _ => c,
    };

    CodepointInfo {
        cp: c,
        displayed_cp: displayed_cp,
        offset: offset,
        is_selected: false,
    }
}


pub fn screen_putstr(mut screen: &mut Screen, s: &str) -> bool {

    let v: Vec<char> = s.chars().collect();
    for c in &v {
        let ok = screen_putchar(&mut screen, *c, 0xffffffffffffffff);
        if ok == false {
            return false;
        }
    }

    true
}


pub fn screen_putchar(mut screen: &mut Screen, c: char, offset: u64) -> bool {
    let (ok, _) = screen.push(filter_codepoint(c, offset));
    ok
}




#[test]
fn test_view() {}
