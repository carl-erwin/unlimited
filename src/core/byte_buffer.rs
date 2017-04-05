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



/// The **ByteBuffer** represent a linear array of bytes.<br/>
/// it can be in memory only or backed by an on disk file.<br/>
/// The editor **Modes** use this api to read/modify the content of the file at the byte level
#[derive(Debug)]
pub struct ByteBuffer {
    pub id: Id,
    pub file_name: String,
    pub size: usize, // proxy to underlying structs
    pub nr_changes: u64, // number of changes since last save
    pub file: File, //
    mode: OpenMode, //
    pub data: Vec<u8>, // file bytes
}


impl ByteBuffer {
    /// Creates a new `Buffer`.
    ///
    /// file_name param[in] path to the file we want to load in the buffer, use "/dev/null" to create empty buffer
    /// this function allocate a buffer
    /// if file_name is null the content will be stored in heap
    /// if file_name is non null the the content will be read from the file
    /// if buffer_name is null , file_name will be used to give a name to the buffer
    /// mode = 0 : read only , mode 1 : read_write
    /// the allocated_bid pointer will be filled on successfull open operation
    pub fn new(file_name: &String, mode: OpenMode) -> Option<ByteBuffer> {

        // TODO: check permission
        let mut file = match File::open(file_name) {
            Ok(f) => f,
            Err(e) => {
                println!("cannot open '{}' : {}", file_name, e);
                return None;
            }
        };

        let mut data = Vec::new();
        let size = file.read_to_end(&mut data).unwrap_or(0);

        println!("'{}' opened mode '{:?}' size '{}'", file_name, mode, size);

        Some(ByteBuffer {
                 id: 0,
                 file_name: file_name.clone(),
                 mode: mode,
                 size: size,
                 nr_changes: 0,
                 file: file,
                 data: data,
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
        for b in &self.data[start_offset..end_offset] {
            data.push(*b);
        }
        (end_offset - start_offset) as usize
    }

    /// insert the 'data' Vec content in the buffer upto 'nr_bytes'
    /// return XXX on error (use ioresult)
    pub fn write(&self, offset: u64, nr_bytes: usize, data: &Vec<u8>) -> usize {
        0
    }

    /// remove up to 'nr_bytes' from the buffer starting at offset
    /// if removed_data is provided will call self.read(offset, nr_bytes, data) before remove the bytes
    pub fn remove(&mut self,
                  offset: u64,
                  nr_bytes: usize,
                  removed_data: Option<&mut Vec<u8>>)
                  -> usize {

        let start_offset = ::std::cmp::min(offset as usize, self.size);
        let end_offset = ::std::cmp::min(start_offset + nr_bytes, self.size);

        // copy removed data
        if let Some(v) = removed_data {
            self.read(offset, nr_bytes, v);
        }

        self.data.drain(start_offset..end_offset);

        let nr_bytes_removed = (end_offset - start_offset) as usize;
        self.size -= nr_bytes_removed;
        nr_bytes_removed
    }


    /// can be used to know the number of blocks that compose the buffer, api to be used by indexer etc...
    pub fn nr_pages(&self) -> u64 {
        1
    }

    /// returns the position and size of a given page
    pub fn get_page_info(&self, page_index: u64) -> (Offset, PageSize) {
        (0, self.size)
    }
}
