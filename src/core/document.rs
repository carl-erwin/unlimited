//
use std::rc::Rc;
use std::cell::RefCell;

//
use core::buffer::Buffer;
use core::buffer::OpenMode;

//
pub type Id = u64;

///
#[derive(Default)]
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
    pub fn internal(&mut self, flag: bool) -> &mut DocumentBuilder {
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

        Some(Rc::new(RefCell::new(Document {
            id: 0,
            name: self.document_name.clone(),
            buffer: buffer,
            changed: false,
        })))
    }
}



#[derive(Debug)]
pub struct Document {
    pub id: Id,
    pub name: String,
    pub buffer: Buffer,
    pub changed: bool,
}

impl Document {
    // FIXME: move to Buffer
    pub fn sync_to_disk(&self) -> ::std::io::Result<()> {
        use std::fs;
        use std::fs::File;
        use std::io::prelude::*;

        let tmp_file_ext = "ued_tmp"; // TODO: move to global config
        let tmp_file_name = format!("{}.{}", self.buffer.file_name, tmp_file_ext);
        let mut f = File::create(&tmp_file_name)?;

        f.write_all(&self.buffer.data)?;
        f.sync_all()?;
        fs::rename(&tmp_file_name, &self.buffer.file_name)
    }
}
