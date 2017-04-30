//
use core::buffer::Buffer;


//
#[derive(Debug)]
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
    pub fn move_forward(&mut self,
                        buffer: &Buffer,
                        get_next_codepoint_start: fn(data: &[u8], from_offset: u64) -> u64) {

        self.offset = get_next_codepoint_start(&buffer.data, self.offset);
    }

    pub fn move_backward(&mut self,
                         buffer: &Buffer,
                         get_previous_codepoint_start: fn(data: &[u8], from_offset: u64) -> u64) {

        if self.offset == 0 {
            return;
        }

        // TODO: if '\r\n' must move - 1
        self.offset = get_previous_codepoint_start(&buffer.data, self.offset);
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
        bb.write(0, 6, &data);
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
        bb.write(0, data.len(), &data);
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
        bb.write(0, data.len(), &data);
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
        bb.write(0, data.len(), &data);
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
        bb.write(0, data.len(), &data);
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
        bb.write(0, data.len(), &data);
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
