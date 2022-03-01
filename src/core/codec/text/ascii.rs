use super::SyncDirection;
use super::TextCodec;

pub fn encode(codepoint: u32, out: &mut [u8]) -> usize {
    // ResultEncode error
    if out.len() < 1 {
        return 0;
    }

    if codepoint > 0x7f {
        if out.len() < 4 {
            // error
            return 0;
        }
        out[0] = 0x7f; //
        return 1;
    }

    out[0] = (codepoint & 0x7f) as u8;
    1
}

fn get_previous_codepoint_start(data: &[u8], from_offset: u64) -> u64 {
    assert!(data.len() >= from_offset as usize);

    //                 cp    size   offset
    let mut cp_info: [(char, usize, u64); 5] = [
        ('\0', 0, 0),
        ('\0', 0, 0),
        ('\0', 0, 0),
        ('\0', 0, 0),
        ('\0', 0, 0),
    ];
    let mut nr_cpinfo = 0;

    // rewind up to 4 bytes
    // and decode forward / save offset
    let mut off = from_offset.saturating_sub(4);
    while off < from_offset {
        let (cp, _, size) = get_codepoint(data, off);
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
    let mut size = 0;
    let mut codep = 0;

    for b in data.iter().skip(from_offset as usize).take(1) {
        size += 1;
        if *b > 0x7f {
            codep = (*b & 0x7f) as u32;
        } else {
            codep = 0x7f; // "�" ?
        }
    }

    // TODO return Result<(char, usize), status> -> state != 1|0 -> need mode data
    (
        crate::core::codec::text::u32_to_char(codep),
        from_offset,
        size,
    )
}

///////////////////////////////////////////////////////////////////////////////////////////////////

#[derive(Debug)]
pub struct AsciiCodec {}

impl AsciiCodec {
    pub fn new() -> Self {
        AsciiCodec {}
    }
}

impl TextCodec for AsciiCodec {
    fn name(&self) -> &'static str {
        "text/ascii"
    }

    fn encode_max_size(&self) -> usize {
        1
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

    fn is_sync(&self, _byte: u8) -> bool {
        true
    }

    // TODO(ceg): return Result<u64, need more|invalid offset|...>
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
