//
use std::fs::File;
use std::io::prelude::*;
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



/// The **Buffer** represent a linear array of bytes.<br/>
/// it can be in memory only or backed by an on disk file.<br/>
/// The editor **Modes** use this api to read/modify the content
/// of the file at the byte level
#[derive(Debug)]
pub struct Buffer {
    pub id: Id,
    pub file_name: String,
    pub size: usize, // proxy to underlying structs
    pub nr_changes: u64, // number of changes since last save
    pub file: File, //
    mode: OpenMode, //
    pub data: Vec<u8>, // file bytes
}


impl Buffer {
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
    pub fn new(file_name: &str, mode: OpenMode) -> Option<Buffer> {

        // TODO: check permission
        let mut file = match File::open(file_name) {
            Ok(f) => f,
            Err(e) => {
                println!("cannot open '{}' : {}", file_name, e);
                return None;
            }
        };

        // TODO: check file's type => ignore directory (for now)

        let mut data = Vec::new();
        let size = file.read_to_end(&mut data).unwrap_or(0);

        println!("'{}' opened mode '{:?}' size '{}'", file_name, mode, size);
        /*
            for c in &data {
                println!("c {} char '{}' ", *c, *c as char);
            }
        */
        Some(Buffer {
            id: 0,
            file_name: file_name.to_owned(),
            mode,
            size,
            nr_changes: 0,
            file,
            data,
        })
    }

    /// close a previously opened buffer see buffer_open
    pub fn close(&mut self) -> bool {
        false
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
    pub fn read(&self, offset: u64, nr_bytes: usize, data: &mut Vec<u8>) -> usize {
        let start_offset = ::std::cmp::min(offset as usize, self.size);
        let end_offset = ::std::cmp::min(start_offset + nr_bytes, self.size);
        let nr_copied = (end_offset - start_offset) as usize;
        data.reserve(nr_copied);
        for b in &self.data[start_offset..end_offset] {
            data.push(*b);
        }
        nr_copied
    }

    /// insert the 'data' Vec content in the buffer up to 'nr_bytes'
    /// return the number of written bytes (TODO: use io::Result)
    pub fn insert(&mut self, offset: u64, nr_bytes: usize, data: &[u8]) -> usize {

        let index = offset as usize;
        for (n, b) in data.iter().enumerate().take(nr_bytes) {
            self.data.insert(index + n, *b);
        }

        self.size += nr_bytes;
        self.nr_changes += 1;

        nr_bytes
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

        self.data.drain(start_offset..end_offset);
        self.size -= nr_bytes_removed;
        self.nr_changes += 1;

        nr_bytes_removed
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
}


#[test]
fn test_buffer() {
    let mut bb = Buffer::new(&"/dev/null".to_owned(), OpenMode::ReadWrite).unwrap();

    let data = vec![0, 1, 2, 3, 4, 5, 6, 7, 8, 9];

    bb.write(0, 10, &data);
    assert_eq!(bb.data, data);
    assert_eq!(data.len(), bb.size());

    let mut rdata = Vec::new();

    let nread = bb.read(0, bb.size(), &mut rdata);
    assert_eq!(rdata, data);
    assert_eq!(nread, bb.size());

    let data = vec![0, 1, 2, 6, 7, 8, 9];
    let mut rm = vec![];
    let n = bb.remove(3, 3, Some(&mut rm));
    assert_eq!(n, 3);
    assert_eq!(bb.data, data);
    let rm_expect = vec![3, 4, 5];
    assert_eq!(rm, rm_expect);
    println!("rm {:?}", rm);
    println!("rm_expect {:?}", rm_expect);
}
