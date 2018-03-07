use std::vec::Vec;

/// The **BufferLog** holds all modifications applied to a given buffer
#[derive(Debug, Clone)]
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
    pub fn new() -> BufferLog {
        BufferLog {
            data: Vec::new(),
            pos: 0,
        }
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

        if v.len() > 0 {
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
