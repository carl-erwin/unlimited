//
use std::cell::RefCell;
use std::rc::Rc;

//
use core::buffer::Buffer;
use core::buffer::OpenMode;

//
use core::bufferlog::BufferLog;
use core::bufferlog::BufferOperation;
use core::bufferlog::BufferOperationType;

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
    pub fn new() -> Self {
        Self {
            internal: false,
            document_name: String::new(),
            file_name: String::new(),
        }
    }

    ///
    pub fn internal(&self, flag: bool) -> Self {
        Self {
            internal: flag,
            document_name: self.document_name.clone(),
            file_name: self.file_name.clone(),
        }
    }

    ///
    pub fn document_name(&self, name: &str) -> Self {
        let mut s = String::new();
        s.push_str(name);

        Self {
            internal: self.internal,
            document_name: s,
            file_name: self.file_name.clone(),
        }
    }

    ///
    pub fn file_name(&self, name: &str) -> Self {
        let mut s = String::new();
        s.push_str(name);

        Self {
            internal: self.internal,
            document_name: self.document_name.clone(),
            file_name: s,
        }
    }

    ///
    pub fn finalize<'a>(&self) -> Option<Rc<RefCell<Document<'a>>>> {

        let buffer = Buffer::new(&self.file_name, OpenMode::ReadWrite);
        let buffer = match buffer {
            Some(bb) => bb,
            None => return None,
        };

        Some(Rc::new(RefCell::new(Document {
            id: 0,
            name: self.document_name.clone(),
            buffer,
            buffer_log: BufferLog::new(),
            changed: false,
        })))
    }
}

#[derive(Debug)]
pub struct Document<'a> {
    pub id: Id,
    pub name: String,
    pub buffer: Buffer<'a>,
    pub buffer_log: BufferLog,
    pub changed: bool,
}

impl<'a> Document<'a> {
    pub fn sync_to_disk(&self) -> ::std::io::Result<()> {
        let tmp_file_ext = "unlimited.bk"; // TODO: move to global config
        let tmp_file_name = format!("{}.{}", self.buffer.file_name, tmp_file_ext);
        self.buffer.sync_to_disk(&tmp_file_name)
    }

    /// copy the content of the buffer up to 'nr_bytes' into the data Vec
    /// the read bytes are appended to the data Vec
    /// return XXX on error (TODO: use ioresult)
    pub fn read(&self, offset: u64, nr_bytes: usize, data: &mut Vec<u8>) -> usize {
        self.buffer.read(offset, nr_bytes, data)
    }

    /// insert the 'data' Vec content in the buffer up to 'nr_bytes'
    /// return the number of written bytes (TODO: use io::Result)
    pub fn insert(&mut self, offset: u64, nr_bytes: usize, data: &[u8]) -> usize {
        // log insert op
        let mut ins_data = Vec::with_capacity(nr_bytes);
        ins_data.extend(&data[..nr_bytes]);

        self.buffer_log
            .add(offset, BufferOperationType::Insert, ins_data);

        self.buffer.insert(offset, nr_bytes, &data[..nr_bytes])
    }

    /// remove up to 'nr_bytes' from the buffer starting at offset
    /// if removed_data is provided will call self.read(offset, nr_bytes, data)
    /// before remove the bytes
    pub fn remove(
        &mut self,
        offset: u64,
        nr_bytes: usize,
        removed_data: Option<&mut Vec<u8>>,
    ) -> usize {
        let mut rm_data = Vec::with_capacity(nr_bytes);

        let nr_bytes_removed = self.buffer.remove(offset, nr_bytes, Some(&mut rm_data));

        if let Some(v) = removed_data {
            v.extend(rm_data.clone());
        }

        self.buffer_log
            .add(offset, BufferOperationType::Remove, rm_data);

        nr_bytes_removed
    }

    fn apply_log_operation(&mut self, op: &BufferOperation) -> Option<u64> {
        // apply op
        let mark_offset = match op.op {
            BufferOperationType::Insert => {
                self.buffer.insert(op.offset, op.data.len(), &op.data);

                op.offset + op.data.len() as u64
            }
            BufferOperationType::Remove => {
                self.buffer.remove(op.offset, op.data.len(), None);
                op.offset
            }
        };

        Some(mark_offset)
    }

    pub fn undo(&mut self) -> Option<u64> {
        // read current log position
        let pos = self.buffer_log.pos;
        if pos == 0 {
            return None;
        }

        // get inverted operation
        let op = self.buffer_log.data[pos - 1].invert();
        self.buffer_log.pos -= 1;
        self.apply_log_operation(&op)
    }

    pub fn redo(&mut self) -> Option<u64> {
        // read current log position
        let pos = self.buffer_log.pos;
        if pos == self.buffer_log.data.len() {
            return None;
        }

        // replay previous op
        let op = self.buffer_log.data[pos].clone();
        self.buffer_log.pos += 1;
        self.apply_log_operation(&op)
    }
}
