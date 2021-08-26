use std::sync::Arc;
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
    pub data: Option<Arc<Vec<u8>>>,
    pub offset: u64,
}

#[derive(Debug, Clone, PartialEq)]
pub enum BufferOperationType {
    Insert,
    Remove,
    Tag {
        time: std::time::Instant,
        marks_offsets: Vec<u64>,
    },
}

impl BufferLog {
    pub fn new() -> Self {
        Default::default()
    }

    pub fn add(
        &mut self,
        offset: u64,
        op_type: BufferOperationType,
        data: Option<Arc<Vec<u8>>>,
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
            BufferOperationType::Tag { marks_offsets, .. } => BufferOperationType::Tag {
                time: std::time::Instant::now(),
                marks_offsets: marks_offsets.clone(),
            },
        };

        BufferOperation {
            op_type,
            data: self.data.clone(), // TODO(ceg): replace data by an enum { byte:[u8;12] , vec:Arc:Vec<u8> } // Arc<> to share the data, depending on the data.size()
            offset: self.offset,
        }
    }
}
