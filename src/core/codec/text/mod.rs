// mode declaration
pub mod ascii;
pub mod utf8;

// std
use std::char;

// ext

// crate

#[derive(Debug, Clone)]
pub enum SyncDirection {
    Backward,
    Forward,
}

#[derive(Debug, Clone)]
pub enum DecodeResult {
    InvalidInput,
    NeedMoreInput,
    ValidCodepoint { cp: char, offset: u64, size: usize },
}

#[derive(Debug, Clone)]
pub enum EncodeResult {
    InvalidInput,
    NeedMoreOutput,
    Value { bytes: [u8; 4], size: usize },
}

// TODO(ceg): add new type : editor:: type Offset = u64;

// TODO(ceg): add incremental decoder, state in impl
// fn decode_byte(&self, direction: SyncDirection, data: &[u8], data_offset: u64) -> Option<(char, Offset, usize)>
//

pub trait TextCodec {
    fn encode_max_size(&self) -> usize;

    // fn decode_byte(&self, direction: SyncDirection, data: u8, data_offset: u64) -> DecodeResult;

    fn decode(&self, direction: SyncDirection, data: &[u8], data_offset: u64)
        -> (char, u64, usize);

    fn encode(&self, codepoint: u32, out: &mut [u8]) -> usize;

    fn is_sync(&self, byte: u8) -> bool;

    // TODO(ceg): return Result<u64, need more|invalid offset|...>
    fn sync(&self, direction: SyncDirection, data: &[u8], data_offset: u64) -> Option<u64>;
}

#[inline(always)]
pub fn u32_to_char(codep: u32) -> char {
    unsafe { char::from_u32_unchecked(codep) }
}
