// Copyright (c) Carl-Erwin Griffith
//
// Permission is hereby granted, free of charge, to any
// person obtaining a copy of this software and associated
// documentation files (the "Software"), to deal in the
// Software without restriction, including without
// limitation the rights to use, copy, modify, merge,
// publish, distribute, sublicense, and/or sell copies of
// the Software, and to permit persons to whom the Software
// is furnished to do so, subject to the following
// conditions:
//
// The above copyright notice and this permission notice
// shall be included in all copies or substantial portions
// of the Software.
//
// THE SOFTWARE IS PROVIDED "AS IS", WITHOUT WARRANTY OF
// ANY KIND, EXPRESS OR IMPLIED, INCLUDING BUT NOT LIMITED
// TO THE WARRANTIES OF MERCHANTABILITY, FITNESS FOR A
// PARTICULAR PURPOSE AND NONINFRINGEMENT. IN NO EVENT
// SHALL THE AUTHORS OR COPYRIGHT HOLDERS BE LIABLE FOR ANY
// CLAIM, DAMAGES OR OTHER LIABILITY, WHETHER IN AN ACTION
// OF CONTRACT, TORT OR OTHERWISE, ARISING FROM, OUT OF OR
// IN CONNECTION WITH THE SOFTWARE OR THE USE OR OTHER

use crate::core::codec::text::u32_to_char;
use crate::core::codec::text::utf8;

use crate::core::codepointinfo::CodepointInfo;
use crate::core::mark::Mark;
use crate::core::screen::Screen;
use crate::core::view::View;

pub struct Filter {}

pub struct FilterContext {}

// content_type == unicode
#[derive(Debug, Clone)]
pub enum FilterData {
    Byte {
        val: u8,
    },

    Unicode {
        cp: u32,
        real_cp: u32,
        cp_index: u64, // be carefull used const u64 invalid_cp_index
        fragment_flag: bool,
        fragment_count: u32,
    },

    // codec_change
    CodecInfo {
        codec_id: u32,
        codec_context_id: u64, //
    },
}

#[derive(Debug, Clone)]
pub struct FilterIoData {
    // general info
    is_valid: bool,
    end_of_pipe: bool, // skip
    quit: bool,        // close pipeline
    is_selected: bool,

    offset: u64,
    size: usize,

    data: FilterData,
    // TODO: add style infos ?
}

/// This function computes start/end of lines between start_offset end_offset.<br/>
/// It (will) run the configured filters/plugins.<br/>
/// using the build_screen_layout function until end_offset is reached.<br/>
pub fn get_lines_offsets<'a>(
    view: &View<'a>,
    start_offset: u64,
    end_offset: u64,
    screen_width: usize,
    screen_height: usize,
) -> Vec<(u64, u64)> {
    let mut v = Vec::<(u64, u64)>::new();

    let mut m = Mark::new(start_offset);

    let doc = view.document.as_ref().unwrap().borrow_mut();

    let screen_width = ::std::cmp::max(1, screen_width);
    let screen_height = ::std::cmp::max(4, screen_height);

    // get beginning of the line @offset
    m.move_to_beginning_of_line(&doc.buffer, utf8::get_prev_codepoint);

    // and build tmp screens until end_offset if found
    let mut screen = Screen::new(screen_width, screen_height);

    let max_offset = doc.buffer.size as u64;
    let max_size = (screen_width * screen_height * 4) as usize;

    loop {
        // fill screen
        let mut data = vec![];
        doc.buffer.read(m.offset, max_size, &mut data);

        let _ = build_screen_layout(&data, m.offset, max_offset, &mut screen);

        if screen.nb_push == 0 {
            return v;
        }

        // push lines offsets
        // FIXME: find a better way to iterate over the used lines
        for i in 0..screen.current_line_index {
            if !v.is_empty() && i == 0 {
                // do not push line range twice
                continue;
            }

            let s = screen.line[i].get_first_cpi().unwrap().offset;
            let e = screen.line[i].get_last_cpi().unwrap().offset;

            v.push((s, e));

            if s >= end_offset || e == max_offset {
                return v;
            }
        }

        // eof reached ?
        // FIXME: the api is not yet READY
        // we must find a way to cover all filled lines
        if screen.current_line_index < screen.height {
            let s = screen.line[screen.current_line_index]
                .get_first_cpi()
                .unwrap()
                .offset;

            let e = screen.line[screen.current_line_index]
                .get_last_cpi()
                .unwrap()
                .offset;
            v.push((s, e));
            return v;
        }

        // TODO: activate only in debug builds
        if 0 == 1 {
            match screen.find_cpi_by_offset(m.offset) {
                (Some(cpi), x, y) => {
                    assert_eq!(x, 0);
                    assert_eq!(y, 0);
                    assert_eq!(cpi.offset, m.offset);
                }
                _ => panic!("implementation error"),
            }
        }

        if let Some(l) = screen.get_last_used_line() {
            if let Some(cpi) = l.get_first_cpi() {
                m.offset = cpi.offset; // update next screen start
            }
        }

        screen.clear(); // prepare next screen
    }
}

// Trait filter context
fn _utf8_filter(
    ctx: &mut FilterContext,
    filter_in: &mut Vec<FilterIoData>,
    filters_out: &mut Vec<FilterIoData>,
) -> u32 {
    0
}

#[derive(Debug, Clone)]
struct _LayoutPlugin {
    plugin_id: u32,
    context_id: u32,
}

// first internal pass : convert raw bytes to vec of FilterIoData::FilterData::Byte
fn layout_filter_prepare_raw_data(
    screen: &Screen,
    data: &[u8],
    base_offset: u64,
    max_offset: u64,
) -> Vec<FilterIoData> {
    let mut data_vec: Vec<FilterIoData> = Vec::with_capacity(screen.width * screen.height);

    for (count, b) in data.iter().enumerate() {
        data_vec.push(FilterIoData {
            is_valid: true,
            end_of_pipe: false, // skip
            quit: false,        // close pipeline
            is_selected: false,
            offset: base_offset + count as u64,
            size: 1,
            data: FilterData::Byte { val: *b },
        });
    }

    // eof handling
    if base_offset + data.len() as u64 == max_offset {
        data_vec.push(FilterIoData {
            is_valid: true,
            end_of_pipe: true, // skip
            quit: false,       // close pipeline
            is_selected: true,
            offset: base_offset + data.len() as u64,
            size: 1,
            data: FilterData::Byte { val: b'$' },
        });
    }

    data_vec
}

fn layout_filter_utf8(filter_in: &Vec<FilterIoData>, filters_out: &mut Vec<FilterIoData>) -> bool {
    if filter_in.is_empty() {
        *filters_out = vec![];
        return true;
    }

    let in_len = filter_in.len();

    // start offset
    let mut from_offset = filter_in[0].offset;
    let last_off = filter_in[in_len - 1].offset;

    let mut state = 0;
    let mut codep = 0;
    let mut cp_size = 0;
    let mut cp_index = 0;

    for d in filter_in {
        match d.data {
            FilterData::Byte { val } => {
                cp_size += 1;
                state = utf8::decode_byte(state, val, &mut codep);
                match state {
                    utf8::UTF8_ACCEPT => {
                        let io = FilterIoData {
                            // general info
                            is_valid: true,
                            end_of_pipe: false, // skip
                            quit: false,        // close pipeline
                            is_selected: false,
                            offset: from_offset,
                            size: cp_size,
                            data: FilterData::Unicode {
                                cp: codep,
                                real_cp: codep,
                                cp_index, // be carefull used const u64 invalid_cp_index
                                fragment_flag: false,
                                fragment_count: 0,
                            },
                        };

                        filters_out.push(io);

                        cp_index += 1;
                        from_offset += cp_size as u64;

                        codep = 0;
                        cp_size = 0;
                    }

                    utf8::UTF8_REJECT => {
                        // decode error : invalid sequence
                        let io = FilterIoData {
                            // general info
                            is_valid: true,
                            end_of_pipe: false, // skip
                            quit: false,        // close pipeline
                            is_selected: false,
                            offset: from_offset,
                            size: 1,
                            data: FilterData::Unicode {
                                cp: 0xfffd,
                                real_cp: 0xfffd,
                                cp_index, // be carefull used const u64 invalid_cp_index
                                fragment_flag: false,
                                fragment_count: 0,
                            },
                        };
                        filters_out.push(io);

                        // restart @ next byte
                        cp_index += 1;
                        from_offset += 1 as u64;

                        codep = 0;
                        cp_size = 0;
                    }
                    _ => { /* need more data */ }
                }
            }

            _ => { /* unexpected */ }
        }
    }

    true
}

fn layout_filter_tabulation(
    filter_in: &Vec<FilterIoData>,
    filter_out: &mut Vec<FilterIoData>,
) -> bool {
    for i in filter_in.iter() {
        filter_out.push(i.clone());
    }

    true
}

fn layout_fill_screen(filter_in: &Vec<FilterIoData>, max_offset: u64, screen: &mut Screen) -> bool {
    if filter_in.is_empty() {
        return false;
    }

    let in_len = filter_in.len();

    // start offset
    let base_offset = filter_in[0].offset;
    let last_off = filter_in[in_len - 1].offset;

    screen.first_offset = base_offset;
    let mut last_pushed_offset = base_offset;

    for io in filter_in.iter() {
        match &*io {
            FilterIoData {
                is_valid,
                end_of_pipe,
                quit,
                is_selected,
                offset,
                size,
                data:
                    FilterData::Unicode {
                        cp,
                        real_cp,
                        cp_index,
                        fragment_flag,
                        fragment_count,
                    },
            } => {
                let _ = screen.push(filter_codepoint(u32_to_char(*cp), *offset));
                last_pushed_offset = *offset;
            }
            _ => {}
        }
    }

    screen.doc_max_offset = max_offset;
    screen.last_offset = last_pushed_offset;

    true
}

/// This function can be considered as the core of the editor.<br/>
/// It will run the configured filters until the screen is filled or eof is reached.<br/>
/// TODO: pass list of filter function to be applied
/// 0 - allocate context for each configurred plugin
/// 1 - utf8 || hexa
/// 2 - tabulation
pub fn build_screen_layout(
    data: &[u8],
    base_offset: u64,
    max_offset: u64,
    mut screen: &mut Screen,
) -> u64 {
    let mut filter_in = layout_filter_prepare_raw_data(&screen, data, base_offset, max_offset);
    let mut filter_out: Vec<FilterIoData> = Vec::with_capacity(filter_in.len());

    let ret = layout_filter_utf8(&filter_in, &mut filter_out);
    filter_in = filter_out;
    let mut filter_out: Vec<FilterIoData> = Vec::with_capacity(filter_in.len());

    let ret = layout_filter_tabulation(&filter_in, &mut filter_out);
    filter_in = filter_out;

    // last pass
    let ret = layout_fill_screen(&filter_in, max_offset, &mut screen);

    screen.last_offset
}

/// This function can be considered as the core of the editor.<br/>
/// It will run the configured filters until the screen is filled or eof is reached.<br/>
/// TODO: pass list of filter function to be applied
/// 0 - allocate context for each configurred plugin
/// 1 - utf8 || hexa
/// 2 - tabulation
pub fn build_screen_layout_old(
    data: &[u8],
    base_offset: u64,
    max_offset: u64,
    mut screen: &mut Screen,
) -> u64 {
    let max_cpi = screen.width * screen.height;

    // utf8
    let (vec, _) = decode_slice_to_vec(data, base_offset, max_offset, max_cpi);

    screen.first_offset = base_offset;
    let mut last_pushed_offset = base_offset;
    let mut prev_cp = ' ';
    let mut column_count = 0;
    for cpi in &vec {
        if prev_cp == '\r' || prev_cp == '\n' {
            column_count = 0;
        }

        let (ok, _) = match (prev_cp, cpi.cp) {
            // TODO: handle \r\n
            (_, '\t') => {
                prev_cp = cpi.cp;
                let mut filtered_cp = *cpi;
                filtered_cp.displayed_cp = ' ';

                let mut last = (false, 0);

                let tab_size = 8;

                let padding = tab_size - (column_count % tab_size);

                for _ in 0..padding {
                    last = screen.push(filtered_cp);
                    // TODO: how to handle errors ?
                    column_count += 1;
                }
                last
            }

            _ => {
                prev_cp = cpi.cp;

                let last = screen.push(*cpi);
                // TODO: how to handle errors ?
                column_count += 1;
                last
            }
        };

        if !ok {
            break;
        }

        last_pushed_offset = cpi.offset;
    }

    screen.doc_max_offset = max_offset;
    screen.last_offset = last_pushed_offset;
    last_pushed_offset
}

fn decode_slice_to_vec(
    data: &[u8],
    base_offset: u64,
    max_offset: u64,
    max_cpi: usize,
) -> (Vec<CodepointInfo>, u64) {
    let mut vec = Vec::with_capacity(max_cpi);

    let mut off: u64 = 0;
    let last_off = data.len() as u64;

    while off != last_off {
        let (cp, _, size) = utf8::get_codepoint(data, off);
        vec.push(filter_codepoint(cp, base_offset + off));
        off += size as u64;
        if vec.len() == max_cpi {
            break;
        }
    }

    // eof handling
    if base_offset + last_off == max_offset {
        vec.push(CodepointInfo {
            cp: ' ',
            displayed_cp: '$',
            offset: base_offset + last_off,
            is_selected: true,
        });
    }

    (vec, base_offset + off)
}

//
fn _raw_slice_to_hex_vec(
    data: &[u8],
    base_offset: u64,
    max_offset: u64,
    max_cpi: usize,
) -> (Vec<CodepointInfo>, u64) {
    let mut vec = Vec::with_capacity(max_cpi);

    let mut off: u64 = base_offset;
    let last_off = data.len() as u64;

    let hexchars: [char; 16] = [
        '0', '1', '2', '3', '4', '5', '6', '7', '8', '9', 'a', 'b', 'c', 'd', 'e', 'f',
    ];

    while off < last_off {
        let mut width = 0;
        for i in 0..16 {
            if off + i >= last_off {
                break;
            }

            let hi: usize = (data[(off + i) as usize] >> 4) as usize;
            let low: usize = (data[(off + i) as usize] & 0x0f) as usize;

            let cp = hexchars[hi];
            vec.push(filter_codepoint(cp, off + i));
            let cp = hexchars[low];
            vec.push(filter_codepoint(cp, off + i));
            vec.push(filter_codepoint(' ', off + i));

            if vec.len() == max_cpi {
                break;
            }
            width += 1;
        }

        if 0 == 1 {
            vec.push(filter_codepoint('|', off + width));
            vec.push(filter_codepoint(' ', off + width));

            for i in 0..16 {
                if off + i >= last_off {
                    break;
                }

                let c: char = data[(off + i) as usize] as char;
                vec.push(filter_codepoint(c, off + i));
                if vec.len() == max_cpi {
                    break;
                }
            }
        }

        vec.push(filter_codepoint('\n', off));
        off += width;
    }

    // eof handling
    if last_off == max_offset {
        vec.push(CodepointInfo {
            cp: ' ',
            displayed_cp: '$',
            offset: last_off,
            is_selected: true,
        });
    }

    (vec, off)
}

// TODO return array of CodePointInfo  0x7f -> <ESC>
pub fn filter_codepoint(c: char, offset: u64) -> CodepointInfo {
    let displayed_cp: char = match c {
        '\r' | '\n' | '\t' => ' ',

        _ if c < ' ' => '�',

        _ if c == 0x7f as char => '�',

        _ => c,
    };

    CodepointInfo {
        cp: c,
        displayed_cp,
        offset,
        is_selected: false,
    }
}
