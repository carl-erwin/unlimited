//
use std::collections::HashMap;

//
use core::byte_buffer::ByteBuffer;
use core::byte_buffer::OpenMode;

use core::view;
use core::view::View;

//
pub type Id = u64;

///
pub struct BufferBuilder {
    internal: bool,
    buffer_name: String,
    file_name: String,
}

///
impl BufferBuilder {
    ///
    pub fn new() -> BufferBuilder {
        BufferBuilder {
            internal: false,
            buffer_name: String::new(),
            file_name: String::new(),
        }
    }

    ///
    pub fn internal<'a>(&'a mut self, flag: bool) -> &'a mut BufferBuilder {
        self.internal = flag;
        self
    }

    ///
    pub fn buffer_name<'a>(&'a mut self, name: &str) -> &'a mut BufferBuilder {
        self.buffer_name.clear();
        self.buffer_name.push_str(name);
        self
    }

    ///
    pub fn file_name<'a>(&'a mut self, name: &str) -> &'a mut BufferBuilder {
        self.file_name.clear();
        self.file_name.push_str(name);
        self
    }


    ///
    pub fn finalize(&self) -> Option<Buffer> {

        let byte_buffer = ByteBuffer::new(&self.file_name, OpenMode::ReadWrite);

        Some(Buffer {
                 id: 0,
                 name: self.buffer_name.clone(),
                 byte_buffer: byte_buffer,
                 views: HashMap::new(),
             })
    }
}


///
#[derive(Debug)]
pub struct Buffer {
    pub id: Id,
    pub name: String,
    pub byte_buffer: Option<ByteBuffer>,
    pub views: HashMap<view::Id, Box<View>>,
}
