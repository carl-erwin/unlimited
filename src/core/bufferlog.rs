// Copyright (c) Carl-Erwin Griffith
//
// Permission is hereby granted, free of charge, to any
// person obtaining a copy of this software and associated
// documentation files (the "Software"), to deal in the
// Software without restriction, including without
// limitation the rights to use, copy, modify, merge,
// publish, distribute, sublicense, and/or sell copies of
// the Software, and to permit persons to whom the Software
// is furnished to do so, subject to the following
// conditions:
//
// The above copyright notice and this permission notice
// shall be included in all copies or substantial portions
// of the Software.
//
// THE SOFTWARE IS PROVIDED "AS IS", WITHOUT WARRANTY OF
// ANY KIND, EXPRESS OR IMPLIED, INCLUDING BUT NOT LIMITED
// TO THE WARRANTIES OF MERCHANTABILITY, FITNESS FOR A
// PARTICULAR PURPOSE AND NONINFRINGEMENT. IN NO EVENT
// SHALL THE AUTHORS OR COPYRIGHT HOLDERS BE LIABLE FOR ANY
// CLAIM, DAMAGES OR OTHER LIABILITY, WHETHER IN AN ACTION
// OF CONTRACT, TORT OR OTHERWISE, ARISING FROM, OUT OF OR
// IN CONNECTION WITH THE SOFTWARE OR THE USE OR OTHER

use std::vec::Vec;

/// The **BufferLog** holds all modifications applied to a given buffer
#[derive(Default, Debug, Clone)]
pub struct BufferLog {
    pub data: Vec<BufferOperation>,
    pub pos: usize,
}

#[derive(Debug, Clone)]
pub struct BufferOperation {
    pub op: BufferOperationType,
    pub data: Vec<u8>,
    pub offset: u64,
}

#[derive(Debug, Clone)]
pub enum BufferOperationType {
    Insert,
    Remove,
}

impl BufferLog {
    pub fn new() -> Self {
        Default::default()
    }

    pub fn add(&mut self, offset: u64, op: BufferOperationType, data: Vec<u8>) -> usize {
        let op = BufferOperation { op, data, offset };

        if self.pos < self.data.len() {
            // commit inverted operations
            let pos = self.pos;
            let v = self.get_reverse_ops(pos).unwrap();
            self.data.extend(v);
            self.pos = self.data.len();
        }

        self.data.push(op);
        self.pos = self.data.len();
        self.data.len()
    }

    pub fn get_reverse_ops(&mut self, from_pos: usize) -> Option<Vec<BufferOperation>> {
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
        let op = match self.op {
            BufferOperationType::Insert => BufferOperationType::Remove,
            BufferOperationType::Remove => BufferOperationType::Insert,
        };

        BufferOperation {
            op,
            data: self.data.clone(),
            offset: self.offset,
        }
    }
}
