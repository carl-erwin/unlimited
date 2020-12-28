// Copyright (c) Carl-Erwin Griffith

use std::char;

pub mod utf8;

#[derive(Debug, Clone)]
pub enum SyncDirection {
    Backward,
    Forward,
}
pub trait TextCodec {
    fn encode_max_size(&self) -> usize;

    fn decode(&self, direction: SyncDirection, data: &[u8], data_offset: u64)
        -> (char, u64, usize);

    fn encode(&self, codepoint: u32, out: &mut [u8]) -> usize;

    fn is_sync(&self, byte: u8) -> bool;

    // TODO: return Result<u64, need more|invalid offset|...>
    fn sync(&self, direction: SyncDirection, data: &[u8], data_offset: u64) -> Option<u64>;
}



pub fn u32_to_char(codep: u32) -> char {
    unsafe { char::from_u32_unchecked(codep) }
}
