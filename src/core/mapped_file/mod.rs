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
// MappedFile is binary tree that provides on-demand data mapping, and keeps only the modified areas in memory.
// the leaves are linked to allow fast sequential traversal.
//

extern crate libc;

use std::collections::HashSet;

use std::cell::RefCell;

use std::ffi::CString;
use std::marker::PhantomData;
use std::mem;
use std::ops::Deref;
use std::ops::Index;
use std::ops::IndexMut;
use std::ptr;
use std::rc::Rc;
use std::rc::Weak;
use std::slice;

const DEBUG: bool = false;

use self::libc::{
    c_int,
    c_void,
    close,
    fstat,
    mmap,
    munmap,
    open,
    posix_fadvise, // posix_madvise,
    size_t,
    unlink,
    write,
    MAP_FAILED,
    MAP_PRIVATE,
    O_CREAT,
    O_RDONLY,
    O_RDWR,
    O_TRUNC,
    PROT_READ,
    S_IFDIR,
    S_IRUSR,
    S_IWUSR,
};

//////////////////////////////////////////////////////////////////////////////////////////

#[derive(Debug, Clone)]
enum Page {
    OnDisk(*const u8, size_t, size_t, c_int), // base, len, skip, fd
    InRam(*const u8, usize, usize),           // base, len, capacity
}

impl Page {
    fn as_slice<'a>(&self) -> Option<&'a [u8]> {
        Some(match *self {
            Page::OnDisk(base, len, ..) => unsafe { slice::from_raw_parts(base, len) },

            Page::InRam(base, len, ..) => unsafe { slice::from_raw_parts(base, len) },
        })
    }
}

impl Drop for Page {
    fn drop(&mut self) {
        match *self {
            Page::OnDisk(base, len, skip, ..) => {
                // eprintln!("munmap {:?}", base);
                let _base =
                    unsafe { munmap(base.offset(-(skip as isize)) as *mut c_void, len + skip) };
            }

            Page::InRam(base, len, capacity) => {
                let v = unsafe { Vec::from_raw_parts(base as *mut u8, len, capacity) };
                drop(v);
            }
        }
    }
}

//////////////////////////////////////////////////////////////////////////////////////////

#[derive(Debug, Clone)]
enum UpdateHierarchyOp {
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

type NodeIndex = usize;
type NodeSize = u64;
type NodeLocalOffset = u64;

pub type FileHandle<'a> = Rc<RefCell<MappedFile<'a>>>;
pub type FileIterator<'a> = MappedFileIterator<'a>;

#[derive(Debug, Clone, Default)]
struct Node {
    // state ?
    used: bool,
    to_delete: bool,

    fd: c_int,

    // idx: NodeIndex, // for DEBUG
    parent: Option<NodeIndex>,
    left: Option<NodeIndex>,
    right: Option<NodeIndex>,
    prev: Option<NodeIndex>,
    next: Option<NodeIndex>,

    // data
    size: u64,
    on_disk_offset: u64,
    skip: u64,

    page: Weak<RefCell<Page>>,
    cow: Option<Rc<RefCell<Page>>>,
}

impl Node {
    fn clear(&mut self) {
        self.used = false;
        self.to_delete = false;
        self.fd = -1;
        // self.idx = 0xffff_ffff_ffff_ffff as NodeIndex;
        self.parent = None;
        self.left = None;
        self.right = None;
        self.prev = None;
        self.next = None;
        self.size = 0;
        self.on_disk_offset = 0xffff_ffff_ffff_ffff as u64;
        self.skip = 0;
        self.page = Weak::new();
        self.cow = None;
    }

    fn map(&mut self) -> Option<Rc<RefCell<Page>>> {
        // ram ?
        if let Some(ref page) = self.cow {
            return Some(Rc::clone(page));
        }

        // already mapped ?
        if let Some(page) = self.page.upgrade() {
            return Some(page);
        }

        // do map
        let ptr = unsafe {
            mmap(
                ptr::null_mut(),
                (self.size + self.skip) as usize,
                PROT_READ,
                MAP_PRIVATE,
                self.fd,
                self.on_disk_offset as i64,
            )
        };

        if ptr == MAP_FAILED {
            eprintln!(
                "mmap error : disk_offset = {}, size = {}",
                self.on_disk_offset, self.size
            );
            return None;
        }

        let page = Rc::new(RefCell::new(Page::OnDisk(
            unsafe { ptr.offset(self.skip as isize) as *const u8 },
            self.size as usize,
            self.skip as usize,
            self.fd,
        )));

        self.page = Rc::downgrade(&page);

        Some(page)
    }

    // will consume v
    fn vec_to_page(mut v: Vec<u8>) -> Page {
        // from Vec doc
        // Pull out the various important pieces of information about `v`
        let base = v.as_mut_ptr() as *const u8;
        let len = v.len();
        let capacity = v.capacity();

        mem::forget(v);

        // 5 - build "new" page
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
                panic!("cannot be used on Ondisk page");
            }
        }
    }

    fn move_to_ram(&mut self) -> Page {
        // 1 - save all page iterators local offsets

        // 2 - map the page // will invalidate iterators base pointer
        // TODO: check
        let page = self.map().unwrap();
        let slice = page.as_ref().borrow().as_slice().unwrap();

        // 3 - allocate a vector big enough to hold page data
        let mut v = Vec::with_capacity(self.size as usize);

        // 4 - copy page to vector
        v.extend_from_slice(slice);

        Node::vec_to_page(v)
    }
}

///////////////////////////////////////////////////////////////////////////////////////////////////

#[derive(Debug)]
struct FreeListAllocator<T> {
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

    fn allocate(&mut self, n: T, check_previous: &dyn Fn(&mut T)) -> (NodeIndex, &mut T) {
        if !self.free_indexes.is_empty() {
            let i = self.free_indexes.pop().unwrap();
            if DEBUG {
                eprintln!("node allocator reuse slot {}", i);
            }
            check_previous(&mut self.slot[i]);
            self.slot[i] = n;
            (i as NodeIndex, &mut self.slot[i])
        } else {
            let i = self.slot.len();
            self.slot.push(n);
            if DEBUG {
                eprintln!("node allocator create new slot {}", i);
            }
            (i as NodeIndex, &mut self.slot[i])
        }
    }

    fn release(&mut self, idx: NodeIndex) {
        //eprintln!("node allocator release slot {}", idx);
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

///////////////////////////////////////////////////////////////////////////////////////////////////

#[derive(Debug)]
pub struct MappedFile<'a> {
    phantom: PhantomData<&'a u8>,
    fd: c_int,
    pool: FreeListAllocator<Node>,
    root_index: Option<NodeIndex>,
    page_size: usize,
    /// size of new allocated blocks when splitting old ones (default 2 mib)
    pub sub_page_size: usize,
    /// reserve storage on new allocated blocks (default 2 kib)
    pub sub_page_reserve: usize,
}

impl<'a> Drop for MappedFile<'a> {
    fn drop(&mut self) {
        unsafe { close(self.fd) };
    }
}

impl<'a> MappedFile<'a> {
    fn assert_node_is_unused(n: &mut Node) {
        assert_eq!(n.used, false);
    }

    pub fn new(path: String, page_size: usize) -> Option<FileHandle<'a>> {
        let path = CString::new(path).unwrap();

        let fd = unsafe { open(path.as_ptr(), O_RDONLY) };
        if fd < 0 {
            return None;
        }

        let mut stbuff: libc::stat = unsafe { ::std::mem::zeroed() };
        unsafe {
            if fstat(fd, &mut stbuff) != 0 {
                panic!("cannot get file informations");
            }
        }

        if S_IFDIR & stbuff.st_mode != 0 {
            return None;
        }

        let file_size = stbuff.st_size as u64;

        unsafe {
            posix_fadvise(fd, 0, 0, 2 /*POSIX_FADV_SEQUENTIAL*/);
        }

        let mut file = MappedFile {
            phantom: PhantomData,
            fd,
            pool: FreeListAllocator::new(),
            root_index: None,
            page_size,
            sub_page_size: 4096 * 256 * 2, // 2 mib
            sub_page_reserve: 2 * 1024,    // 2 kib
        };

        if file_size == 0 {
            return Some(Rc::new(RefCell::new(file)));
        }

        let root_node = Node {
            used: true,
            to_delete: false,
            fd,
            //idx: 0,
            size: file_size,
            parent: None,
            left: None,
            right: None,
            prev: None,
            next: None,
            page: Weak::new(),
            cow: None,
            on_disk_offset: 0,
            skip: 0,
        };

        let (id, _) = file
            .pool
            .allocate(root_node, &MappedFile::assert_node_is_unused);
        file.root_index = Some(id);

        let mut leaves = Vec::new();
        MappedFile::build_tree(
            &mut file.pool,
            fd,
            &mut leaves,
            Some(id),
            page_size as u64,
            file_size as u64,
            0,
        );

        let mut prev_idx = None;
        for idx in leaves {
            MappedFile::link_prev_next_nodes(&mut file.pool, prev_idx, Some(idx));
            prev_idx = Some(idx);

            // TODO: add hints to map all nodes
            /*
            if file_size <= page_size as u64 {
                let p = file.pool[idx as usize].move_to_ram();
                let rc = Rc::new(RefCell::new(p));
                file.pool[idx as usize].page = Rc::downgrade(&rc);
                file.pool[idx as usize].cow = Some(rc);
                file.pool[idx as usize].on_disk_offset = 0xffff_ffff_ffff_ffff;
            }
            */
        }

        MappedFile::check_tree(&mut HashSet::new(), file.root_index, &file.pool);

        Some(Rc::new(RefCell::new(file)))
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
        if let Some(p_idx) = prev_idx {
            pool[p_idx as usize].next = next_idx;
            // eprintln!("link_next : prev({:?})  -> next({:?})", prev_idx, next_idx);
        }

        if let Some(n_idx) = next_idx {
            pool[n_idx as usize].prev = prev_idx;
            // eprintln!("link_prev : prev({:?})  <- next({:?})", prev_idx, next_idx);
        }
    }

    fn _link_parent_child(
        pool: &mut FreeListAllocator<Node>,
        parent_idx: Option<NodeIndex>,
        child_idx: Option<NodeIndex>,
        relation: &NodeRelation,
    ) {
        if let Some(child_idx) = child_idx {
            pool[child_idx].parent = parent_idx;
            if DEBUG {
                eprintln!(
                    "link_parent : child({:?})  -> parent({:?})",
                    child_idx, parent_idx
                );
            }
        }

        let (node_ref, name) = if let Some(parent_idx) = parent_idx {
            match relation {
                NodeRelation::Left => (&mut pool[parent_idx].left, "left"),
                NodeRelation::Right => (&mut pool[parent_idx].right, "right"),
                _ => unimplemented!(),
            }
        } else {
            return;
        };

        *node_ref = child_idx;

        if DEBUG {
            eprintln!(
                "link_child : parent({:?}).{} -> child({:?})",
                parent_idx, name, child_idx
            );
        }
    }

    pub fn print_nodes(file: &MappedFile) {
        for (idx, n) in file.pool.slot.iter().enumerate() {
            if n.used {
                eprintln!(
                    "idx({:?}), parent({:?}) left({:?}) right({:?}) prev({:?}) \
                     next({:?}) size({}) ", // on_disk_off({})",
                    idx,
                    n.parent,
                    n.left,
                    n.right,
                    n.prev,
                    n.next,
                    n.size, // n.on_disk_offset
                )
            }
        }
    }

    fn build_tree(
        pool: &mut FreeListAllocator<Node>,
        fd: i32,
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
                eprintln!(
                    "node_size <= pg_size : \
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
                    pool[idx].on_disk_offset = base_offset;
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

        // create leaves : TODO: use default() ?
        // TODO: None::new(fd, parent, size, on_disk_offset)
        let left_node = Node {
            used: true,
            to_delete: false,
            fd,
            //            idx: 0,
            size: l_sz,
            parent,
            left: None,
            right: None,
            prev: None,
            next: None,
            page: Weak::new(),
            cow: None,
            on_disk_offset: base_offset,
            skip: 0,
        };
        let (l, _) = pool.allocate(left_node, &MappedFile::assert_node_is_unused);

        // TODO: None::new(fd, parent, size, on_disk_offset)
        let right_node = Node {
            used: true,
            to_delete: false,
            fd,
            //          idx: 0,
            size: r_sz,
            parent,
            left: None,
            right: None,
            prev: None,
            next: None,
            page: Weak::new(),
            cow: None,
            on_disk_offset: base_offset + l_sz,
            skip: 0,
        };

        let (r, _) = pool.allocate(right_node, &MappedFile::assert_node_is_unused);

        // build children
        MappedFile::build_tree(pool, fd, leaves, Some(l), pg_size, l_sz, b_off);
        MappedFile::build_tree(pool, fd, leaves, Some(r), pg_size, r_sz, b_off + l_sz);

        // link to parent
        if let Some(idx) = parent {
            let idx = idx as usize;
            pool[idx].left = Some(l);
            pool[idx].right = Some(r);

            //            pool[l as usize].idx = l;
            //            pool[r as usize].idx = r;
            if DEBUG {
                eprintln!("parent = {}, l = {}, r = {}", idx, l, r);
                eprintln!("parent = {:?}", pool[idx]);
                eprintln!("l idx {} = {:?}", l, pool[l]);
                eprintln!("r idx {} = {:?}", r, pool[r]);
            }
        }
    }

    fn find_sub_node_by_offset(
        &self,
        n: NodeIndex,
        offset: u64,
    ) -> (Option<NodeIndex>, NodeSize, NodeLocalOffset) {
        if DEBUG {
            eprintln!("find_sub_node_by_offset Ndi({}) off({})", n, offset);
        }
        let node = &self.pool[n as usize];

        assert!(node.used);

        let is_leaf = node.left.is_none() && node.right.is_none();

        if offset < node.size && is_leaf {
            (Some(n), node.size, offset)
        } else {
            let left_size = if let Some(left) = node.left {
                self.pool[left as usize].size
            } else {
                0
            };

            if DEBUG {
                eprintln!("   off({})  left_size({})", offset, left_size);
            }
            if offset < left_size {
                if DEBUG {
                    eprintln!("go   <----");
                }
                self.find_sub_node_by_offset(node.left.unwrap(), offset)
            } else {
                if DEBUG {
                    eprintln!("go   ---->");
                }
                self.find_sub_node_by_offset(node.right.unwrap(), offset - left_size)
            }
        }
    }

    fn find_node_by_offset(&self, offset: u64) -> (Option<NodeIndex>, NodeSize, NodeLocalOffset) {
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
        let file = file_.borrow_mut();

        let page = file.pool[node_idx as usize].page.upgrade().unwrap();
        let slice = page.as_ref().borrow_mut().as_slice().unwrap();

        MappedFileIterator::Real(IteratorInstance {
            file: Rc::clone(file_),
            file_size: file.size(),
            local_offset: 0,
            page_size: file.pool[node_idx as usize].size,
            node_idx,
            page,
            base: slice,
        })
    }

    pub fn iter_from(file_: &FileHandle<'a>, offset: u64) -> FileIterator<'a> {
        let mut file = file_.borrow_mut();
        let pair = file.find_node_by_offset(offset);
        match pair {
            (Some(node_idx), node_size, local_offset) => {
                let page = file.pool[node_idx as usize].map().unwrap();
                let slice = page.as_ref().borrow_mut().as_slice().unwrap();

                MappedFileIterator::Real(IteratorInstance {
                    file: Rc::clone(file_),
                    file_size: file.size(),
                    local_offset,
                    page_size: node_size,
                    node_idx,
                    page,
                    base: slice,
                })
            }

            (None, _, _) => {
                // eprintln!("ITER FROM END !!!!!");
                MappedFileIterator::End(Rc::clone(file_))
            }
        }
    }

    pub fn copy_to_slice(from: &mut FileIterator<'a>, nr_to_read: usize, vec: &mut [u8]) -> usize {
        if let MappedFileIterator::End(..) = *from {
            return 0;
        }

        let mut nr_read: usize = 0;
        let mut nr_to_read = nr_to_read;

        while nr_to_read > 0 {
            if let Some(ref mut it) = from.get_mut_ref() {
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
        if let MappedFileIterator::End(..) = *it_ {
            return 0;
        }

        let mut nr_read = 0;
        let mut nr_to_read = nr_to_read;

        while nr_to_read > 0 {
            if let Some(ref mut it) = it_.get_mut_ref() {
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
                eprint!(
                    "node({}).size {} op({:?}) {} ---> ",
                    idx, pool[idx as usize].size, op, value
                );
            }

            match op {
                UpdateHierarchyOp::Add => pool[idx as usize].size += value,
                UpdateHierarchyOp::Sub => pool[idx as usize].size -= value,
            }

            if DEBUG {
                eprintln!("{}", pool[idx as usize].size);
            }

            p_idx = pool[idx as usize].parent;
        }
    }

    fn check_free_space(it_: &mut MappedFileIterator) -> u64 {
        match &*it_ {
            MappedFileIterator::End(..) => 0,
            MappedFileIterator::Real(ref it) => match &it.page {
                ref rc => match *rc.as_ref().borrow_mut() {
                    Page::OnDisk { .. } => 0,

                    Page::InRam(_, ref mut len, capacity) => (capacity - *len) as u64,
                },
            },
        }
    }

    fn insert_in_place(it_: &mut FileIterator<'a>, data: &[u8]) {
        match &*it_ {
            MappedFileIterator::End(..) => panic!("trying to write on end iterator"),
            MappedFileIterator::Real(ref it) => match &it.page {
                ref rc => match *rc.as_ref().borrow_mut() {
                    Page::OnDisk { .. } => {
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
    // 7 - TODO: update iterator internal using find + local_offset on the allocated subtree
    pub fn insert(it_: &mut FileIterator<'a>, data: &[u8]) -> usize {
        let data_len = data.len() as u64;
        if data_len == 0 {
            return 0;
        }

        let (node_to_split, node_size, local_offset, it_page) = match &*it_ {
            MappedFileIterator::End(ref rcfile) => {
                let mut file = rcfile.as_ref().borrow_mut();

                MappedFile::print_all_used_nodes(&file, "BEFORE INSERT");

                let file_size = file.size();
                if file_size > 0 {
                    let (idx, node_size, _) = file.find_node_by_offset(file_size - 1);
                    let page = file.pool[idx.unwrap()].map();
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
            let file = rcfile.as_ref().borrow_mut();

            MappedFile::print_all_used_nodes(&file, "BEFORE INSERT");
        }
        if DEBUG {
            eprintln!("node_to_split {:?} / size ({})", node_to_split, node_size);
        }

        let available = MappedFile::check_free_space(it_);
        if DEBUG {
            eprintln!("available space = {}", available);
        }

        /////// in place insert ?

        if available >= data_len {
            // insert in current node
            MappedFile::insert_in_place(it_, data);

            // update parents
            let rcfile = it_.get_file();
            let mut file = rcfile.as_ref().borrow_mut();
            MappedFile::update_hierarchy(
                &mut file.pool,
                node_to_split,
                &UpdateHierarchyOp::Add,
                data_len,
            );
            MappedFile::check_all_nodes(&file);

            MappedFile::print_all_used_nodes(&file, "AFTER INSERT INLINE");

            return data_len as usize;
        }

        ////////////////////////////////////////////////
        // new subtree

        let rcfile = it_.get_file();
        let mut file = rcfile.as_ref().borrow_mut();

        let base_offset = match node_to_split {
            Some(idx) => file.pool[idx as usize].on_disk_offset,
            None => 0,
        };

        let (prev_idx, next_idx, gparent_idx) = if let Some(idx) = node_to_split {
            (
                file.pool[idx].prev,
                file.pool[idx].next,
                file.pool[idx].parent,
            )
        } else {
            (None, None, None)
        };

        let fd = file.fd;
        let room = file.sub_page_reserve;
        let sub_page_size = file.sub_page_size;

        let new_size: usize = (node_size as usize) + data.len();

        // TODO: provide user apis to tweak allocations
        let sub_page_min_size = sub_page_size as usize;
        let new_page_size = ::std::cmp::min(new_size / sub_page_min_size, sub_page_min_size);
        let new_page_size = ::std::cmp::max(new_page_size, sub_page_min_size);

        if DEBUG {
            eprintln!("new_size {}", new_size);
            eprintln!("new_page_size {}", new_page_size);
        }

        let subroot_node = Node {
            used: true,
            to_delete: false,
            fd,
            //            idx: 0,
            size: new_size as u64,
            parent: gparent_idx,
            left: None,
            right: None,
            prev: None,
            next: None,
            page: Weak::new(),
            cow: None,
            on_disk_offset: 0xffff_ffff_ffff_ffff, // base_offset,
            skip: 0,
        };

        let (subroot_idx, _) = file
            .pool
            .allocate(subroot_node, &MappedFile::assert_node_is_unused);
        // file.pool[subroot_idx as usize].idx = subroot_idx;

        if DEBUG {
            eprintln!(
                "create new tree with room for {} bytes \
                 inserts subroot_index({}), base_offset({})",
                new_size, subroot_idx, base_offset
            );
        }

        let mut leaves = Vec::new();
        MappedFile::build_tree(
            &mut file.pool,
            fd,
            &mut leaves,
            Some(subroot_idx),
            new_page_size as u64,
            new_size as u64,
            base_offset,
        );

        if DEBUG {
            eprintln!("number of leaves = {}", leaves.len());
            eprintln!("node_size = {}", node_size);
            eprintln!("local_offset = {}", local_offset);
        }

        // use a flat map for data copying
        let mut input_slc = Vec::new();

        // before it
        if let Some(ref page) = &it_page {
            if local_offset > 0 {
                let slc = page.as_ref().borrow().as_slice().unwrap();
                input_slc.push(&slc[0..local_offset as usize]);
            }
        }

        // at it
        input_slc.push(data);

        // after it
        if let Some(ref page) = &it_page {
            if node_size > 0 {
                let slc = page.as_ref().borrow().as_slice().unwrap();
                input_slc.push(&slc[local_offset as usize..node_size as usize]);
            }
        }

        // build flatmap iterator
        let mut input_data_iter = input_slc.iter().flat_map(|&x| x.iter());

        // copy
        let mut prev_idx = prev_idx;
        let mut remain = new_size;
        for idx in &leaves {
            // alloc+fill node
            {
                let mut n = &mut file.pool[*idx];
                let mut v = Vec::with_capacity(n.size as usize + room);

                for _ in 0..n.size {
                    if let Some(b) = input_data_iter.next() {
                        v.push(*b);
                        remain -= 1;
                    } else {
                        panic!("internal error");
                    }
                }

                // store new page
                let p = Node::vec_to_page(v);
                let rc = Rc::new(RefCell::new(p));
                n.page = Rc::downgrade(&rc);
                n.cow = Some(rc);
            }

            // link leaves
            MappedFile::link_prev_next_nodes(&mut file.pool, prev_idx, Some(*idx));
            prev_idx = Some(*idx);
        }
        // link last leaf
        MappedFile::link_prev_next_nodes(&mut file.pool, prev_idx, next_idx);

        assert_eq!(remain, 0);

        // TODO: check reparenting
        // swap subroot_idx and node_idx
        if let Some(node_to_split) = node_to_split {
            // MappedFile::exchage_nodes(gparent, node_to_split);
            if let Some(gparent_idx) = gparent_idx {
                // update grand parent left or right // delete
                let gparent_left = file.pool[gparent_idx].left;
                let gparent_right = file.pool[gparent_idx].right;

                if let Some(gp_left) = gparent_left {
                    if gp_left == node_to_split {
                        //                        eprintln!("update grand parent left");
                        file.pool[gparent_idx].left = Some(subroot_idx);
                    }
                }

                if let Some(gp_right) = gparent_right {
                    if gp_right == node_to_split {
                        //                        eprintln!("update grand parent right");
                        file.pool[gparent_idx].right = Some(subroot_idx);
                    }
                }

                //                eprintln!("update subroot parent");
                file.pool[subroot_idx].parent = Some(gparent_idx);
            }

            // clear+delete old node
            if DEBUG {
                eprintln!(" clear+delete old node idx({})", node_to_split);
            }
            file.pool[node_to_split].clear();
            file.pool.release(node_to_split);
        }

        // check root
        if let Some(root_idx) = file.root_index {
            if let Some(node_to_split) = node_to_split {
                if root_idx == node_to_split {
                    file.root_index = Some(subroot_idx);
                    if DEBUG {
                        eprintln!("new file.root_index {:?}", file.root_index);
                    }
                }
            }
        } else {
            file.root_index = Some(subroot_idx);
            if DEBUG {
                eprintln!("new file.root_index {:?}", file.root_index);
            }
        }

        // update parent nodes size
        let p_idx = file.pool[subroot_idx as usize].parent;
        MappedFile::update_hierarchy(
            &mut file.pool,
            p_idx,
            &UpdateHierarchyOp::Add,
            data.len() as u64,
        );

        MappedFile::print_all_used_nodes(&file, "AFTER INSERT");

        // TODO:
        // refresh iterator or next will crash
        //        it.file_size += size as u64;
        //        it_offset = base + it.local_offset;

        // refresh iterator or next will crash
        //      *it_ = {
        //          let it = it_.get_mut_ref().unwrap();
        //          MappedFile::iter_from(&it.file, it_offset)
        //      };

        MappedFile::check_all_nodes(&file);

        data.len()
    }

    /// remove data at iterator position, and refresh the iterator
    // 1 - get iterator's node info
    // 2 - remove the data, update hierachy
    // 3 - rebalance the tree, starting at node_index
    // 4 - TODO: update iterator internal using find + local_offset on the modified subtree
    // TODO: split nodes before remove
    pub fn remove(it_: &mut FileIterator<'a>, nr: usize) -> usize {
        if nr == 0 {
            return 0;
        }

        let mut remain = nr;
        let mut nr_removed = 0;

        let (mut file, start_idx, mut local_offset) = match &mut *it_ {
            MappedFileIterator::End(..) => return 0,

            MappedFileIterator::Real(ref it) => {
                (it.file.as_ref().borrow_mut(), it.node_idx, it.local_offset)
            }
        };

        MappedFile::print_all_used_nodes(&file, "remove : BEFORE deletion");

        if DEBUG {
            eprintln!("--- REMOVE {} bytes", nr);
        }

        if DEBUG {
            eprintln!("--- tree before rebalance root_idx = {:?}", file.root_index);
            MappedFile::print_nodes(&file);
        }

        let mut idx = start_idx as usize;
        while remain > 0 {
            if DEBUG {
                eprintln!("--- remain {} / nr {}", remain, nr);
            }

            // copy on write
            if file.pool[idx].cow.is_none() {
                let page = file.pool[idx].move_to_ram();
                let rc = Rc::new(RefCell::new(page));
                file.pool[idx].page = Rc::downgrade(&rc);
                file.pool[idx].cow = Some(rc);
                file.pool[idx].on_disk_offset = 0xffff_ffff_ffff_ffff;
                file.pool[idx].skip = 0;
            }

            let node_subsize = (file.pool[idx].size - local_offset) as usize;
            let to_rm = ::std::cmp::min(remain, node_subsize);

            assert!(to_rm <= node_subsize);

            if DEBUG {
                eprintln!("node_idx {}", idx);
                eprintln!("node_size {}", file.pool[idx].size);
                eprintln!("node_subsize {}", node_subsize);
                eprintln!("to_rm {}", to_rm);
                eprintln!("local_offset {}", local_offset);
            }

            match *file.pool[idx].cow.as_ref().unwrap().as_ref().borrow_mut() {
                Page::OnDisk { .. } => {
                    panic!("trying to write on read only memory");
                }

                Page::InRam(base, ref mut len, capacity) => {
                    let mut v = unsafe { Vec::from_raw_parts(base as *mut u8, *len, capacity) };
                    let index = local_offset as usize;
                    v.drain(index..index + to_rm);
                    *len = v.len();
                    mem::forget(v);
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

            if file.pool[idx].next.is_none() {
                break;
            }
            idx = file.pool[idx].next.unwrap();
        }

        MappedFile::print_all_used_nodes(&file, "remove : BEFORE REBALANCE");

        // rebalance tree
        {
            if DEBUG {
                eprintln!(
                    "--- tree BEFORE rebalance new_root_idx = {:?}",
                    file.root_index
                );
                MappedFile::print_nodes(&file);
            }

            let mut tmp_node = file.root_index;
            tmp_node = MappedFile::rebalance_subtree(&mut file.pool, tmp_node);
            file.root_index = tmp_node;

            if DEBUG {
                eprintln!(
                    "--- tree AFTER rebalance new_root_idx = {:?}",
                    file.root_index
                );
                MappedFile::print_nodes(&file);
            }
        }

        MappedFile::print_all_used_nodes(&file, "remove : AFTER REBALANCE");

        MappedFile::check_all_nodes(&file);

        MappedFile::print_all_used_nodes(&file, "AFTER REMOVE");

        nr_removed
    }

    fn get_parent_relation(
        pool: &FreeListAllocator<Node>,
        parent: NodeIndex,
        child: NodeIndex,
    ) -> NodeRelation {
        if let Some(l) = pool[parent].left {
            if l == child {
                return NodeRelation::Left;
            }
        }

        if let Some(r) = pool[parent].right {
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
                    eprintln!(" mark for deletion idx({})", idx);
                }
                to_delete.push(idx);
                pool[idx].to_delete = true;
            } else if DEBUG {
                eprintln!(" idx({}) ALREDY MARK FOR DELETION", idx);
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
            eprintln!("SWAP parent {:?} and child {:?}", parent_idx, child_idx);
        }

        let p_idx = parent_idx.unwrap();
        let child_idx = child_idx.unwrap();

        // clear children {
        if DEBUG {
            eprintln!("reset {} left  : None", child_idx);
            eprintln!("reset {} right : None", child_idx);
            eprintln!("{:?} reset parent", pool[p_idx].left);
            eprintln!("{:?} reset parent", pool[p_idx].right);
        }

        pool[p_idx].left = None;
        pool[p_idx].right = None;
        // }

        pool[child_idx].parent = None;

        // grand parent ?
        if let Some(gp_idx) = pool[p_idx].parent {
            let relation = MappedFile::get_parent_relation(&pool, gp_idx, p_idx);

            // TODO: helper func
            pool[p_idx].parent = None;
            if relation == NodeRelation::Left {
                pool[gp_idx].left = Some(child_idx);
            }
            if relation == NodeRelation::Right {
                pool[gp_idx].right = Some(child_idx);
            }

            pool[child_idx].parent = Some(gp_idx);
        }
    }

    // rebalance
    // this function shriks the tree by deleting parent nodes with one child
    fn get_best_child(
        to_delete: &mut Vec<NodeIndex>,
        mut pool: &mut FreeListAllocator<Node>,
        node_idx: Option<NodeIndex>,
    ) -> Option<NodeIndex> {
        node_idx?;

        let idx = node_idx.unwrap();
        let mut new_root = Some(idx);

        // leaf
        let have_parent = pool[idx].parent.is_some();
        let is_leaf = pool[idx].left.is_none() && pool[idx].right.is_none();
        let is_empty_leaf = pool[idx].size == 0 && is_leaf;

        // empty ?
        if pool[idx].size == 0 {
            if have_parent {
                // delete only if non root
                MappedFile::mark_node_to_release(&mut pool, to_delete, node_idx);

                // clear parent link
                {
                    let p_idx = pool[idx].parent.unwrap();
                    if pool[p_idx].left == Some(idx) {
                        pool[p_idx].left = None;
                        if DEBUG {
                            eprintln!("clear {:?} left", pool[idx].parent);
                        }
                    }

                    if pool[p_idx].right == Some(idx) {
                        pool[p_idx].right = None;
                        if DEBUG {
                            eprintln!("clear {:?} right", pool[idx].parent);
                        }
                    }
                }
            } else {
                new_root = None;
            }
        }

        if is_empty_leaf {
            // update links
            let prev = pool[idx].prev;
            let next = pool[idx].next;
            MappedFile::link_prev_next_nodes(&mut pool, prev, next);
        }

        if !is_leaf {
            let l = MappedFile::get_best_child(to_delete, pool, pool[idx].left);
            let r = MappedFile::get_best_child(to_delete, pool, pool[idx].right);

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
                eprintln!("({}).l_size = {}", idx, l_size);
                eprintln!("({}).r_size = {}", idx, r_size);
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
    ) -> Option<NodeIndex> {
        let mut to_delete = vec![];

        let tmp_node = MappedFile::get_best_child(&mut to_delete, &mut pool, subroot);

        // clear
        if DEBUG {
            eprintln!("to delete {:?}", to_delete);
        }
        for n in to_delete {
            if pool[n].to_delete {
                assert!(pool[n].used);
                pool[n].clear();
                pool.release(n);
            }
        }

        tmp_node
    }

    fn print_all_used_nodes(file: &MappedFile, rsn: &str) {
        if DEBUG {
            eprintln!("*************  ALL USED NODES ({}) ***********", rsn);
            for i in 0..file.pool.slot.len() {
                let n = &file.pool.slot[i];
                if n.used {
                    eprintln!("[{}] : {:?}", i, file.pool.slot[i]);
                } else {
                    assert_eq!(n.parent, None);
                    assert_eq!(n.prev, None);
                    assert_eq!(n.next, None);
                }
            }
            eprintln!("***********************");
        }
    }

    fn check_tree(
        mut visited: &mut HashSet<NodeIndex>,
        idx: Option<NodeIndex>,
        pool: &FreeListAllocator<Node>,
    ) {
        if idx.is_none() {
            return;
        }
        let idx = idx.unwrap();

        assert!((idx as usize) < pool.slot.len());

        if DEBUG {
            eprintln!(" checking tree idx({})", idx);
        }

        assert_eq!(pool.slot[idx].used, true);

        // already visited ?
        assert_eq!(visited.contains(&idx), false);
        visited.insert(idx);

        // no children ? -> leaf
        let is_leaf = pool.slot[idx].left.is_none() && pool.slot[idx].right.is_none();

        // some children ? -> intermediate node
        let is_intermediate_node = pool.slot[idx].left.is_some() || pool.slot[idx].right.is_some();

        // an intermediate node cannot be a leaf
        assert!(is_leaf != is_intermediate_node);

        // check parent / children idx
        if let Some(l) = pool.slot[idx].left {
            if DEBUG {
                eprintln!(
                    "checking left's parent {:?} == {}.parent == {:?}",
                    Some(idx),
                    l,
                    pool.slot[l].parent
                );
            }
            assert_eq!(Some(idx), pool.slot[l].parent);
        }

        if let Some(r) = pool.slot[idx].right {
            if DEBUG {
                eprintln!(
                    "checking right's parent {:?} == {}.parent == {:?}",
                    Some(idx),
                    r,
                    pool.slot[r].parent
                );
            }

            assert_eq!(Some(idx), pool.slot[r].parent);
        }

        // recurse left / right
        MappedFile::check_tree(&mut visited, pool.slot[idx].left, &pool);
        MappedFile::check_tree(&mut visited, pool.slot[idx].right, &pool);
    }

    fn check_all_nodes(file: &MappedFile) {
        if DEBUG {
            eprintln!("check_all_nodes");
        }

        let mut visited = HashSet::new();

        MappedFile::check_tree(&mut visited, file.root_index, &file.pool);

        MappedFile::check_leaves(&file);

        // check all nodes in allocator
        // if used == false
        // assert prev/next/parent == None
        if DEBUG {
            eprintln!("file.size({})", file.size());
        }

        visited.clear();

        let (idx, _, _) = file.find_node_by_offset(0);
        if idx.is_none() {
            if DEBUG {
                eprintln!("no leaf found");
            }
            return;
        }
        let mut idx = idx.unwrap();

        if let Some(root_index) = file.root_index {
            if DEBUG {
                eprintln!(
                    "file root  idx({}) : {:?} ",
                    root_index, file.pool.slot[root_index]
                );
            }
        }

        if DEBUG {
            eprintln!("first leaf is {}", idx);
        }

        if file.pool.slot[idx].prev.is_some() {
            if DEBUG {
                eprintln!("prev node is set to {:?} ????", file.pool.slot[idx].prev);
                eprintln!("current leaf is idx({}) : {:?} ", idx, file.pool.slot[idx]);
            }
            panic!();
        };

        let file_size = file.size() as u64;
        let mut size_checked = 0;
        loop {
            let n = &file.pool.slot[idx];

            if DEBUG {
                eprintln!("current leaf is idx({}) : {:?} ", idx, n);
            }

            assert!(n.used);

            if visited.contains(&idx) {
                panic!();
            }

            size_checked += n.size;

            if n.next.is_none() {
                break;
            }

            if size_checked >= file_size {
                panic!();
            }

            visited.insert(idx);

            idx = n.next.unwrap();
        }

        if size_checked != file_size {
            if DEBUG {
                eprintln!(
                    "size_checked({}) != file.size({})",
                    size_checked,
                    file.size()
                );
            }
            panic!();
        }

        for i in 0..file.pool.slot.len() {
            let n = &file.pool.slot[i];
            if !n.used {
                assert_eq!(n.parent, None);
                assert_eq!(n.prev, None);
                assert_eq!(n.next, None);
            }
        }
    }

    fn check_leaves(file: &MappedFile) {
        let (idx, _, _) = file.find_node_by_offset(0);
        if idx.is_none() {
            return;
        }

        let mut idx = idx.unwrap() as usize;

        let mut off = 0;
        let file_size = file.size();
        loop {
            if DEBUG {
                eprintln!("{} / {}", off, file_size);
                eprintln!("file.pool[{}].size = {}", idx, file.pool[idx].size);
            }
            if DEBUG {
                eprintln!(
                    "off({}) + {} >= file.size({}) ?",
                    off, file.pool[idx].size, file_size
                );
            }
            off += file.pool[idx].size;
            if off > file_size {
                panic!("invalid tree, broken node size: off > file_size");
            }
            if off == file_size {
                break;
            }

            if let Some(next) = file.pool[idx].next {
                idx = next;
            } else {
                panic!("invalid tree, broken next link");
            }
        }
    }

    pub fn sync_to_disk(
        file: &mut MappedFile,
        tmp_file_name: &str,
        rename_file_name: &str,
    ) -> ::std::io::Result<()> {
        use std::fs;

        let path = CString::new(tmp_file_name).unwrap();
        unsafe { unlink(path.as_ptr()) };
        let fd = unsafe { open(path.as_ptr(), O_CREAT | O_RDWR | O_TRUNC, S_IRUSR | S_IWUSR) };
        if fd < 0 {
            return Ok(());
        }

        let mut offset = 0;
        let (mut n, _, _) = MappedFile::find_node_by_offset(&file, offset);
        while n.is_some() {
            let idx = n.unwrap();

            let node_size = file.pool[idx].size;
            // map
            let page = file.pool[idx].map().unwrap();
            let slice = page.as_ref().borrow().as_slice().unwrap();

            // copy
            let nw = unsafe { write(fd, slice.as_ptr() as *mut c_void, slice.len()) };
            if nw != slice.len() as isize {
                panic!("write error");
            }

            offset += node_size;
            n = file.pool[idx].next;
        }

        // patch on_disk_offset, and file descriptor
        let mut count: u64 = 0;
        let mut offset = 0;
        let (mut n, _, _) = MappedFile::find_node_by_offset(&file, offset);
        while n.is_some() {
            let idx = n.unwrap();
            let node_size = file.pool[idx].size;

            // node on disk ?
            if file.pool[idx].cow.is_none() {
                let align_offset = 4096 * (offset / 4096);
                let skip = offset % 4096;

                file.pool[idx].on_disk_offset = align_offset;
                file.pool[idx].skip = skip;
            } else {
                file.pool[idx].on_disk_offset = 0xffff_ffff_ffff_ffff;
                file.pool[idx].skip = 0;
            }

            if false {
                eprintln!(
                    "offset {}, page {}, size {} disk_offset {}, skip {}",
                    offset,
                    count,
                    file.pool[idx].size,
                    file.pool[idx].on_disk_offset,
                    file.pool[idx].skip,
                );
            }

            offset += node_size;
            n = file.pool[idx].next;
            file.pool[idx].fd = fd;
            count += 1;
        }

        fs::rename(&tmp_file_name, &rename_file_name)?;

        let old_fd = file.fd;
        unsafe { close(old_fd) };
        file.fd = fd;

        Ok(())
    }
}

///////////////////////////////////////////////////////////////////////////////////////////////////
#[derive(Debug)]
pub enum MappedFileIterator<'a> {
    End(FileHandle<'a>),
    Real(IteratorInstance<'a>),
}

impl<'a> MappedFileIterator<'a> {
    fn get_mut_ref(&mut self) -> Option<&mut IteratorInstance<'a>> {
        match *self {
            MappedFileIterator::End(..) => None, // TODO: return sentinel ?
            MappedFileIterator::Real(ref mut it) => Some(it),
        }
    }

    fn get_file(&mut self) -> FileHandle<'a> {
        match *self {
            MappedFileIterator::End(ref file) => Rc::clone(file),
            MappedFileIterator::Real(ref it) => Rc::clone(&it.file),
        }
    }
}

#[derive(Debug)]
pub struct IteratorInstance<'a> {
    file: FileHandle<'a>,
    file_size: u64,
    local_offset: u64,
    page_size: u64,
    node_idx: NodeIndex,
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
                    let mut file = it.file.borrow_mut();

                    let next_node_idx = {
                        let node = &mut file.pool[it.node_idx as usize];

                        // end-of-file ?
                        if node.next == None {
                            return None;
                        }

                        node.next.unwrap()
                    };

                    let next_node = &mut file.pool[next_node_idx as usize];

                    let page = next_node.map().unwrap();
                    let slice = page.as_ref().borrow_mut().as_slice().unwrap();

                    it.node_idx = next_node_idx;
                    it.page_size = next_node.size;
                    it.page = next_node.page.upgrade().unwrap();
                    it.local_offset = 0;
                    it.base = slice;
                }

                it.local_offset += 1;

                Some(MappedFileIterator::Real(IteratorInstance {
                    file: Rc::clone(&it.file),
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

        let fd = -1;
        let mut pool = FreeListAllocator::new();
        let file_size = 1024 * 1024 * 1024 * 1024 * 8; // x Tib
        let page_size = 4096 * 256 * 4; // 4 Mib

        let root_node = Node {
            used: true,
            to_delete: false,
            fd,
            size: file_size,
            parent: None,
            left: None,
            right: None,
            prev: None,
            next: None,
            page: Weak::new(),
            cow: None,
            on_disk_offset: 0,
            skip: 0,
        };

        let (id, _) = pool.allocate(root_node, &MappedFile::assert_node_is_unused);

        let mut leaves = Vec::new();
        MappedFile::build_tree(
            &mut pool,
            fd,
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

        eprintln!("file_size : {}", file_size);
        eprintln!("page_size : {}", page_size);
        eprintln!("number of leaves : {}", leaves.len());
        eprintln!("number of nodes : {}", pool.slot.len());

        let node_ram_size = ::std::mem::size_of::<Node>();
        eprintln!("node_ram_size : bytes {}", node_ram_size);

        let ram = pool.slot.len() * node_ram_size;
        eprintln!("ram : bytes {}", ram);
        eprintln!("ram : Kib {}", ram >> 10);
        eprintln!("ram : Mib {}", ram >> 20);
        eprintln!("ram : Gib {}", ram >> 30);

        use std::io;

        if false {
            eprintln!("Hit [Enter] to stop");
            let mut stop = String::new();
            io::stdin().read_line(&mut stop).expect("something");
        }
    }

    #[test]
    fn test_remove() {
        use super::*;

        // TODO: loop over nb_page [2->256]
        let nb_page = 2;
        let page_size = 4096;
        let file_size = page_size * nb_page;
        let nr_remove = page_size * (nb_page - 1);
        let offset = page_size as u64;

        use std::fs;
        use std::fs::File;
        use std::io::prelude::*;

        let filename = "/tmp/playground_remove_test".to_owned();
        let mut file = File::create(&filename).unwrap();

        // prepare file content
        eprintln!("-- generating test file size({})", file_size);
        let mut slc = Vec::with_capacity(file_size);
        for i in 0..file_size {
            if (i % (1024 * 1024 * 256)) == 0 {
                eprintln!("-- @ bytes {}", i);
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

        eprintln!("-- mapping the test file");
        let file = match MappedFile::new(filename, page_size) {
            Some(file) => file,
            None => panic!("cannot map file"),
        };

        eprintln!(
            "-- testing remove {} @ {} from {}",
            nr_remove, offset, file_size
        );
        let mut it = MappedFile::iter_from(&file, offset);
        MappedFile::remove(&mut it, nr_remove);

        eprintln!("-- file.size() {}", file.as_ref().borrow().size());
        let _ = fs::remove_file("/tmp/playground_remove_test");
    }

    #[test]
    fn test_1m_insert() {
        use super::*;
        use std::fs;
        use std::fs::File;

        let page_size = 4096 * 256;

        let filename = "/tmp/playground_insert_test".to_owned();
        File::create(&filename).unwrap();

        eprintln!("-- mapping the test file");
        let file = match MappedFile::new(filename, page_size) {
            Some(file) => file,
            None => panic!("cannot map file"),
        };

        file.as_ref().borrow_mut().sub_page_size = 4096 * 4;
        file.as_ref().borrow_mut().sub_page_reserve = 1024;

        for _ in 0..1_000_000 {
            let mut it = MappedFile::iter_from(&file, 0);
            MappedFile::insert(&mut it, &['A' as u8]);
        }

        eprintln!("-- file.size() {}", file.as_ref().borrow().size());
        let _ = fs::remove_file("/tmp/playground_insert_test");
    }

    #[test]
    fn test_1b_insert() {
        use super::*;

        let page_size = 4096;

        use std::fs;
        use std::fs::File;
        use std::io::prelude::*;

        let filename = "/tmp/playground_insert_test".to_owned();
        {
            let mut file = File::create(&filename).unwrap();

            // prepare file content
            eprintln!("-- generating test file");
            let file_size = 4096 * 10;
            let mut slc = Vec::with_capacity(file_size);
            for i in 0..file_size {
                if (i % (1024 * 1024 * 256)) == 0 {
                    eprintln!("-- @ bytes {}", i);
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
        }

        eprintln!("-- mapping the test file");
        let file = match MappedFile::new(filename, page_size) {
            Some(file) => file,
            None => panic!("cannot map file"),
        };

        file.as_ref().borrow_mut().sub_page_size = 4096;
        file.as_ref().borrow_mut().sub_page_reserve = 10;

        for i in 0..5 {
            let i = i * 2;
            let mut it = MappedFile::iter_from(&file, i + i * 4096);
            MappedFile::insert(&mut it, &['A' as u8]);
        }

        MappedFile::sync_to_disk(
            &mut file.as_ref().borrow_mut(),
            &"/tmp/mapped_file.sync_test",
            &"/tmp/mapped_file.sync_test.result",
        )
        .unwrap();

        eprintln!("-- file.size() {}", file.as_ref().borrow().size());

        let _ = fs::remove_file("/tmp/mapped_file.sync_test.result");
        let _ = fs::remove_file("/tmp/playground_insert_test");
    }
}
