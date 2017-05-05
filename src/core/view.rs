//
use std::rc::Rc;
use std::cell::RefCell;

//
use core::document::Document;
use core::screen::Screen;

use core::mark::Mark;


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

        for m in &mut self.moving_marks.borrow_mut().iter_mut() {

            if m.offset == 0 {
                continue;
            }

            let doc = self.document.as_mut().unwrap().borrow_mut();

            let mut prev_offset = m.offset;
            loop {
                let (cp, offset, _) = utf8::get_prev_codepoint(&doc.buffer.data, prev_offset);
                if offset == 0 {
                    m.offset = 0;
                    break;
                }

                match cp {
                    '\n' => {
                        m.offset = offset + 1;

                        if prev_offset > 0 {
                            match utf8::get_prev_codepoint(&doc.buffer.data, prev_offset) {
                                ('\r', offset, _) => {
                                    m.offset = offset;
                                }
                                _ => {}
                            }
                        }
                        break;
                    }

                    '\r' => {
                        m.offset = offset + 1;
                        break;
                    }

                    _ => prev_offset = offset,
                }
            }
        }
    }

    pub fn move_marks_to_end_of_line(&mut self) {

        let doc = self.document.as_mut().unwrap().borrow_mut();
        let max_offset = doc.buffer.data.len() as u64;

        for m in &mut self.moving_marks.borrow_mut().iter_mut() {

            let mut prev_offset = m.offset;

            loop {
                let (cp, offset, size) = utf8::get_codepoint(&doc.buffer.data, prev_offset);
                if prev_offset == max_offset {
                    break;
                }
                match cp {

                    '\r' => {
                        // TODO: handle \r\n
                        break;
                    }

                    '\n' => {
                        break;
                    }

                    _ => {}
                }
                prev_offset = offset + size as u64;
            }
            m.offset = prev_offset;
        }
    }

    pub fn move_marks_to_previous_line(&mut self) {

        for m in &mut self.moving_marks.borrow_mut().iter_mut() {
            // if view.is_mark_on_screen(m) {
            // yes get coordinates
            let (_, x, y) = self.screen.find_used_cpi_by_offset(m.offset);
            if y > 0 {
                let new_y = y - 1;
                let l = self.screen.get_line(new_y).unwrap();
                let new_x = ::std::cmp::min(x, l.used - 1);
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

        for m in &mut self.moving_marks.borrow_mut().iter_mut() {

            if m.offset == max_offset {
                continue;
            }

            // if view.is_mark_on_screen(m) {
            // yes get coordinates
            let (_, x, y) = self.screen.find_used_cpi_by_offset(m.offset);
            if y < self.screen.height {
                let new_y = y + 1;
                let l = self.screen.get_line(new_y).unwrap();
                let new_x = ::std::cmp::min(x, l.used - 1);
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
}
