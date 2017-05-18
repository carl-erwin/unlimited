// {
//   Derived from : Bjoern Hoehrmann work
//   Copyright (c) 2008-2009 Bjoern Hoehrmann <bjoern@hoehrmann.de>
//   See http://bjoern.hoehrmann.de/utf-8/decoder/dfa/ for details.


pub const UTF8_ACCEPT: u32 = 0;
pub const UTF8_REJECT: u32 = 1;


static UTF8D: &'static [u8] = &[
  0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0, // 00..1f
  0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0, // 20..3f
  0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0, // 40..5f
  0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0, // 60..7f
  1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,9,9,9,9,9,9,9,9,9,9,9,9,9,9,9,9, // 80..9f
  7,7,7,7,7,7,7,7,7,7,7,7,7,7,7,7,7,7,7,7,7,7,7,7,7,7,7,7,7,7,7,7, // a0..bf
  8,8,2,2,2,2,2,2,2,2,2,2,2,2,2,2,2,2,2,2,2,2,2,2,2,2,2,2,2,2,2,2, // c0..df
  0xa,0x3,0x3,0x3,0x3,0x3,0x3,0x3,0x3,0x3,0x3,0x3,0x3,0x4,0x3,0x3, // e0..ef
  0xb,0x6,0x6,0x6,0x5,0x8,0x8,0x8,0x8,0x8,0x8,0x8,0x8,0x8,0x8,0x8, // f0..ff
  0x0,0x1,0x2,0x3,0x5,0x8,0x7,0x1,0x1,0x1,0x4,0x6,0x1,0x1,0x1,0x1, // s0..s0
  1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,0,1,1,1,1,1,0,1,0,1,1,1,1,1,1, // s1..s2
  1,2,1,1,1,1,1,2,1,2,1,1,1,1,1,1,1,1,1,1,1,1,1,2,1,1,1,1,1,1,1,1, // s3..s4
  1,2,1,1,1,1,1,1,1,2,1,1,1,1,1,1,1,1,1,1,1,1,1,3,1,3,1,1,1,1,1,1, // s5..s6
  1,3,1,1,1,1,1,3,1,3,1,1,1,1,1,1,1,3,1,1,1,1,1,1,1,1,1,1,1,1,1,1, // s7..s8
];


/*
    state UTF8_ACCEPT => initial state or decoding successful
    state UTF8_REJECT => error
    state other => intermediate states need more inputs
*/
#[inline]
pub fn decode_byte(state: &mut u32, byte: u8, codep: &mut u32) -> u32 {

    let cp_type = UTF8D[byte as usize] as u32;

    *codep = if *state != UTF8_ACCEPT {
        (byte & 0x3f) as u32 | (*codep << 6)
    } else {
        (0xff >> cp_type) as u32 & (byte as u32)
    };

    *state = UTF8D[256 + (*state * 16) as usize + cp_type as usize] as u32;
    *state
}


// }


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



// return 0 on error, or the number of written bytes
pub fn encode(codepoint: u32, out: &mut [u8; 4]) -> usize {
    if codepoint < 0x80 {
        out[0] = (codepoint & 0x7F) as u8;
        return 1;
    }

    if codepoint < 0x800 {
        out[0] = 0xC0 | (codepoint >> 6) as u8;
        out[1] = 0x80 | (codepoint & 0x3F) as u8;
        return 2;
    }

    if codepoint < 0xFFFF {
        out[0] = 0xE0 | (codepoint >> 12) as u8;
        out[1] = 0x80 | (codepoint >> 6) as u8;
        out[2] = 0x80 | (codepoint & 0x3F) as u8;
        return 3;
    }

    if codepoint < 0x10FFFF {
        out[0] = 0xF0 | (codepoint >> 18) as u8;
        out[1] = 0x80 | (codepoint >> 12) as u8;
        out[2] = 0x80 | (codepoint >> 6) as u8;
        out[3] = 0x80 | (codepoint & 0x3F) as u8;
        return 4;
    }

    0
}


// TODO: change this with temporary (cp, offset, size) until from_offset
pub fn get_previous_codepoint_start(data: &[u8], from_offset: u64) -> u64 {

    //                 cp    size   offset
    let mut cp_info: [(char, usize, u64); 8] = [('\0', 0, 0),
                                                ('\0', 0, 0),
                                                ('\0', 0, 0),
                                                ('\0', 0, 0),
                                                ('\0', 0, 0),
                                                ('\0', 0, 0),
                                                ('\0', 0, 0),
                                                ('\0', 0, 0)];
    let mut nr_cpinfo = 0;

    // rewind upto 4 bytes
    // and decode forward / save offset
    let mut off = if from_offset > 4 { from_offset - 4 } else { 0 };
    while off < from_offset {
        let (cp, _, size) = get_codepoint(data, off);

        cp_info[nr_cpinfo] = (cp, size, off);
        nr_cpinfo += 1;

        off += size as u64;
    }

    if nr_cpinfo != 0 {
        cp_info[nr_cpinfo - 1].2
    } else {
        from_offset
    }
}

pub fn get_next_codepoint_start(data: &[u8], from_offset: u64) -> u64 {
    let (_, _, size) = get_codepoint(data, from_offset);
    from_offset + size as u64
}


pub fn get_codepoint(data: &[u8], from_offset: u64) -> (char, u64, usize) {

    let mut state = 0;
    let mut codep = 0;
    let mut size = 0;

    for off in from_offset as usize..data.len() {

        let b = data[off];

        size += 1;
        state = decode_byte(&mut state, b, &mut codep);
        match state {
            0 => {
                break;
            }
            1 => {
                // decode error : invalid sequence
                codep = 0xfffd;
                size = 1; // force restart @ next byte
                break;
            }
            _ => {}
        }
    }

    // TODO return Result<(char, usize), status> -> state != 1|0 -> need mode data
    (::core::codec::text::u32_to_char(codep), from_offset, size)
}

pub fn get_prev_codepoint(data: &[u8], from_offset: u64) -> (char, u64, usize) {
    let offset = get_previous_codepoint_start(data, from_offset);
    get_codepoint(data, offset)
}




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

    use core::codec::text::u32_to_char;

    let mut state: u32 = 0;
    let mut codep: u32 = 0;

    let sequence: [u8; 4] = [0xe2, 0x82, 0xac, 0x00];
    for b in &sequence {
        println!("decode byte '{:x}'", *b);
        state = decode_byte(&mut state, *b, &mut codep);
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

    let data: [u8; 27] = [0xe2, 0x82, 0xac, 0xe2, 0x82, 0x61, 0x0a, 0x82, 0xac, 0xe2, 0x82, 0x61,
                          0x0a, 0xac, 0xe2, 0x82, 0x61, 0x0a, 0xe2, 0x82, 0x61, 0x0a, 0x82, 0x61,
                          0x0a, 0x61, 0x0a];

    let mut state: u32 = 0;
    let mut codep: u32 = 0;

    for b in &data {
        println!("decode byte '{:x}'", *b);
        state = decode_byte(&mut state, *b, &mut codep);
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
                state = decode_byte(&mut state, *b, &mut codep);
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
