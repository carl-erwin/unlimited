use std::fmt;

use parking_lot::RwLock;
use std::cell::RefCell;
use std::sync::Arc;
use std::sync::Weak;

use std::fs::File;
use std::io::prelude::*;
use std::io::Result;

//
use crate::core::editor::user_is_active;

use crate::core::mapped_file::MappedFile;
use crate::core::mapped_file::MappedFileEvent;
use crate::core::mapped_file::UpdateHierarchyOp;

use crate::core::mapped_file::NodeIndex;

use super::buffer::Buffer;
use super::buffer::OpenMode;

use super::bufferlog::BufferLog;
use super::bufferlog::BufferOperation;
use super::bufferlog::BufferOperationType;

//

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Id(pub usize);

///
#[derive(Debug)]
pub struct DocumentBuilder {
    internal: bool,
    use_buffer_log: bool,
    document_name: String,
    file_name: String,
    mode: OpenMode,
}

#[derive(Debug)]
struct DocumentMappedFileEventHandler<'a> {
    _doc: Weak<RwLock<Document<'a>>>,
}

fn mapped_file_event_to_document_event(evt: &MappedFileEvent) -> DocumentEvent {
    match evt {
        MappedFileEvent::NodeChanged { node_index } => DocumentEvent::NodeChanged {
            node_index: *node_index,
        },
        MappedFileEvent::NodeAdded { node_index } => DocumentEvent::NodeAdded {
            node_index: *node_index,
        },
        MappedFileEvent::NodeRemoved { node_index } => DocumentEvent::NodeRemoved {
            node_index: *node_index,
        },
    }
}

///
impl DocumentBuilder {
    ///
    pub fn new() -> Self {
        Self {
            internal: false,
            use_buffer_log: false,
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
    pub fn use_buffer_log(&mut self, flag: bool) -> &mut Self {
        self.use_buffer_log = flag;
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
    pub fn finalize<'a>(&self) -> Option<Arc<RwLock<Document<'static>>>> {
        Document::new(
            &self.document_name,
            &self.file_name,
            self.mode.clone(),
            self.use_buffer_log,
        )
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

pub trait DocumentEventCb {
    fn cb(&mut self, doc: &Document, event: &DocumentEvent);
}

#[derive(Debug, Clone)]
pub enum DocumentEvent {
    DocumentAdded,
    DocumentOpened,
    DocumentClosed,
    DocumentRemoved,
    DocumentFullyIndexed,
    NodeAdded { node_index: usize },
    NodeChanged { node_index: usize },
    NodeRemoved { node_index: usize },
    NodeIndexed { node_index: usize },
}

fn document_event_to_string(evt: &DocumentEvent) -> String {
    match evt {
        DocumentEvent::DocumentAdded => "Added".to_owned(),
        DocumentEvent::DocumentOpened => "Opened".to_owned(),
        DocumentEvent::DocumentClosed => "Closed".to_owned(),
        DocumentEvent::DocumentRemoved => "Removed".to_owned(),
        DocumentEvent::DocumentFullyIndexed => "FullyIndexed".to_owned(),

        DocumentEvent::NodeAdded { node_index } => {
            format!("NodeAdded idx: {}", node_index)
        }
        DocumentEvent::NodeChanged { node_index } => {
            format!("NodeChanged idx: {}", node_index)
        }
        DocumentEvent::NodeRemoved { node_index, .. } => {
            format!("NodeRemoved idx: {}", node_index)
        }
        DocumentEvent::NodeIndexed { node_index, .. } => {
            format!("NodeIndexed idx: {}", node_index)
        }
    }
}

pub struct Document<'a> {
    pub id: Id,
    pub name: String,
    pub buffer: Buffer<'a>, // TODO(ceg): provide iterator apis ?
    cache: DocumentReadCache,
    pub buffer_log: BufferLog,
    pub use_buffer_log: bool,
    pub changed: bool,
    pub is_syncing: bool,
    pub abort_indexing: bool,
    pub indexed: bool,
    pub last_tag_time: std::time::Instant,
    pub subscribers: Vec<RefCell<Box<dyn DocumentEventCb>>>,
}

impl<'a> fmt::Debug for Document<'a> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Document {}")
            .field("id", &self.id)
            .field("name", &self.name)
            .finish()
    }
}

// NB: doc MUST be wrapped in Arc<RwLock<XXX>>
unsafe impl<'a> Send for Document<'a> {}
unsafe impl<'a> Sync for Document<'a> {}

impl<'a> Document<'a> {
    pub fn new(
        document_name: &String,
        file_name: &String,
        mode: OpenMode,
        use_buffer_log: bool,
    ) -> Option<Arc<RwLock<Document<'static>>>> {
        dbg_println!("try open {} {} {:?}", document_name, file_name, mode);

        let buffer = if file_name.is_empty() {
            Buffer::empty(mode.clone())
        } else {
            Buffer::new(&file_name, mode.clone())
        };

        let mut changed = false;
        // fallback
        let buffer = if buffer.is_none() {
            changed = true;
            Buffer::empty_with_name(&document_name, mode.clone())
        } else {
            buffer
        };

        let doc = Document {
            id: Id(0),
            name: document_name.clone(),
            buffer: buffer.unwrap(),
            cache: DocumentReadCache::new(), // TODO(ceg): have a per view cache or move to View
            buffer_log: BufferLog::new(),
            use_buffer_log,
            abort_indexing: false,
            indexed: false,
            changed,
            is_syncing: false,
            last_tag_time: std::time::Instant::now(),
            subscribers: vec![],
        };

        Some(Arc::new(RwLock::new(doc)))
    }

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

    pub fn metadata(&self) -> Result<std::fs::Metadata> {
        self.buffer.metadata()
    }

    /// copy the content of the buffer up to 'nr_bytes' into the data Vec
    /// the read bytes are appended to the data Vec
    /// return XXX on error (TODO(ceg): use ioresult)
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

    pub fn notify(&self, evt: &DocumentEvent) {
        dbg_println!(
            "notify {:?}, nb subscribers {}",
            document_event_to_string(&evt),
            self.subscribers.len()
        );
        for s in self.subscribers.iter() {
            s.borrow_mut().cb(self, evt);
        }
    }

    pub fn build_node_byte_count(&self, node_index: usize) {
        // let node_info = doc.get_node_info(node_index);
        let mut file = self.buffer.data.write();
        build_node_byte_count(&mut file, Some(node_index));
    }

    pub fn remove_node_byte_count(&self, node_index: usize) {
        // let node_info = doc.get_node_info(node_index);
        let mut file = self.buffer.data.write();
        remove_node_byte_count(&mut file, Some(node_index));
    }

    pub fn update_node_byte_count(&self, node_index: usize) {
        // let node_info = doc.get_node_info(node_index);
        let mut file = self.buffer.data.write();
        update_node_byte_count(&mut file, Some(node_index));
    }

    pub fn show_root_node_bytes_stats(&self) {
        // let node_info = doc.get_node_info(node_index);
        let file = self.buffer.data.read();
        if let Some(idx) = file.root_index() {
            let node = &file.pool[idx];
            if !node.indexed {
                return;
            }

            for (i, count) in node.byte_count.iter().enumerate() {
                if i == 10 {
                    dbg_println!("ROOT NODE byte_count[{}] = {}", i, count);
                }
            }
        }
    }

    // TODO(ceg): return cb slot / unregister slot_mask
    pub fn register_subscriber(&mut self, cb: Box<dyn DocumentEventCb>) -> usize {
        let len = 1 + self.subscribers.len();
        self.subscribers.push(RefCell::new(cb));
        len
    }

    // read ahead

    /// copy the content of the buffer up to 'nr_bytes' into the data Vec
    /// the read bytes are appended to the data Vec
    /// return XXX on error (TODO(ceg): use ioresult)
    pub fn read(&self, offset: u64, nr_bytes: usize, data: &mut Vec<u8>) -> usize {
        // return self.buffer.read(offset, nr_bytes, data);

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
    /// return XXX on error (TODO(ceg): use ioresult)
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

    pub fn buffer_log_count(&self) -> usize {
        self.buffer_log.data.len()
    }

    pub fn buffer_log_reset(&mut self) {
        self.buffer_log.data.clear();
        self.buffer_log.pos = 0;

        dbg_println!("bufferlog: cleared");
    }

    pub fn tag(
        &mut self,
        time: std::time::Instant,
        offset: u64,
        marks_offsets: Vec<u64>,
        selections_offsets: Vec<u64>,
    ) -> bool {
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
                selections_offsets,
            },
            None,
        );

        self.last_tag_time = time;
        true
    }

    pub fn get_tag_offsets(&mut self) -> Option<(Vec<u64>, Vec<u64>)> {
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
                ref marks_offsets,
                ref selections_offsets,
                ..
            } => {
                Some((marks_offsets.clone(), selections_offsets.clone())) // TODO(ceg): Arc<Vec<u64>>
            }
            _ => None,
        }
    }

    pub fn update_hierarchy_from_events(&self, events: &Vec<MappedFileEvent>) {
        for ev in events {
            match ev {
                MappedFileEvent::NodeChanged { node_index } => {
                    self.remove_node_byte_count(*node_index);
                    self.build_node_byte_count(*node_index);

                    let mut file = self.buffer.data.write();

                    // remove prev counts
                    update_byte_index_hierarchy(
                        &mut file,
                        Some(*node_index),
                        UpdateHierarchyOp::Sub,
                    );

                    // rebuild current counters

                    // add new count
                    update_byte_index_hierarchy(
                        &mut file,
                        Some(*node_index),
                        UpdateHierarchyOp::Add,
                    );
                }
                MappedFileEvent::NodeAdded { node_index } => {
                    self.build_node_byte_count(*node_index);
                }
                MappedFileEvent::NodeRemoved { node_index } => {
                    self.remove_node_byte_count(*node_index);
                }
            }
        }
    }

    /// insert the 'data' Vec content in the buffer up to 'nr_bytes'
    /// return the number of written bytes (TODO(ceg): use io::Result)
    pub fn insert(&mut self, offset: u64, nr_bytes: usize, data: &[u8]) -> usize {
        // TODO(ceg): update cache if possible
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

        let (sz, events) = self.buffer.insert(offset, nr_bytes, &data[..nr_bytes]);
        if sz > 0 {
            self.changed = true;
        }

        self.update_hierarchy_from_events(&events);

        for ev in &events {
            let ev = mapped_file_event_to_document_event(&ev);
            self.notify(&ev);
        }

        sz
    }

    pub fn append(&mut self, data: &[u8]) -> usize {
        let sz = self.size() as u64;
        self.insert(sz, data.len(), &data)
    }

    /// remove up to 'nr_bytes' from the buffer starting at offset
    /// if removed_data is provided will call self.read(offset, nr_bytes, data)
    /// before remove the bytes
    /*
       TODO(ceg): we want
       - remove the data
       - collect each leaf node impacted
       - update byte index from these nodes
       - call event subscriber
       - cleanup impacted nodes
    */
    pub fn remove(
        &mut self,
        offset: u64,
        nr_bytes: usize,
        removed_data: Option<&mut Vec<u8>>,
    ) -> usize {
        // TODO(ceg): update cache if possible
        self.set_cache(0, 0); // invalidate cache,

        let mut rm_data = Vec::with_capacity(nr_bytes);

        let (nr_bytes_removed, events) = self.buffer.remove(offset, nr_bytes, Some(&mut rm_data));

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

        self.update_hierarchy_from_events(&events);

        for ev in &events {
            let ev = mapped_file_event_to_document_event(&ev);
            self.notify(&ev);
        }

        nr_bytes_removed
    }

    pub fn delete_content(&mut self, removed_data: Option<&mut Vec<u8>>) -> usize {
        let sz = self.size();
        self.remove(0, sz, removed_data);

        sz
    }

    pub fn find(&self, data: &[u8], from_offset: u64, to_offset: Option<u64>) -> Option<u64> {
        self.buffer.find(&data, from_offset, to_offset)
    }

    pub fn find_reverse(
        &self,
        data: &[u8],
        from_offset: u64,
        to_offset: Option<u64>,
    ) -> Option<u64> {
        self.buffer.find_reverse(&data, from_offset, to_offset)
    }

    // TODO(ceg): return an array of offsets ?
    pub fn apply_operations(&mut self, ops: &[BufferOperation]) {
        for op in ops {
            self.apply_log_operation(op);
        }
    }

    fn apply_log_operation(&mut self, op: &BufferOperation) -> Option<u64> {
        // apply op

        dbg_println!("apply_log_operation");
        op.dump();

        let mark_offset = match op.op_type {
            BufferOperationType::Insert => {
                let sz = self.buffer.size();

                // TODO(ceg): check i/o errors
                let added = if let Some(data) = &op.data {
                    let (_, events) = self.buffer.insert(op.offset, data.len(), &data);
                    self.changed = true;

                    self.update_hierarchy_from_events(&events);

                    for ev in &events {
                        let ev = mapped_file_event_to_document_event(&ev);
                        self.notify(&ev);
                    }

                    data.len() as u64
                } else {
                    0
                };

                assert_eq!(sz + added as usize, self.buffer.size());

                op.offset + added
            }
            BufferOperationType::Remove => {
                let sz = self.buffer.size();

                // TODO(ceg): check i/o errors
                let _removed = if let Some(data) = &op.data {
                    let (rm, events) = self.buffer.remove(op.offset, data.len(), None);
                    self.changed = true;

                    self.update_hierarchy_from_events(&events);

                    for ev in &events {
                        let ev = mapped_file_event_to_document_event(&ev);
                        self.notify(&ev);
                    }

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

    pub fn is_buffer_log_op_tag(&self, index: usize) -> bool {
        if index >= self.buffer_log.data.len() {
            return false;
        }

        let op = &self.buffer_log.data[index];
        match op.op_type {
            BufferOperationType::Tag { .. } => true,
            _ => false,
        }
    }

    pub fn undo_until_tag(&mut self) -> Vec<BufferOperation> {
        // read current log position
        let mut ops = Vec::new();

        loop {
            if self.buffer_log.pos == 0 {
                break;
            }

            self.buffer_log.pos -= 1;
            let pos = self.buffer_log.pos;
            // get inverted operation
            let op = &self.buffer_log.data[pos];
            match op.op_type {
                BufferOperationType::Tag { .. } => {
                    // stop if no last tag
                    if pos != self.buffer_log.data.len() - 1 {
                        break;
                    }
                }
                _ => {}
            }

            // inverse replay
            let inverted_op = op.invert();
            self.apply_log_operation(&inverted_op);
            ops.push(inverted_op);
        }

        ops
    }

    pub fn redo_until_tag(&mut self) -> Vec<BufferOperation> {
        let mut ops = Vec::new();

        if self.buffer_log.data.is_empty() {
            return ops;
        }

        if self.buffer_log.pos >= self.buffer_log.data.len() - 1 {
            return ops;
        }
        // skip current tag
        if self.is_buffer_log_op_tag(self.buffer_log.pos) {
            self.buffer_log.pos += 1;
        }
        // replay until tag
        while !self.is_buffer_log_op_tag(self.buffer_log.pos) {
            if self.buffer_log.pos >= self.buffer_log.data.len() - 1 {
                break;
            }
            let op = self.buffer_log.data[self.buffer_log.pos].clone();
            // actual replay
            self.apply_log_operation(&op);
            ops.push(op);
            self.buffer_log.pos += 1;
        }

        ops
    }
}

// helper
use std::path::Path;

// TODO(ceg): handle errors
pub fn sync_to_storage(doc: &Arc<RwLock<Document>>) {
    // read/copy
    let mut fd = {
        let doc = doc.read();

        if doc.file_name().is_empty() {
            // TODO(ceg): save as pop up/notification
            dbg_println!("cannot dsave  target filename is empty");
            return;
        }

        let tmp_file_name = format!("{}{}", doc.file_name(), ".update"); // TODO(ceg): move to global config

        let path = Path::new(&tmp_file_name);
        if let Result::Err(_) = std::fs::remove_file(path) {}

        let fd = File::create(path);
        if fd.is_err() {
            dbg_println!("cannot save {}", doc.file_name());
            return;
        }
        fd.unwrap()
    };

    dbg_println!("SYNC: fd = {:?}", fd);

    let mut idx = {
        let doc = doc.read();
        let file = doc.buffer.data.read();
        let (node_index, _, _) = file.find_node_by_offset(0);
        node_index
    };

    while idx != None {
        // do not hold the doc.lock more
        {
            let doc = doc.read();
            let file = doc.buffer.data.read();
            let node = &file.pool[idx.unwrap()];

            let mut data = Vec::with_capacity(node.size as usize);
            unsafe {
                data.set_len(data.capacity());
            };

            if file.fd.is_none() {
                // TODO(ceg): "save as" pop up
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

            idx = node.link.next;
        }

        // NB: experimental throttling based on user input freq/rendering
        // TODO <-- user configuration
        if user_is_active() {
            let wait = std::time::Duration::from_millis(16);
            std::thread::sleep(wait);
        }
    }

    // update
    {
        use std::os::unix::fs::PermissionsExt;

        let mut doc = doc.write();

        // TODO(ceg): use mapped file fd, will panic if file is removed
        let perms = match doc.metadata() {
            Ok(metadata) => metadata.permissions(),
            Err(_) => std::fs::Permissions::from_mode(0o600),
        };

        let tmp_file_name = format!("{}{}", doc.file_name(), ".update"); // TODO(ceg): move '.update' to global config

        {
            // TODO(ceg): large file warning in save ? disable backup ?
            let _tmp_backup_name = format!("{}{}", doc.file_name(), "~");
            // TODO(ceg): move '~' to global config
            // let _ = ::std::fs::rename(&doc.file_name(), &tmp_backup_name);
        }

        let _ = ::std::fs::rename(&tmp_file_name, &doc.file_name());

        // reopen file
        let new_fd = File::open(&doc.file_name()).unwrap();

        // TODO(ceg): handle skip with ReadOnly
        let mapped_file = doc.buffer.data.clone();
        let mut mapped_file = mapped_file.write();
        crate::core::mapped_file::MappedFile::patch_storage_offset_and_file_descriptor(
            &mut mapped_file,
            new_fd,
        );

        // TODO(ceg): check result, handle io results properly
        // set buffer status to : permission denied etc
        let _ = ::std::fs::set_permissions(&doc.file_name(), perms);

        doc.changed = false;
        doc.is_syncing = false;
    }
}

fn update_byte_index_hierarchy(
    file: &mut MappedFile,
    idx: Option<NodeIndex>,
    op: UpdateHierarchyOp,
) {
    if idx.is_none() {
        return;
    }
    let idx = idx.unwrap();

    // get counters
    let node = &mut file.pool[idx];
    let byte_count = node.byte_count.clone();

    let mut p = node.link.parent;

    while p.is_some() {
        let p_idx = p.unwrap();

        let p_node = &mut file.pool[p_idx];
        for (i, count) in byte_count.iter().enumerate() {
            match op {
                UpdateHierarchyOp::Add => p_node.byte_count[i] += count,
                UpdateHierarchyOp::Sub => {
                    p_node.byte_count[i] = p_node.byte_count[i].saturating_sub(*count)
                } // TODO(ceg): -=
            }
        }
        p_node.indexed = true;

        p = p_node.link.parent;
    }
}

pub fn get_node_data(file: &mut MappedFile, idx: Option<NodeIndex>) -> Vec<u8> {
    if idx.is_none() {
        return vec![];
    }

    let idx = idx.unwrap();

    let node = &mut file.pool[idx];
    let mut data = Vec::with_capacity(node.size as usize);
    unsafe {
        data.set_len(node.size as usize);
    };

    let orig_fd = if file.fd.is_none() {
        None
    } else {
        Some(file.fd.as_ref().unwrap().clone())
    };

    if let Some(_n) = node.do_direct_copy(&orig_fd, &mut data) {
        //
    } else {
        // TODO(ceg): return error
        panic!("direct copy failed");
    }

    data
}

// call this on new node
pub fn build_node_byte_count(mut file: &mut MappedFile, idx: Option<NodeIndex>) {
    if idx.is_none() {
        return;
    }

    let idx = idx.unwrap();

    let node = &mut file.pool[idx];
    let mut data = Vec::with_capacity(node.size as usize);
    unsafe {
        data.set_len(node.size as usize);
    };

    let orig_fd = if file.fd.is_none() {
        None
    } else {
        Some(file.fd.as_ref().unwrap().clone())
    };

    if let Some(_n) = node.do_direct_copy(&orig_fd, &mut data) {
        //
    } else {
        // TODO(ceg): return error
        panic!("direct copy failed");
    }

    assert!(!node.indexed);
    //    node.byte_count = [0;256];

    // count node bytes (no lock)
    for b in data.iter() {
        let byte_idx = *b as usize;
        if *b as char == '\n' {
            node.byte_count[byte_idx] += 1;
        }
    }
    node.indexed = true;

    update_byte_index_hierarchy(&mut file, Some(idx), UpdateHierarchyOp::Add);
}

// call this on new node
pub fn remove_node_byte_count(mut file: &mut MappedFile, idx: Option<NodeIndex>) {
    if idx.is_none() {
        return;
    }

    let idx = idx.unwrap();

    let node = &mut file.pool[idx];
    if !node.indexed {
        return;
    }

    update_byte_index_hierarchy(&mut file, Some(idx), UpdateHierarchyOp::Sub);

    let node = &mut file.pool[idx];
    node.byte_count = [0; 256];
    node.indexed = false;
}

// call this on new node
pub fn update_node_byte_count(mut file: &mut MappedFile, idx: Option<NodeIndex>) {
    if idx.is_none() {
        return;
    }

    let idx = idx.unwrap();

    let node = &mut file.pool[idx];
    if !node.indexed {
        return;
    }

    node.indexed = false;
    update_byte_index_hierarchy(&mut file, Some(idx), UpdateHierarchyOp::Sub);
}

// TODO(ceg): split code to provide index_single_node(nid)
pub fn build_index(doc: &Arc<RwLock<Document>>) {
    let mut idx = {
        let doc = doc.read();
        {
            if doc.indexed {
                return;
            }

            let file = doc.buffer.data.read();
            let (node_index, _, _) = file.find_node_by_offset(0);
            node_index
        }
    };

    let t0 = std::time::Instant::now();

    let mut data = vec![];
    while idx != None {
        // read node bytes
        {
            let doc = doc.read();
            if doc.abort_indexing {
                break;
            }

            let file = doc.buffer.data.read();
            let node = &file.pool[idx.unwrap()];
            if node.indexed {
                idx = node.link.next;
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

            let t0_read = std::time::Instant::now();
            if let Some(_n) = node.do_direct_copy(&orig_fd, &mut data) {
                dbg_println!(
                    "build index doc('{}') node {} size {}",
                    doc.file_name(),
                    idx.unwrap(),
                    data.len(),
                );
            } else {
                // TODO(ceg): return error
                panic!("direct copy failed");
            }
            let t1_read = std::time::Instant::now();
            dbg_println!("read node time {:?} ms", (t1_read - t0_read).as_millis());
        }

        // count node bytes (no lock)
        let mut byte_count: [u64; 256] = [0; 256];
        for b in data.iter() {
            let byte_idx = *b as usize;
            byte_count[byte_idx] += 1;
        }

        // yield some cpu time
        if user_is_active() {
            let wait = std::time::Duration::from_millis(16);
            std::thread::sleep(wait);
        }

        // update node info (idx)
        {
            let doc = doc.read();
            let mut file = doc.buffer.data.write();

            let node_index = idx.unwrap();

            // save byte counters
            {
                let mut node = &mut file.pool[node_index];
                node.byte_count = byte_count;
                node.indexed = true;
                idx = node.link.next;
            }

            update_byte_index_hierarchy(&mut file, Some(node_index), UpdateHierarchyOp::Add);
        }

        // notify subscribers
        if idx.is_some() {
            let doc = doc.read();
            doc.notify(&DocumentEvent::NodeIndexed {
                node_index: idx.unwrap(),
            });
        }
    }

    let t1 = std::time::Instant::now();
    dbg_println!("index time {:?} ms", (t1 - t0).as_millis());

    {
        // set index status flags
        {
            let mut doc = doc.write();
            if !doc.abort_indexing {
                doc.indexed = true;
            }

            // display root node info
            let file = doc.buffer.data.read();
            if let Some(root_index) = file.root_index() {
                let node = &file.pool[root_index];
                dbg_println!(
                    "{} : Number of lines {}",
                    doc.file_name(),
                    node.byte_count[b'\n' as usize]
                );
            }
        }

        let doc = doc.read();
        if doc.indexed {
            doc.notify(&DocumentEvent::DocumentFullyIndexed {});
        }
    }
}

//
// walk through the binary tree and while looking for the node containing "offset"
// and track byte_index count
//                                   SZ(19)   ,       LF(9)
//                   _________[ SZ(7+12),  LF(3+6) ]____________________
//                  /                                                 \
//        __[ 7=SZ(3+4), LF 3=(1+2) ]__                        _____[ 12=(5+7),  LF 6=(2+4) ]__
//       /                             \                      /                                 \
//  [SZ(3), LF(1)]={a,LF,b}    [SZ(4), LF(2)]={a,LF,LF,b }   [5, LF(2)] data{a,LF,b,LF,c} [SZ(7), LF(4)]={a ,LF,LF,b ,Lf,LF,c}
//                  0,1 ,2                     3, 4, 5,6                     7, 8,9,10,11                 12,13,14,15,16,17,18
//
//
// return (line_count, offset's node_index)
pub fn get_document_byte_count_at_offset(
    doc: &Document,
    byte_index: usize,
    offset: u64,
) -> (u64, Option<usize>) {
    assert!(byte_index < 256);

    let mut total_count = 0;
    let mut local_offset = offset;

    let mut file = doc.buffer.data.write();

    let mut cur_index = file.root_index();
    while cur_index != None {
        let idx = cur_index.unwrap();
        let p_node = &file.pool[idx];

        let is_leaf = p_node.link.left.is_none() && p_node.link.right.is_none();
        if is_leaf {
            let data = get_node_data(&mut file, Some(idx));

            // count by until local_offset is reached
            for b in data.iter().take(local_offset as usize) {
                if *b as usize == byte_index {
                    total_count += 1;
                }
            }
            return (total_count, cur_index);
        }

        if let Some(left_index) = p_node.link.left {
            let left_node = &file.pool[left_index];

            if local_offset < left_node.size {
                cur_index = Some(left_index);
                continue;
            }

            total_count += left_node.byte_count[byte_index];
            local_offset -= left_node.size
        }

        cur_index = p_node.link.right;
    }

    (0, None)
}

pub fn get_document_byte_count(doc: &Document, byte_index: usize) -> Option<u64> {
    assert!(byte_index < 256);
    let file = doc.buffer.data.read();
    match file.root_index() {
        Some(idx) => Some(file.pool[idx].byte_count[byte_index]),
        _ => None,
    }
}

//
// walk through the binary tree and while looking for the node containing "offset"
// and track byte_index count
//                                   SZ(19)   ,       LF(9)
//                   _________[ SZ(7+12),  LF(3+6) ]____________________
//                  /                                                 \
//        __[ 7=SZ(3+4), LF 3=(1+2) ]__                        _____[ 12=(5+7),  LF 6=(2+4) ]__
//       /                             \                      /                                 \
//  [SZ(3), LF(1)]={a,LF,b}    [SZ(4), LF(2)]={a,LF,LF,b }   [5, LF(2)] data{a,LF,b,LF,c} [SZ(7), LF(4)]={a ,LF,LF,b ,Lf,LF,c}
//                  0,1 ,2                     3, 4, 5,6                     7, 8,9,10,11                 12,13,14,15,16,17,18
//
pub fn find_nth_byte_offset(doc: &Document, byte: u8, index: u64) -> Option<u64> {
    assert!(index > 0);

    let mut index = index;

    let mut file = doc.buffer.data.write();
    let mut global_offset = 0;

    let mut cur_index = file.root_index();
    while cur_index != None {
        let idx = cur_index.unwrap();
        let p_node = &file.pool[idx];

        let is_leaf = p_node.link.left.is_none() && p_node.link.right.is_none();
        if is_leaf {
            let data = get_node_data(&mut file, Some(idx));

            // count by until index is reached
            for b in data.iter() {
                if *b == byte {
                    index -= 1;
                    if index == 0 {
                        break;
                    };
                }
                global_offset += 1;
            }

            return Some(global_offset);
        }

        let count = file.pool[idx].byte_count[byte as usize];
        if index >= count {
            // not fully indexed, or this byte does not exists
            return None;
        }

        if let Some(left_index) = p_node.link.left {
            let left_node = &file.pool[left_index];
            let left_byte_count = left_node.byte_count[byte as usize];

            // byte in left sub-tree ?
            if index <= left_byte_count {
                cur_index = Some(left_index);
                continue;
            }

            global_offset += left_node.size; // count skipped offsets
            index -= left_byte_count;
        }

        // byte in right sub-tree
        cur_index = p_node.link.right;
    }

    None
}

#[cfg(test)]
mod tests {

    extern crate rand;

    use super::*;
    use rand::Rng;

    #[test]
    fn doc_read() {
        use std::io::prelude::*;
        use std::os::unix::prelude::FileExt;

        let filename = "/tmp/unl-test-file".to_owned();
        let _ = std::fs::remove_file(&filename);
        {
            println!("create file....");
            let mut file = std::fs::File::create(&filename).unwrap();
            let size = 2 * 1024 * 1024 * 1024;
            let mut buf = Vec::with_capacity(size);
            //buf.resize(size, 0);
            unsafe {
                buf.set_len(size);
            } // faster in debug build
            file.write_all(&buf).unwrap();
            println!("create file....ok");
        }

        println!("read file....");

        let doc = DocumentBuilder::new()
            .document_name("untitled-1")
            .file_name(&filename)
            .internal(false)
            .finalize();

        build_index(doc.as_ref().unwrap());

        let file = std::fs::File::open(&filename).unwrap();
        let doc = doc.as_ref().unwrap().write();
        let doc_size = doc.size() as u64;
        let step = 1024 * 1024;
        let t0_read = std::time::Instant::now();
        let mut prev_time = 0;

        let mut data: Vec<u8> = Vec::with_capacity(step);
        for offset in (0..doc_size).into_iter().step_by(step) {
            if !true {
                unsafe {
                    data.set_len(step);
                } // faster in debug build

                //                data.clear();
                doc.read(offset, step, &mut data);
            } else {
                unsafe {
                    data.set_len(step);
                } // faster in debug build

                let res = file.read_at(&mut data[0..step], offset);
                match res {
                    Ok(_size) => {
                        //println!("read [{}] @ offset {}", size, offset);
                    }
                    Err(what) => {
                        println!("read error [{}] @ offset {}, what {:?}", step, offset, what);
                    }
                }
            }

            let diff = t0_read.elapsed().as_secs();
            if prev_time != diff {
                let bytes_per_seconds = offset / diff;
                println!("bytes per seconds {}", bytes_per_seconds);
                println!("kib   per seconds {}", bytes_per_seconds / 1024);
                println!("mib   per seconds {}", bytes_per_seconds / (1024 * 1024));
                println!(
                    "gib   per seconds {}",
                    bytes_per_seconds / (1024 * 1024 * 1024)
                );

                prev_time = diff;
            }
        }

        //        let _ = std::fs::remove_file(&filename);
    }

    #[test]
    fn undo_redo() {
        let doc = DocumentBuilder::new()
            .document_name("untitled-1")
            .internal(false)
            .finalize();

        let mut doc = doc.as_ref().unwrap().write();

        const STR_LEN: usize = 1000;

        let mut s = String::new();
        for _ in 0..STR_LEN {
            s.push_str("0123456789012345678901234567890123456789012345678901234567890123456789012345678901234567890123456789\n");
        }

        const NB_INSERT: usize = 100;
        let max = NB_INSERT;

        for _ in 0..5 {
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

        let mut doc = doc.as_ref().unwrap().write();

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
