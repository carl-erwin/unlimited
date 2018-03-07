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

    pub fn new(offset: u64) -> Mark {
        Mark { offset }
    }

    pub fn move_forward(
        &mut self,
        buffer: &Buffer,
        get_next_codepoint_start: fn(data: &[u8], from_offset: u64) -> u64,
    ) {
        self.offset = get_next_codepoint_start(&buffer.data, self.offset);
        // TODO: if '\r\n' must move + 1
    }

    pub fn move_backward(
        &mut self,
        buffer: &Buffer,
        get_previous_codepoint_start: fn(data: &[u8], from_offset: u64) -> u64,
    ) {
        if self.offset == 0 {
            return;
        }

        // TODO: if '\r\n' must move - 1
        self.offset = get_previous_codepoint_start(&buffer.data, self.offset);
    }

    pub fn move_to_beginning_of_line(
        &mut self,
        buffer: &Buffer,
        get_prev_codepoint: fn(data: &[u8], from_offset: u64) -> (char, u64, usize),
    ) {
        if self.offset == 0 {
            return;
        }

        let mut prev_offset = self.offset;
        loop {
            let (cp, offset, _) = get_prev_codepoint(&buffer.data, prev_offset);
            if offset == 0 {
                self.offset = 0;
                break;
            }

            match cp {
                '\n' => {
                    self.offset = offset + 1;

                    if prev_offset > 0 {
                        if let ('\r', offset, _) = get_prev_codepoint(&buffer.data, prev_offset) {
                            self.offset = offset;
                        }
                    }
                    break;
                }

                '\r' => {
                    self.offset = offset + 1;
                    break;
                }

                _ => prev_offset = offset,
            }
        }
    }

    pub fn move_to_end_of_line(
        &mut self,
        buffer: &Buffer,
        get_codepoint: fn(data: &[u8], from_offset: u64) -> (char, u64, usize),
    ) {
        let max_offset = buffer.data.len() as u64;

        let mut prev_offset = self.offset;

        loop {
            let (cp, offset, size) = get_codepoint(&buffer.data, prev_offset);
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
            prev_offset = offset + size as u64;
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
        let mut bb = Buffer::new(&"/dev/null".to_owned(), OpenMode::ReadWrite).unwrap();
        let data = vec![0xe2, 0x82, 0xac, 0xe2, 0x82, 0x61];
        bb.insert(0, 6, &data);
        assert_eq!(bb.data, data);
        assert_eq!(data.len(), bb.size());

        let mut m = Mark { offset: 5 };

        println!("** mark @ {} **", m.offset);
        m.move_backward(&bb, get_previous_codepoint_start);
        println!("** mark @ {} **", m.offset);
        assert_eq!(m.offset, 4);

        let mut m = Mark { offset: 3 };

        println!("** mark @ {} **", m.offset);
        m.move_backward(&bb, get_previous_codepoint_start);
        println!("** mark @ {} **", m.offset);
        assert_eq!(m.offset, 0);
    }

    {
        let mut bb = Buffer::new(&"/dev/null".to_owned(), OpenMode::ReadWrite).unwrap();
        let data = vec![0x82, 0xac, 0xe2, 0x82, 0x61];
        bb.insert(0, data.len(), &data);
        assert_eq!(bb.data, data);
        assert_eq!(data.len(), bb.size());

        let mut m = Mark { offset: 4 };

        println!("** mark @ {} **", m.offset);
        m.move_backward(&bb, get_previous_codepoint_start);
        println!("** mark @ {} **", m.offset);
        assert_eq!(m.offset, 3);
    }

    {
        let mut bb = Buffer::new(&"/dev/null".to_owned(), OpenMode::ReadWrite).unwrap();
        let data = vec![0xac, 0xe2, 0x82, 0x61];
        bb.insert(0, data.len(), &data);
        assert_eq!(bb.data, data);
        assert_eq!(data.len(), bb.size());

        let mut m = Mark { offset: 3 };

        println!("** mark @ {} **", m.offset);
        m.move_backward(&bb, get_previous_codepoint_start);
        println!("** mark @ {} **", m.offset);
        assert_eq!(m.offset, 2);
    }

    {
        let mut bb = Buffer::new(&"/dev/null".to_owned(), OpenMode::ReadWrite).unwrap();
        let data = vec![0xe2, 0x82, 0x61];
        bb.insert(0, data.len(), &data);
        assert_eq!(bb.data, data);
        assert_eq!(data.len(), bb.size());

        let mut m = Mark { offset: 2 };

        println!("** mark @ {} **", m.offset);
        m.move_backward(&bb, get_previous_codepoint_start);
        println!("** mark @ {} **", m.offset);
        assert_eq!(m.offset, 1);
    }

    {
        let mut bb = Buffer::new(&"/dev/null".to_owned(), OpenMode::ReadWrite).unwrap();
        let data = vec![0x61];
        bb.insert(0, data.len(), &data);
        assert_eq!(bb.data, data);
        assert_eq!(data.len(), bb.size());

        let mut m = Mark { offset: 0 };

        println!("** mark @ {} **", m.offset);
        m.move_backward(&bb, get_previous_codepoint_start);
        println!("** mark @ {} **", m.offset);
        assert_eq!(m.offset, 0);
    }

    {
        let mut bb = Buffer::new(&"/dev/null".to_owned(), OpenMode::ReadWrite).unwrap();
        let data = vec![0x82, 0x61];
        bb.insert(0, data.len(), &data);
        assert_eq!(bb.data, data);
        assert_eq!(data.len(), bb.size());

        let mut m = Mark { offset: 1 };

        println!("** mark @ {} **", m.offset);
        m.move_backward(&bb, get_previous_codepoint_start);
        println!("** mark @ {} **", m.offset);
        assert_eq!(m.offset, 0);
    }

    println!("\n*************************************");
}
