// Copyright (c) Carl-Erwin Griffith

use std::rc::Rc;
use std::vec::Vec;

/// The **BufferLog** holds all modifications applied to a given buffer
#[derive(Default, Debug, Clone)]
pub struct BufferLog {
    pub data: Vec<BufferOperation>,
    pub pos: usize,
}

#[derive(Debug, Clone)]
pub struct BufferOperation {
    pub op_type: BufferOperationType,
    pub data: Option<Rc<Vec<u8>>>,
    pub offset: u64,
}

#[derive(Debug, Clone, PartialEq)]
pub enum BufferOperationType {
    Insert,
    Remove,
    Tag { marks: Vec<u64> },
}

impl BufferLog {
    pub fn new() -> Self {
        Default::default()
    }

    pub fn add(
        &mut self,
        offset: u64,
        op_type: BufferOperationType,
        data: Option<Rc<Vec<u8>>>,
    ) -> usize {
        let op = BufferOperation {
            op_type,
            data,
            offset,
        };

        if self.pos < self.data.len() {
            // commit inverted operations
            let pos = self.pos;
            let v = self.get_reverse_ops(pos).unwrap();
            self.data.extend(v);
        }

        self.data.push(op);
        self.pos = self.data.len();

        self.pos
    }

    fn get_reverse_ops(&mut self, from_pos: usize) -> Option<Vec<BufferOperation>> {
        let len = self.data.len();
        let capacity = len - from_pos;
        let mut v = Vec::with_capacity(capacity);

        for i in 0..capacity {
            v.push(self.data[len - i - 1].invert());
        }

        if !v.is_empty() {
            Some(v)
        } else {
            None
        }
    }
}

impl BufferOperation {
    pub fn invert(&self) -> BufferOperation {
        let op_type = match &self.op_type {
            BufferOperationType::Insert => BufferOperationType::Remove,
            BufferOperationType::Remove => BufferOperationType::Insert,
            BufferOperationType::Tag { marks } => BufferOperationType::Tag {
                marks: marks.clone(),
            },
        };

        BufferOperation {
            op_type,
            data: self.data.clone(), // TODO: user Rc<> to share the data, depending on the data.size()
            offset: self.offset,
        }
    }
}
