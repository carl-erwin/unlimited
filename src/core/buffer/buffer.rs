use std::fmt;

use parking_lot::RwLock;
use std::cell::RefCell;
use std::sync::Arc;

use std::fs::File;
use std::io::prelude::*;
use std::io::Result;
use std::sync::atomic::{AtomicUsize, Ordering};

//
use crate::core::editor::user_is_active;

use crate::core::mapped_file::MappedFile;

pub use crate::core::mapped_file::Id;

use crate::core::mapped_file::MappedFileEvent;

use crate::core::mapped_file::UpdateHierarchyOp;

use crate::core::mapped_file::NodeIndex;

use super::inner::InnerBuffer;
use super::inner::OpenMode;

use super::bufferlog::BufferLog;
use super::bufferlog::BufferOperation;
use super::bufferlog::BufferOperationType;

static BUFFER_ID: AtomicUsize = AtomicUsize::new(1);

/*
  We will filter command line file list array

  "+${line}", "filename" ->  FileInfo { name: start_line:${line}, start_column:1 }
  "+@${offset}", "filename" ->  FileInfo { name: start_line:${line}, start_column:1, offset:${off} }
  "filename:${line}:${col}" ->  FileInfo { name: start_line:${num}, start_column:${col} }
*/
#[derive(Debug, Copy, Clone)]
pub struct BufferPosition {
    pub offset: Option<u64>,
    pub line: Option<u64>,
    pub column: Option<u64>,
}

impl BufferPosition {
    pub fn new() -> Self {
        BufferPosition {
            offset: None,
            line: None,
            column: None,
        }
    }
}

///
#[derive(Debug)]
pub struct BufferBuilder {
    kind: BufferKind,
    internal: bool,
    use_buffer_log: bool,
    buffer_name: String,
    file_name: String,
    mode: OpenMode,
    start_position: BufferPosition,
}

fn mapped_file_event_to_buffer_event(buffer_id: Id, evt: &MappedFileEvent) -> BufferEvent {
    match evt {
        MappedFileEvent::NodeChanged { node_index } => BufferEvent::BufferNodeChanged {
            buffer_id,
            node_index: *node_index,
        },
        MappedFileEvent::NodeAdded { node_index } => BufferEvent::BufferNodeAdded {
            buffer_id,
            node_index: *node_index,
        },
        MappedFileEvent::NodeRemoved { node_index } => BufferEvent::BufferNodeRemoved {
            buffer_id,
            node_index: *node_index,
        },
    }
}

///
impl BufferBuilder {
    ///
    pub fn new(kind: BufferKind) -> Self {
        Self {
            kind,
            internal: false,
            use_buffer_log: false,
            buffer_name: String::new(),
            file_name: String::new(),
            mode: OpenMode::ReadOnly,
            start_position: BufferPosition::new(),
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
    pub fn buffer_name(&mut self, name: &str) -> &mut Self {
        self.buffer_name = name.to_string();
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

    pub fn start_position(&mut self, position: BufferPosition) -> &mut Self {
        self.start_position = position;
        self
    }

    ///
    pub fn finalize<'a>(&self) -> Option<Arc<RwLock<Buffer<'static>>>> {
        Buffer::new(
            self.kind,
            &self.buffer_name,
            &self.file_name,
            self.mode.clone(),
            self.use_buffer_log,
            self.start_position,
        )
    }
}

#[derive(Debug)]
pub struct BufferReadCache {
    start: u64,
    end: u64,
    data: Vec<u8>,
    revision: usize,
}

impl BufferReadCache {
    pub fn new() -> Self {
        BufferReadCache {
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
        buffer_revision: usize,
    ) -> Option<usize> {
        if !crate::core::use_read_cache() {
            return None;
        }

        // no cache sync yet
        if self.revision != buffer_revision {
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

pub trait BufferEventCb {
    fn cb(&mut self, buffer: &Buffer, event: &BufferEvent);
}

// PartialEq, Eq
#[derive(Debug, Clone)]
pub enum BufferEvent {
    BufferFullyIndexed { buffer_id: Id },
    BufferNodeAdded { buffer_id: Id, node_index: usize },
    BufferNodeChanged { buffer_id: Id, node_index: usize },
    BufferNodeRemoved { buffer_id: Id, node_index: usize },
    BufferNodeIndexed { buffer_id: Id, node_index: usize },
}

fn buffer_event_to_string(evt: &BufferEvent) -> String {
    match evt {
        BufferEvent::BufferFullyIndexed { .. } => "FullyIndexed".to_owned(),

        BufferEvent::BufferNodeAdded {
            buffer_id: _,
            node_index,
        } => {
            format!("NodeAdded idx: {}", node_index)
        }
        BufferEvent::BufferNodeChanged {
            buffer_id: _,
            node_index,
        } => {
            format!("NodeChanged idx: {}", node_index)
        }
        BufferEvent::BufferNodeRemoved {
            buffer_id: _,
            node_index,
            ..
        } => {
            format!("NodeRemoved idx: {}", node_index)
        }
        BufferEvent::BufferNodeIndexed {
            buffer_id: _,
            node_index,
            ..
        } => {
            format!("NodeIndexed idx: {}", node_index)
        }
    }
}

#[derive(Debug, Copy, Clone, PartialEq)]
pub enum BufferKind {
    File,
    Directory,
}

pub struct Buffer<'a> {
    pub kind: BufferKind,
    pub id: Id,
    pub name: String,
    pub inner: InnerBuffer<'a>, // TODO(ceg): provide iterator apis ?
    cache: BufferReadCache,
    pub buffer_log: BufferLog,
    pub start_position: BufferPosition,
    pub use_buffer_log: bool,
    pub changed: bool,
    pub is_syncing: bool,
    pub abort_indexing: bool,
    pub indexed: bool,
    pub last_tag_time: std::time::Instant,
    pub subscribers: Vec<RefCell<Box<dyn BufferEventCb>>>,
}

impl<'a> fmt::Debug for Buffer<'a> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Buffer {}")
            .field("id", &self.id)
            .field("name", &self.name)
            .finish()
    }
}

// NB: buffer MUST be wrapped in Arc<RwLock<XXX>>
unsafe impl<'a> Send for Buffer<'a> {}
unsafe impl<'a> Sync for Buffer<'a> {}

impl<'a> Buffer<'a> {
    pub fn new(
        kind: BufferKind,
        buffer_name: &String,
        file_name: &String,
        mode: OpenMode,
        use_buffer_log: bool,
        start_position: BufferPosition,
    ) -> Option<Arc<RwLock<Buffer<'static>>>> {
        dbg_println!("try open {} {} {:?}", buffer_name, file_name, mode);

        let id = BUFFER_ID.fetch_add(1, Ordering::SeqCst);
        let id = Id(id);

        let inner = match kind {
            BufferKind::File => {
                if file_name.is_empty() {
                    InnerBuffer::empty(id, mode)
                } else {
                    InnerBuffer::new(id, &file_name, mode)
                }
            }

            BufferKind::Directory => InnerBuffer::empty(id, mode),
        };

        let mut changed = false;
        // fallback
        let inner = if inner.is_none() {
            changed = true;
            InnerBuffer::empty_with_name(id, &buffer_name, mode)
        } else {
            inner
        };

        let buffer = Buffer {
            id,
            kind,
            name: buffer_name.clone(),
            inner: inner.unwrap(),
            cache: BufferReadCache::new(), // TODO(ceg): have a per view cache or move to View
            buffer_log: BufferLog::new(),
            use_buffer_log,
            start_position,
            abort_indexing: false,
            indexed: false,
            changed,
            is_syncing: false,
            last_tag_time: std::time::Instant::now(),
            subscribers: vec![],
        };

        Some(Arc::new(RwLock::new(buffer)))
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
        let sz = self.inner.read(start, size, &mut self.cache.data);
        self.cache.end = start + sz as u64;
        self.cache.data.shrink_to_fit(); // ?
    }

    pub fn build_cache(&self, start: u64, end: u64) -> BufferReadCache {
        let mut cache = BufferReadCache::new(); // TODO ::with_capacity()

        assert!(start <= end);
        cache.start = start;
        cache.end = end;
        if start == end {
            return cache;
        }

        let size = (end - start) as usize;
        let sz = self.inner.read(start, size, &mut cache.data);
        cache.end = start + sz as u64;
        cache.data.shrink_to_fit(); // ?
        cache
    }

    pub fn get_cache_range(&self) -> (u64, u64) {
        (self.cache.start, self.cache.end)
    }

    pub fn file_name(&self) -> String {
        self.inner.file_name.clone()
    }

    pub fn metadata(&self) -> Result<std::fs::Metadata> {
        self.inner.metadata()
    }

    pub fn size(&self) -> usize {
        self.inner.size
    }

    pub fn start_offset(&self) -> u64 {
        0
    }

    pub fn end_offset(&self) -> u64 {
        self.inner.size as u64
    }

    pub fn nr_changes(&self) -> usize {
        self.inner.nr_changes() as usize
    }

    pub fn is_cached(&self, start: u64, end: u64) -> bool {
        self.cache.contains(start, end)
    }

    pub fn readahead(&mut self, start: u64, end: u64) {
        self.cache = self.build_cache(start, end)
    }

    pub fn notify(&self, evt: &BufferEvent) {
        dbg_println!(
            "notify {:?}, nb subscribers {}",
            buffer_event_to_string(&evt),
            self.subscribers.len()
        );
        for s in self.subscribers.iter() {
            s.borrow_mut().cb(self, evt);
        }
    }

    pub fn build_node_byte_count(&self, node_index: usize) {
        // let node_info = buffer.get_node_info(node_index);
        let mut file = self.inner.data.write();
        build_node_byte_count(&mut file, Some(node_index));
    }

    pub fn remove_node_byte_count(&self, node_index: usize) {
        // let node_info = buffer.get_node_info(node_index);
        let mut file = self.inner.data.write();
        remove_node_byte_count(&mut file, Some(node_index));
    }

    pub fn update_node_byte_count(&self, node_index: usize) {
        // let node_info = buffer.get_node_info(node_index);
        let mut file = self.inner.data.write();
        update_node_byte_count(&mut file, Some(node_index));
    }

    pub fn show_root_node_bytes_stats(&self) {
        // let node_info = buffer.get_node_info(node_index);
        let file = self.inner.data.read();
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
    pub fn register_subscriber(&mut self, cb: Box<dyn BufferEventCb>) -> usize {
        let len = 1 + self.subscribers.len();
        self.subscribers.push(RefCell::new(cb));
        len
    }

    // read ahead

    /// copy the content of the inner buffer up to 'nr_bytes' into the data Vec
    /// the read bytes are appended to the data Vec
    /// return XXX on error (TODO(ceg): use ioresult)
    pub fn read(&self, offset: u64, nr_bytes: usize, data: &mut Vec<u8>) -> usize {
        // return self.inner.read(offset, nr_bytes, data);

        let buffer_rev = self.nr_changes();

        if let Some(size) = self.cache.read(offset, nr_bytes, data, buffer_rev) {
            //dbg_println!("DATA IN CACHE offset {} size {}", offset, nr_bytes);

            // cache validation checks
            if false {
                let mut real = vec![];
                self.inner.read(offset, nr_bytes, &mut real);
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

        self.inner.read(offset, nr_bytes, data)
    }

    /// copy the content of the inner buffer up to 'nr_bytes' into the data Vec
    /// the read bytes are appended to the data Vec
    /// return XXX on error (TODO(ceg): use ioresult)
    pub fn read_cached(
        &self,
        offset: u64,
        nr_bytes: usize,
        data: &mut Vec<u8>,
        cache: &BufferReadCache,
    ) -> usize {
        let buffer_rev = self.nr_changes();

        if let Some(size) = cache.read(offset, nr_bytes, data, buffer_rev) {
            //dbg_println!("DATA IN CACHE offset {} size {}", offset, nr_bytes);

            // cache validation checks
            if false {
                let mut real = vec![];
                self.inner.read(offset, nr_bytes, &mut real);
                assert!(real.len() == data.len());
                for i in 0..real.len() {
                    assert!(real[i] == data[i]);
                }
            }
            return size;
        }

        dbg_println!("DATA NOT IN CACHE offset {} size {}", offset, nr_bytes);

        self.inner.read(offset, nr_bytes, data) // reread cache
    }

    pub fn buffer_log_dump(&self) {
        self.buffer_log.dump()
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

        //dbg_println!("// buffer.tag(..) offsets = {:?}", marks_offset);
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

                    let mut file = self.inner.data.write();

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

    /// insert the 'data' Vec content in the inner buffer up to 'nr_bytes'
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

        let (sz, events) = self.inner.insert(offset, nr_bytes, &data[..nr_bytes]);
        if sz > 0 {
            self.changed = true;
        }

        self.update_hierarchy_from_events(&events);

        for ev in &events {
            let ev = mapped_file_event_to_buffer_event(self.id, &ev);
            self.notify(&ev);
        }

        sz
    }

    pub fn append(&mut self, data: &[u8]) -> usize {
        let sz = self.size() as u64;
        self.insert(sz, data.len(), &data)
    }

    /// remove up to 'nr_bytes' from the inner buffer starting at offset
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

        let (nr_bytes_removed, events) = self.inner.remove(offset, nr_bytes, Some(&mut rm_data));

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
            let ev = mapped_file_event_to_buffer_event(self.id, &ev);
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
        self.inner.find(&data, from_offset, to_offset)
    }

    pub fn find_reverse(
        &self,
        data: &[u8],
        from_offset: u64,
        to_offset: Option<u64>,
    ) -> Option<u64> {
        self.inner.find_reverse(&data, from_offset, to_offset)
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
                let sz = self.inner.size();

                // TODO(ceg): check i/o errors
                let added = if let Some(data) = &op.data {
                    let (_, events) = self.inner.insert(op.offset, data.len(), &data);
                    self.changed = true;

                    self.update_hierarchy_from_events(&events);

                    for ev in &events {
                        let ev = mapped_file_event_to_buffer_event(self.id, &ev);
                        self.notify(&ev);
                    }

                    data.len() as u64
                } else {
                    0
                };

                assert_eq!(sz + added as usize, self.inner.size());

                op.offset + added
            }
            BufferOperationType::Remove => {
                let sz = self.inner.size();

                // TODO(ceg): check i/o errors
                let _removed = if let Some(data) = &op.data {
                    let (rm, events) = self.inner.remove(op.offset, data.len(), None);
                    self.changed = true;

                    self.update_hierarchy_from_events(&events);

                    for ev in &events {
                        let ev = mapped_file_event_to_buffer_event(self.id, &ev);
                        self.notify(&ev);
                    }

                    assert_eq!(rm, data.len());
                    rm
                } else {
                    0
                };

                assert_eq!(sz - _removed, self.inner.size());

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
pub fn sync_to_storage(buffer: &Arc<RwLock<Buffer>>) {
    // read/copy
    let mut fd = {
        let buffer = buffer.read();

        if buffer.file_name().is_empty() {
            // TODO(ceg): save as pop up/notification
            dbg_println!("cannot dsave  target filename is empty");
            return;
        }

        let tmp_file_name = format!("{}{}", buffer.file_name(), ".update"); // TODO(ceg): move to global config

        let path = Path::new(&tmp_file_name);
        if let Result::Err(_) = std::fs::remove_file(path) {}

        let fd = File::create(path);
        if fd.is_err() {
            dbg_println!("cannot save {}", buffer.file_name());
            return;
        }
        fd.unwrap()
    };

    dbg_println!("SYNC: fd = {:?}", fd);

    let mut idx = {
        let buffer = buffer.read();
        let file = buffer.inner.data.read();
        let (node_index, _, _) = file.find_node_by_offset(0);
        node_index
    };

    while idx != None {
        // do not hold the buffer.lock more
        {
            let buffer = buffer.read();
            let file = buffer.inner.data.read();
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
                    dbg_println!("cannot save {}", buffer.file_name());
                    panic!("");
                    // return false;
                }
                // dbg_println!("sync doc('{}') node {}", buffer.file_name(), idx.unwrap());
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
        // TODO(ceg): store permission when opening the file, compare to current permission, etc..

        let mut buffer = buffer.write();

        let tmp_file_name = format!("{}{}", buffer.file_name(), ".update"); // TODO(ceg): move '.update' to global config

        {
            // TODO(ceg): large file warning in save ? disable backup ?
            let _tmp_backup_name = format!("{}{}", buffer.file_name(), "~");
            // TODO(ceg): move '~' to global config
            // let _ = ::std::fs::rename(&buffer.file_name(), &tmp_backup_name);
        }

        #[cfg(not(windows))]
        {
            use std::os::unix::fs::PermissionsExt;

            // TODO(ceg): use mapped file fd, will panic if file is removed
            let perms = match buffer.metadata() {
                Ok(metadata) => metadata.permissions(),
                Err(_) => std::fs::Permissions::from_mode(0o600),
            };

            // TODO(ceg): check result, handle io results properly
            // set buffer status to : permission denied etc
            let _ = ::std::fs::set_permissions(&tmp_file_name, perms);
        }

        let _ = ::std::fs::rename(&tmp_file_name, &buffer.file_name());

        // reopen file
        let new_fd = File::open(&buffer.file_name()).unwrap();

        // TODO(ceg): handle skip with ReadOnly
        let mapped_file = buffer.inner.data.clone();
        let mut mapped_file = mapped_file.write();
        crate::core::mapped_file::MappedFile::patch_storage_offset_and_file_descriptor(
            &mut mapped_file,
            new_fd,
        );

        buffer.changed = false;
        buffer.is_syncing = false;
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
        node.byte_count[byte_idx] += 1;
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
pub fn build_index(buffer: &Arc<RwLock<Buffer>>) -> bool {
    let mut idx = {
        let buffer = buffer.read();
        {
            if buffer.indexed {
                return true;
            }

            let file = buffer.inner.data.read();
            let (node_index, _, _) = file.find_node_by_offset(0);
            node_index
        }
    };

    let t0 = std::time::Instant::now();

    let wait_min = 1000 / 30; // config or

    let mut wait = wait_min;
    let mut data = vec![];
    while idx != None {
        // read node bytes
        {
            let buffer = buffer.read();
            if buffer.abort_indexing {
                break;
            }

            let file = buffer.inner.data.read();
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
                    buffer.file_name(),
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
            dbg_println!("user_is_active pause indexing");
            wait = std::cmp::min(wait * 2, 50);
            std::thread::sleep(std::time::Duration::from_millis(wait));
        } else {
            //std::thread::sleep(std::time::Duration::from_millis(1));

            wait = wait_min;
        }

        // update node info (idx)
        {
            let buffer = buffer.read();
            let mut file = buffer.inner.data.write();

            let node_index = idx.unwrap();

            // save byte counters
            {
                let node = &mut file.pool[node_index];
                node.byte_count = byte_count;
                node.indexed = true;
                idx = node.link.next;
            }

            update_byte_index_hierarchy(&mut file, Some(node_index), UpdateHierarchyOp::Add);
        }

        // notify subscribers
        if idx.is_some() {
            let buffer = buffer.read();
            buffer.notify(&BufferEvent::BufferNodeIndexed {
                buffer_id: buffer.id,
                node_index: idx.unwrap(),
            });
        }
    }

    let t1 = std::time::Instant::now();
    dbg_println!("index time {:?} ms", (t1 - t0).as_millis());

    {
        // set index status flags
        {
            let mut buffer = buffer.write();
            if !buffer.abort_indexing {
                buffer.indexed = true;
            }

            // display root node info
            let file = buffer.inner.data.read();
            if let Some(root_index) = file.root_index() {
                let node = &file.pool[root_index];
                dbg_println!(
                    "{} : Number of lines {}",
                    buffer.file_name(),
                    node.byte_count[b'\n' as usize]
                );
            }
        }

        let buffer = buffer.read();
        if buffer.indexed {
            buffer.notify(&BufferEvent::BufferFullyIndexed {
                buffer_id: buffer.id,
            });
        }

        buffer.indexed
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
pub fn get_byte_count_at_offset(
    buffer: &Buffer,
    byte_index: usize,
    offset: u64,
) -> (u64, Option<usize>) {
    assert!(byte_index < 256);

    let mut total_count = 0;
    let mut local_offset = offset;

    let mut file = buffer.inner.data.write();

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

pub fn get_byte_count(buffer: &Buffer, byte_index: usize) -> Option<u64> {
    assert!(byte_index < 256);
    let file = buffer.inner.data.read();
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
pub fn find_nth_byte_offset(buffer: &Buffer, byte: u8, index: u64) -> Option<u64> {
    assert!(index > 0);

    let mut index = index;

    let mut file = buffer.inner.data.write();
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
                        return Some(global_offset);
                    }
                }
                global_offset += 1;
            }

            return None;
        }

        let count = file.pool[idx].byte_count[byte as usize];
        if index > count {
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
    #[cfg(not(windows))]
    fn buffer_read() {
        use std::io::prelude::*;
        use std::os::unix::prelude::FileExt;

        #[cfg(unix)]
        let filename = "/tmp/unl-test-file".to_owned();

        #[cfg(windows)]
        let filename = {
            use std::env;

            let key = "TEMP";
            let base = match env::var(key) {
                Ok(val) => val,
                Err(e) => panic!("couldn't find {key}: {e}"),
            };

            let sep = std::path::MAIN_SEPARATOR;
            format!("{base}{sep}unl-test-file")
        };

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

        let buffer = BufferBuilder::new(BufferKind::File)
            .buffer_name("untitled-1")
            .file_name(&filename)
            .internal(false)
            .finalize();

        build_index(buffer.as_ref().unwrap());

        let file = std::fs::File::open(&filename).unwrap();
        let buffer = buffer.as_ref().unwrap().write();
        let buffer_size = buffer.size() as u64;
        let step = 1024 * 1024;
        let t0_read = std::time::Instant::now();
        let mut prev_time = 0;

        let mut data: Vec<u8> = Vec::with_capacity(step);
        for offset in (0..buffer_size).into_iter().step_by(step) {
            if !true {
                unsafe {
                    data.set_len(step);
                } // faster in debug build

                //                data.clear();
                buffer.read(offset, step, &mut data);
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
        let buffer = BufferBuilder::new(BufferKind::File)
            .buffer_name("untitled-1")
            .internal(false)
            .finalize();

        let mut buffer = buffer.as_ref().unwrap().write();

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

                let off_update = buffer.insert(off, s.len(), s.as_ref());
                off += off_update as u64;
            }

            println!("buffer.size = {}", buffer.size());

            println!("start undo test");
            for i in 0..max {
                println!("undo ({}/{}) -------", i + 1, max);
                buffer.undo();
            }

            println!("buffer.size = {}", buffer.size());

            println!("start redo test");

            for i in 0..max {
                println!("redo ({}/{}) -------", i + 1, max);
                buffer.redo();
            }

            println!("buffer.size = {}", buffer.size());

            println!("start undo test (2nd pass)");
            for i in 0..max {
                println!("undo ({}/{}) -------", i + 1, max);
                buffer.undo();
            }

            println!("buffer.size = {}", buffer.size());
        }
    }

    #[test]
    fn buffer_random_size_inserts() {
        let buffer = BufferBuilder::new(BufferKind::File)
            .buffer_name("untitled-1")
            .internal(false)
            .finalize();

        let mut buffer = buffer.as_ref().unwrap().write();

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

                let random_size: usize = rng.gen_range(0..s.len());
                println!("random insert size = {}", random_size);
                let off_update = buffer.insert(off, random_size, s.as_ref());
                off += off_update as u64;
            }

            println!("buffer.size = {}", buffer.size());

            for i in 0..max {
                println!("undo ({}/{}) -------", i + 1, max);

                buffer.undo();
            }

            println!("buffer.size = {}", buffer.size());

            println!("start redo test");

            for i in 0..max {
                println!("redo ({}/{}) -------", i + 1, max);

                buffer.redo();
            }

            println!("buffer.size = {}", buffer.size());

            for i in 0..max {
                println!("undo ({}/{}) -------", i + 1, max);

                buffer.undo();
            }

            println!("buffer.size = {}", buffer.size());
        }
    }

    #[test]
    fn test_buffer_save() {
        use super::*;

        use std::fs;
        use std::fs::File;

        let start_size = 1024 * 1;
        let max_size = 1024 * 8;
        let max_insert_size = 64;

        for test_size in (start_size..=max_size).step_by(256) {
            for insert_size in (1..=max_insert_size).step_by(1) {
                let max_insert_offset = std::cmp::min::<u64>(1024, max_insert_size as u64);

                for insert_offset in (32..=max_insert_offset).step_by(32) {
                    let mut ref_vec: Vec<u8> = Vec::new();

                    // (re)create file
                    #[cfg(unix)]
                    let filename = "/tmp/playground_save_test";

                    #[cfg(windows)]
                    let filename = {
                        use std::env;

                        let key = "TEMP";
                        let base = match env::var(key) {
                            Ok(val) => val,
                            Err(e) => panic!("couldn't find {key}: {e}"),
                        };

                        let sep = std::path::MAIN_SEPARATOR;
                        format!("{base}{sep}playground_save_test")
                    };

                    let _ = fs::remove_file(&filename);
                    let filename = filename.to_owned();
                    let mut file = File::create(&filename).unwrap();

                    // fill file
                    let mut slc = Vec::with_capacity(test_size);
                    for _ in 0..test_size {
                        slc.push(b'x');
                    }

                    // write to disk
                    file.write_all(&slc).unwrap();
                    file.sync_all().unwrap();

                    ref_vec.append(&mut slc);

                    // open buffer
                    let buffer = BufferBuilder::new(BufferKind::File)
                        .file_name(&filename)
                        .buffer_name("untitled-1")
                        .internal(false)
                        .finalize();

                    let expected_size = {
                        let mut data = vec![];
                        for _ in 0..insert_size {
                            data.push(b'd');
                        }

                        let mut buffer = buffer.as_ref().unwrap().write();
                        let buffer_size = buffer.size();

                        crate::core::enable_dbg_println();
                        dbg_println!("checking test_size {test_size} : insert @ {insert_offset} size {insert_size}");
                        crate::core::disable_dbg_println();
                        buffer.insert(insert_offset, data.len(), data.as_ref());
                        crate::core::enable_dbg_println();
                        dbg_println!("done");
                        crate::core::disable_dbg_println();

                        for i in 0..data.len() {
                            ref_vec.insert(i + insert_offset as usize, data[i]);
                        }

                        buffer_size + insert_size
                    };

                    {
                        let buffer = buffer.as_ref().unwrap();
                        sync_to_storage(&buffer);

                        // check on disk size
                        match std::fs::metadata(filename) {
                            Err(e) => panic!("{}", e.to_string()),
                            Ok(m) => {
                                assert_eq!(m.len(), expected_size as u64);
                            }
                        }
                    }

                    // check content
                    let mut tmp_vec = vec![];
                    let buffer = buffer.as_ref().unwrap().read();
                    let nread = buffer.read(0, ref_vec.len(), &mut tmp_vec);
                    assert_eq!(nread, expected_size);
                    let matching = ref_vec
                        .iter()
                        .zip(&tmp_vec)
                        .filter(|&(a, b)| a == b)
                        .count();
                    assert_eq!(matching, expected_size);
                }
            }
        }
    }
}
