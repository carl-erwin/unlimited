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
use core::buffer::Buffer;

//
#[derive(Debug, Clone)]
pub struct Mark {
    pub offset: u64,
}

impl Mark {
    /* TODO: add TextCodec trait
     TextCodec {
         fn get_previous_codepoint_start(data: &[u8], from_offset: u64) -> u64)
         fn get_next_codepoint_start(data: &[u8], from_offset: u64) -> u64)

         fn get_prev_codepoint(data: &[u8], from_offset: u64) -> (char, u64, usize)
         fn get_codepoint(data: &[u8], from_offset: u64) -> (char, u64, usize)

         fn encode(codepoint: u32, out: &mut [u8; 4]) -> usize;
     }
    */

    pub fn new(offset: u64) -> Self {
        Mark { offset }
    }

    pub fn move_forward(
        &mut self,
        buffer: &Buffer,
        get_next_codepoint_start: fn(data: &[u8], from_offset: u64) -> u64,
    ) {
        let mut data = Vec::with_capacity(4);
        buffer.read(self.offset, data.capacity(), &mut data);

        let size = get_next_codepoint_start(&data, 0);
        self.offset += size;
        // TODO: if '\r\n' must move + 1
    }

    // TODO: check multi-byte utf8 sequence
    pub fn move_backward(
        &mut self,
        buffer: &Buffer,
        get_previous_codepoint_start: fn(data: &[u8], from_offset: u64) -> u64,
    ) {
        if self.offset == 0 {
            return;
        }

        let base_offset = if self.offset > 4 { self.offset - 4 } else { 0 };
        let relative_offset = self.offset - base_offset;

        let mut data = Vec::with_capacity(4);
        let _ = buffer.read(base_offset, data.capacity(), &mut data) as u64;

        // TODO: if '\r\n' must move - 1
        let off = get_previous_codepoint_start(&data, relative_offset);
        let delta = relative_offset - off;
        self.offset -= delta as u64;
    }

    pub fn move_to_beginning_of_line(
        &mut self,
        buffer: &Buffer,
        get_prev_codepoint: fn(data: &[u8], from_offset: u64) -> (char, u64, usize),
    ) {
        if self.offset == 0 {
            return;
        }

        let mut prev_cp = 0 as char;
        let mut prev_cp_size = 0 as usize;

        loop {
            let base_offset = if self.offset > 4 { self.offset - 4 } else { 0 };
            let relative_offset = self.offset - base_offset;

            let mut data = Vec::with_capacity(4);
            buffer.read(base_offset, data.capacity(), &mut data);

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
    }

    pub fn move_to_end_of_line(
        &mut self,
        buffer: &Buffer,
        get_codepoint: fn(data: &[u8], from_offset: u64) -> (char, u64, usize),
    ) {
        let max_offset = buffer.size as u64;

        let mut prev_offset = self.offset;

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
    }
}

#[test]
fn test_marks() {
    use core::buffer::OpenMode;
    use core::codec::text::utf8::get_previous_codepoint_start;

    // TODO: move to utf8 tests

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
