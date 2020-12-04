// Copyright (c) Carl-Erwin Griffith

//
use crate::core::buffer::Buffer;

//
#[derive(Debug, Clone)]
pub struct Mark {
    pub offset: u64,
}

// TODO: codec...
pub fn read_char(
    buffer: &Buffer,
    from_offset: u64,
    get_codepoint: fn(data: &[u8], from_offset: u64) -> (char, u64, usize),
) -> (char, u64, usize) {
    let mut data = Vec::with_capacity(4);
    buffer.read(from_offset, data.capacity(), &mut data); // TODO: decode upto capacity ?
    get_codepoint(&data, 0)
}

impl Mark {
    /* TODO: add TextCodec trait

     RawCodec {
        encode(Writer: , offset, bytes: vec<u8>)
        decode(Writer: , offset, bytes: &mut vec<u8>) // n = vec::capacity

        read/write from ll stack (doc/buffer)
        read(offset, BACKWARD | FORWARD) -> ( IoData(u8), offset', size )

        sync(offset, n, BACKWARD | FORWARD) -> offset' // also used  to skip
        fn write(codepoint: u32, out: &mut [u8; 4]) -> usize;
     }


     TextCodec {

        write(Writer: , offset: enum {cur,abs}, cp: char/u32)
        read/write from ll stack (doc/buffer)
        read(offset, BACKWARD | FORWARD) -> (char, offset', encode_size=abs(offset - offset') )
        sync(offset, n, BACKWARD | FORWARD) -> offset' // also used  to skip
        fn write(codepoint: u32, out: &mut [u8; 4]) -> usize;
     }



    */

    pub fn new(offset: u64) -> Self {
        Mark { offset }
    }

    pub fn move_forward(
        &mut self,
        buffer: &Buffer,
        get_codepoint: fn(data: &[u8], from_offset: u64) -> (char, u64, usize),
    ) -> &mut Mark {
        // TODO: if '\r\n' must move + 1 in codec
        let (_, _, size) = read_char(&buffer, self.offset, get_codepoint);
        self.offset += size as u64;

        self
    }

    // TODO: check multi-byte utf8 sequence
    pub fn move_backward(
        &mut self,
        buffer: &Buffer,
        get_previous_codepoint_start: fn(data: &[u8], from_offset: u64) -> u64,
    ) -> &mut Mark {
        if self.offset == 0 {
            return self;
        }

        let base_offset = if self.offset > 4 { self.offset - 4 } else { 0 };
        let relative_offset = self.offset - base_offset;

        let mut data = Vec::with_capacity(4);
        let _ = buffer.read(base_offset, data.capacity(), &mut data) as u64;

        // TODO: if '\r\n' must move - 1
        let off = get_previous_codepoint_start(&data, relative_offset);
        let delta = relative_offset - off;
        self.offset -= delta as u64;

        self
    }

    pub fn move_to_start_of_line(
        &mut self,
        buffer: &Buffer,
        get_prev_codepoint: fn(data: &[u8], from_offset: u64) -> (char, u64, usize),
    ) -> &mut Mark {
        if self.offset == 0 {
            return self;
        }

        let mut prev_cp = 0 as char;
        let mut prev_cp_size = 0 as usize;

        loop {
            let base_offset = if self.offset > 4 { self.offset - 4 } else { 0 };
            let relative_offset = self.offset - base_offset;

            assert!(self.offset <= buffer.size as u64);
            assert!(base_offset <= buffer.size as u64);

            let mut data = Vec::with_capacity(4);
            let nb = buffer.read(base_offset, data.capacity(), &mut data);

            assert!(nb <= data.capacity());
            assert!(nb == data.len());

            let (cp, off, size) = get_prev_codepoint(&data, relative_offset);
            let delta = relative_offset - off;
            self.offset -= delta as u64;
            if self.offset == 0 {
                break;
            }

            match cp {
                '\n' => {
                    self.offset += size as u64;
                    break;
                }

                '\r' => {
                    if prev_cp == '\n' {
                        self.offset += (size + prev_cp_size) as u64;
                    } else {
                        self.offset += size as u64;
                    }
                    break;
                }

                _ => {}
            }

            prev_cp = cp;
            prev_cp_size = size;
        }

        self
    }

    pub fn move_to_end_of_line(
        &mut self,
        buffer: &Buffer,
        get_codepoint: fn(data: &[u8], from_offset: u64) -> (char, u64, usize),
    ) -> &mut Mark {
        let max_offset = buffer.size as u64;

        let mut prev_offset = self.offset;

        // TODO: end_of_buffer().or_return()
        if prev_offset == max_offset {
            return self;
        }

        loop {
            let mut data = Vec::with_capacity(4);
            buffer.read(prev_offset, data.capacity(), &mut data);
            let (cp, _, size) = get_codepoint(&data, 0);
            if prev_offset == max_offset {
                break;
            }
            match cp {
                '\r' | '\n' => {
                    // TODO: handle \r\n
                    break;
                }

                _ => {}
            }
            prev_offset += size as u64;
        }
        self.offset = prev_offset;

        self
    }

    fn is_word(&mut self, cp: char) -> bool {
        // TODO: put defintion of word in array of cahr and use any(is_word_vec)
        match cp {
            '"' | ' ' | '\r' | '\n' | '\t' | ',' | ';' | '{' | '}' | '(' | ')' | '-' => false,
            _ => true,
        }
    }

    fn is_blank(&mut self, cp: char) -> bool {
        // TODO: put defintion of word in array of cahr and use any(is_word_vec)
        match cp {
            ' ' | '\r' | '\n' | '\t' => true,
            _ => false,
        }
    }

    pub fn at_end_of_buffer(&self, buffer: &Buffer) -> bool {
        // TODO: end_of_buffer().or_return()
        self.offset == buffer.size as u64
    }

    pub fn move_to_token_start(
        &mut self,
        buffer: &Buffer,
        get_previous_codepoint_start: fn(data: &[u8], from_offset: u64) -> u64,
    ) -> &mut Mark {
        if self.offset == 0 {
            return self;
        }

        //        let (cp, _, size) = read_char_backward(&buffer, prev_offset, get_codepoint);

        self
    }

    pub fn move_to_token_end(
        &mut self,
        buffer: &Buffer,
        get_codepoint: fn(data: &[u8], from_offset: u64) -> (char, u64, usize),
    ) -> &mut Mark {
        if self.at_end_of_buffer(buffer) {
            return self;
        }

        let max_offset = buffer.size as u64;
        let mut prev_offset = self.offset;

        let (cp, _, size) = read_char(&buffer, prev_offset, get_codepoint);
        prev_offset += size as u64;

        // skip blanks
        if self.is_blank(cp) {
            loop {
                let (cp, _, size) = read_char(&buffer, prev_offset, get_codepoint);
                if prev_offset == max_offset {
                    break;
                }

                if self.is_blank(cp) == false {
                    break;
                }

                prev_offset += size as u64;
            }
        }

        // skip non blanck
        loop {
            let (cp, _, size) = read_char(&buffer, prev_offset, get_codepoint);
            if prev_offset == max_offset {
                break;
            }

            if self.is_blank(cp) == true {
                break;
            }

            prev_offset += size as u64;
        }

        self.offset = prev_offset;

        self
    }
}

#[test]
fn test_marks() {
    use crate::core::buffer::OpenMode;
    use crate::core::codec::text::utf8::get_previous_codepoint_start;

    // TODO: move to utf8 tests
    // add more tests move etc

    println!("\n**************** test marks *****************");

    {
        let mut bb = Buffer::new("/dev/null", OpenMode::ReadWrite).unwrap();
        let data = vec![0xe2, 0x82, 0xac, 0xe2, 0x82, 0x61];
        bb.insert(0, 6, &data);
        let mut rdata = vec![];
        bb.read(0, data.len(), &mut rdata);
        assert_eq!(rdata, data);
        assert_eq!(rdata.len(), data.len());
        assert_eq!(rdata.len(), bb.size());

        for i in 4..6 {
            let mut m = Mark { offset: i };
            println!("** mark @ {} **", m.offset);
            m.move_backward(&bb, get_previous_codepoint_start);
            println!("** mark @ {} **", m.offset);
            assert_eq!(m.offset, 3);
        }
        let mut m = Mark { offset: 3 };

        println!("** mark @ {} **", m.offset);
        m.move_backward(&bb, get_previous_codepoint_start);
        println!("** mark @ {} **", m.offset);
        assert_eq!(m.offset, 0);
    }

    {
        let mut bb = Buffer::new("/dev/null", OpenMode::ReadWrite).unwrap();
        let data = vec![0x82, 0xac, 0xe2, 0x82, 0x61];
        bb.insert(0, data.len(), &data);
        let mut rdata = vec![];
        bb.read(0, data.len(), &mut rdata);
        assert_eq!(rdata, data);
        assert_eq!(rdata.len(), data.len());
        assert_eq!(rdata.len(), bb.size());

        let mut m = Mark { offset: 4 };

        println!("** mark @ {} **", m.offset);
        m.move_backward(&bb, get_previous_codepoint_start);
        println!("** mark @ {} **", m.offset);
        assert_eq!(m.offset, 2);
    }

    {
        let mut bb = Buffer::new("/dev/null", OpenMode::ReadWrite).unwrap();
        let data = vec![0xac, 0xe2, 0x82, 0x61];
        bb.insert(0, data.len(), &data);

        let mut rdata = vec![];
        bb.read(0, data.len(), &mut rdata);
        assert_eq!(rdata, data);
        assert_eq!(rdata.len(), data.len());
        assert_eq!(rdata.len(), bb.size());

        let mut m = Mark { offset: 3 };

        println!("** mark @ {} **", m.offset);
        m.move_backward(&bb, get_previous_codepoint_start);
        println!("** mark @ {} **", m.offset);
        assert_eq!(m.offset, 2);
    }

    {
        let mut bb = Buffer::new("/dev/null", OpenMode::ReadWrite).unwrap();
        let data = vec![0xe2, 0x82, 0x61];
        bb.insert(0, data.len(), &data);
        let mut rdata = vec![];
        bb.read(0, data.len(), &mut rdata);
        assert_eq!(rdata, data);
        assert_eq!(rdata.len(), data.len());
        assert_eq!(rdata.len(), bb.size());

        let mut m = Mark { offset: 2 };

        println!("** mark @ {} **", m.offset);
        m.move_backward(&bb, get_previous_codepoint_start);
        println!("** mark @ {} **", m.offset);
        assert_eq!(m.offset, 1);
    }

    {
        let mut bb = Buffer::new("/dev/null", OpenMode::ReadWrite).unwrap();
        let data = vec![0x61];
        bb.insert(0, data.len(), &data);
        let mut rdata = vec![];
        bb.read(0, data.len(), &mut rdata);
        assert_eq!(rdata, data);
        assert_eq!(rdata.len(), data.len());
        assert_eq!(rdata.len(), bb.size());

        let mut m = Mark { offset: 0 };

        println!("** mark @ {} **", m.offset);
        m.move_backward(&bb, get_previous_codepoint_start);
        println!("** mark @ {} **", m.offset);
        assert_eq!(m.offset, 0);
    }

    {
        let mut bb = Buffer::new("/dev/null", OpenMode::ReadWrite).unwrap();
        let data = vec![0x82, 0x61];
        bb.insert(0, data.len(), &data);
        let mut rdata = vec![];
        bb.read(0, data.len(), &mut rdata);
        assert_eq!(rdata, data);
        assert_eq!(rdata.len(), data.len());
        assert_eq!(rdata.len(), bb.size());

        let mut m = Mark { offset: 1 };

        println!("** mark @ {} **", m.offset);
        m.move_backward(&bb, get_previous_codepoint_start);
        println!("** mark @ {} **", m.offset);
        assert_eq!(m.offset, 0);
    }

    println!("\n*************************************");
}
