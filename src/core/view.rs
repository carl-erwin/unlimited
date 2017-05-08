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

        for m in &mut self.moving_marks.borrow_mut().iter_mut() {
            // if view.is_mark_on_screen(m) {
            // yes get coordinates
            let (_, x, y) = self.screen.find_cpi_by_offset(m.offset);
            if y > 0 {
                let new_y = y - 1;
                let l = self.screen.get_line(new_y).unwrap();
                let new_x = ::std::cmp::min(x, l.nb_chars - 1);
                let cpi = self.screen.get_cpinfo(new_x, new_y).unwrap();
                m.offset = cpi.offset;
            } else {

            }

            // } else {
            //    build_screen_by_offset(m.offset) and call the code above / in util function
            //
            // }

        }
    }


    pub fn move_marks_to_next_line(&mut self) {

        let doc = self.document.as_mut().unwrap().borrow_mut();
        let max_offset = doc.buffer.data.len() as u64;

        let mut screen = self.screen.clone(); // TODO: use cache

        for m in &mut self.moving_marks.borrow_mut().iter_mut() {

            if m.offset == max_offset {
                continue;
            }

            if screen.contains_offset(m.offset) {
                // yes get coordinates
                let (_, x, y) = screen.find_cpi_by_offset(m.offset);
                if y < screen.height - 1 {
                    let new_y = y + 1;
                    let l = screen.get_line(new_y).unwrap();
                    if l.nb_chars > 0 {
                        let new_x = ::std::cmp::min(x, l.nb_chars - 1);
                        let cpi = screen.get_cpinfo(new_x, new_y).unwrap();
                        m.offset = cpi.offset;
                    }
                } else {

                }

            } else {
                //    build_screen_by_offset(m.offset) and call the code above / in util function

            }

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
        if let Some(screen) = self.build_screen_by_offset(m.offset, width, height * 2) {

            // offset is on this virtual screen ?
            match screen.find_cpi_by_offset(offset_to_find) {
                (Some(cpi), x, y) => {
                    assert_eq!(x, 0);
                    let new_start_y = if y > height { y - height + 1 } else { 0 };

                    if let Some(l) = screen.get_line(new_start_y) {
                        if let Some(cpi) = l.get_first_cpi() {
                            m.offset = cpi.offset;
                            let doc = self.document.as_mut().unwrap().borrow_mut();
                            m.move_to_beginning_of_line(&doc.buffer, utf8::get_prev_codepoint);
                            self.start_offset = m.offset;
                        }
                    }
                }
                _ => {}
            }

        }
    }

    pub fn scroll_to_next_screen(&mut self) {

        let max_offset = {
            let doc = self.document.as_mut().unwrap().borrow_mut();
            doc.buffer.data.len() as u64
        };

        if self.screen.contains_offset(max_offset) {
            return;
        }

        // get last used line , if contains eof return
        if let Some(l) = self.screen.get_last_used_line() {
            if let Some(cpi) = l.get_first_cpi() {

                // set first offset of last used line as next screen start
                self.start_offset = cpi.offset;
                // let off = cpi.offset;

                /*
                    // build_screen from this offset
                    if let Some(screen) =
                    self.build_screen_by_offset(offset, self.screen.width, self.screen.height * 2){
                        self.start_offset = screen.get_last_used_line
                    }
                */
            }
        }

        // test
        let w = self.screen.width;
        let h = self.screen.height;
        let offset = self.start_offset;
        if let Some(screen) = self.build_screen_by_offset(offset, w, h) {
            match screen.find_cpi_by_offset(self.start_offset) {
                (Some(_), x, y) => {
                    assert_eq!(x, 0);
                }
                _ => {
                    panic!("cannot find offset");
                }
            }
        }

    }

    // TODO: move to view::
    pub fn build_screen_by_offset(&mut self,
                                  offset: u64,
                                  screen_width: usize,
                                  screen_height: usize)
                                  -> Option<Screen> {

        let mut m = Mark::new(offset);

        let doc = self.document.as_mut().unwrap().borrow_mut();

        // get beginning of the line @offset
        m.move_to_beginning_of_line(&doc.buffer, utf8::get_prev_codepoint);

        // and build tmp screens until offset if found
        let mut screen = Screen::new(screen_width, screen_height);

        // fill screen
        let data = &doc.buffer.data;
        let len = data.len();
        let max_offset = len as u64;
        let mut found = false;

        loop {
            let end_offset =
                decode_slice_to_screen(&data[0 as usize..len], m.offset, max_offset, &mut screen);

            match screen.find_cpi_by_offset(m.offset) {
                (Some(cpi), x, y) => {
                    assert_eq!(x, 0);
                    assert_eq!(y, 0);
                    assert_eq!(cpi.offset, m.offset);
                }
                _ => panic!("implementation error"),
            }

            if screen.contains_offset(offset) {
                return Some(screen);
            }

            if end_offset == max_offset {
                return Some(screen);
            }

            if let Some(l) = screen.get_last_used_line() {
                if let Some(cpi) = l.get_first_cpi() {
                    m.offset = cpi.offset; // update next screen start
                } else {
                    found = true;
                }
            } else {
                found = true;
            }

            if found {
                return Some(screen);
            }

            screen.clear(); // prepare next screen
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
