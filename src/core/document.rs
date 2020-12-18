// Copyright (c) Carl-Erwin Griffith

//
use std::cell::RefCell;
use std::rc::Rc;

//

use crate::core::buffer::Buffer;
pub use crate::core::buffer::OpenMode;

//
use crate::core::bufferlog::BufferLog;
pub use crate::core::bufferlog::BufferOperation;
pub use crate::core::bufferlog::BufferOperationType;

//
pub type Id = u64; // TODO change to usize

///
#[derive(Debug)]
pub struct DocumentBuilder {
    internal: bool,
    document_name: String,
    file_name: String,
    mode: OpenMode,
}

///
impl DocumentBuilder {
    ///
    pub fn new() -> Self {
        Self {
            internal: false,
            document_name: String::new(),
            file_name: String::new(),
            mode: OpenMode::ReadOnly,
        }
    }

    ///
    pub fn internal(&mut self, flag: bool) -> &mut Self {
        self.internal = flag;
        self
    }

    ///
    pub fn document_name(&mut self, name: &str) -> &mut Self {
        self.document_name = name.to_string();
        self
    }

    ///
    pub fn file_name(&mut self, name: &str) -> &mut Self {
        self.file_name = name.to_string();
        self
    }

    ///
    pub fn mode(&mut self, mode: OpenMode) -> &mut Self {
        self.mode = mode;
        self
    }

    ///
    pub fn finalize<'a>(&self) -> Option<Rc<RefCell<Document<'a>>>> {
        let buffer = Buffer::new(&self.file_name, self.mode.clone());
        let buffer = match buffer {
            Some(bb) => bb,
            None => return None,
        };

        let mut doc = Document {
            id: 0,
            name: self.document_name.clone(),
            buffer,
            buffer_log: BufferLog::new(),
            changed: false,
        };
        // TODO: move to view
        // first tag at @
        doc.tag(0, vec![0]);

        Some(Rc::new(RefCell::new(doc)))
    }
}

#[derive(Debug)]
pub struct Document<'a> {
    pub id: Id,
    pub name: String,
    buffer: Buffer<'a>,
    pub buffer_log: BufferLog,
    pub changed: bool,
}

impl<'a> Document<'a> {
    pub fn sync_to_disk(&mut self) -> ::std::io::Result<()> {
        let tmp_file_ext = "unlimited.bk"; // TODO: move to global config
        let tmp_file_name = format!("{}.{}", self.buffer.file_name, tmp_file_ext);
        self.buffer.sync_to_disk(&tmp_file_name).unwrap();
        self.changed = false;

        Ok(())
    }

    /// copy the content of the buffer up to 'nr_bytes' into the data Vec
    /// the read bytes are appended to the data Vec
    /// return XXX on error (TODO: use ioresult)
    pub fn size(&self) -> usize {
        self.buffer.size
    }

    pub fn nr_changes(&self) -> usize {
        self.buffer.nr_changes() as usize
    }

    /// copy the content of the buffer up to 'nr_bytes' into the data Vec
    /// the read bytes are appended to the data Vec
    /// return XXX on error (TODO: use ioresult)
    pub fn read(&self, offset: u64, nr_bytes: usize, data: &mut Vec<u8>) -> usize {
        self.buffer.read(offset, nr_bytes, data)
    }

    pub fn tag(&mut self, offset: u64, marks: Vec<u64>) {
        //dbg_println!("doc.tag(..) offsets = {:?}", marks);
        self.buffer_log
            .add(offset, BufferOperationType::Tag { marks }, None);
    }

    pub fn get_tag_offset(&mut self) -> Option<Vec<u64>> {
        let dlen = self.buffer_log.data.len();
        if dlen == 0 {
            return None;
        }

        let pos = if self.buffer_log.pos == dlen {
            self.buffer_log.pos - 1
        } else {
            self.buffer_log.pos
        };

        // get inverted operation
        let op = &self.buffer_log.data[pos];
        match op.op_type {
            BufferOperationType::Tag { ref marks } => {
                Some(marks.clone()) // TODO: Rc<Vec<u64>>
            }
            _ => None,
        }
    }

    /// insert the 'data' Vec content in the buffer up to 'nr_bytes'
    /// return the number of written bytes (TODO: use io::Result)
    pub fn insert(&mut self, offset: u64, nr_bytes: usize, data: &[u8]) -> usize {
        // log insert op
        let mut ins_data = Vec::with_capacity(nr_bytes);
        ins_data.extend(&data[..nr_bytes]);

        self.buffer_log
            .add(offset, BufferOperationType::Insert, Some(Rc::new(ins_data)));

        let sz = self.buffer.insert(offset, nr_bytes, &data[..nr_bytes]);
        if sz > 0 {
            self.changed = true;
        }
        sz
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
            .add(offset, BufferOperationType::Remove, Some(Rc::new(rm_data)));
        if nr_bytes_removed > 0 {
            self.changed = true;
        }
        nr_bytes_removed
    }

    fn apply_log_operation(&mut self, op: &BufferOperation) -> Option<u64> {
        // apply op
        let mark_offset = match op.op_type {
            BufferOperationType::Insert => {
                // TODO: check i/o errors
                let added = if let Some(data) = &op.data {
                    self.buffer.insert(op.offset, data.len(), &data);
                    self.changed = true;
                    data.len() as u64
                } else {
                    0
                };
                op.offset + added
            }
            BufferOperationType::Remove => {
                // TODO: check i/o errors
                let _removed = if let Some(data) = &op.data {
                    let rm = self.buffer.remove(op.offset, data.len(), None);
                    self.changed = true;
                    assert_eq!(rm, data.len());
                    rm
                } else {
                    0
                };

                op.offset
            }
            BufferOperationType::Tag { marks: _ } => {
                /* nothing */
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

    pub fn undo_until_tag(&mut self) -> Vec<BufferOperation> {
        // read current log position
        let mut ops = Vec::new();
        loop {
            if self.buffer_log.pos == 0 {
                //dbg_println!("bufflog: undo self.buffer_log.pos == 0");
                break;
            }

            self.buffer_log.pos -= 1;
            let pos = self.buffer_log.pos;

            // get inverted operation
            let op = &self.buffer_log.data[pos];
            //dbg_println!("bufflog: op[{}] = {:?}", pos, op);
            match op.op_type {
                BufferOperationType::Tag { .. } => {
                    break;
                }
                _ => {}
            }

            // replay
            let inverted_op = op.invert();
            self.apply_log_operation(&inverted_op);
            //
            ops.push(inverted_op);
        }

        //dbg_println!(
        //    "bufflog: undo until tag END : self.buffer_log.pos == {}",
        //    self.buffer_log.pos
        //);

        ops
    }

    pub fn redo_until_tag(&mut self) -> Vec<BufferOperation> {
        let mut ops = Vec::new();

        loop {
            // read current log position
            if self.buffer_log.pos == self.buffer_log.data.len() {
                break;
            }
            // skip tag ?
            self.buffer_log.pos += 1;

            if self.buffer_log.pos == self.buffer_log.data.len() {
                break;
            }

            let pos = self.buffer_log.pos;
            // replay previous op
            let op = self.buffer_log.data[pos].clone();
            //dbg_println!("bufflog: op[{}] = {:?}", pos, op);
            match op.op_type {
                BufferOperationType::Tag { .. } => {
                    break;
                }
                _ => {}
            }

            self.apply_log_operation(&op);
            ops.push(op);
        }

        //dbg_println!(
        //    "bufflog: redo until tag END : self.buffer_log.pos == {}",
        //    self.buffer_log.pos
        //);

        ops
    }
}

#[cfg(test)]
mod tests {

    extern crate rand;

    use super::*;
    use rand::Rng;

    #[test]
    fn undo_redo() {
        let mut doc = DocumentBuilder::new()
            .document_name("untitled-1")
            .file_name("/dev/null")
            .internal(false)
            .finalize();

        let mut doc = doc.as_mut().unwrap().borrow_mut();

        const STR_LEN: usize = 1000;

        let mut s = String::new();
        for _ in 0..STR_LEN {
            s.push_str("0123456789012345678901234567890123456789012345678901234567890123456789012345678901234567890123456789\n");
        }

        const NB_INSERT: usize = 1000;
        let max = NB_INSERT;

        for _ in 0..10 {
            println!("start insert test");

            let mut off: u64 = 0;

            for i in 0..max {
                println!("insert ({}/{}) -------", i + 1, max);

                let off_update = doc.insert(off, s.len(), s.as_ref());
                off += off_update as u64;
            }

            println!("doc.size = {}", doc.size());

            println!("start undo test");
            for i in 0..max {
                println!("undo ({}/{}) -------", i + 1, max);
                doc.undo();
            }

            println!("doc.size = {}", doc.size());

            println!("start redo test");

            for i in 0..max {
                println!("redo ({}/{}) -------", i + 1, max);
                doc.redo();
            }

            println!("doc.size = {}", doc.size());

            println!("start undo test (2nd pass)");
            for i in 0..max {
                println!("undo ({}/{}) -------", i + 1, max);
                doc.undo();
            }

            println!("doc.size = {}", doc.size());
        }
    }

    #[test]
    fn doc_random_size_inserts() {
        let mut doc = DocumentBuilder::new()
            .document_name("untitled-1")
            .file_name("/dev/null")
            .internal(false)
            .finalize();

        let mut doc = doc.as_mut().unwrap().borrow_mut();

        const NB_STR: usize = 10000;

        let mut s = String::new();
        for _ in 0..NB_STR {
            s.push_str("0123456789012345678901234567890123456789012345678901234567890123456789012345678901234567890123456789\n");
        }

        const NB_INSERT: usize = 150;
        let max = NB_INSERT;

        let mut rng = rand::thread_rng();

        for _ in 0..10 {
            println!("start insert test");

            let mut off: u64 = 0;

            for i in 0..max {
                println!("insert ({}/{}) -------", i, max);

                // randomize s.len

                let random_size: usize = rng.gen_range(0, s.len());
                println!("random insert size = {}", random_size);
                let off_update = doc.insert(off, random_size, s.as_ref());
                off += off_update as u64;
            }

            println!("doc.size = {}", doc.size());

            for i in 0..max {
                println!("undo ({}/{}) -------", i + 1, max);

                doc.undo();
            }

            println!("doc.size = {}", doc.size());

            println!("start redo test");

            for i in 0..max {
                println!("redo ({}/{}) -------", i + 1, max);

                doc.redo();
            }

            println!("doc.size = {}", doc.size());

            for i in 0..max {
                println!("undo ({}/{}) -------", i + 1, max);

                doc.undo();
            }

            println!("doc.size = {}", doc.size());
        }
    }
}
