// Copyright (c) Carl-Erwin Griffith

//
use crate::core::document::Document;

use crate::dbg_println;

use crate::core::codec::text::utf8::SyncDirection;
use crate::core::codec::text::utf8::TextCodec;

//
#[derive(Debug, Clone, Ord, Eq, PartialOrd, PartialEq)]
pub struct Mark {
    pub offset: u64,
}

// TODO: codec...
pub fn read_char_forward(
    doc: &Document,
    from_offset: u64,
    codec: &dyn TextCodec,
) -> (char, u64, usize) {
    if from_offset == doc.size() as u64 {
        // return None;
        return (b'\0' as char, 0, 0);
    }

    let mut data = Vec::with_capacity(4);
    doc.read(from_offset, data.capacity(), &mut data); // TODO: decode upto capacity ?
    codec.decode(SyncDirection::Forward, &data, 0)
}

// TODO: codec...
pub fn read_char_backward(
    doc: &Document,
    from_offset: u64,
    codec: &dyn TextCodec,
) -> (char, u64, usize) {
    if from_offset == 0 {
        // return None;
        return (b'\0' as char, 0, 0);
    }

    //
    let rewind_offset = if from_offset > 4 { from_offset - 4 } else { 0 };
    let rewind_size = from_offset - rewind_offset;

    // fill buf
    let mut data = Vec::with_capacity(4);
    let _rd = doc.read(rewind_offset, data.capacity(), &mut data) as u64;
    //
    let ret = codec.decode(SyncDirection::Backward, &data, rewind_size);

    /* result aare always relative to from_offset/direction */
    (ret.0, from_offset - ret.2 as u64, ret.2)
}

// TODO: codec...
pub fn read_char(
    _direction: SyncDirection,
    codec: &dyn TextCodec,
    doc: &Document,
    from_offset: u64,
) -> (char, u64, usize) {
    if from_offset == 0 {
        // return None;
        return (b'\0' as char, 0, 0);
    }

    //
    let rewind_offset = if from_offset > 4 { from_offset - 4 } else { 0 };
    let rewind_size = from_offset - rewind_offset;

    // fill buf
    let mut data = Vec::with_capacity(4);
    let _rd = doc.read(rewind_offset, data.capacity(), &mut data) as u64;
    //
    let ret = codec.decode(SyncDirection::Backward, &data, rewind_size);

    /* result aare always relative to from_offset/direction */
    (ret.0, from_offset - ret.2 as u64, ret.2)
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

    pub fn move_forward(&mut self, doc: &Document, codec: &dyn TextCodec) -> &mut Mark {
        // TODO: if '\r\n' must move + 1 in codec
        let (_, _, size) = read_char_forward(&doc, self.offset, codec);
        self.offset += size as u64;

        self
    }

    // TODO: check multi-byte utf8 sequence
    pub fn move_backward(&mut self, doc: &Document, codec: &dyn TextCodec) -> &mut Mark {
        let (_, offset, size) = read_char_backward(&doc, self.offset, codec);
        dbg_println!(
            "self.offset({}) = offset({}), size({})",
            self.offset,
            offset,
            size
        );
        self.offset = offset;

        self
    }

    pub fn move_to_start_of_line(&mut self, doc: &Document, codec: &dyn TextCodec) -> &mut Mark {
        if self.offset == 0 {
            return self;
        }

        let mut prev_cp = 0 as char;
        let mut prev_cp_size = 0 as usize;

        loop {
            let base_offset = if self.offset > 4 { self.offset - 4 } else { 0 };
            let relative_offset = self.offset - base_offset;

            assert!(self.offset <= doc.size() as u64);
            assert!(base_offset <= doc.size() as u64);

            let mut data = Vec::with_capacity(4);
            let nb = doc.read(base_offset, data.capacity(), &mut data);

            assert!(nb <= data.capacity());
            assert!(nb == data.len());

            let (cp, off, size) = codec.decode(SyncDirection::Backward, &data, relative_offset);
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

    pub fn move_to_end_of_line(&mut self, doc: &Document, codec: &dyn TextCodec) -> &mut Mark {
        let max_offset = doc.size() as u64;

        let mut prev_offset = self.offset;

        // TODO: end_of_buffer().or_return()
        if prev_offset == max_offset {
            return self;
        }

        loop {
            let mut data = Vec::with_capacity(4);
            doc.read(prev_offset, data.capacity(), &mut data);
            let (cp, _, size) = codec.decode(SyncDirection::Forward, &data, 0);
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

    fn is_blank(&mut self, cp: char) -> bool {
        // TODO: put defintion of word in array of cahr and use any(is_word_vec)
        match cp {
            ' ' | '\r' | '\n' | '\t' => true,
            _ => false,
        }
    }

    pub fn at_end_of_buffer(&self, doc: &Document) -> bool {
        // TODO: end_of_buffer().or_return()
        self.offset == doc.size() as u64
    }

    pub fn move_to_token_start(&mut self, _doc: &Document, _codec: &dyn TextCodec) -> &mut Mark {
        if self.offset == 0 {
            return self;
        }

        //        let (cp, _, size) = read_char_forward_backward(&buffer, prev_offset, codec);

        self
    }

    pub fn move_to_token_end(&mut self, doc: &Document, codec: &dyn TextCodec) -> &mut Mark {
        if self.at_end_of_buffer(doc) {
            return self;
        }

        let max_offset = doc.size() as u64;
        let mut prev_offset = self.offset;

        let (cp, _, size) = read_char_forward(&doc, prev_offset, codec);
        prev_offset += size as u64;

        // skip blanks
        if self.is_blank(cp) {
            loop {
                let (cp, _, size) = read_char_forward(&doc, prev_offset, codec);
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
            let (cp, _, size) = read_char_forward(&doc, prev_offset, codec);
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
    use crate::core::document::DocumentBuilder;
    use crate::core::document::OpenMode;

    // TODO: move to utf8 tests
    // add more tests move etc

    println!("\n**************** test marks *****************");

    let codec = &Utf8Codec::new();

    {
        let mut builder = DocumentBuilder::new();
        let doc = builder
            .document_name("test-1")
            .file_name("/dev/null")
            .internal(false)
            .mode(OpenMode::ReadWrite)
            .finalize()
            .unwrap();

        let mut bb = doc.as_ref().borrow_mut();

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
            m.move_backward(&bb, codec);
            println!("** mark @ {} **", m.offset);
            assert_eq!(m.offset, 3);
        }
        let mut m = Mark { offset: 3 };

        println!("** mark @ {} **", m.offset);
        m.move_backward(&bb, codec);
        println!("** mark @ {} **", m.offset);
        assert_eq!(m.offset, 0);
    }

    {
        let mut builder = DocumentBuilder::new();
        let doc = builder
            .document_name("test-1")
            .file_name("/dev/null")
            .internal(false)
            .mode(OpenMode::ReadWrite)
            .finalize()
            .unwrap();
        let mut bb = doc.as_ref().borrow_mut();
        let data = vec![0x82, 0xac, 0xe2, 0x82, 0x61];
        bb.insert(0, data.len(), &data);
        let mut rdata = vec![];
        bb.read(0, data.len(), &mut rdata);
        assert_eq!(rdata, data);
        assert_eq!(rdata.len(), data.len());
        assert_eq!(rdata.len(), bb.size());

        let mut m = Mark { offset: 4 };

        println!("** mark @ {} **", m.offset);
        m.move_backward(&bb, codec);
        println!("** mark @ {} **", m.offset);
        assert_eq!(m.offset, 2);
    }

    {
        let mut builder = DocumentBuilder::new();
        let doc = builder
            .document_name("test-1")
            .file_name("/dev/null")
            .internal(false)
            .mode(OpenMode::ReadWrite)
            .finalize()
            .unwrap();
        let mut bb = doc.as_ref().borrow_mut();
        let data = vec![0xac, 0xe2, 0x82, 0x61];
        bb.insert(0, data.len(), &data);

        let mut rdata = vec![];
        bb.read(0, data.len(), &mut rdata);
        assert_eq!(rdata, data);
        assert_eq!(rdata.len(), data.len());
        assert_eq!(rdata.len(), bb.size());

        let mut m = Mark { offset: 3 };

        println!("** mark @ {} **", m.offset);
        m.move_backward(&bb, codec);
        println!("** mark @ {} **", m.offset);
        assert_eq!(m.offset, 2);
    }

    {
        let mut builder = DocumentBuilder::new();
        let doc = builder
            .document_name("test-1")
            .file_name("/dev/null")
            .internal(false)
            .mode(OpenMode::ReadWrite)
            .finalize()
            .unwrap();
        let mut bb = doc.as_ref().borrow_mut();
        let data = vec![0xe2, 0x82, 0x61];
        bb.insert(0, data.len(), &data);
        let mut rdata = vec![];
        bb.read(0, data.len(), &mut rdata);
        assert_eq!(rdata, data);
        assert_eq!(rdata.len(), data.len());
        assert_eq!(rdata.len(), bb.size());

        let mut m = Mark { offset: 2 };

        println!("** mark @ {} **", m.offset);
        m.move_backward(&bb, codec);
        println!("** mark @ {} **", m.offset);
        assert_eq!(m.offset, 1);
    }

    {
        let mut builder = DocumentBuilder::new();
        let doc = builder
            .document_name("test-1")
            .file_name("/dev/null")
            .internal(false)
            .mode(OpenMode::ReadWrite)
            .finalize()
            .unwrap();
        let mut bb = doc.as_ref().borrow_mut();
        let data = vec![0x61];
        bb.insert(0, data.len(), &data);
        let mut rdata = vec![];
        bb.read(0, data.len(), &mut rdata);
        assert_eq!(rdata, data);
        assert_eq!(rdata.len(), data.len());
        assert_eq!(rdata.len(), bb.size());

        let mut m = Mark { offset: 0 };

        println!("** mark @ {} **", m.offset);
        m.move_backward(&bb, codec);
        println!("** mark @ {} **", m.offset);
        assert_eq!(m.offset, 0);
    }

    {
        let mut builder = DocumentBuilder::new();
        let doc = builder
            .document_name("test-1")
            .file_name("/dev/null")
            .internal(false)
            .mode(OpenMode::ReadWrite)
            .finalize()
            .unwrap();
        let mut bb = doc.as_ref().borrow_mut();
        let data = vec![0x82, 0x61];
        bb.insert(0, data.len(), &data);
        let mut rdata = vec![];
        bb.read(0, data.len(), &mut rdata);
        assert_eq!(rdata, data);
        assert_eq!(rdata.len(), data.len());
        assert_eq!(rdata.len(), bb.size());

        let mut m = Mark { offset: 1 };

        println!("** mark @ {} **", m.offset);
        m.move_backward(&bb, codec);
        println!("** mark @ {} **", m.offset);
        assert_eq!(m.offset, 0);
    }

    println!("\n*************************************");
}
