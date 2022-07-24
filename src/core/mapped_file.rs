//
// MappedFile is a binary tree that provides on-demand data mapping,
// and keeps only the modified areas in memory.
// The leaves are linked to allow fast sequential traversal.
//
// The "Mapped" prefix here is a bit misleading.
// Earlier versions used the mmap syscall, but to ease portability and error handling
// it was removed and replaced by allocation + std::File::read()
//

use std::collections::HashSet;
use std::fmt;

use std::cell::RefCell;
use std::marker::PhantomData;
use std::mem;
use std::ops::Deref;
use std::ops::Index;
use std::ops::IndexMut;
use std::ptr;
use std::rc::Rc;
use std::rc::Weak;
use std::slice;

use std::fs;
use std::fs::File;
use std::io::prelude::*;
use std::io::SeekFrom;

use parking_lot::RwLock;
use std::sync::Arc;

const DEBUG: bool = false;

////////////////////////////////////////////////////////////////////////////////////////////////////

type RcLockFile = Arc<RwLock<File>>;

#[derive(Debug, Clone, PartialEq)]
enum PageSource {
    FromStorage,
    FromRam,
}

#[derive(Debug, Clone)]
enum Page {
    ReadOnlyStorageCopy(*const u8, usize), // base, len

    // Copy on write
    InRam(*const u8, usize, usize), // base, len, capacity
}

impl Page {
    fn as_slice<'a>(&self) -> Option<&'a [u8]> {
        Some(match *self {
            Page::ReadOnlyStorageCopy(base, len) => unsafe { slice::from_raw_parts(base, len) },

            Page::InRam(base, len, ..) => unsafe { slice::from_raw_parts(base, len) },
        })
    }
}

impl Drop for Page {
    fn drop(&mut self) {
        match *self {
            Page::ReadOnlyStorageCopy(base, len) => {
                let v = unsafe { Vec::from_raw_parts(base as *mut u8, len, len) };
                drop(v);
            }

            Page::InRam(base, len, capacity) => {
                let v = unsafe { Vec::from_raw_parts(base as *mut u8, len, capacity) };
                drop(v);
            }
        }
    }
}

////////////////////////////////////////////////////////////////////////////////////////////////////

// TODO: move to prover module
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Id(pub usize);

#[derive(Debug, Clone)]
pub enum UpdateHierarchyOp {
    Add,
    Sub,
}

#[derive(Debug, Clone, PartialEq)]
enum NodeRelation {
    NoRelation,
    // Parent,
    Left,
    Right,
    // Prev,
    // Next,
}

pub type NodeIndex = usize;
type NodeSize = u64;
type NodeLocalOffset = u64;

pub type FileHandle<'a> = Arc<RwLock<MappedFile<'a>>>;
pub type FileIterator<'a> = MappedFileIterator<'a>;

#[derive(Debug)]
pub struct NodeLinks {
    pub parent: Option<NodeIndex>,
    pub left: Option<NodeIndex>,
    pub right: Option<NodeIndex>,
    pub prev: Option<NodeIndex>,
    pub next: Option<NodeIndex>,
}

impl NodeLinks {
    pub fn new() -> Self {
        NodeLinks {
            parent: None,
            left: None,
            right: None,
            prev: None,
            next: None,
        }
    }

    pub fn with_parent(parent: Option<NodeIndex>) -> Self {
        NodeLinks {
            parent,
            left: None,
            right: None,
            prev: None,
            next: None,
        }
    }
}

#[derive(Debug)]
pub struct Node {
    pub byte_count: [u64; 256],
    //
    page: Weak<RefCell<Page>>,
    cow: Option<Rc<RefCell<Page>>>,
    //
    pub link: NodeLinks,
    // data
    storage_offset: Option<u64>,
    pub size: u64,

    pub indexed: bool,
    used: bool,
    to_delete: bool,
}

impl Node {
    pub fn new() -> Self {
        Node {
            byte_count: [0; 256],
            //
            page: Weak::new(),
            cow: None,
            //
            link: NodeLinks::new(),
            //
            size: 0,
            storage_offset: None,
            indexed: false,
            used: false,
            to_delete: false,
        }
    }

    fn clear(&mut self) {
        *self = Self::new();
    }

    // TODO: offset + size: allow to yield disk
    // use this to read copy data directly to 'out' slice
    // (try to copy out.len() bytes)
    pub fn do_direct_copy_at_pos(
        &self,
        fd: &Option<RcLockFile>,
        pos: usize,
        size: usize,
        out: &mut [u8],
    ) -> Option<usize> {
        // in ram ? -
        if let Some(ref page) = self.cow {
            let p = page.borrow().as_slice().unwrap();
            let n = std::cmp::min(size, p.len() - pos);
            assert!(n > 0);
            unsafe {
                ptr::copy(p.as_ptr().offset(pos as isize), out.as_mut_ptr(), n);
            }
            return Some(n);
        }

        // already mapped ?
        if let Some(ref page) = self.page.upgrade() {
            let p = page.borrow().as_slice().unwrap();
            let n = std::cmp::min(size, p.len() - pos);
            assert!(n > 0);
            unsafe {
                ptr::copy(p.as_ptr().offset(pos as isize), out.as_mut_ptr(), n);
            }
            return Some(n);
        }

        // access storage
        if let Some(storage_offset) = self.storage_offset {
            let n = std::cmp::min(size, self.size as usize);
            assert!(n > 0);

            let mut pos = pos;
            while pos < n {
                let chunk_size = std::cmp::min(n - pos, 1024 * 32);
                // not atomic
                {
                    //let t0_read = std::time::Instant::now();
                    let mut fd = fd.as_ref().unwrap().write();
                    let _ = fd.seek(SeekFrom::Start(storage_offset + pos as u64));
                    let nrd = fd.read(&mut out[pos..pos + chunk_size]).unwrap(); // remove unwrap() )?; TODO(ceg): io error

                    //let t1_read = std::time::Instant::now();
                    //dbg_println!("read node chunk[{}..{}]/{} time {:?} ms", pos, pos+chunk_size, n, (t1_read - t0_read).as_millis());
                    assert!(nrd == chunk_size);
                }
                pos += chunk_size;
            }

            return Some(n);
        }

        None
    }

    // TODO: offset + size: allow to yield disk
    // use this to read copy data directly to 'out' slice
    // (try to copy out.len() bytes)
    pub fn do_direct_copy(&self, fd: &Option<RcLockFile>, out: &mut [u8]) -> Option<usize> {
        // in ram ? -
        if let Some(ref page) = self.cow {
            let p = page.borrow().as_slice().unwrap();
            let n = std::cmp::min(out.len(), p.len());
            //assert!(n >= 0);
            unsafe {
                ptr::copy(p.as_ptr(), out.as_mut_ptr(), n);
            }
            return Some(n);
        }

        // already mapped ?
        if let Some(ref page) = self.page.upgrade() {
            let p = page.borrow().as_slice().unwrap();
            let n = std::cmp::min(out.len(), p.len());
            assert!(n > 0);
            unsafe {
                ptr::copy(p.as_ptr(), out.as_mut_ptr(), n);
            }
            return Some(n);
        }

        // access storage
        if let Some(storage_offset) = self.storage_offset {
            let n = std::cmp::min(out.len(), self.size as usize);
            assert!(n > 0);

            let mut pos = 0;
            while pos < n {
                let chunk_size = std::cmp::min(n - pos, 1024 * 32);
                // not atomic
                {
                    //let t0_read = std::time::Instant::now();
                    let mut fd = fd.as_ref().unwrap().write();
                    let _ = fd.seek(SeekFrom::Start(storage_offset + pos as u64));
                    let nrd = fd.read(&mut out[pos..pos + chunk_size]).unwrap(); // remove unwrap() )?; TODO(ceg): io error

                    //let t1_read = std::time::Instant::now();
                    //dbg_println!("read node chunk[{}..{}]/{} time {:?} ms", pos, pos+chunk_size, n, (t1_read - t0_read).as_millis());
                    if nrd != chunk_size {
                        dbg_println!(
                            "cannot read node : nrd {} != chunk_size {}",
                            nrd,
                            chunk_size
                        );
                    }
                }
                pos += chunk_size;
            }

            return Some(n);
        }

        None
    }

    fn build_page_from_base_offset(
        &mut self,
        fd: &Option<RcLockFile>,
        storage_offset: u64,
    ) -> Option<Rc<RefCell<Page>>> {
        let mut v = Vec::with_capacity(self.size as usize);

        // from Vec doc
        // Pull out the various important pieces of information about `v`
        let base = v.as_mut_ptr() as *const u8;
        let capacity = v.capacity();
        unsafe {
            v.set_len(capacity);
        };

        let mut fd = fd.as_ref().unwrap().write();

        let _ = fd.seek(SeekFrom::Start(storage_offset));

        let nrd = fd.read(&mut v[..capacity]);
        match nrd {
            Ok(nrd) => {
                if nrd != capacity {
                    eprintln!(
                        "MAPPED FILE: read error error : disk_offset = {}, size = {}, nrd {} != capacity {}",
                        storage_offset,
                        self.size,
                        nrd,
                        capacity
                    );

                    return None;

                    // panic!("read error"); // if file changed on disk ...
                }
            }

            Err(e) => {
                panic!("read error {:?}", e); // if file changed on disk ...
            }
        }

        // 5 - build "MAPPED FILE: new" page
        mem::forget(v);

        let ro_page = Page::ReadOnlyStorageCopy(base, capacity);

        let page = Rc::new(RefCell::new(ro_page));

        self.page = Rc::downgrade(&page);

        Some(page)
    }

    fn map(&mut self, fd: &Option<RcLockFile>) -> Option<Rc<RefCell<Page>>> {
        // ram ?
        if let Some(ref page) = self.cow {
            return Some(Rc::clone(page));
        }

        // already mapped ?
        if let Some(page) = self.page.upgrade() {
            return Some(page);
        }

        if let Some(storage_offset) = self.storage_offset {
            self.build_page_from_base_offset(fd, storage_offset)
        } else {
            panic!("mapped_file internal error: invalid storage offset");
        }
    }

    // will consume v
    fn vec_to_page(mut v: Vec<u8>) -> Page {
        // from Vec doc
        // Pull out the various important pieces of information about `v`
        let base = v.as_mut_ptr() as *const u8;
        let len = v.len();
        let capacity = v.capacity();

        mem::forget(v);

        // 5 - build "MAPPED FILE: new" page
        Page::InRam(
            // from a Vec<u8> raw parts
            base, len, capacity,
        )

        // 6 - restore page iterators base pointer
    }

    // will clear p
    fn _page_to_vec(p: &mut Page) -> Vec<u8> {
        match *p {
            crate::core::mapped_file::Page::InRam(ref mut base, ref mut len, ref mut capacity) => {
                let v = unsafe { Vec::from_raw_parts(*base as *mut u8, *len, *capacity) };

                *base = ptr::null_mut();
                *len = 0;
                *capacity = 0;

                v
            }

            _ => {
                panic!("cannot be used on MappedStorage page");
            }
        }
    }

    fn move_to_ram(&mut self, fd: &Option<RcLockFile>) -> Page {
        // 1 - save all page iterators local offsets

        // 2 - map the page // will invalidate iterators base pointer
        // TODO(ceg): check
        let page = self.map(fd).unwrap();
        let slice = page.borrow().as_slice().unwrap();

        // 3 - allocate a vector big enough to hold page data
        let mut v = Vec::with_capacity(self.size as usize);

        // 4 - copy page to vector
        v.extend_from_slice(slice);

        Node::vec_to_page(v)
    }
}

/////////////////////////////////////////////////////////////////////////////////////////////////////////////

#[derive(Debug)]
pub struct FreeListAllocator<T> {
    // simple allocator with free list
    slot: Vec<T>,
    free_indexes: Vec<usize>,
}

impl<T> FreeListAllocator<T> {
    fn new() -> Self {
        FreeListAllocator {
            slot: vec![],
            free_indexes: vec![],
        }
    }

    fn allocate(&mut self, n: T, check_slot: &dyn Fn(&mut T)) -> (NodeIndex, &mut T) {
        if !self.free_indexes.is_empty() {
            let i = self.free_indexes.pop().unwrap();
            if DEBUG {
                dbg_println!("node allocator reuse slot {}", i);
            }
            check_slot(&mut self.slot[i]);
            self.slot[i] = n;
            (i as NodeIndex, &mut self.slot[i])
        } else {
            let i = self.slot.len();
            self.slot.push(n);
            if DEBUG {
                dbg_println!("node allocator create new slot {}", i);
            }
            (i as NodeIndex, &mut self.slot[i])
        }
    }

    fn release(&mut self, idx: NodeIndex) {
        //dbg_println!("node allocator release slot {}", idx);
        let idx = idx as usize;
        self.free_indexes.push(idx);
    }
}

impl Index<usize> for FreeListAllocator<Node> {
    type Output = Node;

    fn index(&self, index: usize) -> &Node {
        &self.slot[index]
    }
}

impl IndexMut<usize> for FreeListAllocator<Node> {
    fn index_mut(&mut self, index: usize) -> &mut Node {
        &mut self.slot[index]
    }
}

/////////////////////////////////////////////////////////////////////////////////////////////////////////////

pub struct MappedFile<'a> {
    phantom: PhantomData<&'a u8>,
    pub id: Id,
    pub fd: Option<RcLockFile>,
    pub pool: FreeListAllocator<Node>,
    root_index: Option<NodeIndex>,
    page_size: usize,
    /// size of new allocated blocks when splitting old ones
    pub sub_page_size: usize,
    /// reserve storage on new allocated blocks
    pub sub_page_reserve: usize,
    // list of past node events, cleared on insert/remove
    events: Vec<MappedFileEvent>,
}

impl<'a> fmt::Debug for MappedFile<'a> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("MappedFile {}")
            .field("fd", &self.fd)
            .field("root_index", &self.root_index)
            .field("page_size", &self.page_size)
            .finish()
    }
}

#[derive(Debug)]
pub struct NodeOperationData<'a> {
    pub data: &'a Vec<u8>,
    pub offset: u64,
    pub local_offset: u64,
}

#[derive(Debug, Copy, Clone)]
pub enum MappedFileEvent /*<'a>*/ {
    NodeChanged {
        node_index: usize,
        //byte_count_before: Vec<u8>,
        //byte_count_after: Vec<u8>,
    }, // ?
    NodeAdded {
        node_index: usize,
        //        data: NodeOperationData<'a>,
    },
    NodeRemoved {
        node_index: usize,
        //        data: NodeOperationData<'a>,
    },
}

impl<'a> Drop for MappedFile<'a> {
    fn drop(&mut self) {}
}

impl<'a> MappedFile<'a> {
    fn assert_node_is_unused(n: &mut Node) {
        assert!(!n.used);
    }

    pub fn empty(id: Id) -> Option<FileHandle<'a>> {
        let file = MappedFile {
            phantom: PhantomData,
            id,
            fd: None,
            pool: FreeListAllocator::new(),
            root_index: None,
            page_size: 2 * 1024 * 1024,
            sub_page_size: 4096,
            sub_page_reserve: 2 * 1024,
            events: vec![],
        };

        Some(Arc::new(RwLock::new(file)))
    }

    pub fn new(id: Id, path: String) -> Option<FileHandle<'a>> {
        // TODO(ceg): check page size % 4096 // sysconfig

        let fd = File::open(path.clone());
        if fd.is_err() {
            return None;
        }
        let fd = Some(Arc::new(RwLock::new(fd.unwrap())));

        let metadata = fs::metadata(path.clone()).unwrap();

        let file_size = metadata.len();

        // TODO(ceg): find good sizes, add user configuration
        let sub_page_size = 1024 * 1024 * 2;

        let page_size = match file_size {
            _ if file_size < (1024 * 4) => 32,
            _ if file_size < (1024 * 8) => 64,
            _ if file_size < (1024 * 16) => 128,
            _ if file_size < (1024 * 32) => 256,
            _ if file_size < (1024 * 64) => 512,
            _ if file_size < (1024 * 128) => 1024 * 1,
            _ if file_size < (1024 * 256) => 1024 * 2,
            _ if file_size < (1024 * 512) => 1024 * 4,
            _ if file_size < (1 * 1024 * 1024) => 1024 * 8,
            _ if file_size < (2 * 1024 * 1024) => 1024 * 16,
            _ if file_size < (4 * 1024 * 1024) => 1024 * 32,
            _ if file_size < (8 * 1024 * 1024) => 1024 * 64,
            _ if file_size < (16 * 1024 * 1024) => 1024 * 128,
            _ if file_size < (32 * 1024 * 1024) => 1024 * 256,
            _ if file_size < (64 * 1024 * 1024) => 1024 * 512,
            _ if file_size < (128 * 1024 * 1024) => 1024 * 1024,
            _ if file_size < (256 * 1024 * 1024) => 1024 * 1024 * 2,
            _ if file_size < (512 * 1024 * 1024) => 1024 * 1024 * 4,
            _ if file_size < (1024 * 1024 * 1024) => 1024 * 1024 * 8,
            _ => 1024 * 1024 * 16,
        };

        dbg_println!("MappedFile::new() : file_size {}", file_size);
        dbg_println!("MappedFile::new() : page_size {}", page_size);

        let mut file = MappedFile {
            phantom: PhantomData,
            id,
            fd,
            pool: FreeListAllocator::new(),
            root_index: None,
            page_size,
            sub_page_size,
            sub_page_reserve: 2 * 1024,
            events: vec![],
        };

        if file_size == 0 {
            return Some(Arc::new(RwLock::new(file)));
        }

        // TODO(ceg): Node::new()
        let root_node = Node {
            used: true,
            to_delete: false,
            size: file_size,
            link: NodeLinks::new(),
            page: Weak::new(),
            cow: None,
            storage_offset: None,
            indexed: false,
            byte_count: [0; 256],
        };

        let (id, _) = file
            .pool
            .allocate(root_node, &MappedFile::assert_node_is_unused);
        file.root_index = Some(id);

        let mut leaves = Vec::new();
        MappedFile::build_tree(
            PageSource::FromStorage,
            &mut file.pool,
            &mut leaves,
            Some(id),
            page_size as u64,
            file_size as u64,
            0,
        );

        dbg_println!("MappedFile::new() : leaves.len() {}", leaves.len());

        let mut prev_idx = None;
        for idx in leaves {
            MappedFile::link_prev_next_nodes(&mut file.pool, prev_idx, Some(idx));
            prev_idx = Some(idx);

            // TODO(ceg): add hints to map all nodes
            /*
            if file_size <= page_size as u64 {
                let p = file.pool[idx as usize].move_to_ram();
                let rc = Rc::new(RefCell::new(p));
                file.pool[idx as usize].page = Rc::downgrade(&rc);
                file.pool[idx as usize].cow = Some(rc);
                file.pool[idx as usize].storage_offset = None;
            }
            */
        }

        MappedFile::check_tree(&mut HashSet::new(), file.root_index, &file.pool);

        Some(Arc::new(RwLock::new(file)))
    }

    pub fn cleanup_events(&mut self) {
        dbg_println!("CLEANUP EVENTS {:?}", self.events);

        for event in &self.events {
            match event {
                MappedFileEvent::NodeRemoved { node_index } => {
                    dbg_println!("CLEANUP {:?}", event);
                    if self.pool[*node_index].to_delete {
                        self.pool[*node_index].clear();
                        self.pool.release(*node_index);
                    } else {
                        panic!("INVALID CLEANUP {:?}", event);
                    }
                }
                _ => {}
            }
        }

        self.events.clear();
    }

    pub fn root_index(&self) -> Option<NodeIndex> {
        self.root_index
    }

    pub fn size(&self) -> u64 {
        if let Some(idx) = self.root_index {
            self.pool[idx].size as u64
        } else {
            0
        }
    }

    fn link_prev_next_nodes(
        pool: &mut FreeListAllocator<Node>,
        prev_idx: Option<NodeIndex>,
        next_idx: Option<NodeIndex>,
    ) {
        if let Some(prev_idx) = prev_idx {
            pool[prev_idx].link.next = next_idx;
            // dbg_println!("link_next : prev({:?})  -> next({:?})", prev_idx, next_idx);
        }

        if let Some(next_idx) = next_idx {
            pool[next_idx].link.prev = prev_idx;
            // dbg_println!("link_prev : prev({:?})  <- next({:?})", prev_idx, next_idx);
        }
    }

    fn _link_parent_child(
        pool: &mut FreeListAllocator<Node>,
        parent_idx: Option<NodeIndex>,
        child_idx: Option<NodeIndex>,
        relation: &NodeRelation,
    ) {
        if let Some(child_idx) = child_idx {
            pool[child_idx].link.parent = parent_idx;
            if DEBUG {
                dbg_println!(
                    "MAPPED FILE: link_parent : child({:?})  -> parent({:?})",
                    child_idx,
                    parent_idx
                );
            }
        }

        let (node_ref, name) = if let Some(parent_idx) = parent_idx {
            match relation {
                NodeRelation::Left => (&mut pool[parent_idx].link.left, "MAPPED FILE: left"),
                NodeRelation::Right => (&mut pool[parent_idx].link.right, "MAPPED FILE: right"),
                _ => unimplemented!(),
            }
        } else {
            return;
        };

        *node_ref = child_idx;

        if DEBUG {
            dbg_println!(
                "MAPPED FILE: link_child : parent({:?}).{} -> child({:?})",
                parent_idx,
                name,
                child_idx
            );
        }
    }

    pub fn print_nodes(file: &MappedFile) {
        for (idx, n) in file.pool.slot.iter().enumerate() {
            if n.used {
                dbg_println!(
                    "MAPPED FILE: idx({:?}), parent({:?}) left({:?}) right({:?}) prev({:?}) \
                     next({:?}) size({}) ", // on_disk_off({})",
                    idx,
                    n.link.parent,
                    n.link.left,
                    n.link.right,
                    n.link.prev,
                    n.link.next,
                    n.size, // n.storage_offset
                )
            }
        }
    }

    fn build_tree(
        source: PageSource,
        pool: &mut FreeListAllocator<Node>,
        leaves: &mut Vec<NodeIndex>,
        parent: Option<NodeIndex>,
        pg_size: u64,
        node_size: u64,
        base_offset: u64,
    ) {
        let b_off = base_offset;

        // is leaf ?
        if node_size <= pg_size {
            if DEBUG {
                dbg_println!(
                    "MAPPED FILE: node_size <= pg_size : \
                     leaf_node({}), pg_size({}), node_size({}), base_offset({})",
                    parent.unwrap_or(0),
                    pg_size,
                    node_size,
                    base_offset,
                );
            }

            let leaf = parent;

            match leaf {
                Some(idx) => {
                    let idx = idx as usize;
                    if source == PageSource::FromStorage {
                        pool[idx].storage_offset = Some(base_offset);
                    }
                    leaves.push(idx);
                }
                _ => panic!("internal error"),
            }

            return;
        }

        // adjust page size
        let half = node_size / 2;
        let mut sz = pg_size;
        while sz < half {
            sz += pg_size;
        }

        // split
        let l_sz = sz;
        let r_sz = node_size - l_sz;

        // create leaves : TODO(ceg): use default() ?
        // TODO(ceg): Node::new(fd, parent, size, storage_offset)
        let left_node = Node {
            used: true,
            to_delete: false,
            size: l_sz,
            link: NodeLinks::with_parent(parent),
            page: Weak::new(),
            cow: None,
            storage_offset: None,
            indexed: false,
            byte_count: [0; 256],
        };
        let (l, _) = pool.allocate(left_node, &MappedFile::assert_node_is_unused);

        let right_node = Node {
            used: true,
            to_delete: false,
            size: r_sz,
            link: NodeLinks::with_parent(parent),
            page: Weak::new(),
            cow: None,
            storage_offset: None,
            indexed: false,
            byte_count: [0; 256],
        };

        let (r, _) = pool.allocate(right_node, &MappedFile::assert_node_is_unused);

        // build children
        // left
        let l_base = b_off;
        MappedFile::build_tree(source.clone(), pool, leaves, Some(l), pg_size, l_sz, l_base);
        // right
        let r_base = b_off + l_sz;
        MappedFile::build_tree(source.clone(), pool, leaves, Some(r), pg_size, r_sz, r_base);

        // update parent's links
        if let Some(idx) = parent {
            let idx = idx as usize;
            pool[idx].link.left = Some(l);
            pool[idx].link.right = Some(r);

            if DEBUG {
                dbg_println!("parent = {}, l = {}, r = {}", idx, l, r);
                dbg_println!("parent = {:?}", pool[idx]);
                dbg_println!("l idx {} = {:?}", l, pool[l]);
                dbg_println!("r idx {} = {:?}", r, pool[r]);
            }
        }
    }

    // TODO(ceg): non recursive version
    fn find_sub_node_by_offset(
        &self,
        n: NodeIndex,
        offset: u64,
    ) -> (Option<NodeIndex>, NodeSize, NodeLocalOffset) {
        if DEBUG {
            dbg_println!("find_sub_node_by_offset Ndi({}) off({})", n, offset);
        }
        let node = &self.pool[n as usize];

        assert!(node.used);

        let is_leaf = node.link.left.is_none() && node.link.right.is_none();

        if offset < node.size && is_leaf {
            (Some(n), node.size, offset)
        } else {
            let left_size = if let Some(left) = node.link.left {
                self.pool[left as usize].size
            } else {
                0
            };

            if DEBUG {
                dbg_println!("   off({})  left_size({})", offset, left_size);
            }
            if offset < left_size {
                if DEBUG {
                    dbg_println!("go   <----");
                }
                self.find_sub_node_by_offset(node.link.left.unwrap(), offset)
            } else {
                if DEBUG {
                    dbg_println!("go   ---->");
                }
                self.find_sub_node_by_offset(node.link.right.unwrap(), offset - left_size)
            }
        }
    }

    // TODO(ceg): use idiomatic map/iter
    pub fn for_each_node(&self, cb: impl Fn(&Node) -> bool) {
        let (node_index, _, _) = self.find_node_by_offset(0);
        if node_index.is_none() {
            return;
        };

        let mut idx = node_index.unwrap();
        loop {
            let node = &self.pool[idx];
            let ret = cb(node);
            if !ret || node.link.next.is_none() {
                return;
            }
            idx = node.link.next.unwrap();
        }
    }

    pub fn find_node_by_offset(
        &self,
        offset: u64,
    ) -> (Option<NodeIndex>, NodeSize, NodeLocalOffset) {
        if let Some(idx) = self.root_index {
            if offset >= self.pool[idx as usize].size {
                // offset is to big
                return (None, 0, 0);
            }
            self.find_sub_node_by_offset(idx, offset)
        } else {
            (None, 0, 0)
        }
    }

    pub fn iter(file: &FileHandle<'a>) -> FileIterator<'a> {
        MappedFile::iter_from(file, 0)
    }

    // creates an iterator over an arbitrary node index
    // always start @ local_offset 0
    pub fn iter_from_node_index(file_: &FileHandle<'a>, node_idx: NodeIndex) -> FileIterator<'a> {
        let file = file_.write();

        let page = file.pool[node_idx as usize].page.upgrade().unwrap();
        let slice = page.borrow().as_slice().unwrap();

        MappedFileIterator::Real(IteratorInstance {
            file: Arc::clone(file_),
            file_size: file.size(),
            local_offset: 0,
            page_size: file.pool[node_idx as usize].size,
            node_idx,
            page,
            base: slice,
        })
    }

    pub fn iter_from(file_: &FileHandle<'a>, offset: u64) -> FileIterator<'a> {
        let mut file = file_.write();

        let fd = if let Some(fd) = &file.fd {
            Some(Arc::clone(fd))
        } else {
            None
        };

        let pair = if file.size() == 0 {
            (None, 0, 0)
        } else {
            file.find_node_by_offset(offset)
        };

        match pair {
            (Some(node_idx), node_size, local_offset) => {
                let page = file.pool[node_idx as usize].map(&fd).unwrap();
                let slice = page.borrow().as_slice().unwrap();

                MappedFileIterator::Real(IteratorInstance {
                    file: Arc::clone(file_),
                    file_size: file.size(),
                    local_offset,
                    page_size: node_size,
                    node_idx,
                    page,
                    base: slice,
                })
            }

            (None, _, _) => MappedFileIterator::End(Arc::clone(file_)),
        }
    }

    pub fn copy_to_slice(from: &mut FileIterator<'a>, nr_to_read: usize, vec: &mut [u8]) -> usize {
        if let MappedFileIterator::End(..) = *from {
            return 0;
        }

        let mut nr_read: usize = 0;
        let mut nr_to_read = nr_to_read;

        while nr_to_read > 0 {
            if let Some(it) = from.get_mut_ref() {
                let off = it.local_offset as usize;
                let max_read = ::std::cmp::min(it.page_size as usize - off, nr_to_read);
                if max_read == 0 {
                    break;
                }
                unsafe {
                    ptr::copy(&it.base[off], vec.as_mut_ptr().add(nr_read), max_read);
                }
                nr_to_read -= max_read;
                nr_read += max_read;

                it.local_offset += max_read as u64;

                //
                if it.local_offset == it.page_size {
                    let next_it = from.next();
                    if next_it.is_none() {
                        break;
                    }

                    *from = next_it.unwrap();
                }
            }
        }

        nr_read
    }

    pub fn read(it_: &mut FileIterator<'a>, nr_to_read: usize, vec: &mut Vec<u8>) -> usize {
        // TODO(ceg): if file has changed ? return Result<usize, MappedfileError { ExternalChangeDetected }>

        if let MappedFileIterator::End(..) = *it_ {
            return 0;
        }

        let mut nr_read = 0;
        let mut nr_to_read = nr_to_read;

        while nr_to_read > 0 {
            if let Some(it) = it_.get_mut_ref() {
                let off = it.local_offset as usize;

                let max_read = ::std::cmp::min(it.page_size as usize - off, nr_to_read);
                vec.extend_from_slice(&it.base[off..off + max_read]);

                nr_to_read -= max_read;
                nr_read += max_read;
                it.local_offset += max_read as u64;

                if it.local_offset == it.page_size {
                    if let Some(next_it) = it_.next() {
                        *it_ = next_it;
                    } else {
                        break;
                    }
                }
            } else {
                panic!();
            }
        }

        nr_read
    }

    fn find_in_vec(v: &Vec<u8>, data: &[u8]) -> Option<usize> {
        let last_byte = *data.last().unwrap();

        let mut pos = 0;
        while pos < v.len() {
            // look for last_byte starting from pos
            let mut found_last = None;
            for i in pos..v.len() {
                if v[i] == last_byte {
                    found_last = Some(i);
                    break;
                }
            }

            let found_idx = found_last?;

            if data.len() - 1 > found_idx {
                // too short
                pos = found_idx + 1;
                continue;
            }

            let start_idx = found_idx - (data.len() - 1);
            let mut diff = false;
            for i in 0..data.len() {
                if v[start_idx + i] != data[i] {
                    diff = true;
                    break;
                }
            }

            if !diff {
                return Some(start_idx);
            }

            pos = found_idx + 1;
        }

        None
    }

    /// find
    pub fn find(
        file: &FileHandle<'a>,
        data: &[u8],
        from_offset: u64,
        to_offset: Option<u64>,
    ) -> Option<u64> {
        if data.is_empty() {
            return None;
        }

        let mut it = MappedFile::iter_from(&file, from_offset);
        if let MappedFileIterator::End(..) = it {
            return None;
        }

        let mut cur_offset = from_offset;

        let mut max_offset = file.read().size();
        if to_offset.is_some() {
            max_offset = std::cmp::min(to_offset.unwrap(), max_offset);
        }
        let mut remain = max_offset - from_offset;

        // TODO(ceg): rd.len() < data.len()
        let mut chunk: Vec<u8> = Vec::with_capacity(1024 * 1024 * 2);

        while remain > 0 {
            chunk.clear();

            let rd_size = std::cmp::min(chunk.capacity(), remain as usize);
            let n_read = MappedFile::read(&mut it, rd_size, &mut chunk);
            if n_read == 0 {
                // end of range
                break;
            }

            // look in block
            let found = MappedFile::find_in_vec(&chunk, &data);
            if let Some(found) = found {
                return Some(cur_offset + found as u64);
            }

            // skip block
            cur_offset += n_read as u64;
            remain -= n_read as u64;
        }

        None
    }

    fn find_reverse_in_vec(v: &Vec<u8>, data: &[u8]) -> Option<usize> {
        'outer: for (d_pos, b) in v.iter().enumerate().rev() {
            if *b == data[0] && d_pos + data.len() < v.len() {
                // let matching = v.iter().skip(d_pos).zip(data).filter(|&(a, b)| a == b).count();
                // if matching == data.len() {
                //     return Some(d_pos);
                // }

                for i in 0..data.len() {
                    if data[i] != v[d_pos + i] {
                        continue 'outer;
                    }
                }
                return Some(d_pos);
            }
        }
        None
    }

    pub fn find_reverse(
        file: &FileHandle<'a>,
        data: &[u8],
        mut from_offset: u64,
        to_offset: Option<u64>,
    ) -> Option<u64> {
        if data.is_empty() || from_offset == 0 {
            return None;
        }

        let min_offset = to_offset.unwrap_or(0);
        if min_offset >= from_offset {
            return None;
        }

        let mut chunk: Vec<u8> = Vec::with_capacity(1024 * 1024 * 2);
        loop {
            let remain = from_offset.saturating_sub(min_offset);
            if remain == 0 {
                break;
            }

            chunk.clear();
            let rd_size = std::cmp::min(chunk.capacity() + data.len(), remain as usize);
            let base_offset = from_offset.saturating_sub(rd_size as u64);

            let mut it = MappedFile::iter_from(&file, base_offset);
            let _n_read = MappedFile::read(&mut it, rd_size, &mut chunk); // TODO(ceg) io error

            let index = MappedFile::find_reverse_in_vec(&chunk, &data);
            if let Some(index) = index {
                return Some(base_offset + index as u64);
            } else {
                from_offset = base_offset.saturating_sub(1);
            }
        }

        None
    }

    fn update_hierarchy(
        pool: &mut FreeListAllocator<Node>,
        parent_idx: Option<NodeIndex>,
        op: &UpdateHierarchyOp,
        value: u64,
    ) {
        let mut p_idx = parent_idx;
        while p_idx != None {
            let idx = p_idx.unwrap();
            if DEBUG {
                dbg_print!(
                    "MAPPED FILE: node({}).size {} op({:?}) {} ---> ",
                    idx,
                    pool[idx as usize].size,
                    op,
                    value
                );
            }

            match op {
                UpdateHierarchyOp::Add => pool[idx as usize].size += value,
                UpdateHierarchyOp::Sub => pool[idx as usize].size -= value,
            }

            if DEBUG {
                dbg_println!("{}", pool[idx as usize].size);
            }

            p_idx = pool[idx as usize].link.parent;
        }
    }

    fn check_free_space(it_: &mut MappedFileIterator) -> u64 {
        match &*it_ {
            MappedFileIterator::End(..) => 0,
            MappedFileIterator::Real(ref it) => match &it.page {
                ref rc => match *rc.borrow_mut() {
                    Page::ReadOnlyStorageCopy(..) => {
                        if DEBUG {
                            dbg_println!(
                                "MAPPED FILE: Page::ReadOnlyStorageCopy: check_free_space 0"
                            );
                        }

                        0
                    }

                    Page::InRam(_, ref mut len, capacity) => {
                        if DEBUG {
                            dbg_println!(
                                "MAPPED FILE: Page::InRam: check_free_space capacity {}, len {}",
                                capacity,
                                len
                            );
                        }
                        (capacity - *len) as u64
                    }
                },
            },
        }
    }

    fn insert_in_place(it_: &mut FileIterator<'a>, data: &[u8]) {
        match &*it_ {
            MappedFileIterator::End(..) => panic!("trying to write on end iterator"),
            MappedFileIterator::Real(ref it) => match &it.page {
                ref rc => match *rc.borrow_mut() {
                    Page::ReadOnlyStorageCopy(..) => {
                        panic!("trying to write on read only memory");
                    }

                    Page::InRam(base, ref mut len, capacity) => {
                        let mut v = unsafe { Vec::from_raw_parts(base as *mut u8, *len, capacity) };

                        let index = it.local_offset as usize;
                        for (n, b) in data.iter().enumerate() {
                            v.insert(index + n, *b);
                        }

                        *len = v.len();

                        mem::forget(v);
                    }
                },
            },
        }
    }

    /// insert data at iterator position, and advance the iterator
    // 1 - get iterator's node info
    // 2 - is there room to insert data ? (imply the node was previously splitted) -> insert data
    // 3 - build a sub-tree to insert the data (make room for more inserts)
    // 4 - copy the data
    // 5 - replace the parent node
    // 6 - update hierachy
    // 7 - TODO(ceg): update iterator internal using find + local_offset on the allocated subtree
    pub fn insert(it_: &mut FileIterator<'a>, data: &[u8]) -> (usize, Vec<MappedFileEvent>) {
        dbg_println!("CALL CLEANUP");
        {
            let rcfile = it_.get_file();
            let mut file = rcfile.write();
            file.cleanup_events();
        }

        // TODO(ceg): underlying file has changed  ? return
        // fd.as_ref().read().metadata() != file.previous_metadata()

        let mut events = vec![];

        let data_len = data.len() as u64;
        if data_len == 0 {
            return (0, events);
        }

        // check iterator type
        let (node_to_split, node_size, local_offset, it_page) = match &*it_ {
            MappedFileIterator::End(ref rcfile) => {
                let mut file = rcfile.write();
                let fd = if let Some(fd) = &file.fd {
                    Some(Arc::clone(fd))
                } else {
                    None
                };

                MappedFile::print_all_used_nodes(&file, "MAPPED FILE: BEFORE INSERT - @ eof");

                // TODO: return available SPACE HERE

                let file_size = file.size();
                if file_size > 0 {
                    let (idx, node_size, _) = file.find_node_by_offset(file_size - 1);
                    let page = file.pool[idx.unwrap()].map(&fd);
                    dbg_println!("MAPPED FILE: use idx {:?}, node_size {}", idx, node_size);
                    (idx, node_size, node_size, page)
                } else {
                    (None, 0, 0, None)
                }
            }

            MappedFileIterator::Real(ref it) => (
                Some(it.node_idx),
                it.page_size,
                it.local_offset,
                Some(Rc::clone(&it.page)),
            ),
        };

        if DEBUG {
            let rcfile = it_.get_file();
            let file = rcfile.write();

            MappedFile::print_all_used_nodes(&file, "MAPPED FILE: BEFORE INSERT");
        }
        if DEBUG {
            dbg_println!(
                "MAPPED FILE: node_to_split {:?} / size ({})",
                node_to_split,
                node_size
            );
        }

        let available = MappedFile::check_free_space(it_);
        if DEBUG {
            dbg_println!("MAPPED FILE: available space = {}", available);
        }

        /////// in place insert ?

        if available >= data_len {
            if DEBUG {
                dbg_println!(
                    "MAPPED FILE: available({})>= data_len({})",
                    available,
                    data_len
                );
                dbg_println!("MAPPED FILE: insert in place");
            }

            // insert in current node
            MappedFile::insert_in_place(it_, data);

            // update parents
            let rcfile = it_.get_file();
            let mut file = rcfile.write();

            MappedFile::update_hierarchy(
                &mut file.pool,
                node_to_split,
                &UpdateHierarchyOp::Add,
                data_len,
            );
            //MappedFile::check_all_nodes(&file);

            MappedFile::print_all_used_nodes(&file, "MAPPED FILE: AFTER INSERT INLINE");

            events.push(MappedFileEvent::NodeChanged {
                node_index: node_to_split.unwrap(),
            });

            file.events = events.clone();

            return (data_len as usize, events);
        }

        ////////////////////////////////////////////////
        // new subtree

        if DEBUG {
            dbg_println!("MAPPED FILE: allocate new subtree");
        }

        let rcfile = it_.get_file();
        let mut file = rcfile.write();

        let base_offset = match node_to_split {
            Some(idx) => file.pool[idx as usize].storage_offset.unwrap_or(0),
            None => 0,
        };

        let (prev_idx, next_idx, gparent_idx) = if let Some(idx) = node_to_split {
            (
                file.pool[idx].link.prev,
                file.pool[idx].link.next,
                file.pool[idx].link.parent,
            )
        } else {
            assert_eq!(file.size(), 0);
            (None, None, None)
        };

        let room = file.sub_page_reserve;
        let sub_page_size = file.sub_page_size;

        let new_size: usize = (node_size as usize) + data.len();

        // TODO(ceg): provide user apis to tweak allocations
        let sub_page_min_size = sub_page_size as usize;
        //let new_page_size = ::std::cmp::min(new_size / sub_page_min_size, sub_page_min_size);
        //let new_page_size = ::std::cmp::max(new_page_size, sub_page_min_size);
        //let new_page_size = new_page_size / 2;
        let new_page_size = sub_page_min_size;

        if DEBUG {
            dbg_println!("MAPPED FILE: new_size {}", new_size);
            dbg_println!("MAPPED FILE: new_page_size {}", new_page_size);
        }

        let subroot_node = Node {
            used: true,
            to_delete: false,
            size: new_size as u64,
            link: NodeLinks::with_parent(gparent_idx),
            page: Weak::new(),
            cow: None,
            storage_offset: None,
            indexed: false,
            byte_count: [0; 256],
        };

        let (subroot_idx, _) = file
            .pool
            .allocate(subroot_node, &MappedFile::assert_node_is_unused);

        if DEBUG {
            dbg_println!(
                "MAPPED FILE: create new tree with room for {} bytes \
                 inserts subroot_index({}), base_offset({:?})",
                new_size,
                subroot_idx,
                base_offset
            );
        }

        let mut leaves = Vec::new();
        MappedFile::build_tree(
            PageSource::FromRam,
            &mut file.pool,
            &mut leaves,
            Some(subroot_idx),
            new_page_size as u64,
            new_size as u64,
            base_offset,
        );

        if DEBUG {
            dbg_println!("MAPPED FILE: number of leaves = {}", leaves.len());
            dbg_println!("MAPPED FILE: node_size = {}", node_size);
            dbg_println!("MAPPED FILE: local_offset = {}", local_offset);
        }

        // use a flat map for data copying
        let mut input_slc = Vec::new();

        // before it
        if let Some(ref page) = &it_page {
            if local_offset > 0 {
                let slc = page.borrow().as_slice().unwrap();
                input_slc.push(&slc[0..local_offset as usize]);
            }
        }

        // at it
        input_slc.push(data);

        // after it
        if let Some(ref page) = &it_page {
            if node_size > 0 {
                let slc = page.borrow().as_slice().unwrap();
                input_slc.push(&slc[local_offset as usize..node_size as usize]);
            }
        }

        // build flatmap iterator
        let mut input_data_iter = input_slc.iter().flat_map(|&x| x.iter());

        // copy
        let mut prev_idx = prev_idx;
        let mut remain = new_size;
        for idx in &leaves {
            if DEBUG {
                dbg_println!("MAPPED FILE: copy data",);
                dbg_println!("MAPPED FILE: node_size = {}", node_size);
                dbg_println!("MAPPED FILE: local_offset = {}", local_offset);
            }

            // alloc+fill node
            {
                let mut n = &mut file.pool[*idx];
                let mut v = Vec::with_capacity(n.size as usize + room);

                if DEBUG {
                    dbg_println!("MAPPED FILE: v.len() = {}", v.len());
                    dbg_println!("MAPPED FILE: v.capacity() = {}", v.capacity());
                }

                for _ in 0..n.size {
                    if let Some(b) = input_data_iter.next() {
                        v.push(*b);
                        remain -= 1;
                    } else {
                        panic!("MAPPED FILE: internal error");
                    }
                }

                // store new page
                let p = Node::vec_to_page(v);
                let rc = Rc::new(RefCell::new(p));
                n.page = Rc::downgrade(&rc);
                n.cow = Some(rc);

                assert_eq!(n.storage_offset, None);
            }

            // link leaves
            MappedFile::link_prev_next_nodes(&mut file.pool, prev_idx, Some(*idx));
            prev_idx = Some(*idx);

            // push events
            events.push(MappedFileEvent::NodeAdded { node_index: *idx }); // copy data ?
        }
        // link last leaf
        MappedFile::link_prev_next_nodes(&mut file.pool, prev_idx, next_idx);

        assert_eq!(remain, 0);

        // TODO(ceg): check reparenting
        // swap subroot_idx and node_idx
        if let Some(node_to_split) = node_to_split {
            // MappedFile::exchage_nodes(gparent, node_to_split);
            if let Some(gparent_idx) = gparent_idx {
                // update grand parent left or right // delete
                let gparent_left = file.pool[gparent_idx].link.left;
                let gparent_right = file.pool[gparent_idx].link.right;

                if let Some(gp_left) = gparent_left {
                    if gp_left == node_to_split {
                        //                        dbg_println!("update grand parent left");
                        file.pool[gparent_idx].link.left = Some(subroot_idx);
                    }
                }

                if let Some(gp_right) = gparent_right {
                    if gp_right == node_to_split {
                        //                        dbg_println!("update grand parent right");
                        file.pool[gparent_idx].link.right = Some(subroot_idx);
                    }
                }

                //                dbg_println!("update subroot parent");
                file.pool[subroot_idx].link.parent = Some(gparent_idx);
            }

            // mark old node for deletion
            if DEBUG {
                dbg_println!("MAPPED FILE: MARK NODE {} TO DELETE", node_to_split);
            }

            file.pool[node_to_split].to_delete = true;
            events.push(MappedFileEvent::NodeRemoved {
                node_index: node_to_split,
            });
        }

        // check root
        if let Some(root_idx) = file.root_index {
            if let Some(node_to_split) = node_to_split {
                if root_idx == node_to_split {
                    file.root_index = Some(subroot_idx);
                    if DEBUG {
                        dbg_println!("new file.root_index {:?}", file.root_index);
                    }
                }
            }
        } else {
            file.root_index = Some(subroot_idx);
            if DEBUG {
                dbg_println!("new file.root_index {:?}", file.root_index);
            }
        }

        // update parent nodes size
        let p_idx = file.pool[subroot_idx as usize].link.parent;
        MappedFile::update_hierarchy(
            &mut file.pool,
            p_idx,
            &UpdateHierarchyOp::Add,
            data.len() as u64,
        );

        MappedFile::print_all_used_nodes(&file, "MAPPED FILE: AFTER INSERT");

        // TODO(ceg):
        // refresh iterator or next will crash
        //        it.file_size += size as u64;
        //        it_offset = base + it.local_offset;

        // refresh iterator or next will crash
        //      *it_ = {
        //          let it = it_.get_mut_ref().unwrap();
        //          MappedFile::iter_from(&it.file, it_offset)
        //      };

        //MappedFile::check_all_nodes(&file);

        file.events = events.clone();

        (data.len(), events)
    }

    /// remove data at iterator position, and refresh the iterator
    // 1 - get iterator's node info
    // 2 - remove the data, update hierarchy
    // 3 - re-balance the tree, starting at node_index
    // 4 - TODO(ceg): update iterator internal using find + local_offset on the modified subtree
    // TODO(ceg): split nodes before remove
    pub fn remove(it_: &mut FileIterator<'a>, nr: usize) -> (usize, Vec<MappedFileEvent>) {
        let mut events = vec![];
        if nr == 0 {
            return (0, events);
        }

        let mut remain = nr;
        let mut nr_removed = 0;

        let (mut file, start_idx, mut local_offset) = match &mut *it_ {
            MappedFileIterator::End(..) => return (0, events),

            MappedFileIterator::Real(ref it) => (it.file.write(), it.node_idx, it.local_offset),
        };

        dbg_println!("CALL CLEANUP");
        file.cleanup_events();

        MappedFile::print_all_used_nodes(&file, "MAPPED FILE: remove : BEFORE deletion");

        if DEBUG {
            dbg_println!("--- REMOVE {} bytes", nr);
        }

        if DEBUG {
            dbg_println!("--- tree before rebalance root_idx = {:?}", file.root_index);
            MappedFile::print_nodes(&file);
        }

        let mut idx = start_idx as usize;
        while remain > 0 {
            if DEBUG {
                dbg_println!("--- remain {} / nr {}", remain, nr);
            }

            // copy on write
            if file.pool[idx].cow.is_none() {
                // no need to check, not in ram
                let fd = if let Some(fd) = &file.fd {
                    Some(Arc::clone(fd))
                } else {
                    None
                };

                let page = file.pool[idx].move_to_ram(&fd);
                let rc = Rc::new(RefCell::new(page));
                file.pool[idx].page = Rc::downgrade(&rc);
                file.pool[idx].cow = Some(rc);
                file.pool[idx].storage_offset = None;
            }

            let node_subsize = (file.pool[idx].size - local_offset) as usize;
            let to_rm = ::std::cmp::min(remain, node_subsize);

            assert!(to_rm <= node_subsize);

            if DEBUG {
                dbg_println!("node_idx {}", idx);
                dbg_println!("node_size {}", file.pool[idx].size);
                dbg_println!("node_subsize {}", node_subsize);
                dbg_println!("to_rm {}", to_rm);
                dbg_println!("local_offset {}", local_offset);
            }

            match *file.pool[idx].cow.as_ref().unwrap().borrow_mut() {
                Page::ReadOnlyStorageCopy(..) => {
                    panic!("trying to write on read only memory");
                }

                Page::InRam(base, ref mut len, capacity) => {
                    let mut v = unsafe { Vec::from_raw_parts(base as *mut u8, *len, capacity) };
                    let index = local_offset as usize;

                    // Do not generate event for removed node
                    if to_rm != v.len() {
                        // DataRemove { idx, &v[index..index+to_rm], index , sz } )
                        events.push(MappedFileEvent::NodeChanged { node_index: idx });
                    }

                    v.drain(index..index + to_rm);
                    *len = v.len(); // update Page::InRam::len
                    mem::forget(v); // do not drop v
                }
            }

            remain -= to_rm;
            nr_removed += to_rm;
            local_offset = 0;

            // update nodes
            MappedFile::update_hierarchy(
                &mut file.pool,
                Some(idx),
                &UpdateHierarchyOp::Sub,
                to_rm as u64,
            );

            if file.pool[idx].link.next.is_none() {
                break;
            }
            idx = file.pool[idx].link.next.unwrap();
        }

        MappedFile::print_all_used_nodes(&file, "MAPPED FILE: remove : BEFORE REBALANCE");

        // re-balance tree
        {
            if DEBUG {
                dbg_println!(
                    "MAPPED FILE: --- tree BEFORE rebalance new_root_idx = {:?}",
                    file.root_index
                );
                MappedFile::print_nodes(&file);
            }

            let mut tmp_node = file.root_index;
            tmp_node = MappedFile::rebalance_subtree(&mut file.pool, tmp_node, &mut events);
            file.root_index = tmp_node;

            if DEBUG {
                dbg_println!(
                    "MAPPED FILE: --- tree AFTER rebalance new_root_idx = {:?}",
                    file.root_index
                );
                MappedFile::print_nodes(&file);
            }
        }

        MappedFile::print_all_used_nodes(&file, "MAPPED FILE: remove : AFTER REBALANCE");

        //MappedFile::check_all_nodes(&file);

        MappedFile::print_all_used_nodes(&file, "MAPPED FILE: AFTER REMOVE");

        file.events = events.clone();

        (nr_removed, events)
    }

    fn get_parent_relation(
        pool: &FreeListAllocator<Node>,
        parent: NodeIndex,
        child: NodeIndex,
    ) -> NodeRelation {
        if let Some(l) = pool[parent].link.left {
            if l == child {
                return NodeRelation::Left;
            }
        }

        if let Some(r) = pool[parent].link.right {
            if r == child {
                return NodeRelation::Right;
            }
        }

        NodeRelation::NoRelation
    }

    fn mark_node_to_release(
        pool: &mut FreeListAllocator<Node>,
        to_delete: &mut Vec<NodeIndex>,
        node: Option<NodeIndex>,
    ) {
        if let Some(idx) = node {
            if !pool[idx].to_delete {
                if DEBUG {
                    dbg_println!(" mark for deletion idx({})", idx);
                }
                to_delete.push(idx);
                pool[idx].to_delete = true;
            } else if DEBUG {
                dbg_println!(" idx({}) ALREDY MARK FOR DELETION", idx);
            }
        }
    }

    fn swap_parent_child(
        pool: &mut FreeListAllocator<Node>,
        parent_idx: Option<NodeIndex>,
        child_idx: Option<NodeIndex>,
    ) {
        if parent_idx.is_none() || child_idx.is_none() {
            return;
        }

        if DEBUG {
            dbg_println!("SWAP parent {:?} and child {:?}", parent_idx, child_idx);
        }

        let p_idx = parent_idx.unwrap();
        let child_idx = child_idx.unwrap();

        // clear children {
        if DEBUG {
            dbg_println!("reset {} left  : None", child_idx);
            dbg_println!("reset {} right : None", child_idx);
            dbg_println!("{:?} reset parent", pool[p_idx].link.left);
            dbg_println!("{:?} reset parent", pool[p_idx].link.right);
        }

        pool[p_idx].link.left = None;
        pool[p_idx].link.right = None;
        // }

        pool[child_idx].link.parent = None;

        // grand parent ?
        if let Some(gp_idx) = pool[p_idx].link.parent {
            let relation = MappedFile::get_parent_relation(&pool, gp_idx, p_idx);

            // TODO(ceg): helper func
            pool[p_idx].link.parent = None;
            if relation == NodeRelation::Left {
                pool[gp_idx].link.left = Some(child_idx);
            }
            if relation == NodeRelation::Right {
                pool[gp_idx].link.right = Some(child_idx);
            }

            pool[child_idx].link.parent = Some(gp_idx);
        }
    }

    // TODO(ceg): avoid this , really slow
    // rebalance
    // this function shrinks the tree by deleting parent nodes with one child
    fn get_best_child(
        to_delete: &mut Vec<NodeIndex>,
        mut pool: &mut FreeListAllocator<Node>,
        node_idx: Option<NodeIndex>,
    ) -> Option<NodeIndex> {
        node_idx?;

        let idx = node_idx.unwrap();
        let mut new_root = Some(idx);

        // leaf
        let have_parent = pool[idx].link.parent.is_some();
        let is_leaf = pool[idx].link.left.is_none() && pool[idx].link.right.is_none();
        let is_empty_leaf = pool[idx].size == 0 && is_leaf;

        // empty ?
        if pool[idx].size == 0 {
            if have_parent {
                // delete only if non root
                MappedFile::mark_node_to_release(&mut pool, to_delete, node_idx);

                // clear parent link
                {
                    let p_idx = pool[idx].link.parent.unwrap();
                    if pool[p_idx].link.left == Some(idx) {
                        pool[p_idx].link.left = None;
                        if DEBUG {
                            dbg_println!("clear {:?} left", pool[idx].link.parent);
                        }
                    }

                    if pool[p_idx].link.right == Some(idx) {
                        pool[p_idx].link.right = None;
                        if DEBUG {
                            dbg_println!("clear {:?} right", pool[idx].link.parent);
                        }
                    }
                }
            } else {
                new_root = None;
            }
        }

        if is_empty_leaf {
            // update links
            let prev = pool[idx].link.prev;
            let next = pool[idx].link.next;
            MappedFile::link_prev_next_nodes(&mut pool, prev, next);
        }

        if !is_leaf {
            let l = MappedFile::get_best_child(to_delete, pool, pool[idx].link.left);
            let r = MappedFile::get_best_child(to_delete, pool, pool[idx].link.right);

            let mut have_l = false;
            let mut have_r = false;

            let mut l_size = 0;
            let mut r_size = 0;
            if let Some(l) = l {
                have_l = true;
                l_size = pool[l].size;
            }
            if let Some(r) = r {
                have_r = true;
                r_size = pool[r].size;
            }

            if DEBUG {
                dbg_println!("({}).l_size = {}", idx, l_size);
                dbg_println!("({}).r_size = {}", idx, r_size);
            }

            // use match + guard
            if have_l && l_size > 0 && r_size == 0 {
                // move left upper
                MappedFile::swap_parent_child(pool, node_idx, l);
                new_root = l;
            }

            if have_r && r_size > 0 && l_size == 0 {
                // move right upper
                MappedFile::swap_parent_child(pool, node_idx, r);
                new_root = r;
            }
        }

        new_root
    }

    fn rebalance_subtree(
        mut pool: &mut FreeListAllocator<Node>,
        subroot: Option<NodeIndex>,
        events: &mut Vec<MappedFileEvent>,
    ) -> Option<NodeIndex> {
        let mut to_delete = vec![];

        let tmp_node = MappedFile::get_best_child(&mut to_delete, &mut pool, subroot);

        // clear
        if DEBUG {
            dbg_println!("to delete {:?}", to_delete);
        }
        for n in to_delete {
            if pool[n].to_delete {
                dbg_println!("MARK node {} for CLEANUP", n);
                events.push(MappedFileEvent::NodeRemoved { node_index: n });
            }
        }

        tmp_node
    }

    fn print_all_used_nodes(file: &MappedFile, rsn: &str) {
        if DEBUG {
            dbg_println!("*************  ALL USED NODES ({}) ***********", rsn);
            for i in 0..file.pool.slot.len() {
                let n = &file.pool.slot[i];
                if n.used {
                    dbg_println!("[{}] : {:?}", i, file.pool.slot[i]);
                } else {
                    assert_eq!(n.link.parent, None);
                    assert_eq!(n.link.prev, None);
                    assert_eq!(n.link.next, None);
                }
            }
            dbg_println!("***********************");
        }
    }

    fn check_tree(
        mut visited: &mut HashSet<NodeIndex>,
        idx: Option<NodeIndex>,
        pool: &FreeListAllocator<Node>,
    ) {
        if !DEBUG {
            return;
        }

        if idx.is_none() {
            return;
        }
        let idx = idx.unwrap();

        assert!((idx as usize) < pool.slot.len());

        if DEBUG {
            dbg_println!(" checking tree idx({})", idx);
        }

        assert!(pool.slot[idx].used);

        // already visited ?
        assert!(!visited.contains(&idx));
        visited.insert(idx);

        // check parent / children idx
        if let Some(l) = pool.slot[idx].link.left {
            if DEBUG {
                dbg_println!(
                    "MAPPED FILE: checking left's parent {:?} == {}.parent == {:?}",
                    Some(idx),
                    l,
                    pool.slot[l].link.parent
                );
            }
            assert_eq!(Some(idx), pool.slot[l].link.parent);
        }

        if let Some(r) = pool.slot[idx].link.right {
            if DEBUG {
                dbg_println!(
                    "MAPPED FILE: checking right's parent {:?} == {}.parent == {:?}",
                    Some(idx),
                    r,
                    pool.slot[r].link.parent
                );
            }

            assert_eq!(Some(idx), pool.slot[r].link.parent);
        }

        // recurse left / right
        MappedFile::check_tree(&mut visited, pool.slot[idx].link.left, &pool);
        MappedFile::check_tree(&mut visited, pool.slot[idx].link.right, &pool);
    }

    // This function can be very slow O(n)
    fn _check_all_nodes(file: &MappedFile) {
        if DEBUG {
            return;
        }

        if DEBUG {
            dbg_println!("check_all_nodes");
        }

        let mut visited = HashSet::new();

        MappedFile::check_tree(&mut visited, file.root_index, &file.pool);

        MappedFile::_check_leaves(&file);

        // check all nodes in allocator
        // if !used
        // assert prev/next/parent == None
        if DEBUG {
            dbg_println!("file.size({})", file.size());
        }

        visited.clear();

        let (idx, _, _) = file.find_node_by_offset(0);
        if idx.is_none() {
            if DEBUG {
                dbg_println!("no leaf found");
            }
            return;
        }
        let mut idx = idx.unwrap();

        if let Some(root_index) = file.root_index {
            if DEBUG {
                dbg_println!(
                    "MAPPED FILE: file root  idx({}) : {:?} ",
                    root_index,
                    file.pool.slot[root_index]
                );
            }
        }

        if DEBUG {
            dbg_println!("first leaf is {}", idx);
        }

        if file.pool.slot[idx].link.prev.is_some() {
            if DEBUG {
                dbg_println!(
                    "MAPPED FILE: prev node is set to {:?} ????",
                    file.pool.slot[idx].link.prev
                );
                dbg_println!("current leaf is idx({}) : {:?} ", idx, file.pool.slot[idx]);
            }
            panic!();
        };

        let file_size = file.size() as u64;
        let mut size_checked = 0;
        loop {
            let n = &file.pool.slot[idx];

            if DEBUG {
                dbg_println!("current leaf is idx({}) : {:?} ", idx, n);
            }

            assert!(n.used);

            if visited.contains(&idx) {
                panic!();
            }

            size_checked += n.size;

            if n.link.next.is_none() {
                break;
            }

            if size_checked >= file_size {
                panic!();
            }

            visited.insert(idx);

            idx = n.link.next.unwrap();
        }

        if size_checked != file_size {
            if DEBUG {
                dbg_println!(
                    "MAPPED FILE: size_checked({}) != file.size({})",
                    size_checked,
                    file.size()
                );
            }
            panic!();
        }

        for i in 0..file.pool.slot.len() {
            let n = &file.pool.slot[i];
            if !n.used {
                assert_eq!(n.link.parent, None);
                assert_eq!(n.link.prev, None);
                assert_eq!(n.link.next, None);
            }
        }
    }

    fn _check_leaves(file: &MappedFile) {
        let (idx, _, _) = file.find_node_by_offset(0);
        if idx.is_none() {
            return;
        }

        let mut idx = idx.unwrap() as usize;

        let mut off = 0;
        let file_size = file.size();
        loop {
            if DEBUG {
                dbg_println!("{} / {}", off, file_size);
                dbg_println!("file.pool[{}].size = {}", idx, file.pool[idx].size);
            }
            if DEBUG {
                dbg_println!(
                    "MAPPED FILE: off({}) + {} >= file.size({}) ?",
                    off,
                    file.pool[idx].size,
                    file_size
                );
            }
            off += file.pool[idx].size;
            if off > file_size {
                panic!("invalid tree, broken node size: off > file_size");
            }
            if off == file_size {
                break;
            }

            if let Some(next) = file.pool[idx].link.next {
                idx = next;
            } else {
                panic!("invalid tree, broken next link");
            }
            //TODO(ceg): check prev
        }
    }

    pub fn patch_storage_offset_and_file_descriptor(file: &mut MappedFile, new_fd: File) {
        let new_fd = Some(Arc::new(RwLock::new(new_fd)));

        let mut count: u64 = 0;
        let mut offset = 0;
        let (mut n, _, _) = MappedFile::find_node_by_offset(&file, offset);
        while n.is_some() {
            let idx = n.unwrap();
            let node_size = file.pool[idx].size;

            // node on storage ?
            if file.pool[idx].cow.is_none() {
                // ReadOnlyStorageCopy
                file.pool[idx].storage_offset = Some(offset);
            } else {
                assert_eq!(file.pool[idx].storage_offset, None); // TODO(ceg): check
            }

            if false {
                dbg_println!(
                    "MAPPED FILE: offset {}, page {}, size {} disk_offset {:?}",
                    offset,
                    count,
                    file.pool[idx].size,
                    file.pool[idx].storage_offset
                );
            }

            offset += node_size;
            n = file.pool[idx].link.next;

            count += 1;
        }

        file.fd = new_fd;
        dbg_println!("SYNC: file.fd = {:?}", file.fd);
    }

    pub fn sync_to_storage(file: &mut MappedFile, tmp_file_name: &str) -> ::std::io::Result<()> {
        let fd = File::open(tmp_file_name);
        if fd.is_err() {
            return Ok(());
        }
        let mut fd = fd.unwrap();

        let orig_fd = if let Some(fd) = &file.fd {
            Some(Arc::clone(fd))
        } else {
            None
        };

        let mut offset = 0;
        let (mut n, _, _) = MappedFile::find_node_by_offset(&file, offset);
        while n.is_some() {
            let idx = n.unwrap();

            let node_size = file.pool[idx].size;

            // map
            let page = file.pool[idx].map(&orig_fd).unwrap();
            let slice = page.borrow().as_slice().unwrap();

            // copy
            let nw = fd.write(slice).unwrap(); //TODO(ceg): handle result
            if nw != slice.len() {
                panic!("write error");
            }

            offset += node_size;
            n = file.pool[idx].link.next;
        }

        MappedFile::patch_storage_offset_and_file_descriptor(file, fd);

        Ok(())
    }
}

/////////////////////////////////////////////////////////////////////////////////////////////////////////////
#[derive(Debug, Clone)]
pub enum MappedFileIterator<'a> {
    End(FileHandle<'a>),
    Real(IteratorInstance<'a>),
}

impl<'a> MappedFileIterator<'a> {
    fn get_mut_ref(&mut self) -> Option<&mut IteratorInstance<'a>> {
        match *self {
            MappedFileIterator::End(..) => None, // TODO(ceg): return sentinel ?
            MappedFileIterator::Real(ref mut it) => Some(it),
        }
    }

    fn get_file(&mut self) -> FileHandle<'a> {
        match *self {
            MappedFileIterator::End(ref file) => Arc::clone(file),
            MappedFileIterator::Real(ref it) => Arc::clone(&it.file),
        }
    }

    pub fn get_real_iterator_ref(&self) -> Option<&IteratorInstance<'a>> {
        match *self {
            MappedFileIterator::End(..) => None, // TODO(ceg): return sentinel ?
            MappedFileIterator::Real(ref it) => Some(it),
        }
    }

    pub fn get_offset(&self) -> Option<u64> {
        match *self {
            MappedFileIterator::End(..) => None, // TODO(ceg): return sentinel ?
            MappedFileIterator::Real(ref it) => {
                let mut pos = it.local_offset;
                let mut idx = it.node_idx;
                let file = it.file.read();
                loop {
                    let node = &file.pool[idx];
                    if node.link.parent.is_none() {
                        break;
                    }
                    let parent_idx = node.link.parent.unwrap();
                    let relation = MappedFile::get_parent_relation(&file.pool, parent_idx, idx);
                    match relation {
                        NodeRelation::Right => {
                            /* add left node size*/
                            if let Some(left_idx) = file.pool[parent_idx].link.left {
                                pos += file.pool[left_idx].size;
                            }
                            idx = parent_idx;
                        }
                        NodeRelation::Left => {
                            idx = parent_idx;
                        }
                        _ => {
                            panic!("");
                        }
                    }

                    /*

                                  [ parent ]
                                 /          \
                                /            \
                       [   |l_local_off| ]  [  |r_local_offset| ]

                    */
                }
                Some(pos)
            }
        }
    }
}

#[derive(Debug, Clone)]
pub struct IteratorInstance<'a> {
    file: FileHandle<'a>,
    file_size: u64,
    local_offset: u64,
    page_size: u64,
    pub node_idx: NodeIndex,
    page: Rc<RefCell<Page>>,
    base: &'a [u8],
}

impl<'a> Deref for MappedFileIterator<'a> {
    type Target = u8;
    fn deref(&self) -> &u8 {
        match *self {
            MappedFileIterator::End(..) => panic!("invalid iterator"),
            MappedFileIterator::Real(ref it) => &it.base[it.local_offset as usize],
        }
    }
}

impl<'a> Iterator for MappedFileIterator<'a> {
    type Item = Self;

    fn next(&mut self) -> Option<Self> {
        match *self {
            MappedFileIterator::End(..) => None,

            MappedFileIterator::Real(ref mut it) => {
                if it.local_offset == it.page_size {
                    let mut file = it.file.write();

                    let fd = if file.fd.is_none() {
                        None
                    } else {
                        Some(file.fd.as_ref().unwrap().clone())
                    };

                    let next_node_idx = {
                        let node = &mut file.pool[it.node_idx as usize];

                        // end-of-file ?
                        if node.link.next == None {
                            return None;
                        }

                        node.link.next.unwrap()
                    };

                    let next_node = &mut file.pool[next_node_idx as usize];

                    let page = next_node.map(&fd).unwrap();
                    let slice = page.borrow().as_slice().unwrap();

                    it.node_idx = next_node_idx;
                    it.page_size = next_node.size;
                    it.page = next_node.page.upgrade().unwrap();
                    it.local_offset = 0;
                    it.base = slice;
                }

                it.local_offset += 1;

                Some(MappedFileIterator::Real(IteratorInstance {
                    file: Arc::clone(&it.file),
                    file_size: it.file_size,
                    node_idx: it.node_idx,
                    local_offset: it.local_offset - 1,
                    page: Rc::clone(&it.page),
                    page_size: it.page_size,
                    base: it.base,
                }))
            }
        }
    }
}

/// tests
#[cfg(test)]
mod tests {

    #[test]
    fn test_tree() {
        use super::*;
        use std::rc::Weak;

        let mut pool = FreeListAllocator::new();
        let file_size = 1024 * 1024 * 1024 * 1024 * 8; // x Tib
        let page_size = 4096 * 256 * 4; // 4 Mib

        let root_node = Node {
            used: true,
            to_delete: false,
            size: file_size,
            link: NodeLinks::new(),
            page: Weak::new(),
            cow: None,
            storage_offset: None,
            indexed: false,
            byte_count: [0; 256],
        };

        let (id, _) = pool.allocate(root_node, &MappedFile::assert_node_is_unused);

        let mut leaves = Vec::new();
        MappedFile::build_tree(
            PageSource::FromRam,
            &mut pool,
            &mut leaves,
            Some(id),
            page_size as u64,
            file_size as u64,
            0,
        );
        let mut prev_idx = None;
        for idx in &leaves {
            MappedFile::link_prev_next_nodes(&mut pool, prev_idx, Some(*idx));
            prev_idx = Some(*idx);
        }

        dbg_println!("file_size : bytes {}", file_size);
        dbg_println!("file_size : Kib {}", file_size >> 10);
        dbg_println!("file_size : Mib {}", file_size >> 20);
        dbg_println!("file_size : Gib {}", file_size >> 30);
        dbg_println!("file_size : Tib {}", file_size >> 40);

        dbg_println!("page_size : bytes {}", page_size);
        dbg_println!("page_size : Kib {}", page_size >> 10);
        dbg_println!("page_size : Mib {}", page_size >> 20);
        dbg_println!("page_size : Gib {}", page_size >> 30);

        dbg_println!("number of leaves : {}", leaves.len());
        dbg_println!("number of nodes : {}", pool.slot.len());

        let node_ram_size = ::std::mem::size_of::<Node>();
        dbg_println!("size_of::<Node> : {}", node_ram_size);

        let ram = pool.slot.capacity() * node_ram_size;
        dbg_println!("ram : bytes {}", ram);
        dbg_println!("ram : Kib {}", ram >> 10);
        dbg_println!("ram : Mib {}", ram >> 20);
        dbg_println!("ram : Gib {}", ram >> 30);

        use std::io;

        if false {
            dbg_println!("Hit [Enter] to stop");
            let mut stop = String::new();
            io::stdin().read_line(&mut stop).expect("something");
        }
    }

    #[test]
    fn test_remove() {
        use super::*;

        // TODO(ceg): loop over nb_page [2->256]
        let nb_page = 2;
        let page_size = 4096;
        let file_size = page_size * nb_page;
        let nr_remove = page_size * (nb_page - 1);
        let offset = page_size as u64;

        use std::fs;
        use std::fs::File;

        let filename = "/tmp/playground_remove_test".to_owned();
        let mut file = File::create(&filename).unwrap();

        // prepare file content
        dbg_println!("-- generating test file size({})", file_size);
        let mut slc = Vec::with_capacity(file_size);
        for i in 0..file_size {
            if (i % (1024 * 1024 * 256)) == 0 {
                dbg_println!("-- @ bytes {}", i);
            }

            let val = if i % 100 == 0 {
                '\n' as u8
            } else {
                (('0' as i32) + (i as i32 % 10)) as u8
            };
            slc.push(val);
        }
        file.write_all(slc.as_slice()).unwrap();
        file.sync_all().unwrap();
        drop(slc);

        dbg_println!("-- mapping the test file");
        let file = match MappedFile::new(Id(0), filename) {
            Some(file) => file,
            None => panic!("cannot map file"),
        };

        dbg_println!(
            "MAPPED FILE: -- testing remove {} @ {} from {}",
            nr_remove,
            offset,
            file_size
        );
        let mut it = MappedFile::iter_from(&file, offset);
        MappedFile::remove(&mut it, nr_remove);

        dbg_println!("-- file.size() {}", file.read().size());
        let _ = fs::remove_file("/tmp/playground_remove_test");
    }

    #[test]
    fn test_1m_insert() {
        use super::*;
        use std::fs;
        use std::fs::File;

        let filename = "/tmp/playground_insert_test".to_owned();
        let _ = fs::remove_file("/tmp/playground_insert_test");
        File::create(&filename).unwrap();

        dbg_println!("-- mapping the test file");
        let file = match MappedFile::new(Id(0), filename) {
            Some(file) => file,
            None => panic!("cannot map file"),
        };

        file.write().sub_page_size = 1024 * 128;
        file.write().sub_page_reserve = 1024 * 4;

        for i in 0..1_000_000 {
            {
                dbg_println!("-- test loop {}", i);
                let mut it = MappedFile::iter_from(&file, 0);
                MappedFile::insert(&mut it, &['A' as u8]);
            }
        }

        dbg_println!("-- file.size() {}", file.read().size());
        let _ = fs::remove_file("/tmp/playground_insert_test");
    }

    #[test]
    fn test_1b_insert() {
        use super::*;

        use std::fs;
        use std::fs::File;
        //        use std::io::prelude::*;

        let filename = "/tmp/playground_insert_test".to_owned();
        {
            let mut file = File::create(&filename).unwrap();

            // prepare file content
            dbg_println!("-- generating test file");
            let file_size = 4096 * 10;
            let mut slc = Vec::with_capacity(file_size);
            for i in 0..file_size {
                if (i % (1024 * 1024 * 256)) == 0 {
                    dbg_println!("-- @ bytes {}", i);
                }

                let val = if i % 100 == 0 {
                    '\n' as u8
                } else {
                    (('0' as i32) + (i as i32 % 10)) as u8
                };
                slc.push(val);
            }
            file.write_all(slc.as_slice()).unwrap();
            file.sync_all().unwrap();
        }

        dbg_println!("-- mapping the test file");
        let file = match MappedFile::new(Id(0), filename) {
            Some(file) => file,
            None => panic!("cannot map file"),
        };

        file.write().sub_page_size = 4096;
        file.write().sub_page_reserve = 10;

        for i in 0..5 {
            dbg_println!("-- insert loop {}", i);
            let i = i * 2;
            dbg_println!("-- build it @ {}", i + i * 4096);
            {
                let mut it = MappedFile::iter_from(&file, i + i * 4096);
                dbg_println!("--  sub insert 1");
                // TODO(ceg): change interface to consume iterator on insert to show that it is invalid
                MappedFile::insert(&mut it, &['\n' as u8]);
            }
        }

        MappedFile::sync_to_storage(&mut file.write(), &"/tmp/mapped_file.sync_test").unwrap();

        dbg_println!("-- file.size() {}", file.read().size());

        let _ = fs::remove_file("/tmp/mapped_file.sync_test.result");
        let _ = fs::remove_file("/tmp/playground_insert_test");
    }
}
