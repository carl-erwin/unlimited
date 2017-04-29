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
                print!("invalid utf8 sequence");
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
