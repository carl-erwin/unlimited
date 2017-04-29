use std::char;

pub mod utf8;


pub fn u32_to_char(codep: u32) -> char {
    unsafe { char::from_u32_unchecked(codep) }
}
