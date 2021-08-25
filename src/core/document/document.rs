use std::sync::Arc;
use std::sync::RwLock;

use std::fs::File;
use std::io::prelude::*;

//
use crate::core::editor::user_is_active;

use super::buffer::Buffer;
use super::buffer::OpenMode;

//
pub use super::bufferlog::BufferLog;
pub use super::bufferlog::BufferOperation;
pub use super::bufferlog::BufferOperationType;

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
    pub fn finalize<'a>(&self) -> Option<Arc<RwLock<Document<'a>>>> {
        let buffer = if self.file_name.is_empty() {
            Buffer::empty(self.mode.clone())?
        } else {
            Buffer::new(&self.file_name, self.mode.clone())?
        };

        let doc = Document {
            id: 0,
            name: self.document_name.clone(),
            buffer,
            cache: DocumentReadCache::new(), // TODO: have a per view cache or move to View
            buffer_log: BufferLog::new(),
            use_buffer_log: true,
            abort_indexing: false,
            changed: false,
            is_syncing: false,
            last_tag_time: std::time::Instant::now(),
            subscribers: vec![],
        };

        Some(Arc::new(RwLock::new(doc)))
    }
}

#[derive(Debug)]
pub struct DocumentReadCache {
    start: u64,
    end: u64,
    data: Vec<u8>,
    revision: usize,
}

impl DocumentReadCache {
    pub fn new() -> Self {
        DocumentReadCache {
            start: 0,
            end: 0,
            data: vec![],
            revision: 0,
        }
    }

    pub fn contains(&self, min: u64, max: u64) -> bool {
        if min < self.start || min > self.end {
            return false;
        }

        if max < self.start || max > self.end {
            return false;
        }

        return true;
    }

    pub fn read(
        &self,
        offset: u64,
        nr_bytes: usize,
        data: &mut Vec<u8>,
        doc_revision: usize,
    ) -> Option<usize> {
        if !crate::core::use_read_cache() {
            return None;
        }

        // no cache sync yet
        if self.revision != doc_revision {
            return None;
        }

        if self.start == self.end {
            return None;
        }

        if offset < self.start {
            return None;
        }

        if offset + nr_bytes as u64 > self.end {
            return None;
        }

        let idx = (offset - self.start) as usize;
        for i in 0..nr_bytes {
            data.push(self.data[i + idx]);
        }

        Some(nr_bytes)
    }
}

#[derive(Debug, Clone, Copy)]
pub struct DocumentEventSource {
    pub id: Id,
}

#[derive(Debug, Clone, Copy)]
pub struct DocumentEventDestination {
    pub id: Id,
}

pub enum DocumentEvent {
    Add,
    Open,
    Close,
    Remove,
    Change { op: BufferOperation },
}

type DocumentEventCb = fn(DocumentEventDestination, DocumentEvent);

#[derive(Debug)]
pub struct Document<'a> {
    pub id: Id,
    pub name: String,
    buffer: Buffer<'a>,
    cache: DocumentReadCache,
    pub buffer_log: BufferLog,
    pub use_buffer_log: bool,
    pub changed: bool,
    pub is_syncing: bool,
    pub abort_indexing: bool,
    pub last_tag_time: std::time::Instant,

    pub subscribers: Vec<(
        DocumentEventSource,
        DocumentEventDestination,
        DocumentEventCb,
    )>,
}

// NB: doc MUST be wrapped in Arc<RwLock<XXX>>
unsafe impl<'a> Send for Document<'a> {}
unsafe impl<'a> Sync for Document<'a> {}

impl<'a> Document<'a> {
    pub fn set_cache(&mut self, start: u64, end: u64) {
        if start > end {
            panic!("start {} > end {}", start, end);
        }
        self.cache.start = start;
        self.cache.end = end;
        if start == end {
            return;
        }
        self.cache.data.clear();

        let size = (end - start) as usize;
        let sz = self.buffer.read(start, size, &mut self.cache.data);
        self.cache.end = start + sz as u64;
        self.cache.data.shrink_to_fit(); // ?
    }

    pub fn build_cache(&self, start: u64, end: u64) -> DocumentReadCache {
        let mut cache = DocumentReadCache::new(); // TODO ::with_capacity()

        assert!(start <= end);
        cache.start = start;
        cache.end = end;
        if start == end {
            return cache;
        }

        let size = (end - start) as usize;
        let sz = self.buffer.read(start, size, &mut cache.data);
        cache.end = start + sz as u64;
        cache.data.shrink_to_fit(); // ?
        cache
    }

    pub fn get_cache_range(&self) -> (u64, u64) {
        (self.cache.start, self.cache.end)
    }

    pub fn file_name(&self) -> String {
        self.buffer.file_name.clone()
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

    pub fn is_cached(&self, start: u64, end: u64) -> bool {
        self.cache.contains(start, end)
    }

    pub fn readahead(&mut self, start: u64, end: u64) {
        self.cache = self.build_cache(start, end)
    }

    // read ahead

    /// copy the content of the buffer up to 'nr_bytes' into the data Vec
    /// the read bytes are appended to the data Vec
    /// return XXX on error (TODO: use ioresult)
    pub fn read(&self, offset: u64, nr_bytes: usize, data: &mut Vec<u8>) -> usize {
        let doc_rev = self.nr_changes();

        if let Some(size) = self.cache.read(offset, nr_bytes, data, doc_rev) {
            //dbg_println!("DATA IN CACHE offset {} size {}", offset, nr_bytes);

            // cache validation checks
            if false {
                let mut real = vec![];
                self.buffer.read(offset, nr_bytes, &mut real);
                assert!(real.len() == data.len());
                for i in 0..real.len() {
                    assert!(real[i] == data[i]);
                }
            }
            return size;
        }

        // dbg_println!("DATA NOT IN CACHE offset {} size {}", offset, nr_bytes);

        // TODO(ceg): --panic-on-read-cache-miss
        // panic!("");

        self.buffer.read(offset, nr_bytes, data)
    }

    /// copy the content of the buffer up to 'nr_bytes' into the data Vec
    /// the read bytes are appended to the data Vec
    /// return XXX on error (TODO: use ioresult)
    pub fn read_cached(
        &self,
        offset: u64,
        nr_bytes: usize,
        data: &mut Vec<u8>,
        cache: &DocumentReadCache,
    ) -> usize {
        let doc_rev = self.nr_changes();

        if let Some(size) = cache.read(offset, nr_bytes, data, doc_rev) {
            //dbg_println!("DATA IN CACHE offset {} size {}", offset, nr_bytes);

            // cache validation checks
            if false {
                let mut real = vec![];
                self.buffer.read(offset, nr_bytes, &mut real);
                assert!(real.len() == data.len());
                for i in 0..real.len() {
                    assert!(real[i] == data[i]);
                }
            }
            return size;
        }

        dbg_println!("DATA NOT IN CACHE offset {} size {}", offset, nr_bytes);

        self.buffer.read(offset, nr_bytes, data) // reread cache
    }

    pub fn buffer_log_pos(&self) -> usize {
        self.buffer_log.pos
    }

    pub fn buffer_log_reset(&mut self) {
        self.buffer_log.data.clear();
        self.buffer_log.pos = 0;
    }

    pub fn tag(&mut self, time: std::time::Instant, offset: u64, marks_offsets: Vec<u64>) -> bool {
        if !self.use_buffer_log {
            // return log disabled ?
            return false;
        }

        if self.last_tag_time == time {
            // ignore contiguous event ? config
            // return;
        }

        //dbg_println!("// doc.tag(..) offsets = {:?}", marks_offset);
        self.buffer_log.add(
            offset,
            BufferOperationType::Tag {
                time,
                marks_offsets,
            },
            None,
        );

        self.last_tag_time = time;
        true
    }

    pub fn get_tag_offsets(&mut self) -> Option<Vec<u64>> {
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
            BufferOperationType::Tag {
                ref marks_offsets, ..
            } => {
                Some(marks_offsets.clone()) // TODO: Arc<Vec<u64>>
            }
            _ => None,
        }
    }

    /// insert the 'data' Vec content in the buffer up to 'nr_bytes'
    /// return the number of written bytes (TODO: use io::Result)
    pub fn insert(&mut self, offset: u64, nr_bytes: usize, data: &[u8]) -> usize {
        // TODO: update cache if possible
        self.set_cache(0, 0); // invalidate cache,

        // log insert op
        let mut ins_data = Vec::with_capacity(nr_bytes);
        ins_data.extend(&data[..nr_bytes]);

        if self.use_buffer_log {
            self.buffer_log.add(
                offset,
                BufferOperationType::Insert,
                Some(Arc::new(ins_data)),
            );
        }

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
        // TODO: update cache if possible
        self.set_cache(0, 0); // invalidate cache,

        let mut rm_data = Vec::with_capacity(nr_bytes);

        let nr_bytes_removed = self.buffer.remove(offset, nr_bytes, Some(&mut rm_data));

        if let Some(v) = removed_data {
            v.extend(rm_data.clone());
        }

        if self.use_buffer_log {
            self.buffer_log
                .add(offset, BufferOperationType::Remove, Some(Arc::new(rm_data)));
        }

        if nr_bytes_removed > 0 {
            self.changed = true;
        }
        nr_bytes_removed
    }

    pub fn find(&self, offset: u64, data: &Vec<u8>) -> Option<u64> {
        self.buffer.find(offset, &data)
    }

    // TODO(ceg): return an array of offsets ?
    pub fn apply_operations(&mut self, ops: &[BufferOperation]) {
        for op in ops {
            self.apply_log_operation(op);
        }
    }

    fn apply_log_operation(&mut self, op: &BufferOperation) -> Option<u64> {
        // apply op
        dbg_println!("apply log op {:?}", op);

        let mark_offset = match op.op_type {
            BufferOperationType::Insert => {
                let sz = self.buffer.size();

                // TODO: check i/o errors
                let added = if let Some(data) = &op.data {
                    self.buffer.insert(op.offset, data.len(), &data);
                    self.changed = true;
                    data.len() as u64
                } else {
                    0
                };

                assert_eq!(sz + added as usize, self.buffer.size());

                op.offset + added
            }
            BufferOperationType::Remove => {
                let sz = self.buffer.size();

                // TODO: check i/o errors
                let _removed = if let Some(data) = &op.data {
                    let rm = self.buffer.remove(op.offset, data.len(), None);
                    self.changed = true;
                    assert_eq!(rm, data.len());
                    rm
                } else {
                    0
                };

                assert_eq!(sz - _removed, self.buffer.size());

                op.offset
            }
            BufferOperationType::Tag {
                marks_offsets: _, ..
            } => {
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

        // apply inverted previous operation
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

        // apply next operation
        let op = self.buffer_log.data[pos].clone();
        self.buffer_log.pos += 1;
        self.apply_log_operation(&op)
    }

    pub fn undo_until_tag(&mut self) -> Vec<BufferOperation> {
        dbg_println!("redo_until_tag: log data {:?}", self.buffer_log.data);

        // read current log position
        let mut ops = Vec::new();
        loop {
            if self.buffer_log.pos == 0 {
                dbg_println!("bufflog: undo self.buffer_log.pos == 0");
                break;
            }

            self.buffer_log.pos -= 1;
            let pos = self.buffer_log.pos;

            // get inverted operation
            let op = &self.buffer_log.data[pos];
            dbg_println!("bufflog: op[{}] = {:?}", pos, op);
            match op.op_type {
                BufferOperationType::Tag { .. } => {
                    if pos == self.buffer_log.data.len() - 1 {
                        // if on last op and last op is tag -> skip
                        dbg_println!("ignore last tag");
                        continue;
                    }
                    dbg_println!("found tag at pos {}", pos);

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

        dbg_println!(
            "bufflog: undo until tag END : self.buffer_log.pos == {}",
            self.buffer_log.pos
        );

        ops
    }

    pub fn redo_until_tag(&mut self) -> Vec<BufferOperation> {
        dbg_println!("redo_until_tag: log data {:?}", self.buffer_log.data);

        let mut ops = Vec::new();

        loop {
            // read current log position
            if self.buffer_log.pos == self.buffer_log.data.len() {
                dbg_println!("bufflog: no more op to redo");
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
            dbg_println!("bufflog: op[{}] = {:?}", pos, op);
            match op.op_type {
                BufferOperationType::Tag { .. } => {
                    dbg_println!("bufflog: redo_until_tag found tag at pos {}", pos);
                    break;
                }
                _ => {}
            }

            self.apply_log_operation(&op);
            ops.push(op);
        }

        dbg_println!(
            "bufflog: redo until tag END : self.buffer_log.pos == {}",
            self.buffer_log.pos
        );

        ops
    }
}

// helper

use std::ffi::CString;

extern crate libc;

use self::libc::unlink;

// TODO:
pub fn sync_to_storage(doc: &Arc<RwLock<Document>>) {
    // read/copy
    let mut fd = {
        let doc = doc.read().unwrap();

        if doc.file_name().is_empty() {
            // TODO: save as pop up/notification
            return;
        }

        let tmp_file_name = format!("{}{}", doc.file_name(), ".update"); // TODO: move to global config

        let path = CString::new(tmp_file_name.clone()).unwrap();
        unsafe { unlink(path.as_ptr()) };

        let fd = File::create(tmp_file_name);
        if fd.is_err() {
            dbg_println!("cannot save {}", doc.file_name());
            return;
        }
        fd.unwrap()
    };

    dbg_println!("SYNC: fd = {:?}", fd);

    let mut idx = {
        let doc = doc.read().unwrap();
        let file = doc.buffer.data.read().unwrap();
        let (node_index, _, _) = file.find_node_by_offset(0);
        node_index
    };

    while idx != None {
        // do not hold the doc.lock more
        {
            let doc = doc.read().unwrap();
            let file = doc.buffer.data.read().unwrap();
            let node = &file.pool[idx.unwrap()];

            let mut data = Vec::with_capacity(node.size as usize);
            unsafe {
                data.set_len(data.capacity());
            };

            if file.fd.is_none() {
                // TODO: save as pop up
                break;
            }

            let orig_fd = { Some(file.fd.as_ref().unwrap().clone()) };

            if let Some(_n) = node.do_direct_copy(&orig_fd, &mut data) {
                let nw = fd.write(&data).unwrap();
                if nw != data.len() {
                    dbg_println!("cannot save {}", doc.file_name());
                    panic!("");
                    // return false;
                }
                // dbg_println!("sync doc('{}') node {}", doc.file_name(), idx.unwrap());
            } else {
                panic!("direct copy failed");
            }

            idx = node.next;
        }

        // NB: experimental throttling based on user input freq/rendering
        // TODO <-- user configuration
        if user_is_active() == true {
            let wait = std::time::Duration::from_millis(16);
            std::thread::sleep(wait);
        }
    }

    // update
    {
        let mut doc = doc.write().unwrap();

        let metadata = ::std::fs::metadata(&doc.file_name()).unwrap();
        let perms = metadata.permissions();

        let tmp_file_name = format!("{}{}", doc.file_name(), ".update"); // TODO: move '.update' to global config

        {
            // TODO: large file warning in save ? disable backup ?
            let _tmp_backup_name = format!("{}{}", doc.file_name(), "~");
            // TODO: move '~' to global config
            // let _ = ::std::fs::rename(&doc.file_name(), &tmp_backup_name);
        }

        let _ = ::std::fs::rename(&tmp_file_name, &doc.file_name());

        // reopen file
        let new_fd = File::open(&doc.file_name()).unwrap();

        // TODO: handle skip with ReadOnly
        let mapped_file = doc.buffer.data.clone();
        let mut mapped_file = mapped_file.write().unwrap();
        crate::core::mapped_file::MappedFile::patch_storage_offset_and_file_descriptor(
            &mut mapped_file,
            new_fd,
        );

        // TODO: check result, handle io results properly
        // set buffer status to : permission denied etc
        let _ = ::std::fs::set_permissions(&doc.file_name(), perms);

        doc.changed = false;
        doc.is_syncing = false;
    }
}

pub fn build_index(doc: &Arc<RwLock<Document>>) {
    let mut idx = {
        let doc = doc.read().unwrap();
        {
            let file = doc.buffer.data.read().unwrap();
            let (node_index, _, _) = file.find_node_by_offset(0);
            if node_index.is_none() {
                return;
            };
            node_index
        }
    };

    let t0 = std::time::Instant::now();

    let mut total_byte_count: [u64; 256] = [0; 256];

    let mut data = vec![];
    while idx != None {
        // read node bytes
        {
            let doc = doc.read().unwrap();

            if doc.abort_indexing == true {
                break;
            }

            let file = doc.buffer.data.read().unwrap();
            let node = &file.pool[idx.unwrap()];
            if node.indexed == true {
                idx = node.next;
                continue;
            }

            data.reserve(node.size as usize);
            unsafe {
                data.set_len(node.size as usize);
            };

            let orig_fd = if file.fd.is_none() {
                None
            } else {
                Some(file.fd.as_ref().unwrap().clone())
            };

            if let Some(_n) = node.do_direct_copy(&orig_fd, &mut data) {
                dbg_println!(
                    "build index doc('{}') node {} size {}",
                    doc.file_name(),
                    idx.unwrap(),
                    data.len(),
                );
            } else {
                // TODO: return error
                panic!("direct copy failed");
            }
        }

        // count node bytes (no lock)
        let mut byte_count: [u64; 256] = [0; 256];
        for b in data.iter() {
            let byte_idx = *b as usize;
            byte_count[byte_idx] += 1;
            total_byte_count[byte_idx] += 1;
        }

        if user_is_active() == true {
            let wait = std::time::Duration::from_millis(16);
            std::thread::sleep(wait);
        }

        // update node info
        {
            let doc = doc.read().unwrap();
            let mut file = doc.buffer.data.write().unwrap();
            let mut node = &mut file.pool[idx.unwrap()];
            node.byte_count = byte_count;
            node.indexed = true;

            // TODO: notify subscribers
            /*
            doc.register_node_event_cb(cb);

            enum NodeEvent
            {
             NodeAdded { idx } ),
             NodeRemoved { idx } ),
             NodeIndexed { idx } ),
             NodeContentChanged { idx, Op { insert, remove }, local_offset, size }} ,
            }

            doc.get_node_mut(index) ->  &mut Node;

            fn node_event_cb(doc, ev: NodeEvent, &mut node);


            }


            doc.event_cb(NodeAdded, &mut node, idx);
            doc.event_cb(NodeRemoved, &mut node, idx);
            doc.event_cb(NodeIndexed, &mut node, idx);
            doc.event_cb(
                NodeContentChanged { Op { insert, remove }, local_offset, size }} ,
                &mut node,
                idx
            );
            */

            //
            idx = node.next;
        }
    }

    let t1 = std::time::Instant::now();
    dbg_println!("index time {:?} ms", (t1 - t0).as_millis());

    dbg_println!("Number of lines {}", total_byte_count[b'\n' as usize]);
}

#[cfg(test)]
mod tests {

    extern crate rand;

    use super::*;
    use rand::Rng;

    #[test]
    fn undo_redo() {
        let doc = DocumentBuilder::new()
            .document_name("untitled-1")
            .internal(false)
            .finalize();

        let mut doc = doc.as_ref().unwrap().write().unwrap();

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
        let doc = DocumentBuilder::new()
            .document_name("untitled-1")
            .internal(false)
            .finalize();

        let mut doc = doc.as_ref().unwrap().write().unwrap();

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
