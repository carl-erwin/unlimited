//
use std::rc::Rc;

//
use core::buffer::Buffer;
use core::screen::Screen;

pub type Id = u64;

#[derive(Debug)]
pub struct View {
    pub id: Id,
    pub start_offset: u64,
    pub end_offset: u64,
    pub buffer: Option<Rc<Buffer>>, // Rc<Buffer> ?
    pub screen: Box<Screen>, // mandatory ?
}


impl View {
    pub fn new(id: Id,
               start_offset: u64,
               width: usize,
               height: usize,
               buffer: Option<Rc<Buffer>>)
               -> View {

        let screen = Box::new(Screen::new(width, height));

        View {
            id: id,
            start_offset: start_offset,
            end_offset: start_offset, // will be recomputed later
            buffer: buffer,
            screen: screen,
        }
    }
}
