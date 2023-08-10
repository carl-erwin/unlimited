//
use crate::core::buffer::Buffer;

use crate::core::codec::text::SyncDirection;
use crate::core::codec::text::TextCodec;

const DEBUG: bool = false;

//
#[derive(Debug, Copy, Clone, Ord, Eq, PartialOrd, PartialEq)]
pub struct Mark {
    pub offset: u64,
}

// TODO(ceg): handle CR LF properly

fn is_blank(cp: char) -> bool {
    // TODO(ceg): put definition of word in array of char and use any(is_word_vec)
    match cp {
        ' ' /*| '\r'*/ | '\n' | '\t' => true,
        _ => false,
    }
}

fn is_end_of_line(cp: char) -> bool {
    match cp {
        '\n' => true,
        _ => false,
    }
}

// decode + extract raw data
pub fn read_char_raw_forward(
    buffer: &Buffer,
    from_offset: u64,
    codec: &dyn TextCodec,
) -> (char, u64, usize, Vec<u8>) {
    if from_offset == buffer.size() as u64 {
        // return None;
        return (b'\0' as char, 0, 0, vec![]);
    }

    let mut data = Vec::with_capacity(4);
    let sz = buffer.read(from_offset, data.capacity(), &mut data); // TODO(ceg): decode up to capacity ?

    if DEBUG {
        dbg_println!(
            "DOC read {} bytes from offset {} {:x?}",
            sz,
            from_offset,
            data
        );
    }

    let r = codec.decode(SyncDirection::Forward, &data, 0);
    data.drain(r.2..);
    (r.0, r.1, r.2, data)
}

// TODO(ceg): codec...
pub fn read_char_forward(
    buffer: &Buffer,
    from_offset: u64,
    codec: &dyn TextCodec,
) -> (char, u64, usize) {
    if from_offset == buffer.size() as u64 {
        // return None;
        return (b'\0' as char, 0, 0);
    }

    let mut data = Vec::with_capacity(4);
    let sz = buffer.read(from_offset, data.capacity(), &mut data); // TODO(ceg): decode up to capacity ?

    if DEBUG {
        dbg_println!(
            "DOC read {} bytes from offset {} {:x?}",
            sz,
            from_offset,
            data
        );
    }

    codec.decode(SyncDirection::Forward, &data, 0)
}

// TODO(ceg): codec..., remove temporary vec -> slice
// from_offset must be sync
pub fn read_char_raw_backward(
    buffer: &Buffer,
    from_offset: u64,
    codec: &dyn TextCodec,
) -> (char, u64, usize, Vec<u8>) {
    if from_offset == 0 {
        // return None;
        return ('\u{0}', 0, 0, vec![]);
    }

    if DEBUG {
        dbg_println!("mark :: read_char_backward from_offset {}", from_offset);
    }

    let rewind_offset = from_offset.saturating_sub(4);
    let rewind_size = from_offset - rewind_offset;

    if DEBUG {
        dbg_println!("mark :: rewind_offset {}", rewind_offset);
        dbg_println!("mark :: rewind_size {}", rewind_size);
    }

    // fill buf
    let mut data = Vec::with_capacity(4);
    let rd = buffer.read(rewind_offset, data.capacity(), &mut data) as u64;

    if DEBUG {
        dbg_println!("mark :: read_char_backward rd {} data {:?}", rd, data);
    }

    //
    let ret = codec.decode(SyncDirection::Backward, &data, rewind_size);

    if DEBUG {
        dbg_println!("code.decode = {:?}", ret);
    }

    dbg_println!("CEG data before drain = {:?}", data);

    // keep trailer
    data.drain(0..(4 - ret.2));

    /* result are always relative to from_offset/direction */
    (ret.0, from_offset - ret.2 as u64, ret.2, data)
}

// TODO(ceg): codec..., remove temporary vec -> slice
pub fn read_char_backward(
    buffer: &Buffer,
    from_offset: u64,
    codec: &dyn TextCodec,
) -> (char, u64, usize) {
    if from_offset == 0 {
        // return None;
        return ('\u{0}', 0, 0);
    }

    if DEBUG {
        dbg_println!("mark :: read_char_backward from_offset {}", from_offset);
    }

    let rewind_offset = from_offset.saturating_sub(4);
    let rewind_size = from_offset - rewind_offset;

    if DEBUG {
        dbg_println!("mark :: rewind_offset {}", rewind_offset);
        dbg_println!("mark :: rewind_size {}", rewind_size);
    }

    // fill buf
    let mut data = Vec::with_capacity(4);
    let rd = buffer.read(rewind_offset, data.capacity(), &mut data) as u64;

    if DEBUG {
        dbg_println!("mark :: read_char_backward rd {} data {:?}", rd, data);
    }

    //
    let ret = codec.decode(SyncDirection::Backward, &data, rewind_size);

    if DEBUG {
        dbg_println!("code.decode = {:?}", ret);
    }

    /* result are always relative to from_offset/direction */
    (ret.0, from_offset - ret.2 as u64, ret.2)
}

// TODO(ceg): codec...
pub fn read_char(
    _direction: SyncDirection,
    codec: &dyn TextCodec,
    buffer: &Buffer,
    from_offset: u64,
) -> (char, u64, usize) {
    if from_offset == 0 {
        // return None;
        return (b'\0' as char, 0, 0);
    }

    //
    let rewind_offset = from_offset.saturating_sub(4);
    let rewind_size = from_offset - rewind_offset;

    // fill buf
    let mut data = Vec::with_capacity(4);
    let _rd = buffer.read(rewind_offset, data.capacity(), &mut data) as u64;
    //
    let ret = codec.decode(SyncDirection::Backward, &data, rewind_size);

    /* result are always relative to from_offset/direction */
    (ret.0, from_offset - ret.2 as u64, ret.2)
}

pub fn decode_until_offset_or_char(
    mark: &mut Mark,
    buffer: &Buffer,
    codec: &dyn TextCodec,
    limit: u64,
    c: Option<char>,
    build_data: bool,
) -> Option<Vec<(u64, char, usize)>> {
    let max_offset = std::cmp::min(buffer.size() as u64, limit);

    let mut prev_offset = mark.offset;

    if prev_offset == max_offset {
        return None;
    }

    // must limit alloc size
    let cache_size = std::cmp::min(1024 * 1024 * 2, limit - prev_offset);
    let mut codepoints = vec![];
    let mut rd_cache = buffer.build_cache(prev_offset, prev_offset + cache_size);

    loop {
        // TODO(ceg): avoid this, use single read before loop
        // and pass &buff[pos..pos+4] for decode
        // pos += size
        let mut data = Vec::with_capacity(4);
        //buffer.read(prev_offset, data.capacity(), &mut data);

        // update cache
        if !rd_cache.contains(prev_offset, prev_offset + data.len() as u64) {
            rd_cache = buffer.build_cache(prev_offset, prev_offset + cache_size);
        }

        buffer.read_cached(prev_offset, data.capacity(), &mut data, &rd_cache);

        let (cp, _, size) = codec.decode(SyncDirection::Forward, &data, 0);
        if prev_offset >= max_offset {
            // dbg_println!("MARK: prev_offset >= max_offset");
            break;
        }

        if let Some(c) = c {
            if cp == c {
                // dbg_println!("MARK:     found cp {}", cp);
                break;
            }
        }

        if build_data {
            codepoints.push((prev_offset, cp, size));
        }

        prev_offset += size as u64;
    }

    //dbg_println!("MARK:     mark.offset = prev_offset = {}", prev_offset);

    mark.offset = prev_offset;

    if build_data {
        // dbg_println!("MARK:     return some vec");
        Some(codepoints)
    } else {
        None
    }
}

pub fn decode_until_end_of_line_or_offset(
    mark: &mut Mark,
    buffer: &Buffer,
    codec: &dyn TextCodec,
    limit: u64,
    build_data: bool,
) -> Option<Vec<(u64, char, usize)>> {
    decode_until_offset_or_char(mark, buffer, codec, limit, Some('\n'), build_data)
}

impl Mark {
    /* TODO(ceg): add TextCodec trait

     Codec {
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

        eol_bytes(&mut [u8;4]) -> usize
    }



    */

    pub fn new(offset: u64) -> Self {
        Mark { offset }
    }

    pub fn move_forward(&mut self, buffer: &Buffer, codec: &dyn TextCodec) -> &mut Mark {
        if self.offset < buffer.size() as u64 {
            // TODO(ceg): if '\r\n' must move + 1 in codec
            let (_, _, size) = read_char_forward(&buffer, self.offset, codec);
            self.offset += size as u64;
        }

        self
    }

    // TODO(ceg): check multi-byte utf8 sequence
    pub fn move_backward(&mut self, buffer: &Buffer, codec: &dyn TextCodec) -> &mut Mark {
        let (c, offset, size) = read_char_backward(&buffer, self.offset, codec);
        dbg_println!(
            "move_backward : char = '{:?}', self.offset({}) = offset({}), size({})",
            c,
            self.offset,
            offset,
            size
        );
        self.offset = offset;

        self
    }

    pub fn move_to_start_of_line(&mut self, buffer: &Buffer, codec: &dyn TextCodec) -> &mut Mark {
        if self.offset == 0 {
            return self;
        }

        dbg_println!("move_to_start_of_line: self.offset {}", self.offset);

        let mut encode = [0; 4];
        let sz = codec.encode('\n' as u32, &mut encode);

        let target_offset =
            if let Some(offset) = buffer.find_reverse(&encode[..sz], self.offset, None) {
                dbg_println!("move_to_start_of_line: find at offset {}", offset);
                dbg_println!("move_to_start_of_line: encode sz {}", sz);

                offset + sz as u64
            } else {
                dbg_println!("move_to_start_of_line: not found");
                0
            };
        dbg_println!("move_to_start_of_line: target_offset {}", target_offset);

        self.offset = target_offset;

        self
    }

    pub fn move_to_end_of_line(&mut self, buffer: &Buffer, codec: &dyn TextCodec) -> &mut Mark {
        let mut encode = [0; 4];
        let sz = codec.encode('\n' as u32, &mut encode);
        self.offset = if let Some(offset) = buffer.find(&encode[..sz], self.offset, None) {
            offset
        } else {
            buffer.size() as u64
        };

        self
    }

    pub fn at_beginning_of_buffer(&self, _buffer: &Buffer) -> bool {
        self.offset == 0
    }

    pub fn at_start_of_buffer(&self, _buffer: &Buffer) -> bool {
        self.at_beginning_of_buffer(_buffer)
    }

    pub fn at_end_of_buffer(&self, buffer: &Buffer) -> bool {
        // TODO(ceg): end_of_buffer().or_return()
        self.offset == buffer.size() as u64
    }

    pub fn skip_end_of_line(&mut self, buffer: &Buffer, codec: &dyn TextCodec) -> &mut Mark {
        let (cp, _, size) = read_char_forward(&buffer, self.offset, codec);
        // skip_backward blanks
        if is_end_of_line(cp) {
            self.offset += size as u64;
        }
        self
    }

    // skip_class(&mut self, direction, fn class_match, buffer, codec)
    // class_match(char) -> bool
    pub fn skip_blanks_backward(&mut self, buffer: &Buffer, codec: &dyn TextCodec) -> &mut Mark {
        let mut prev_offset = self.offset;
        let (cp, _, _) = read_char_forward(&buffer, prev_offset, codec);

        // skip_backward blanks
        if is_blank(cp) {
            while prev_offset > 0 {
                let ret = read_char_backward(&buffer, prev_offset, codec);
                prev_offset -= ret.2 as u64;
                if !is_blank(ret.0) {
                    break;
                }
            }
            self.offset = prev_offset;
        }

        self
    }

    // skip_class(&mut self, direction, fn class_match, buffer, codec)
    // class_match(char) -> bool
    pub fn skip_blanks_backward_until_end_of_line(
        &mut self,
        buffer: &Buffer,
        codec: &dyn TextCodec,
    ) -> &mut Mark {
        let mut prev_offset = self.offset;

        // read current char
        let (cp, _, _) = read_char_forward(&buffer, prev_offset, codec);

        if is_end_of_line(cp) {
            return self;
        }

        // blank ?
        if is_blank(cp) {
            while prev_offset > 0 {
                let ret = read_char_backward(&buffer, prev_offset, codec);
                prev_offset = ret.1;
                if !is_blank(ret.0) || is_end_of_line(ret.0) {
                    break;
                }
            }
            self.offset = prev_offset;
        }

        self
    }

    pub fn skip_non_blanks_backward(
        &mut self,
        buffer: &Buffer,
        codec: &dyn TextCodec,
    ) -> &mut Mark {
        let mut prev_offset = self.offset;
        let (cp, _, _) = read_char_forward(&buffer, prev_offset, codec);

        if !is_blank(cp) {
            while prev_offset > 0 {
                let ret = read_char_backward(&buffer, prev_offset, codec);
                if is_blank(ret.0) {
                    prev_offset = ret.1;
                    break;
                }
                prev_offset -= ret.2 as u64;
            }
            self.offset = prev_offset;
        }

        self
    }

    pub fn move_to_token_start(&mut self, buffer: &Buffer, codec: &dyn TextCodec) -> &mut Mark {
        if self.at_start_of_buffer(buffer) {
            return self;
        }

        self.move_backward(buffer, codec);
        let ret = read_char_forward(&buffer, self.offset, codec);
        if is_end_of_line(ret.0) {
            return self;
        }

        self.skip_blanks_backward_until_end_of_line(buffer, codec);
        self.skip_non_blanks_backward(buffer, codec);

        if self.at_start_of_buffer(buffer) {
            return self;
        }

        self.move_forward(buffer, codec);

        self
    }

    pub fn skip_blanks_forward_until_end_of_line(
        &mut self,
        buffer: &Buffer,
        codec: &dyn TextCodec,
    ) -> &mut Mark {
        let max_offset = buffer.size() as u64;
        let mut prev_offset = self.offset;

        // skip blanks except end of line
        while prev_offset < max_offset {
            let (cp, _, size) = read_char_forward(&buffer, prev_offset, codec);
            if !is_blank(cp) || is_end_of_line(cp) {
                break;
            }

            prev_offset += size as u64;
        }

        self.offset = prev_offset;

        self
    }

    pub fn skip_blanks_forward(&mut self, buffer: &Buffer, codec: &dyn TextCodec) -> &mut Mark {
        let max_offset = buffer.size() as u64;
        let mut prev_offset = self.offset;
        let (cp, _, _) = read_char_forward(&buffer, prev_offset, codec);

        // skip blanks
        if is_blank(cp) {
            while prev_offset < max_offset {
                let (cp, _, size) = read_char_forward(&buffer, prev_offset, codec);
                if !is_blank(cp) {
                    break;
                }
                prev_offset += size as u64;
            }
            self.offset = prev_offset;
        }

        self
    }

    pub fn skip_non_blanks_forward(&mut self, buffer: &Buffer, codec: &dyn TextCodec) -> &mut Mark {
        let max_offset = buffer.size() as u64;
        let mut prev_offset = self.offset;
        let (cp, _, _) = read_char_forward(&buffer, prev_offset, codec);

        if !is_blank(cp) {
            while prev_offset < max_offset {
                let (cp, _, size) = read_char_forward(&buffer, prev_offset, codec);
                if is_blank(cp) {
                    break;
                }
                prev_offset += size as u64;
            }
            self.offset = prev_offset;
        }

        self
    }

    pub fn move_to_token_end(&mut self, buffer: &Buffer, codec: &dyn TextCodec) -> &mut Mark {
        if self.at_end_of_buffer(buffer) {
            return self;
        }

        self.skip_end_of_line(buffer, codec);
        self.skip_blanks_forward_until_end_of_line(buffer, codec);
        self.skip_non_blanks_forward(buffer, codec);

        self
    }
}

#[test]
fn test_marks() {
    use crate::core::buffer::BufferBuilder;
    use crate::core::buffer::BufferKind;
    use crate::core::buffer::OpenMode;
    use crate::core::codec::text::utf8::Utf8Codec;

    // TODO(ceg): move to utf8 tests
    // add more tests move etc

    println!("\n**************** test marks *****************");

    let codec = &Utf8Codec::new();

    {
        let mut builder = BufferBuilder::new(BufferKind::File);
        let buffer = builder
            .buffer_name("test-1")
            .internal(false)
            .mode(OpenMode::ReadWrite)
            .finalize()
            .unwrap();

        let mut bb = buffer.write();

        let data = vec![0xe2, 0x82, 0xac, 0xe2, 0x82, 0xac];
        bb.insert(0, 6, &data);

        let mut rdata = vec![];
        bb.read(0, data.len(), &mut rdata);
        assert_eq!(rdata, data);
        assert_eq!(rdata.len(), data.len());
        assert_eq!(rdata.len(), bb.size());

        let mut m = Mark { offset: 3 };

        println!("** mark @ {} **", m.offset);
        m.move_backward(&bb, codec);
        println!("** mark @ {} **", m.offset);
        assert_eq!(m.offset, 0);
    }

    {
        let mut builder = BufferBuilder::new(BufferKind::File);
        let buffer = builder
            .buffer_name("test-1")
            .internal(false)
            .mode(OpenMode::ReadWrite)
            .finalize()
            .unwrap();
        let mut bb = buffer.write();
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
        assert_eq!(m.offset, 3);
    }

    {
        let mut builder = BufferBuilder::new(BufferKind::File);
        let buffer = builder
            .buffer_name("test-1")
            .internal(false)
            .mode(OpenMode::ReadWrite)
            .finalize()
            .unwrap();
        let mut bb = buffer.write();
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
        let mut builder = BufferBuilder::new(BufferKind::File);
        let buffer = builder
            .buffer_name("test-1")
            .internal(false)
            .mode(OpenMode::ReadWrite)
            .finalize()
            .unwrap();
        let mut bb = buffer.write();
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
        let mut builder = BufferBuilder::new(BufferKind::File);
        let buffer = builder
            .buffer_name("test-1")
            .internal(false)
            .mode(OpenMode::ReadWrite)
            .finalize()
            .unwrap();
        let mut bb = buffer.write();
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
        let mut builder = BufferBuilder::new(BufferKind::File);
        let buffer = builder
            .buffer_name("test-1")
            .internal(false)
            .mode(OpenMode::ReadWrite)
            .finalize()
            .unwrap();
        let mut bb = buffer.write();
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
