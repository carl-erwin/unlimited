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

            let start_offset = m.offset;
            let mut prev_offset = m.offset;
            loop {
                let (cp, offset, size) = utf8::get_prev_codepoint(&doc.buffer.data, prev_offset);
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
                let (cp, mut offset, size) = utf8::get_codepoint(&doc.buffer.data, prev_offset);
                if prev_offset == max_offset {
                    break;
                }
                match cp {

                    '\r' => {
                        match utf8::get_codepoint(&doc.buffer.data, offset) {
                            ('\n', _, _) => {
                                offset += 1;
                            }
                            _ => {}
                        }
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
}
