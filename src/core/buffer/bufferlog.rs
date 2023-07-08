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

impl BufferOperation {
    pub fn dump(&self) {
        dbg_println!("BufferOperation: op_type {:?}", self.op_type);
        if self.data.is_some() {
            dbg_println!(
                "BufferOperation: data.len {:?}",
                self.data.as_ref().unwrap().len()
            );
        } else {
            dbg_println!("BufferOperation: data = None");
        }
        dbg_println!("BufferOperation: offset {:?}", self.offset);
    }
}

#[derive(Debug, Clone, PartialEq)]
pub enum BufferOperationType {
    Insert,
    Remove,
    Tag {
        time: std::time::Instant,
        marks_offsets: Vec<u64>,
        selections_offsets: Vec<u64>,
    },
}

impl BufferLog {
    pub fn new() -> Self {
        Default::default()
    }

    pub fn dump(&self) {
        dbg_println!("-- BufferLog::dump() {{\r");

        dbg_println!("--  pos {}\r", self.pos);

        for (idx, op) in self.data.iter().enumerate() {
            dbg_println!("dump buffer log [{}] = \r", idx);
            op.dump();
        }
        dbg_println!("dump buffer log pos = {}\r", self.pos);
        dbg_println!("}} ---\r");
    }

    pub fn dump_to_current_log_pos(&self) {
        for (idx, op) in self.data.iter().enumerate().take(self.pos + 1) {
            dbg_println!("dump (..pos) buffer log [{}] = {:?}\r", idx, op);
        }
        dbg_println!("dump (..pos) buffer log pos = {}", self.pos);
        dbg_println!("----------------------------------\r");
    }

    pub fn last_index(&self) -> Option<usize> {
        let len = self.data.len();
        if len > 0 {
            Some(len - 1)
        } else {
            None
        }
    }

    pub fn add(
        &mut self,
        offset: u64,
        op_type: BufferOperationType,
        data: Option<Arc<Vec<u8>>>,
    ) -> usize {
        dbg_println!("-- BufferLog::add() before");
        self.dump();

        let op = BufferOperation {
            op_type,
            data,
            offset,
        };

        op.dump();

        if let Some(v) = self.get_reverse_ops(self.pos) {
            self.data.extend(v);
        }

        self.data.push(op);
        self.pos = self.data.len();

        dbg_println!("-- BufferLog::add() after");
        self.dump();

        self.pos
    }

    fn get_reverse_ops(&mut self, from_pos: usize) -> Option<Vec<BufferOperation>> {
        let len = self.data.len();
        let capacity = len - from_pos;
        if capacity == 0 {
            return None;
        }

        let mut v = Vec::with_capacity(capacity);
        for op in self.data.iter().rev().take(capacity) {
            v.push(op.invert());
        }

        Some(v)
    }
}

impl BufferOperation {
    pub fn invert(&self) -> BufferOperation {
        let op_type = match &self.op_type {
            BufferOperationType::Insert => BufferOperationType::Remove,
            BufferOperationType::Remove => BufferOperationType::Insert,
            BufferOperationType::Tag {
                marks_offsets,
                selections_offsets,
                ..
            } => BufferOperationType::Tag {
                time: std::time::Instant::now(),
                marks_offsets: marks_offsets.clone(),
                selections_offsets: selections_offsets.clone(),
            },
        };

        BufferOperation {
            op_type,
            data: self.data.clone(), // TODO(ceg): replace data by an enum { byte:[u8;12] , vec:Arc:Vec<u8> } // Arc<> to share the data, depending on the data.size()
            offset: self.offset,
        }
    }
}
