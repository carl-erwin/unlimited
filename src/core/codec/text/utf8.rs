// {
//   Derived from : Bjoern Hoehrmann work
//   Copyright (c) 2008-2010 Bjoern Hoehrmann <bjoern@hoehrmann.de>
//   See http://bjoern.hoehrmann.de/utf-8/decoder/dfa/ for details.

use super::SyncDirection;
use super::TextCodec;

pub const UTF8_ACCEPT: u32 = 0;
pub const UTF8_REJECT: u32 = 12;

const DEBUG: bool = false;

use crate::core::codec::text::u32_to_char;

#[rustfmt::skip]
static UTF8D: &[u8] = &[
  // The first part of the table maps bytes to character classes that
  // to reduce the size of the transition table and create bitmasks.
  0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,  0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,
  0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,  0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,
  0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,  0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,
  0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,  0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,
  1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,  9,9,9,9,9,9,9,9,9,9,9,9,9,9,9,9,
  7,7,7,7,7,7,7,7,7,7,7,7,7,7,7,7,  7,7,7,7,7,7,7,7,7,7,7,7,7,7,7,7,
  8,8,2,2,2,2,2,2,2,2,2,2,2,2,2,2,  2,2,2,2,2,2,2,2,2,2,2,2,2,2,2,2,
  10,3,3,3,3,3,3,3,3,3,3,3,3,4,3,3, 11,6,6,6,5,8,8,8,8,8,8,8,8,8,8,8,

  // The second part is a transition table that maps a combination
  // of a state of the automaton and a character class to a state.
  0,12,24,36,60,96,84,12,12,12,48,72, 12,12,12,12,12,12,12,12,12,12,12,12,
  12, 0,12,12,12,12,12, 0,12, 0,12,12, 12,24,12,12,12,12,12,24,12,24,12,12,
  12,12,12,12,12,12,12,24,12,12,12,12, 12,24,12,12,12,12,12,12,12,24,12,12,
  12,12,12,12,12,12,12,36,12,36,12,12, 12,36,12,12,12,12,12,36,12,36,12,12,
  12,36,12,12,12,12,12,12,12,12,12,12,
];

/*
    state UTF8_ACCEPT => initial state or decoding successful
    state UTF8_REJECT => error
    state other => intermediate states need more inputs
*/
#[inline(always)]
pub fn decode_byte(state: u32, byte: u8, codep: &mut u32) -> u32 {
    let cp_type = u32::from(UTF8D[byte as usize]);

    *codep = if state != UTF8_ACCEPT {
        u32::from(byte & 0x3f) | (*codep << 6)
    } else {
        (0xff >> cp_type) & u32::from(byte)
    };

    u32::from(UTF8D[(256 + state + cp_type) as usize])
}

// } end-of-derived code

// rename is_sync()
#[inline]
pub fn is_codepoint_start(byte: u8) -> bool {
    if byte < 0x80 {
        return true;
    }

    if byte >= 0xC2 && byte <= 0xDF {
        return true;
    }

    if byte >= 0xC2 && byte <= 0xDF {
        return true;
    }

    if byte >= 0xE0 && byte <= 0xEC {
        return true;
    }

    if byte >= 0xED && byte <= 0xEF {
        return true;
    }

    if byte >= 0xF0 && byte <= 0xF4 {
        return true;
    }

    false
}

// naive utf8 encoder
// return 0 on error, or the number of written bytes
// do encode_unchecked and remove test
pub fn encode(codepoint: u32, out: &mut [u8]) -> usize {
    if out.len() < 1 {
        return 0;
    }

    if codepoint < 0x80 {
        out[0] = codepoint as u8;
        return 1;
    }

    if out.len() < 2 {
        return 0;
    }

    if codepoint < 0x800 {
        out[0] = 0xC0 | (codepoint >> 6) as u8;
        out[1] = 0x80 | (codepoint & 0x3F) as u8;
        return 2;
    }

    if out.len() < 3 {
        return 0;
    }

    if codepoint < 0xFFFF {
        out[0] = 0xE0 | ((codepoint >> 12) & 0x3F) as u8;
        out[1] = 0x80 | ((codepoint >> 6) & 0x3F) as u8;
        out[2] = 0x80 | ((codepoint >> 0) & 0x3F) as u8;
        return 3;
    }

    if out.len() < 4 {
        return 0;
    }

    if codepoint < 0x0010_FFFF {
        out[0] = 0xF0 | ((codepoint >> 18) & 0x3F) as u8;
        out[1] = 0xE0 | ((codepoint >> 12) & 0x3F) as u8;
        out[2] = 0x80 | ((codepoint >> 6) & 0x3F) as u8;
        out[3] = 0x80 | ((codepoint >> 0) & 0x3F) as u8;
        return 4;
    }

    0
}

// TODO: rename sync_backward
// TODO: change this with temporary (cp, offset, size) until from_offset
// TODO: rename in sync(BACKWARD, offset) -> offset
// get_previous
fn get_previous_codepoint_start(data: &[u8], from_offset: u64) -> u64 {
    assert!(data.len() >= from_offset as usize);

    //                 cp    size   offset
    let mut cp_info: [(char, usize, u64); 8] = [
        ('\0', 0, 0),
        ('\0', 0, 0),
        ('\0', 0, 0),
        ('\0', 0, 0),
        ('\0', 0, 0),
        ('\0', 0, 0),
        ('\0', 0, 0),
        ('\0', 0, 0),
    ];
    let mut nr_cpinfo = 0;

    if DEBUG {
        dbg_println!("get_previous_codepoint_start from {}", from_offset);
    }

    // rewind up to 4 bytes
    // and decode forward / save offset
    let mut off = from_offset.saturating_sub(4);

    if DEBUG {
        dbg_println!("off = {}", off);
    }

    while off < from_offset {
        let (cp, off2, size) = get_codepoint(data, off);
        if DEBUG {
            dbg_println!(" get_codepoint {} off {} size {}", cp, off2, size);
        }
        cp_info[nr_cpinfo] = (cp, size, off);
        nr_cpinfo += 1;

        off += size as u64;
        if nr_cpinfo == 4 {
            break;
        }
    }

    if nr_cpinfo != 0 {
        cp_info[nr_cpinfo - 1].2
    } else {
        from_offset
    }
}

pub fn get_codepoint(data: &[u8], from_offset: u64) -> (char, u64, usize) {
    let mut v = vec![]; // TODO: move to array
    let mut ctx = Utf8DecodeContext::new();
    ctx.from_offset = from_offset;
    ctx.current_offset = from_offset;
    for val in data.iter().skip(from_offset as usize) {
        utf8_decode_byte_checked(&mut ctx, *val, &mut v);
        if v.len() > 0 {
            // 1 is enough
            break;
        }
    }

    if v.len() == 0 {
        // incomplete/truncated stream ?
        return (u32_to_char(0xfffd), from_offset, 1);
    }

    (v[0].0, from_offset, v[0].2)
}

// TODO: pass codec in env
struct Utf8DecodeContext {
    current_offset: u64,
    from_offset: u64,
    state: u32,
    codep: u32,
    cp_size: usize,
    accum: [u8; 4],
    accum_size: usize,
}

impl Utf8DecodeContext {
    pub fn new() -> Self {
        Utf8DecodeContext {
            current_offset: 0,
            from_offset: 0,
            state: 0,
            codep: 0,
            cp_size: 0,
            accum: [0; 4],
            accum_size: 0,
        }
    }
}

fn utf8_decode_byte_checked(
    ctx: &mut Utf8DecodeContext,
    val: u8,
    out: &mut Vec<(char, u64, usize)>,
) {
    ctx.accum[ctx.cp_size] = val;
    ctx.accum_size += 1;
    loop {
        ctx.state = decode_byte(ctx.state, ctx.accum[ctx.cp_size], &mut ctx.codep);
        ctx.cp_size += 1;

        if DEBUG {
            dbg_println!("utf8 decode byte  '0x{:x}'", ctx.accum[ctx.cp_size - 1]);
            dbg_println!(
                "utf8 ACCUM {:x?}' accum_size {} cp_size = {}",
                ctx.accum,
                ctx.accum_size,
                ctx.cp_size
            );
        }

        match ctx.state {
            UTF8_ACCEPT => {
                if DEBUG {
                    dbg_println!(
                ">>> utf8 decode cp OK current_offset = {:?} from_offset = {:?} ctx.cp_size {} cp:u32 {}",
                ctx.current_offset,
                ctx.from_offset,
                ctx.cp_size, ctx.codep);
                }

                let io = (u32_to_char(ctx.codep), ctx.from_offset, ctx.cp_size);
                out.push(io);

                ctx.from_offset = ctx.from_offset + ctx.cp_size as u64;

                // restart
                ctx.codep = 0;
                ctx.cp_size = 0;
                ctx.state = 0;

                ctx.accum_size = 0;

                // drop accum debug only  ?
                ctx.accum[0] = 0;
                ctx.accum[1] = 0;
                ctx.accum[2] = 0;
                ctx.accum[3] = 0;
                break;
            }

            UTF8_REJECT => {
                if DEBUG {
                    dbg_println!(
                        "utf8 decode cp ERROR current_offset = {:?} from_offset = {:?} cp_size {}",
                        ctx.current_offset,
                        ctx.from_offset,
                        ctx.cp_size
                    );
                }

                // decode error : invalid sequence
                let io = (u32_to_char(0xfffd), ctx.from_offset, 1);
                out.push(io);

                // restart @ next byte
                ctx.from_offset = ctx.from_offset + 1;

                // restart
                ctx.codep = 0;
                ctx.cp_size = 0;
                ctx.state = 0; // reset state on error

                // shift accum
                ctx.accum[0] = ctx.accum[1];
                ctx.accum[1] = ctx.accum[2];
                ctx.accum[2] = ctx.accum[3];
                ctx.accum[3] = 0;
                ctx.accum_size -= 1;
                if ctx.accum_size == 0 {
                    break;
                }
            }

            _ => {
                if DEBUG {
                    /* need more data */
                    dbg_println!(
                "utf8 decoder need more data , ctx.current_offset {:?} ctx.offset = {:?} ctx.cp_size {}",
                ctx.current_offset,
                ctx.from_offset,
                ctx.cp_size
                );
                }

                break;
            }
        }
    }

    ctx.current_offset = ctx.current_offset + 1; // ext ?
}

///////////////////////////////////////////////////////////////////////////////////////////////////

#[derive(Debug)]
pub struct Utf8Codec {}

impl Utf8Codec {
    pub fn new() -> Self {
        Utf8Codec {}
    }
}

impl TextCodec for Utf8Codec {
    fn encode_max_size(&self) -> usize {
        4
    }

    fn decode(
        &self,
        direction: SyncDirection,
        data: &[u8],
        data_offset: u64,
    ) -> (char, u64, usize) {
        match direction {
            SyncDirection::Backward => {
                let offset = get_previous_codepoint_start(data, data_offset);
                let ret = get_codepoint(data, offset);
                ret
            }

            SyncDirection::Forward => get_codepoint(data, data_offset),
        }
    }

    fn encode(&self, codepoint: u32, out: &mut [u8]) -> usize {
        encode(codepoint, out)
    }

    fn is_sync(&self, byte: u8) -> bool {
        is_codepoint_start(byte)
    }

    // TODO: return Result<u64, need more|invalid offset|...>
    fn sync(&self, direction: SyncDirection, data: &[u8], data_offset: u64) -> Option<u64> {
        let data_offset = data_offset as usize;

        let data_len = data.len();
        if data_offset > data_len {
            return None;
        }

        if !self.is_sync(data[data_offset]) {
            return Some(data_offset as u64);
        }

        match direction {
            SyncDirection::Backward => {
                for i in (0..data_offset).rev() {
                    if self.is_sync(data[i]) {
                        return Some(i as u64);
                    }
                }
                return None;
            }

            SyncDirection::Forward => {
                for i in data_offset..data_len {
                    if self.is_sync(data[i]) {
                        return Some(i as u64);
                    }
                }
                return None;
            }
        }
    }
}

///////////////////////////////////////////////////////////////////////////////////////////////////

#[test]
fn test_codec_encode() {
    let expect_cp: [u8; 4] = [0xe2, 0x82, 0xac, 0x00];
    let mut mut_cp: [u8; 4] = [0x00, 0x00, 0x00, 0x00];

    let n = encode(0x20ac as u32, &mut mut_cp);
    assert_eq!(n, 3);
    assert_eq!(mut_cp, expect_cp);
}

#[test]
fn test_codec_decode() {
    use crate::core::codec::text::u32_to_char;

    let mut state: u32 = 0;
    let mut codep: u32 = 0;

    let sequence: [u8; 4] = [0xe2, 0x82, 0xac, 0x00];
    for b in &sequence {
        println!("decode byte '{:x}'", *b);
        state = decode_byte(state, *b, &mut codep);
        match state {
            UTF8_ACCEPT => {
                break;
            }
            UTF8_REJECT => {
                println!("invalid utf8 sequence");
                break;
            }
            _ => {
                continue;
            }
        }
    }

    println!("decoded codepoint value {:x}", codep);
    let c = u32_to_char(codep);
    println!("decoded codepoint char {}", c);

    assert_eq!(codep, 0x20ac);
}

#[test]
fn test2_codec_decode() {
    let data: [u8; 27] = [
        0xe2, 0x82, 0xac, 0xe2, 0x82, 0x61, 0x0a, 0x82, 0xac, 0xe2, 0x82, 0x61, 0x0a, 0xac, 0xe2,
        0x82, 0x61, 0x0a, 0xe2, 0x82, 0x61, 0x0a, 0x82, 0x61, 0x0a, 0x61, 0x0a,
    ];

    let mut state: u32 = 0;
    let mut codep: u32 = 0;

    for b in &data {
        println!("decode byte '{:x}'", *b);
        state = decode_byte(state, *b, &mut codep);
        println!("state  '{}'", state);
        match state {
            UTF8_ACCEPT => {
                println!("decoded cp = '{:?}'", codep);
                state = 0;
                codep = 0;
            }
            UTF8_REJECT => {
                println!("invalid utf8 sequence, restart");
                state = 0;
                codep = 0;
                state = decode_byte(state, *b, &mut codep);
                match state {
                    UTF8_ACCEPT => {
                        println!("decoded cp = '{:?}'", codep);
                        state = 0;
                        codep = 0;
                    }
                    UTF8_REJECT => {
                        println!("invalid utf8 sequence, restart");
                        state = 0;
                        codep = 0;
                    }
                    _ => {}
                }
            }

            _ => {
                continue;
            }
        }
    }
}

#[test]
fn test_backward_decode() {
    let data: [u8; 24] = [
        0xe2, 0x82, 0xac, 0xe2, 0x82, 0xac, 0xe2, 0x82, 0xac, 0xe2, 0x82, 0xac, 0xe2, 0x82, 0xac,
        0xe2, 0x82, 0xac, 0xe2, 0x82, 0xac, 0xe2, 0x82, 0xac,
    ];

    let mut start_offset: u64 = 6;
    println!(
        "start_offset {} byte is '{:x}'",
        start_offset, data[start_offset as usize] as u32
    );

    // TODO: transform all return code into Error
    // for example start_offset can be greater than data.len()
    // we could return something like Error(invalid offset)
    let off = get_previous_codepoint_start(&data, start_offset as u64);
    println!(
        "start_offset({}) - off({}) = {}",
        start_offset,
        off,
        start_offset - off
    );
    let delta = start_offset - off;
    start_offset -= delta;

    println!("new start_offset({})", start_offset);
}
