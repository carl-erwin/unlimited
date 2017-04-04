//
use std::collections::HashMap;

//
use core;
use core::byte_buffer::ByteBuffer;

use core::view;
use core::view::View;

//
pub type Id = u64;

pub struct BufferBuilder {
    internal: bool,
    name: String,
}

impl BufferBuilder {
    pub fn new() -> BufferBuilder {
        BufferBuilder {
            internal: false,
            name: String::new(),
        }
    }

    pub fn internal<'a>(&'a mut self, flag: bool) -> &'a mut BufferBuilder {
        self.internal = flag;
        self
    }

    pub fn buffer_name<'a>(&'a mut self, name: &str) -> &'a mut BufferBuilder {
        self.name.clear();
        self.name.push_str(name);
        self
    }


    pub fn finalize(&self) -> Buffer {
        Buffer {
            id: 0,
            name: self.name.clone(),
            byte_buffer: None,
            views: HashMap::new(),
        }
    }
}


//
pub struct Buffer {
    pub id: Id,
    pub name: String,
    pub byte_buffer: Option<ByteBuffer>,
    pub views: HashMap<view::Id, Box<View>>,
}
