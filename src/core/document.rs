//
use std::rc::Rc;
use std::cell::RefCell;

//
use core::buffer::Buffer;
use core::buffer::OpenMode;

use core::mark::Mark;

//
pub type Id = u64;

///
pub struct DocumentBuilder {
    internal: bool,
    document_name: String,
    file_name: String,
}

///
impl DocumentBuilder {
    ///
    pub fn new() -> DocumentBuilder {
        DocumentBuilder {
            internal: false,
            document_name: String::new(),
            file_name: String::new(),
        }
    }

    ///
    pub fn internal<'a>(&'a mut self, flag: bool) -> &'a mut DocumentBuilder {
        self.internal = flag;
        self
    }

    ///
    pub fn document_name<'a>(&'a mut self, name: &str) -> &'a mut DocumentBuilder {
        self.document_name.clear();
        self.document_name.push_str(name);
        self
    }

    ///
    pub fn file_name<'a>(&'a mut self, name: &str) -> &'a mut DocumentBuilder {
        self.file_name.clear();
        self.file_name.push_str(name);
        self
    }


    ///
    pub fn finalize(&self) -> Option<Rc<RefCell<Document>>> {

        let buffer = Buffer::new(&self.file_name, OpenMode::ReadWrite);
        let buffer = match buffer {
            Some(bb) => bb,
            None => return None,
        };

        // TODO: in future version will be stored in buffer meta data
        let moving_marks = vec![Mark { offset: 0 }];

        Some(Rc::new(RefCell::new(Document {
                                      id: 0,
                                      name: self.document_name.clone(),
                                      buffer: buffer,
                                      changed: false,
                                      moving_marks: moving_marks,
                                      fixed_marks: Vec::new(),
                                  })))
    }
}


///
#[derive(Debug)]
pub struct Document {
    pub id: Id,
    pub name: String,
    pub buffer: Buffer,
    pub changed: bool,

    // TODO: in future version marks will be stored in buffer meta data
    pub moving_marks: Vec<Mark>,
    pub fixed_marks: Vec<Mark>,
}
