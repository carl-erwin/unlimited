//
use std::rc::Rc;

//
use core::document::Document;
use core::screen::Screen;

pub type Id = u64;

#[derive(Debug)]
pub struct View {
    pub id: Id,
    pub start_offset: u64,
    pub end_offset: u64,
    pub document: Option<Rc<Document>>, // Rc<Buffer> ?
    pub screen: Box<Screen>, // mandatory ?
}


impl View {
    pub fn new(id: Id,
               start_offset: u64,
               width: usize,
               height: usize,
               document: Option<Rc<Document>>)
               -> View {

        let screen = Box::new(Screen::new(width, height));

        View {
            id: id,
            start_offset: start_offset,
            end_offset: start_offset, // will be recomputed later
            document: document,
            screen: screen,
        }
    }
}
