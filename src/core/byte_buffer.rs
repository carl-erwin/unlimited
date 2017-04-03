//
pub type Id = u64;
pub type Offset = u64;
pub type PageSize = usize;


pub enum OpenMode {
    ReadOnly = 0,
    ReadWrite = 1,
}



/// The **ByteBuffer** represent a linear array of bytes.<br/>
/// it can be in memory only or backed by an on disk file.<br/>
/// The editor **Modes** use this api to read/modify the content of the file at the byte level
pub struct ByteBuffer {
    pub id: Id,
    name: String,
    filename: String,
    size: usize, // proxy to underlying structs
    nr_changes: u64, // number of changes since last save
}


impl ByteBuffer {
    /// filename param[in] path to the file we want to load in the buffer, use "/dev/null" to create empty buffer
    /// this function allocate a buffer
    /// if filename is null the content will be stored in heap
    /// if filename is non null the the content will be read from the file
    /// if buffer_name is null , filename will be used to give a name to the buffer
    /// mode = 0 : read only , mode 1 : read_write
    /// the allocated_bid pointer will be filled on successfull open operation
    pub fn new(buffer_name: String, filename: String, mode: OpenMode) -> ByteBuffer {
        ByteBuffer {
            id: 0,
            name: buffer_name,
            filename: filename,
            size: 0,
            nr_changes: 0,
        }
    }

    /// close a previously opened buffer see buffer_open
    pub fn close(&mut self) -> bool {
        false
    }


    /// set the name of the buffer
    pub fn set_name(&mut self, bid: Id, buffer_name: String) -> bool {
        false
    }

    /// returns the name of the buffer
    pub fn get_name(&self) -> String {
        self.name.clone()
    }

    /// returns the name of the file associated to the buffer
    pub fn get_filename(&self) -> String {
        self.filename.clone()
    }


    /// change the on disk target file
    fn set_filename(&mut self, name: String) -> bool {
        self.filename = name;
        true
    }

    /// returns the number of bytes a given buffer contains
    fn size(&self) -> usize {
        self.size
    }


    /// returns the number of changes sine the last save
    ///     0  => the no change since last save
    ///     >0 => the number of changes since last save
    fn nr_changes(&self) -> u64 {
        self.nr_changes
    }

    /// copy the content of the buffer up to 'nr_bytes' into the data Vec
    /// the read bytes are appended to the data Vec
    /// return XXX on error (use ioresult)
    fn read(&self, offset: u64, nr_bytes: usize, data: &mut Vec<u8>) -> usize {
        0
    }

    /// insert the 'data' Vec content in the buffer upto 'nr_bytes'
    /// return XXX on error (use ioresult)
    fn write(&self, offset: u64, nr_bytes: usize, data: &Vec<u8>) -> usize {
        0
    }

    /// remove up to 'nr_bytes' from the buffer starting at offset
    /// if removed_data is provided will call self.read(offset, nr_bytes, data) before remove the bytes
    fn remove(&self, offset: u64, nr_bytes: usize, removed_data: Option<&mut Vec<u8>>) -> usize {

        // copy removed data
        if let Some(v) = removed_data {
            self.read(offset, nr_bytes, v);
        }

        // TODO: impl

        0
    }

    // future blocvk api:

    // can be used to know the number of blocks that compose the buffer, api to be used by indexer etc...
    fn nr_pages(&self) -> u64 {
        0
    }

    //
    fn get_page_info(&self, page_index: u64) -> (Offset, PageSize) {
        (0, 0)

    }
}
