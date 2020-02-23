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

//
use std::cell::RefCell;
use std::rc::Rc;

//
use crate::core::buffer::Buffer;
use crate::core::buffer::OpenMode;

//
use crate::core::bufferlog::BufferLog;
use crate::core::bufferlog::BufferOperation;
use crate::core::bufferlog::BufferOperationType;

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
            .add(offset, BufferOperationType::Remove, rm_data);
        if nr_bytes_removed > 0 {
            self.changed = true;
        }
        nr_bytes_removed
    }

    fn apply_log_operation(&mut self, op: &BufferOperation) -> Option<u64> {
        // apply op
        let mark_offset = match op.op {
            BufferOperationType::Insert => {
                // TODO: check i/o errors
                self.buffer.insert(op.offset, op.data.len(), &op.data);
                self.changed = true;

                op.offset + op.data.len() as u64
            }
            BufferOperationType::Remove => {
                // TODO: check i/o errors
                self.buffer.remove(op.offset, op.data.len(), None);
                self.changed = true;

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

            println!("doc.size = {}", doc.buffer.size());

            println!("start undo test");
            for i in 0..max {
                println!("undo ({}/{}) -------", i + 1, max);
                doc.undo();
            }

            println!("doc.size = {}", doc.buffer.size());

            println!("start redo test");

            for i in 0..max {
                println!("redo ({}/{}) -------", i + 1, max);
                doc.redo();
            }

            println!("doc.size = {}", doc.buffer.size());

            println!("start undo test (2nd pass)");
            for i in 0..max {
                println!("undo ({}/{}) -------", i + 1, max);
                doc.undo();
            }

            println!("doc.size = {}", doc.buffer.size());
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

            println!("doc.size = {}", doc.buffer.size());

            for i in 0..max {
                println!("undo ({}/{}) -------", i + 1, max);

                doc.undo();
            }

            println!("doc.size = {}", doc.buffer.size());

            println!("start redo test");

            for i in 0..max {
                println!("redo ({}/{}) -------", i + 1, max);

                doc.redo();
            }

            println!("doc.size = {}", doc.buffer.size());

            for i in 0..max {
                println!("undo ({}/{}) -------", i + 1, max);

                doc.undo();
            }

            println!("doc.size = {}", doc.buffer.size());
        }
    }
}
