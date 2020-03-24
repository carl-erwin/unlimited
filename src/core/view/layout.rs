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
pub enum FilterData<'a> {
    ByteArray {
        array: &'a [u8],
    },

    Byte {
        val: u8,
    },

    Unicode {
        cp: u32,
        real_cp: u32,
        cp_index: u64, // be careful used const u64 invalid_cp_index
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
pub struct FilterIoData<'a> {
    // general info
    is_valid: bool,
    end_of_pipe: bool, // skip
    quit: bool,        // close pipeline

    is_selected: bool,
    color: (u8, u8, u8),

    offset: u64,
    size: usize,

    data: FilterData<'a>,
    // TODO: add style infos ?
}

impl<'a> FilterIoData<'a> {
    pub fn replace_codepoint(io: &FilterIoData<'a>, new_cp: char) -> FilterIoData<'a> {
        if let &FilterIoData {
            // general info
            is_valid,
            end_of_pipe, // skip
            quit,        // close pipeline
            is_selected,
            color,
            offset: from_offset,
            size: cp_size,
            data:
                FilterData::Unicode {
                    real_cp,
                    cp_index, // be careful used const u64 invalid_cp_index
                    fragment_flag,
                    fragment_count,
                    ..
                },
        } = io
        {
            return FilterIoData {
                // general info
                is_valid,
                end_of_pipe, // skip
                quit,        // close pipeline
                is_selected,
                offset: from_offset,
                color,
                size: cp_size,
                data: FilterData::Unicode {
                    cp: new_cp as u32,
                    real_cp,
                    cp_index, // be careful used const u64 invalid_cp_index
                    fragment_flag,
                    fragment_count,
                },
            };
        }

        io.clone()
    }
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
    let max_size = (screen_width * screen_height * 2) as usize;

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
        if screen.current_line_index < screen.height() {
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

#[derive(Debug, Clone)]
struct _LayoutPlugin {
    plugin_id: u32,
    context_id: u32,
}

// first internal pass : convert raw bytes to vec of FilterIoData::FilterData::ByteArray / Byte
fn layout_filter_prepare_raw_data<'a>(
    screen: &Screen,
    data: &'a [u8],
    base_offset: u64,
    max_offset: u64,
) -> Vec<FilterIoData<'a>> {
    let mut data_vec: Vec<FilterIoData<'a>> = Vec::with_capacity(screen.width() * screen.height());

    data_vec.push(FilterIoData {
        is_valid: true,
        end_of_pipe: false, // skip
        quit: false,        // close pipeline
        is_selected: false,
        color: CodepointInfo::default_color(),
        offset: base_offset,
        size: 1,
        data: FilterData::ByteArray { array: data },
    });

    /*
     as byte
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
    */

    // eof handling
    if base_offset + data.len() as u64 == max_offset {
        data_vec.push(FilterIoData {
            is_valid: true,
            end_of_pipe: false, // skip
            quit: false,        // close pipeline
            is_selected: true,
            color: CodepointInfo::default_color(),
            offset: base_offset + data.len() as u64,
            size: 1,
            data: FilterData::Byte { val: b' ' },
        });
    }

    data_vec
}

struct Utf8FilterCtx<'a, 'b> {
    from_offset: u64,
    state: u32,
    codep: u32,
    cp_size: usize,
    cp_index: u64,
    filter_out: &'a mut Vec<FilterIoData<'b>>,
}

fn filter_utf8_byte<'a, 'b>(ctx: &mut Utf8FilterCtx<'a, 'b>) {
    match ctx.state {
        utf8::UTF8_ACCEPT => {
            let io = FilterIoData {
                // general info
                is_valid: true,
                end_of_pipe: false, // skip
                quit: false,        // close pipeline
                is_selected: false,
                color: CodepointInfo::default_color(),
                offset: ctx.from_offset,
                size: ctx.cp_size,
                data: FilterData::Unicode {
                    cp: ctx.codep,
                    real_cp: ctx.codep,
                    cp_index: ctx.cp_index, // be carefull used const u64 invalid_cp_index
                    fragment_flag: false,
                    fragment_count: 0,
                },
            };

            ctx.filter_out.push(io);

            ctx.cp_index += 1;
            ctx.from_offset += ctx.cp_size as u64;

            ctx.codep = 0;
            ctx.cp_size = 0;
        }

        utf8::UTF8_REJECT => {
            // decode error : invalid sequence
            let io = FilterIoData {
                // general info
                is_valid: true,
                end_of_pipe: false, // skip
                quit: false,        // close pipeline
                is_selected: false,
                color: CodepointInfo::default_color(),

                offset: ctx.from_offset,
                size: 1,
                data: FilterData::Unicode {
                    cp: 0xfffd,
                    real_cp: 0xfffd,
                    cp_index: ctx.cp_index, // be carefull used const u64 invalid_cp_index
                    fragment_flag: false,
                    fragment_count: 0,
                },
            };
            ctx.filter_out.push(io);

            // restart @ next byte
            ctx.cp_index += 1;
            ctx.from_offset += 1 as u64;

            ctx.codep = 0;
            ctx.cp_size = 0;
        }
        _ => { /* need more data */ }
    }
}

fn layout_filter_utf8<'a>(
    filter_in: &'a [FilterIoData],
    mut filter_out: &mut Vec<FilterIoData>,
) -> bool {
    if filter_in.is_empty() {
        *filter_out = vec![];
        return true;
    }

    let mut ctx = Utf8FilterCtx {
        from_offset: filter_in[0].offset, // start offset
        state: 0,
        codep: 0,
        cp_size: 0,
        cp_index: 0,
        filter_out: &mut filter_out,
    };

    for d in filter_in {
        match d.data {
            FilterData::ByteArray { array } => {
                for val in array {
                    ctx.cp_size += 1;
                    ctx.state = utf8::decode_byte(ctx.state, *val, &mut ctx.codep);
                    filter_utf8_byte(&mut ctx);
                }
            }

            FilterData::Byte { val } => {
                ctx.cp_size += 1;
                ctx.state = utf8::decode_byte(ctx.state, val, &mut ctx.codep);
                filter_utf8_byte(&mut ctx);
            }

            _ => { /* unexpected */ }
        }
    }

    true
}

fn layout_filter_tabulation<'a>(
    filter_in: &Vec<FilterIoData<'a>>,
    filter_out: &mut Vec<FilterIoData<'a>>,
) -> bool {
    let mut prev_cp = ' ';
    let mut column_count = 0;

    for io in filter_in.iter() {
        if let FilterIoData {
            data: FilterData::Unicode { cp, .. },
            ..
        } = &*io
        {
            if prev_cp == '\r' || prev_cp == '\n' {
                column_count = 0;
            }

            match (prev_cp, u32_to_char(*cp)) {
                (_, '\t') => {
                    prev_cp = '\t';

                    let tab_size = 8;
                    let padding = tab_size - (column_count % tab_size);

                    for _ in 0..padding {
                        let new_io = FilterIoData::replace_codepoint(io, ' ');
                        filter_out.push(new_io);
                        column_count += 1;
                    }
                }

                (_, codepoint) => {
                    prev_cp = codepoint;
                    filter_out.push(io.clone());
                    column_count += 1;
                }
            }
        }
    }

    true
}

fn layout_keyword_highlighting<'a>(
    filter_in: &Vec<FilterIoData<'a>>,
    filter_out: &mut Vec<FilterIoData<'a>>,
) -> bool {
    let mut accum = vec![];
    let mut utf8_word = vec![];

    for io in filter_in {
        match &*io {
            FilterIoData {
                data: FilterData::Unicode { cp, .. },
                ..
            } => {
                let in_word = match u32_to_char(*cp) {
                    ' ' | '\n' | '\t' => false,
                    '(' | ')' => false,
                    '{' | '}' => false,
                    '[' | ']' => false,
                    ',' | ';' => false,

                    _ => true,
                };

                if in_word {
                    accum.push(io.clone());
                    let mut utf8_out: [u8; 4] = [0x00, 0x00, 0x00, 0x00];
                    let nr_bytes = utf8::encode(*cp, &mut utf8_out);
                    for b in utf8_out.iter().take(nr_bytes) {
                        utf8_word.push(*b);
                    }
                    continue;
                }

                let mut new_color = (128, 0, 128);

                let word_found = match String::from_utf8(utf8_word.clone()).unwrap().as_ref() {
                    // some Rust keywords
                    "use" | "crate" => {
                        new_color = (255, 0, 0);
                        true
                    }

                    // some Rust keywords
                    "let" | "mut" | "fn" | "impl" | "trait" => {
                        new_color = (0, 128, 128);
                        true
                    }

                    "u8" | "u16" | "u32" | "u64" | "u128" | "i8" | "i16" | "i32" | "i64"
                    | "i128" | "f32" | "f64" => {
                        new_color = (0, 128, 128);
                        true
                    }

                    // C preprocessor
                    "#include" | "#if" | "#ifdef" | "#ifndef" | "#endif" | "#define" => {
                        new_color = (255, 0, 0);
                        true
                    }

                    // C keywords
                    "if" | "auto" | "break" | "case" | "char" | "const" | "continue"
                    | "default" | "do" | "double" | "else" | "enum" | "extern" | "float"
                    | "for" | "goto" | "int" | "long" | "register" | "return" | "short"
                    | "signed" | "sizeof" | "static" | "struct" | "switch" | "typedef"
                    | "union" | "unsigned" | "void" | "volatile" | "while" => {
                        new_color = (0, 128, 128);
                        true
                    }

                    // C operators
                    "." | "->" | "=" | "==" | "!=" | "&&" | "||" | "~" | "^" => {
                        new_color = (0, 128, 0);
                        true
                    }

                    "," | ";" => {
                        new_color = (0, 128, 0);
                        true
                    }

                    _ => {
                        let mut is_digit = true;
                        for c in utf8_word.iter() {
                            if *c < b'0' || *c > b'9' {
                                is_digit = false;
                                break;
                            }
                        }

                        if is_digit {
                            new_color = (111, 100, 80);
                            true
                        } else {
                            false
                        }
                    }
                };

                if word_found {
                    for mut io in accum.iter_mut() {
                        io.color = new_color;
                    }
                }

                // flush
                if !accum.is_empty() {
                    filter_out.append(&mut accum);
                    utf8_word.clear();
                }

                filter_out.push(io.clone());
            }

            _ => {}
        }
    }

    // flush
    filter_out.append(&mut accum);

    true
}

fn layout_fill_screen(filter_in: &Vec<FilterIoData>, max_offset: u64, screen: &mut Screen) -> bool {
    if filter_in.is_empty() {
        return false;
    }

    // start offset
    let base_offset = filter_in[0].offset;

    screen.first_offset = base_offset;
    let mut last_pushed_offset = base_offset;

    for io in filter_in.iter() {
        if let FilterIoData {
            offset,
            data: FilterData::Unicode { cp, .. },
            color,
            ..
        } = &*io
        {
            let (push_ok, _) = screen.push(filter_codepoint(u32_to_char(*cp), *offset, *color));
            if !push_ok {
                break;
            }

            last_pushed_offset = *offset;
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

    //
    let mut filter_out: Vec<FilterIoData> = Vec::with_capacity(filter_in.len());
    let _ret = layout_filter_utf8(&filter_in, &mut filter_out);
    filter_in = filter_out;

    //
    let mut filter_out: Vec<FilterIoData> = Vec::with_capacity(filter_in.len());
    let _ret = layout_filter_tabulation(&filter_in, &mut filter_out);
    filter_in = filter_out;

    let mut filter_out: Vec<FilterIoData> = Vec::with_capacity(filter_in.len());
    let _ret = layout_keyword_highlighting(&filter_in, &mut filter_out);
    filter_in = filter_out;

    // last pass
    let _ret = layout_fill_screen(&filter_in, max_offset, &mut screen);

    screen.last_offset
}

// TODO return array of CodePointInfo  0x7f -> <ESC>
pub fn filter_codepoint(c: char, offset: u64, color: (u8, u8, u8)) -> CodepointInfo {
    let displayed_cp: char = match c {
        '\r' | '\n' | '\t' => ' ',

        _ if c < ' ' => '�',

        _ if c == 0x7f as char => '�',

        _ => c,
    };

    CodepointInfo {
        metadata: false,
        cp: c,
        displayed_cp,
        offset,
        is_selected: false,
        color,
    }
}
