//
// MappedFile is binary tree that provides on-demand data mapping, and keeps only the modified areas in memory.
// the leaves are linked to allow fast sequential traversal.
//

extern crate libc;

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

use self::libc::{c_int,
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
                 S_IWUSR};

//////////////////////////////////////////////////////////////////////////////////////////

#[derive(Debug, Clone)]
enum Page {
    OnDisk(*const u8, size_t, size_t, c_int), // base, len, skip, fd
    InRam(*const u8, usize, usize),           // base, len, capacity
}

impl Page {
    fn as_slice<'a>(&self) -> Option<&'a [u8]> {
        Some(match &*self {
            &Page::OnDisk(base, len, ..) => unsafe { slice::from_raw_parts(base, len) },

            &Page::InRam(base, len, ..) => unsafe { slice::from_raw_parts(base, len) },
        })
    }
}

impl Drop for Page {
    fn drop(&mut self) {
        match *self {
            Page::OnDisk(base, len, skip, ..) => {
                // println!("munmap {:?}", base);
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

#[derive(Debug, Clone)]
struct Node {
    used: bool,
    fd: c_int,

    // idx: NodeIndex, // for debug
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
        self.fd = -1;
        // self.idx = 0xffffffffffffffff as NodeIndex;
        self.parent = None;
        self.left = None;
        self.right = None;
        self.prev = None;
        self.next = None;
        self.size = 0;
        self.on_disk_offset = 0xffffffffffffffff as u64;
        self.skip = 0;
        self.page = Weak::new();
        self.cow = None;
    }

    fn map<'a>(&mut self) -> Option<Rc<RefCell<Page>>> {
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
            base,
            len,
            capacity,
        )

        // 6 - restore page iterators base pointer
    }

    // will clear p
    fn _page_to_vec(p: &mut Page) -> Vec<u8> {
        match p {
            &mut Page::OnDisk(..) => {
                panic!("cannot be used on Ondisk page");
            }

            &mut Page::InRam(ref mut base, ref mut len, ref mut capacity) => {
                let v = unsafe { Vec::from_raw_parts(*base as *mut u8, *len, *capacity) };

                *base = 0 as *mut u8;
                *len = 0;
                *capacity = 0;

                v
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
    fn new() -> FreeListAllocator<T> {
        FreeListAllocator {
            slot: vec![],
            free_indexes: vec![],
        }
    }

    fn allocate(&mut self, n: T) -> NodeIndex {
        let i = if !self.free_indexes.is_empty() {
            let i = self.free_indexes.pop().unwrap();
            self.slot[i] = n;
            i
        } else {
            let i = self.slot.len();
            self.slot.push(n);
            i
        };

        i as NodeIndex
    }

    fn release(&mut self, idx: NodeIndex) {
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
    fd: c_int,
    nodepool: FreeListAllocator<Node>,
    root_index: Option<NodeIndex>,
    phantom: PhantomData<&'a u8>,
    page_size: usize,
    pub cow_subpage_size: usize, // when writting to a given node, split using cow_subpage_size
    pub cow_subpage_reserve: usize, //  when spltting a given node, reserve cow_subpage_reserve
}

impl<'a> Drop for MappedFile<'a> {
    fn drop(&mut self) {
        unsafe { close(self.fd) };
    }
}

impl<'a> MappedFile<'a> {
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
            fd,
            nodepool: FreeListAllocator::new(),
            root_index: None,
            phantom: PhantomData,
            page_size,
            cow_subpage_size: 4096 * 256 * 2, // 2 mib
            cow_subpage_reserve: 64,          //
        };

        if file_size == 0 {
            return Some(Rc::new(RefCell::new(file)));
        }

        let root_node = Node {
            used: true,
            fd: fd,
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

        let id = file.nodepool.allocate(root_node);
        file.root_index = Some(id);

        let mut leaves = Vec::new();
        MappedFile::build_tree(
            &mut file.nodepool,
            fd,
            &mut leaves,
            Some(id),
            page_size as u64,
            file_size as u64,
            0,
        );

        let mut prev_idx = None;
        for idx in leaves {
            MappedFile::link_prev_next_nodes(&mut file.nodepool, prev_idx, Some(idx));
            prev_idx = Some(idx);

            // TODO: add hints to map all nodes
            if false && file_size <= page_size as u64 {
                let p = file.nodepool[idx as usize].move_to_ram();
                let rc = Rc::new(RefCell::new(p));
                file.nodepool[idx as usize].page = Rc::downgrade(&rc);
                file.nodepool[idx as usize].cow = Some(rc);
            }
        }

        Some(Rc::new(RefCell::new(file)))
    }

    pub fn size(&self) -> u64 {
        if let Some(idx) = self.root_index {
            self.nodepool[idx].size as u64
        } else {
            0
        }
    }

    fn link_prev_next_nodes(
        nodepool: &mut FreeListAllocator<Node>,
        prev_idx: Option<NodeIndex>,
        next_idx: Option<NodeIndex>,
    ) {
        if let Some(p_idx) = prev_idx {
            nodepool[p_idx as usize].next = next_idx;
            // println!("link_next : prev({:?})  -> next({:?})", prev_idx, next_idx);
        }

        if let Some(n_idx) = next_idx {
            nodepool[n_idx as usize].prev = prev_idx;
            // println!("link_prev : prev({:?})  <- next({:?})", prev_idx, next_idx);
        }
    }

    fn link_parent_child(
        nodepool: &mut FreeListAllocator<Node>,
        parent_idx: Option<NodeIndex>,
        child_idx: Option<NodeIndex>,
        relation: NodeRelation,
    ) {
        let debug = false;

        if let Some(child_idx) = child_idx {
            nodepool[child_idx].parent = parent_idx;
            if debug {
                println!(
                    "link_parent : child({:?})  -> parent({:?})",
                    child_idx, parent_idx
                );
            }
        }

        if let Some(parent_idx) = parent_idx {
            if relation == NodeRelation::Left {
                nodepool[parent_idx].left = child_idx;
                if debug {
                    println!(
                        "link_prev : parent({:?}).left -> child({:?})",
                        parent_idx, child_idx
                    );
                }
            }

            if relation == NodeRelation::Right {
                nodepool[parent_idx].right = child_idx;
                if debug {
                    println!(
                        "link_prev : parent({:?}).right -> child({:?})",
                        parent_idx, child_idx
                    );
                }
            }
        }
    }

    pub fn print_nodes(file: &MappedFile) {
        for (idx, n) in file.nodepool.slot.iter().enumerate() {
            if n.used {
                println!(
                    "idx({:?}), parent({:?}) left({:?}) right({:?}) prev({:?}) \
                     next({:?}) size({}) on_disk_off({})",
                    idx, n.parent, n.left, n.right, n.prev, n.next, n.size, n.on_disk_offset
                )
            }
        }
    }

    fn build_tree(
        nodepool: &mut FreeListAllocator<Node>,
        fd: i32,
        leaves: &mut Vec<NodeIndex>,
        parent: Option<NodeIndex>,
        pg_size: u64,
        node_size: u64,
        base_offset: u64,
    ) -> () {
        // is leaf ?
        if node_size <= pg_size {
            if !true {
                println!(
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
                    nodepool[idx].on_disk_offset = base_offset;
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
        // TODO: None::new(fd, parent, size, on_diskoffset)
        let left_node = Node {
            used: true,
            fd: fd,
            //            idx: 0,
            size: l_sz,
            parent: parent,
            left: None,
            right: None,
            prev: None,
            next: None,
            page: Weak::new(),
            cow: None,
            on_disk_offset: base_offset,
            skip: 0,
        };
        let l = nodepool.allocate(left_node);

        // TODO: None::new(fd, parent, size, on_diskoffset)
        let right_node = Node {
            used: true,
            fd: fd,
            //          idx: 0,
            size: r_sz,
            parent: parent,
            left: None,
            right: None,
            prev: None,
            next: None,
            page: Weak::new(),
            cow: None,
            on_disk_offset: base_offset + l_sz,
            skip: 0,
        };
        let r = nodepool.allocate(right_node);

        // build children
        MappedFile::build_tree(nodepool, fd, leaves, Some(l), pg_size, l_sz, base_offset);
        MappedFile::build_tree(
            nodepool,
            fd,
            leaves,
            Some(r),
            pg_size,
            r_sz,
            base_offset + l_sz,
        );

        // link to parent
        if let Some(idx) = parent {
            let idx = idx as usize;
            nodepool[idx].left = Some(l);
            nodepool[idx].right = Some(r);

            //            nodepool[l as usize].idx = l;
            //            nodepool[r as usize].idx = r;

            //      println!("parent = {}, l = {}, r = {}", idx, l, r);
        }
    }

    fn find_subnode_by_offset(
        &self,
        n: NodeIndex,
        offset: u64,
    ) -> (Option<NodeIndex>, NodeSize, NodeLocalOffset) {
        let debug = !true;

        if debug {
            println!("find_subnode_by_offset Ndi({}) off({})", n, offset);
        }
        let node = &self.nodepool[n as usize];

        let is_leaf = node.left.is_none() && node.right.is_none();

        if offset < node.size && is_leaf {
            (Some(n), node.size, offset)
        } else {
            let left_size = if let Some(left) = node.left {
                self.nodepool[left as usize].size
            } else {
                0
            };

            if debug {
                println!("   off({})  left_size({})", offset, left_size);
            }
            if offset < left_size {
                if debug {
                    println!("go   <----");
                }
                self.find_subnode_by_offset(node.left.unwrap(), offset)
            } else {
                if debug {
                    println!("go   ---->");
                }
                self.find_subnode_by_offset(node.right.unwrap(), offset - left_size)
            }
        }
    }

    fn find_node_by_offset(&self, offset: u64) -> (Option<NodeIndex>, NodeSize, NodeLocalOffset) {
        if let Some(idx) = self.root_index {
            if offset >= self.nodepool[idx as usize].size {
                // offset is to big
                return (None, 0, 0);
            }
            self.find_subnode_by_offset(idx, offset)
        } else {
            (None, 0, 0)
        }
    }

    pub fn iter(file: &FileHandle<'a>) -> FileIterator<'a> {
        MappedFile::iter_from(file, 0)
    }

    // creates an iterator over an abitray node index
    // always start @ local_offset 0
    pub fn iter_from_node_index(file_: &FileHandle<'a>, node_idx: NodeIndex) -> FileIterator<'a> {
        let file = file_.borrow_mut();
        match node_idx {
            _ => {
                let page = file.nodepool[node_idx as usize].page.upgrade().unwrap();
                let slice = page.as_ref().borrow_mut().as_slice().unwrap();

                MappedFileIterator::Real(IteratorInstance {
                    file: Rc::clone(file_),
                    file_size: file.size(),
                    local_offset: 0,
                    page_size: file.nodepool[node_idx as usize].size,
                    node_idx,
                    page,
                    base: slice,
                })
            }
        }
    }

    pub fn iter_from(file_: &FileHandle<'a>, offset: u64) -> FileIterator<'a> {
        let mut file = file_.borrow_mut();
        let pair = file.find_node_by_offset(offset);
        match pair {
            (Some(node_idx), node_size, local_offset) => {
                let page = file.nodepool[node_idx as usize].map().unwrap();
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
                //                println!("ITER FROM END !!!!!");
                MappedFileIterator::End(Rc::clone(file_))
            }
        }
    }

    pub fn copy_to_slice(it_: &mut FileIterator<'a>, nr_to_read: usize, vec: &mut [u8]) -> usize {
        match &*it_ {
            &MappedFileIterator::End(..) => return 0,
            _ => {}
        };

        let mut nr_read: usize = 0;
        let mut nr_to_read = nr_to_read;

        while nr_to_read > 0 {
            if let Some(ref mut it) = it_.get_mut_repr() {
                let off = it.local_offset as usize;
                let max_read = ::std::cmp::min(it.page_size as usize - off, nr_to_read);
                if max_read == 0 {
                    break;
                }

                unsafe {
                    ptr::copy(
                        &it.base[off],
                        vec.as_mut_ptr().offset(nr_read as isize),
                        max_read,
                    );
                }

                nr_to_read -= max_read;
                nr_read += max_read;

                it.local_offset += max_read as u64;

                if it.local_offset != it.page_size {
                    continue;
                }
            }

            let next_it = it_.next();
            if next_it.is_none() {
                break;
            }

            *it_ = next_it.unwrap();
        }

        nr_read
    }

    pub fn read(it_: &mut FileIterator<'a>, nr_to_read: usize, vec: &mut Vec<u8>) -> usize {
        match &*it_ {
            &MappedFileIterator::End(..) => return 0,
            _ => {}
        };

        let mut nr_read = 0;
        let mut nr_to_read = nr_to_read;

        while nr_to_read > 0 {
            if let Some(ref mut it) = it_.get_mut_repr() {
                let off = it.local_offset as usize;

                let max_read = ::std::cmp::min(it.page_size as usize - off, nr_to_read);
                vec.extend_from_slice(&it.base[off..off + max_read]);

                nr_to_read -= max_read;
                nr_read += max_read;
                it.local_offset += max_read as u64;

                if it.local_offset != it.page_size {
                    continue;
                }
            }

            let next_it = it_.next();
            if next_it.is_none() {
                break;
            }

            *it_ = next_it.unwrap();
        }

        nr_read
    }

    fn update_hierarchy(
        nodepool: &mut FreeListAllocator<Node>,
        parent_idx: Option<NodeIndex>,
        op: UpdateHierarchyOp,
        value: u64,
    ) {
        let debug = !true;

        let mut p_idx = parent_idx;
        while p_idx != None {
            let idx = p_idx.unwrap();
            if debug {
                print!(
                    "node({}).size {} op({:?}) {} ---> ",
                    idx, nodepool[idx as usize].size, op, value
                );
            }

            match op {
                UpdateHierarchyOp::Add => nodepool[idx as usize].size += value,
                UpdateHierarchyOp::Sub => nodepool[idx as usize].size -= value,
            }

            if debug {
                println!("{}", nodepool[idx as usize].size);
            }

            p_idx = nodepool[idx as usize].parent;
        }
    }

    fn check_free_space(it_: &mut MappedFileIterator) -> u64 {
        match &*it_ {
            &MappedFileIterator::End(..) => return 0,
            &MappedFileIterator::Real(ref it) => {
                match &it.page {
                    ref rc => match *rc.as_ref().borrow_mut() {
                        Page::OnDisk { .. } => {
                            return 0;
                        }

                        Page::InRam(_, ref mut len, capacity) => {
                            return (capacity - *len) as u64;
                        }
                    },
                };
            }
        }
    }

    fn insert_in_place(it_: &mut FileIterator<'a>, data: &[u8]) {
        match &*it_ {
            &MappedFileIterator::End(..) => panic!("trying to write on end iterator"),
            &MappedFileIterator::Real(ref it) => match &it.page {
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
        let debug = !true;

        let data_len = data.len() as u64;
        if data_len == 0 {
            return 0;
        }

        let (node_to_split, node_size, local_offset, it_page) = match &*it_ {
            &MappedFileIterator::End(ref rcfile) => {
                let mut file = rcfile.as_ref().borrow_mut();
                let file_size = file.size();
                if file_size > 0 {
                    let (idx, node_size, _) = file.find_node_by_offset(file_size - 1);
                    let page = file.nodepool[idx.unwrap()].map();
                    (idx, node_size, node_size, page)
                } else {
                    (None, 0, 0, None)
                }
            }

            &MappedFileIterator::Real(ref it) => (
                Some(it.node_idx),
                it.page_size,
                it.local_offset,
                Some(Rc::clone(&it.page)),
            ),
        };

        if debug {
            println!("node_to_split {:?} / size ({})", node_to_split, node_size);
        }

        let available = MappedFile::check_free_space(it_);
        if debug {
            println!("available space = {}", available);
        }

        /////// in place insert ?

        if available >= data_len {
            // insert in current node
            MappedFile::insert_in_place(it_, data);

            // update parents
            let rcfile = it_.get_file();
            let mut file = rcfile.as_ref().borrow_mut();
            MappedFile::update_hierarchy(
                &mut file.nodepool,
                node_to_split,
                UpdateHierarchyOp::Add,
                data_len,
            );
            MappedFile::check_leaves(&file);
            return data_len as usize;
        }

        ////////////////////////////////////////////////
        // new subtree

        let rcfile = it_.get_file();
        let mut file = rcfile.as_ref().borrow_mut();

        let base_offset = match node_to_split {
            Some(idx) => file.nodepool[idx as usize].on_disk_offset,
            None => 0,
        };

        let (prev_idx, next_idx, gparent_idx) = if let Some(idx) = node_to_split {
            (
                file.nodepool[idx].prev,
                file.nodepool[idx].next,
                file.nodepool[idx].parent,
            )
        } else {
            (None, None, None)
        };

        let fd = file.fd;
        let room = file.cow_subpage_reserve;
        let cow_subpage_size = file.cow_subpage_size;

        let new_size: usize = (node_size as usize) + data.len();

        // TODO: provide user apis to tweak allocations
        let sub_page_min_size = cow_subpage_size as usize;
        let new_page_size = ::std::cmp::min(new_size / sub_page_min_size, sub_page_min_size);
        let new_page_size = ::std::cmp::max(new_page_size, sub_page_min_size);

        if debug {
            println!("new_size {}", new_size);
            println!("new_page_size {}", new_page_size);
        }

        let subroot_node = Node {
            used: true,
            fd: fd,
            //            idx: 0,
            size: new_size as u64,
            parent: gparent_idx,
            left: None,
            right: None,
            prev: None,
            next: None,
            page: Weak::new(),
            cow: None,
            on_disk_offset: base_offset,
            skip: 0,
        };

        let subroot_idx = file.nodepool.allocate(subroot_node);
        //        file.nodepool[subroot_idx as usize].idx = subroot_idx;

        if debug {
            println!(
                "create new tree with room for {} bytes \
                 inserts subroot_index({}), base_offset({})",
                new_size, subroot_idx, base_offset
            );
        }

        let mut leaves = Vec::new();
        MappedFile::build_tree(
            &mut file.nodepool,
            fd,
            &mut leaves,
            Some(subroot_idx),
            new_page_size as u64,
            new_size as u64,
            base_offset,
        );

        if debug {
            println!("number of leaves = {}", leaves.len());
            println!("node_size = {}", node_size);
            println!("local_offset = {}", local_offset);
        }

        // use a flat map for data copying
        let mut input_slc = Vec::new();

        // before it
        if let &Some(ref page) = &it_page {
            if local_offset > 0 {
                let slc = page.as_ref().borrow().as_slice().unwrap();
                input_slc.push(&slc[0..local_offset as usize]);
            }
        }

        // at it
        input_slc.push(data);

        // after it
        if let &Some(ref page) = &it_page {
            if node_size > 0 {
                let slc = page.as_ref().borrow().as_slice().unwrap();
                input_slc.push(&slc[local_offset as usize..node_size as usize]);
            }
        }

        // build flatmap iterator
        let mut input_data_iter = input_slc.iter().flat_map(|&x| x.iter()).into_iter();

        // copy
        let mut prev_idx = prev_idx;
        let mut remain = new_size;
        for idx in &leaves {
            // alloc+fill node
            {
                let mut n = &mut file.nodepool[*idx];
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
            MappedFile::link_prev_next_nodes(&mut file.nodepool, prev_idx, Some(*idx));
            prev_idx = Some(*idx);
        }
        // link last leaf
        MappedFile::link_prev_next_nodes(&mut file.nodepool, prev_idx, next_idx);

        assert_eq!(remain, 0);

        // TODO: check reparenting
        // swap subroot_idx and node_idx
        if let Some(node_to_split) = node_to_split {
            // MappedFile::exchage_nodes(gparent, node_to_split);
            if let Some(gparent_idx) = gparent_idx {
                // update grand parent left or right // delete
                let gparent_left = file.nodepool[gparent_idx].left;
                let gparent_right = file.nodepool[gparent_idx].right;

                if let Some(gp_left) = gparent_left {
                    if gp_left == node_to_split {
                        //                        println!("update grand parent left");
                        file.nodepool[gparent_idx].left = Some(subroot_idx);
                    }
                }

                if let Some(gp_right) = gparent_right {
                    if gp_right == node_to_split {
                        //                        println!("update grand parent right");
                        file.nodepool[gparent_idx].right = Some(subroot_idx);
                    }
                }

                //                println!("update subroot parent");
                file.nodepool[subroot_idx].parent = Some(gparent_idx);
            }

            // clear+delete old node
            file.nodepool[node_to_split].clear();
            file.nodepool.release(node_to_split);
        }

        // check root
        if let Some(root_idx) = file.root_index {
            if let Some(node_to_split) = node_to_split {
                if root_idx == node_to_split {
                    file.root_index = Some(subroot_idx);
                    //                    println!("new file.root_index {:?}", file.root_index);
                }
            }
        } else {
            file.root_index = Some(subroot_idx);
            //            println!("new file.root_index {:?}", file.root_index);
        }

        // update parent nodes size
        let p_idx = file.nodepool[subroot_idx as usize].parent;
        MappedFile::update_hierarchy(
            &mut file.nodepool,
            p_idx,
            UpdateHierarchyOp::Add,
            data.len() as u64,
        );

        // TODO:
        // refresh iterator or next will crash
        //        it.file_size += size as u64;
        //        it_offset = base + it.local_offset;

        // refresh iterator or next will crash
        //      *it_ = {
        //          let it = it_.get_mut_repr().unwrap();
        //          MappedFile::iter_from(&it.file, it_offset)
        //      };

        MappedFile::check_leaves(&file);

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

        let debug = !true;

        let mut remain = nr;
        let mut nr_removed = 0;

        let (mut file, start_idx, mut local_offset) = match &mut *it_ {
            &mut MappedFileIterator::End(..) => return 0,

            &mut MappedFileIterator::Real(ref it) => {
                (it.file.as_ref().borrow_mut(), it.node_idx, it.local_offset)
            }
        };

        if debug {
            println!("--- tree before rebalance root_idx = {:?}", file.root_index);
            MappedFile::print_nodes(&file);
        }

        let mut idx = start_idx as usize;
        while remain > 0 {
            if file.nodepool[idx].cow.is_none() {
                let page = file.nodepool[idx].move_to_ram();
                let rc = Rc::new(RefCell::new(page));
                file.nodepool[idx].page = Rc::downgrade(&rc);
                file.nodepool[idx].cow = Some(rc);
            }

            let node_subsize = (file.nodepool[idx].size - local_offset) as usize;
            let to_rm = ::std::cmp::min(remain, node_subsize);

            assert!(to_rm <= node_subsize);

            if debug {
                println!("node_idx {}", idx);
                println!("node_size {}", file.nodepool[idx].size);
                println!("node_subsize {}", node_subsize);
                println!("to_rm {}", to_rm);
                println!("local_offset {}", local_offset);
            }

            match &mut *file.nodepool[idx]
                .cow
                .as_ref()
                .unwrap()
                .as_ref()
                .borrow_mut()
            {
                &mut Page::OnDisk { .. } => {
                    panic!("trying to write on read only memory");
                }

                &mut Page::InRam(base, ref mut len, capacity) => {
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
                &mut file.nodepool,
                Some(idx),
                UpdateHierarchyOp::Sub,
                to_rm as u64,
            );

            if file.nodepool[idx].next.is_none() {
                break;
            }
            idx = file.nodepool[idx].next.unwrap();
        }

        if true {
            if debug {
                println!("-----------------------------------------");
            }
            let mut tmp_node = file.root_index;
            tmp_node = MappedFile::rebalance_subtree(&mut file.nodepool, tmp_node);
            file.root_index = tmp_node;

            if debug {
                println!(
                    "--- tree after rebalance new_root_idx = {:?}",
                    file.root_index
                );
                MappedFile::print_nodes(&file);
            }
        }

        MappedFile::check_leaves(&file);

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

        return NodeRelation::NoRelation;
    }

    fn mark_node_to_release(
        pool: &mut FreeListAllocator<Node>,
        to_delete: &mut Vec<NodeIndex>,
        node: Option<NodeIndex>,
    ) {
        if let Some(idx) = node {
            assert_eq!(pool[idx].used, true);
            pool[idx].used = false;
            to_delete.push(idx);
        }
    }

    fn release_subtree(
        mut pool: &mut FreeListAllocator<Node>,
        mut to_delete: &mut Vec<NodeIndex>,
        subroot: Option<NodeIndex>,
    ) {
        if let Some(idx) = subroot {
            assert_eq!(pool[idx].used, true);

            let left = pool[idx].left;
            let right = pool[idx].right;
            MappedFile::release_subtree(&mut pool, &mut to_delete, left);
            MappedFile::release_subtree(&mut pool, &mut to_delete, right);
            MappedFile::mark_node_to_release(&mut pool, &mut to_delete, Some(idx));
        }
    }

    // rebalance
    fn get_best_child(
        mut leaves: &mut Vec<NodeIndex>,
        mut to_delete: &mut Vec<NodeIndex>,
        mut pool: &mut FreeListAllocator<Node>,
        node_idx: Option<NodeIndex>,
    ) -> Option<NodeIndex> {
        if node_idx.is_none() {
            return None;
        }
        let idx = node_idx.unwrap();

        let debug = false;
        if debug {
            println!(
                "get_best_child for node{:?} left({:?}), right({:?})",
                idx, pool[idx].left, pool[idx].right
            );
        }

        let node_size = { pool[idx].size };
        if node_size == 0 {
            MappedFile::mark_node_to_release(&mut pool, &mut to_delete, Some(idx));
        }

        let is_leaf = pool[idx].left.is_none() && pool[idx].right.is_none();
        if is_leaf {
            if node_size > 0 {
                leaves.push(idx);
            } else {
                // MappedFile::mark_node_to_release(&mut pool, &mut to_delete, Some(idx));
            }
            return Some(idx); // no change
        }

        let left_size = {
            if let Some(l) = pool[idx].left {
                pool[l].size
            } else {
                0
            }
        };

        let right_size = {
            if let Some(r) = pool[idx].right {
                pool[r].size
            } else {
                0
            }
        };

        if debug {
            println!(
                "left({:?}).size {} | right({:?}).size {}",
                pool[idx].left, left_size, pool[idx].right, right_size
            );
        }

        let mut candidate = None;

        if left_size == 0 && right_size != 0 {
            let r_idx = pool[idx].right;
            let best = MappedFile::get_best_child(&mut leaves, &mut to_delete, &mut pool, r_idx);
            if let Some(del) = pool[idx].left {
                MappedFile::release_subtree(pool, &mut to_delete, Some(del));
            }
            candidate = best;
        }

        if left_size != 0 && right_size == 0 {
            let l_idx = pool[idx].left;
            let best = MappedFile::get_best_child(&mut leaves, &mut to_delete, &mut pool, l_idx);
            if let Some(del) = pool[idx].right {
                MappedFile::release_subtree(pool, &mut to_delete, Some(del));
            }

            candidate = best;
        }

        if candidate == None {
            let left = pool[idx].left;
            let best_left =
                MappedFile::get_best_child(&mut leaves, &mut to_delete, &mut pool, left);

            let right = pool[idx].right;
            let best_right =
                MappedFile::get_best_child(&mut leaves, &mut to_delete, &mut pool, right);

            candidate = if best_left.is_none() {
                best_right
            } else if best_right.is_none() {
                best_left
            } else {
                Some(idx)
            };
        }

        if debug {
            println!("best replacement for {} is {:?}", idx, candidate);
        }

        if let Some(best_idx) = candidate {
            if best_idx == idx {
                return candidate;
            }
        }

        if let Some(parent) = pool[idx].parent {
            MappedFile::mark_node_to_release(&mut pool, &mut to_delete, Some(idx));

            let relation = MappedFile::get_parent_relation(&pool, parent, idx);
            match relation {
                NodeRelation::Left => {
                    to_delete.push(pool[parent].left.unwrap());
                    MappedFile::link_parent_child(&mut pool, Some(parent), candidate, relation);
                }
                NodeRelation::Right => {
                    to_delete.push(pool[parent].right.unwrap());
                    MappedFile::link_parent_child(&mut pool, Some(parent), candidate, relation);
                }
                _ => {}
            }
        } else {
            // the tree's root will be set to candidate
        }

        candidate
    }

    fn rebalance_subtree(
        mut nodepool: &mut FreeListAllocator<Node>,
        subroot: Option<NodeIndex>,
    ) -> Option<NodeIndex> {
        let mut leaves = vec![];
        let mut to_delete = vec![];

        let tmp_node =
            MappedFile::get_best_child(&mut leaves, &mut to_delete, &mut nodepool, subroot);

        // clear
        // println!("to delete {:?}", to_delete);
        for n in to_delete {
            nodepool[n].used = false;
            nodepool[n].clear();
            nodepool.release(n);
        }

        if leaves.len() > 0 {
            // println!("leaves: {:?}", leaves);
            let mut prev_idx = nodepool[leaves[0]].prev;
            for idx in leaves {
                MappedFile::link_prev_next_nodes(&mut nodepool, prev_idx, Some(idx));
                prev_idx = Some(idx);
            }
        }

        tmp_node
    }

    fn check_leaves(file: &MappedFile) {
        let debug = !true;

        let (idx, _, _) = file.find_node_by_offset(0);
        if idx.is_none() {
            return;
        }

        let mut idx = idx.unwrap() as usize;

        let mut off = 0;
        let file_size = file.size();
        loop {
            if debug {
                println!("{} / {}", off, file_size);
                println!("file.nodepool[{}].size = {}", idx, file.nodepool[idx].size);
            }
            if debug {
                println!(
                    "off({}) + {} >= file.size({}) ?",
                    off, file.nodepool[idx].size, file_size
                );
            }
            off += file.nodepool[idx].size;
            if off > file_size {
                panic!("invalid tree, broken node size: off > file_size");
            }
            if off == file_size {
                break;
            }

            if let Some(next) = file.nodepool[idx].next {
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

            let node_size = file.nodepool[idx].size;
            // map
            let page = file.nodepool[idx].map().unwrap();
            let slice = page.as_ref().borrow().as_slice().unwrap();

            // copy
            let nw = unsafe { write(fd, slice.as_ptr() as *mut c_void, slice.len()) };
            if nw != slice.len() as isize {
                panic!("write error");
            }

            offset += node_size;
            n = file.nodepool[idx].next;
        }

        // patch on_disk_offset, and file descriptor
        let mut count: u64 = 0;
        let mut offset = 0;
        let (mut n, _, _) = MappedFile::find_node_by_offset(&file, offset);
        while n.is_some() {
            let idx = n.unwrap();
            let node_size = file.nodepool[idx].size;

            // node on disk ?
            if file.nodepool[idx].cow.is_none() {
                let align_offset = 4096 * (offset / 4096);
                let skip = offset % 4096;

                file.nodepool[idx].on_disk_offset = align_offset;
                file.nodepool[idx].skip = skip;
            } else {
                file.nodepool[idx].on_disk_offset = 0xffffffffffffffff;
                file.nodepool[idx].skip = 0;
            }

            if false {
                println!(
                    "offset {}, page {}, size {} disk_offset {}, skip {}",
                    offset,
                    count,
                    file.nodepool[idx].size,
                    file.nodepool[idx].on_disk_offset,
                    file.nodepool[idx].skip,
                );
            }

            offset += node_size;
            n = file.nodepool[idx].next;
            file.nodepool[idx].fd = fd;
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
    fn get_mut_repr(&mut self) -> Option<&mut IteratorInstance<'a>> {
        match &mut *self {
            &mut MappedFileIterator::End(..) => None,
            &mut MappedFileIterator::Real(ref mut it) => Some(it),
        }
    }

    fn get_file(&mut self) -> FileHandle<'a> {
        match &mut *self {
            &mut MappedFileIterator::End(ref file) => Rc::clone(file),
            &mut MappedFileIterator::Real(ref mut it) => Rc::clone(&it.file),
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
        match &*self {
            &MappedFileIterator::End(..) => panic!("invalid iterator"),
            &MappedFileIterator::Real(ref it) => &it.base[it.local_offset as usize],
        }
    }
}

impl<'a> Iterator for MappedFileIterator<'a> {
    type Item = Self;

    fn next(&mut self) -> Option<Self> {
        match &mut *self {
            &mut MappedFileIterator::End(..) => None,

            &mut MappedFileIterator::Real(ref mut it) => {
                if it.local_offset == it.page_size {
                    let mut file = it.file.borrow_mut();

                    let next_node_idx = {
                        let node = &mut file.nodepool[it.node_idx as usize];

                        // end-of-file ?
                        if node.next == None {
                            return None;
                        }

                        node.next.unwrap()
                    };

                    let next_node = &mut file.nodepool[next_node_idx as usize];

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
        let mut nodepool = FreeListAllocator::new();
        let file_size = 1024 * 1024 * 1024 * 1024 * 8; // x Tib
        let page_size = 4096 * 256 * 4; // 4 Mib

        let root_node = Node {
            used: true,
            fd: fd,
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

        let id = nodepool.allocate(root_node);

        let mut leaves = Vec::new();
        MappedFile::build_tree(
            &mut nodepool,
            fd,
            &mut leaves,
            Some(id),
            page_size as u64,
            file_size as u64,
            0,
        );
        let mut prev_idx = None;
        for idx in &leaves {
            MappedFile::link_prev_next_nodes(&mut nodepool, prev_idx, Some(*idx));
            prev_idx = Some(*idx);
        }

        println!("file_size : {}", file_size);
        println!("page_size : {}", page_size);
        println!("number of leaves : {}", leaves.len());
        println!("number of nodes : {}", nodepool.slot.len());

        let node_ram_size = ::std::mem::size_of::<Node>();
        println!("node_ram_size : bytes {}", node_ram_size);

        let ram = nodepool.slot.len() * node_ram_size;
        println!("ram : bytes {}", ram);
        println!("ram : Kib {}", ram >> 10);
        println!("ram : Mib {}", ram >> 20);
        println!("ram : Gib {}", ram >> 30);

        use std::io;

        if !true {
            println!("Hit [Enter] to stop");
            let mut stop = String::new();
            io::stdin().read_line(&mut stop).expect("something");
        }
    }

    #[test]
    fn test_remove() {
        use super::*;

        let file_size = 4096 * 16;
        let page_size = 4096;
        let nr_remove = page_size * 14;
        let offset = page_size as u64;

        use std::fs;
        use std::fs::File;
        use std::io::prelude::*;

        let filename = "/tmp/playground_remove_test".to_owned();
        let mut file = File::create(&filename).unwrap();

        // prepare file content
        println!("-- generating test file");
        let mut slc = Vec::with_capacity(file_size);
        for i in 0..file_size {
            if (i % (1024 * 1024 * 256)) == 0 {
                println!("-- @ bytes {}", i);
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

        println!("-- mapping the test file");
        let file = match MappedFile::new(filename, page_size) {
            Some(file) => file,
            None => panic!("cannot map file"),
        };

        println!(
            "-- testing remove {} @ {} from {}",
            nr_remove, offset, file_size
        );
        let mut it = MappedFile::iter_from(&file, offset);
        MappedFile::remove(&mut it, nr_remove);

        println!("-- file.size() {}", file.as_ref().borrow().size());
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

        println!("-- mapping the test file");
        let file = match MappedFile::new(filename, page_size) {
            Some(file) => file,
            None => panic!("cannot map file"),
        };

        file.as_ref().borrow_mut().cow_subpage_size = 4096 * 4;
        file.as_ref().borrow_mut().cow_subpage_reserve = 1024;

        for _ in 0..1_000_000 {
            let mut it = MappedFile::iter_from(&file, 0);
            MappedFile::insert(&mut it, &['A' as u8]);
        }

        println!("-- file.size() {}", file.as_ref().borrow().size());
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
            println!("-- generating test file");
            let file_size = 4096 * 10;
            let mut slc = Vec::with_capacity(file_size);
            for i in 0..file_size {
                if (i % (1024 * 1024 * 256)) == 0 {
                    println!("-- @ bytes {}", i);
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

        println!("-- mapping the test file");
        let file = match MappedFile::new(filename, page_size) {
            Some(file) => file,
            None => panic!("cannot map file"),
        };

        file.as_ref().borrow_mut().cow_subpage_size = 4096;
        file.as_ref().borrow_mut().cow_subpage_reserve = 10;

        for i in 0..5 {
            let i = i * 2;
            let mut it = MappedFile::iter_from(&file, i + i * 4096);
            MappedFile::insert(&mut it, &['A' as u8]);
        }

        MappedFile::sync_to_disk(
            &mut file.as_ref().borrow_mut(),
            &"/tmp/mapped_file.sync_test",
            &"/tmp/mapped_file.sync_test.result",
        ).unwrap();

        println!("-- file.size() {}", file.as_ref().borrow().size());

        let _ = fs::remove_file("/tmp/mapped_file.sync_test.result");
        let _ = fs::remove_file("/tmp/playground_insert_test");
    }

}
