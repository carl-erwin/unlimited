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

use crate::core::mapped_file::FileHandle;
use crate::core::mapped_file::MappedFile;

//
pub type Id = u64;
pub type Offset = u64;
pub type PageSize = usize;

//
#[derive(Debug)]
pub enum OpenMode {
    ReadOnly = 0,
    ReadWrite = 1,
}

/// The **Buffer** represents a linear array of bytes.<br/>
/// it can be in memory only or backed by an on disk file.<br/>
/// The editor **Modes** will use this api to read/modify the content
/// of the file at the byte level
#[derive(Debug)]
pub struct Buffer<'a> {
    pub id: Id,
    /// the name of the file where the data will be synced
    pub file_name: String,
    /// the current size of the buffer
    pub size: usize,
    /// the number of changes (since last save TODO)
    pub nr_changes: u64,
    mode: OpenMode,
    pub data: FileHandle<'a>,
}

impl<'a> Buffer<'a> {
    /// Creates a new `Buffer`.
    ///
    /// file_name param[in] path to the file we want to load in the buffer,
    /// use "/dev/null" to create empty buffer
    /// this function allocate a buffer
    /// if file_name is null the content will be stored in heap
    /// if file_name is non null the the content will be read from the file
    /// if document_name is null , file_name will be used to give a name to the buffer
    /// mode = 0 : read only , mode 1 : read_write
    /// the allocated_bid pointer will be filled on successfull open operation
    pub fn new(file_name: &str, mode: OpenMode) -> Option<Buffer<'a>> {
        // TODO: check permission
        // TODO: check file's type => ignore directory (for now)
        // println!("-- mapping file {}", file_name);

        let page_size = 4096 * 256 * 2;
        let file = match MappedFile::new(file_name.to_owned(), page_size) {
            Some(file) => file,
            None => {
                // TODO: return Result
                // eprintln!("cannot map file '{}'", file_name);
                return None;
            }
        };

        let size = file.as_ref().borrow().size() as usize;

        // println!("'{}' opened size '{}'", file_name, size);

        Some(Buffer {
            id: 0,
            file_name: file_name.to_owned(),
            mode,
            size,
            nr_changes: 0,
            data: file,
        })
    }

    /// not implemented: close a previously opened buffer see buffer_open
    pub fn close(&mut self) -> bool {
        unimplemented!();
        // false
    }

    /// returns the name of the file associated to the buffer
    pub fn get_file_name(&self) -> String {
        self.file_name.clone()
    }

    /// change the on disk target file
    pub fn set_file_name(&mut self, name: String) -> bool {
        self.file_name = name;
        true
    }

    /// returns the number of bytes a given buffer contains
    pub fn size(&self) -> usize {
        self.size
    }

    /// returns the number of changes sine the last save<br/>
    ///     0  => the no change since last save<br/>
    ///     >0 => the number of changes since last save<br/>
    pub fn nr_changes(&self) -> u64 {
        self.nr_changes
    }

    /// copy the content of the buffer up to 'nr_bytes' into the data Vec
    /// the read bytes are appended to the data Vec
    /// return XXX on error (use ioresult)
    pub fn read(&self, offset: u64, nr_bytes: usize, mut data: &mut Vec<u8>) -> usize {
        let mut it = MappedFile::iter_from(&self.data, offset);
        MappedFile::read(&mut it, nr_bytes, &mut data)
    }

    /// insert the 'data' Vec content in the buffer up to 'nr_bytes'
    /// return the number of written bytes (TODO: use io::Result)
    pub fn insert(&mut self, offset: u64, nr_bytes: usize, data: &[u8]) -> usize {
        let mut it = MappedFile::iter_from(&self.data, offset);
        let nb = MappedFile::insert(&mut it, &data);
        assert_eq!(nb, nr_bytes);
        self.size += nb;
        self.nr_changes += 1;

        nb
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
        let start_offset = ::std::cmp::min(offset as usize, self.size);
        let end_offset = ::std::cmp::min(start_offset + nr_bytes, self.size);
        let nr_bytes_removed = (end_offset - start_offset) as usize;

        // copy removed data
        if let Some(v) = removed_data {
            self.read(offset, nr_bytes_removed, v);
        }

        let mut it = MappedFile::iter_from(&self.data, start_offset as u64);
        let nb = MappedFile::remove(&mut it, nr_bytes_removed);
        assert!(nb <= nr_bytes_removed);
        self.size -= nb;
        self.nr_changes += 1;

        nb
    }

    /// can be used to know the number of blocks that compose the buffer,
    /// api to be used by indexer etc...
    pub fn nr_pages(&self) -> u64 {
        1
    }

    /*
        /// returns the position and size of a given page
        pub fn get_page_info(&self, page_index: u64) -> (Offset, PageSize) {
            // if page_index > 0 {
            //    (0, self.size)
            // } else {
            //    (0, self.size)
            // }
            (0, self.size)
        }
    */

    pub fn sync_to_disk(&self, tmp_file_name: &str) -> ::std::io::Result<()> {
        let metadata = ::std::fs::metadata(&self.file_name).unwrap();
        let perms = metadata.permissions();

        let res = MappedFile::sync_to_disk(
            &mut self.data.as_ref().borrow_mut(),
            &tmp_file_name,
            &self.file_name,
        );

        // TODO: check result, handle io results properly
        // set buffer status to : permission denied etc
        let _ = ::std::fs::set_permissions(&self.file_name, perms);

        res
    }
}

#[test]
fn test_buffer() {
    let mut bb = Buffer::new("/dev/null", OpenMode::ReadWrite).unwrap();

    let data = vec![0, 1, 2, 3, 4, 5, 6, 7, 8, 9];

    bb.insert(0, 10, &data);
    let mut rdata = Vec::new();
    let nread = bb.read(0, bb.size(), &mut rdata);
    assert_eq!(rdata, data);
    assert_eq!(nread, bb.size());

    let data = vec![0, 1, 2, 6, 7, 8, 9];
    let rm_expect = vec![3, 4, 5];

    let mut rm = vec![];
    let n = bb.remove(3, 3, Some(&mut rm));
    assert_eq!(n, 3);
    assert_eq!(rm, rm_expect);

    rdata.clear();
    let nread = bb.read(0, bb.size(), &mut rdata);
    assert_eq!(rdata, data);
    assert_eq!(nread, bb.size());

    println!("rm {:?}", rm);
    println!("rm_expect {:?}", rm_expect);
}
