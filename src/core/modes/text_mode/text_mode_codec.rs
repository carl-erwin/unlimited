extern crate utf8parse;
//use ::utf8parse::*;
use utf8parse::Receiver;

use std::rc::Rc;
use std::sync::RwLock;

use crate::core::codec::text::utf8;
use crate::core::view::layout::ContentFilter;
use crate::core::view::layout::FilterData;
use crate::core::view::layout::FilterIo;
use crate::core::view::layout::LayoutEnv;
use crate::core::view::layout::Unicode;
use crate::core::view::View;
use crate::core::Editor;

use crate::core::codepointinfo::TextStyle;

// the consecutive offsets are derive from size
pub struct TextCodecFilter {
    // data
}

impl TextCodecFilter {
    pub fn new() -> Self {
        TextCodecFilter {}
    }
}

impl ContentFilter<'_> for TextCodecFilter {
    fn name(&self) -> &'static str {
        &"TextCodecFilter"
    }

    fn run(
        &mut self,
        _view: &View,
        _env: &mut LayoutEnv,
        filter_in: &Vec<FilterIo>,
        filter_out: &mut Vec<FilterIo>,
    ) {
        // ref ? in setup
        //        let _tm = view.mode_ctx::<TextModeContext>("text-mode");

        // put in common
        if filter_in.is_empty() {
            dbg_println!("TextCodecFilter : empty input !!!!");
            *filter_out = vec![];
            return;
        }

        dbg_println!(
            "TextCodecFilter : start @ offset {}",
            filter_in[0].offset.unwrap()
        );

        for d in filter_in.iter() {
            let base_offset = d.offset.unwrap();

            match &d.data {
                FilterData::ByteArray { vec } => {
                    //let mut decoded = vec![];
                    let mut decoded = Vec::with_capacity(vec.len());
                    unsafe {
                        decoded.set_len(vec.len());
                    }
                    for (idx, val) in vec.iter().enumerate() {
                        let new_io = Unicode {
                            size: 1,
                            cp: *val as u32,
                        };
                        // decoded.push(new_io);
                        // decoded[idx] = new_io;
                        unsafe {
                            *decoded.get_unchecked_mut(idx) = new_io;
                        }
                    }

                    // single bloc with base offset
                    filter_out.push(FilterIo {
                        // general info
                        metadata: false,
                        style: TextStyle::new(),
                        //
                        offset: Some(base_offset),
                        size: 0, // count(data) ?
                        data: FilterData::UnicodeArray { vec: decoded },
                    });
                }

                FilterData::EndOfStream => {
                    filter_out.push(d.clone());
                }

                _ => {
                    /* unexpected */
                    dbg_println!("receive unexpected io {:?}", d.data);
                    panic!("");
                }
            }
        }
    }
}

///////////////////////////////////////////////////////////////////////////////////////////////////

// TODO(ceg): pass codec in env
pub struct Utf8FilterCtx {
    cp_size: usize,
    state: u32,
    codep: u32,
    accum_size: u32,
    accum: [u8; 4],
    error: bool,

    //
    out: Vec<Unicode>,
}

impl Utf8FilterCtx {
    pub fn new() -> Self {
        Utf8FilterCtx {
            state: 0,
            codep: 0,
            cp_size: 0,
            accum: [0; 4],
            accum_size: 0,
            error: false,
            out: vec![],
        }
    }
}

fn utf8_default_codepoint(offset: u64, size: usize, cp: u32) -> FilterIo {
    assert!(size > 0);

    FilterIo {
        // general info
        metadata: false,
        style: TextStyle::new(),
        offset: Some(offset),
        size,
        data: FilterData::TextInfo {
            real_cp: cp,
            displayed_cp: cp,
        },
    }
}

// TODO(ceg): remove Option<>
// always ?
#[inline(always)]
fn filter_utf8_byte(ctx: &mut Utf8FilterCtx, val: u8) {
    ctx.accum[ctx.cp_size] = val;
    ctx.accum_size += 1;
    loop {
        // (re)decode current byte: restart on previous bytes if error
        ctx.state = utf8::decode_byte(ctx.state, ctx.accum[ctx.cp_size], &mut ctx.codep);
        /*
                if DEBUG {
                    dbg_println!("utf8 decode byte  '0x{:x}'", ctx.accum[ctx.cp_size - 1]);
                    dbg_println!(
                        "utf8 ACCUM {:x?}' accum_size {} cp_size = {}",
                        ctx.accum,
                        ctx.accum_size,
                        ctx.cp_size
                    );
                }
        */
        match ctx.state {
            utf8::UTF8_ACCEPT => {
                /*
                                if DEBUG {
                                    dbg_println!(
                                ">>> utf8 decode cp OK current_offset = {:?} from_offset = {:?} ctx.cp_size {} cp:u32 {}",
                                ctx.current_offset,
                                ctx.from_offset,
                                ctx.cp_size, ctx.codep);
                                }
                */
                ctx.out.push(Unicode {
                    cp: ctx.codep,
                    size: 1 + ctx.cp_size as u32,
                });

                // restart
                ctx.state = 0;
                ctx.codep = 0;
                ctx.cp_size = 0;

                ctx.accum_size = 0;
                // drop accum debug only  ?
                // ctx.accum.fill(0);
                break;
            }

            utf8::UTF8_REJECT => {
                /*
                                if DEBUG {
                                    dbg_println!(
                                        "utf8 decode cp ERROR current_offset = {:?} from_offset = {:?} cp_size {}",
                                        ctx.current_offset,
                                        ctx.from_offset,
                                        ctx.cp_size
                                    );
                                }
                */
                // decode error : invalid sequence
                //let io = utf8_default_codepoint(ctx.from_offset, 1, 0xfffd);
                ctx.out.push(Unicode {
                    cp: 0xfffd,
                    size: 1,
                });

                // restart @ next byte
                ctx.codep = 0;
                ctx.cp_size = 0;
                ctx.state = 0; // reset state on error

                // TODO(ceg) use raw_data[pos] to restart
                // accum is an extract of raw data: to handle truncated input
                // shift accum
                ctx.accum.rotate_left(1);
                // ctx.accum[3] = 0; debug

                /* restart decoder at accum[ctx.cp_size-1] */
                ctx.accum_size -= 1;
                if ctx.accum_size == 0 {
                    return;
                }
                continue;
            }

            _ => {
                ctx.cp_size += 1; // valid intermediate state, need more data
                break;
            }
        }
    }
}

impl Receiver for Utf8FilterCtx {
    fn codepoint(&mut self, c: char) {
        self.error = false;
        self.codep = 0;
        self.cp_size = 0;
        self.accum_size = 0;
        self.out.push(Unicode {
            cp: c as u32,
            size: 1 + self.cp_size as u32,
        });
    }

    fn invalid_sequence(&mut self) {
        // restart @ next byte
        self.error = true;
        self.codep = 0;
        self.cp_size = 0;

        // TODO(ceg) use raw_data[pos] to restart
        // accum is an extract of raw data: to handle truncated input
        // shift accum
        self.accum.rotate_left(1);

        // ctx.accum[3] = 0; debug
        /* restart decoder at accum[ctx.cp_size-1] */
        self.accum_size -= 1;
        self.out.push(Unicode {
            cp: 0xfffd,
            size: 1,
        });
    }
}

// if no encoding error found and first input byte is "utf8" synchronized
#[inline]
pub fn filter_utf8_bytearray_shift_accum(
    mut ctx: &mut Utf8FilterCtx,
    vec: &Vec<u8>,
    _filter_out: &mut Vec<FilterIo>,
) {
    for b in vec {
        let val = *b as u32;

        //        dbg_println!("read val  {:x}", val);
        if val < 0b10000000 {
            // flush previous ?
            if ctx.cp_size > 1 {
                ctx.out.push(Unicode {
                    cp: ctx.codep as u32,
                    size: ctx.cp_size as u32,
                });

                //                dbg_println!("flush previous ctx.codep({}) as '{}'", ctx.codep, unsafe {
                //                    char::from_u32_unchecked(ctx.codep)
                //                });
            }

            //            dbg_println!("decoded ctx.codep({}) as '{}'", ctx.codep, unsafe {
            //                char::from_u32_unchecked(val)
            //            });

            ctx.out.push(Unicode { cp: val, size: 1 });

            // reset
            ctx.codep = 0;
            ctx.cp_size = 0;
            continue;
        }

        //          mmxxxxxx   continuation
        if (val & 0b11000000) == 0b10000000 {
            ctx.codep = (ctx.codep << 6) | (val & 0b00111111);
            ctx.cp_size += 1;
        } else {
            // flush
            if ctx.cp_size > 1 {
                ctx.out.push(Unicode {
                    cp: ctx.codep as u32,
                    size: ctx.cp_size as u32,
                });

                //                dbg_println!("decoded ctx.codep({}) as '{}'", ctx.codep, unsafe {
                //                    char::from_u32_unchecked(ctx.codep)
                //                });
            }

            ctx.cp_size += 1;
            // check seq len
            if (val & 0b11000000) == 0b11000000 {
                ctx.codep = val & 0b00011111;
            }
            if (val & 0b11100000) == 0b11100000 {
                ctx.codep = val & 0b00001111;
            }
            if (val & 0b11110000) == 0b00000111 {
                ctx.codep = val & 0b00111111;
            }
        }
    }
}

#[inline]
pub fn filter_utf8_bytearray(
    ctx: &mut Utf8FilterCtx,
    vec: &Vec<u8>,
    filter_out: &mut Vec<FilterIo>,
) {
    //    dbg_println!("filter_utf8_bytearray : input len =  {}", vec.len());

    ctx.out.clear();

    let mut ctx = ctx;

    // TODO(ceg): move accum between state structs
    if true {
        for val in vec {
            filter_utf8_byte(&mut ctx, *val);
        }
    } else {
        let mut parser = utf8parse::Parser::new();
        for val in vec {
            ctx.accum[ctx.cp_size] = *val;
            ctx.accum_size += 1;
            let pos = ctx.cp_size;
            let b = ctx.accum[pos];
            ctx.cp_size += 1;
            parser.advance(ctx, b);
            // no error handling on truncated input
        }
    }

    let new_io = FilterIo {
        // general info
        metadata: false,
        style: TextStyle::new(),
        offset: None, // not here
        size: 0,      // count(data) ?
        data: FilterData::UnicodeArray {
            vec: ctx.out.clone(),
        },
        // TODO(ceg): add style infos ?
    };

    filter_out.push(new_io);
}

pub struct Utf8Filter {
    // data
    ctx: Utf8FilterCtx,
}

impl Utf8Filter {
    pub fn new() -> Self {
        Utf8Filter {
            ctx: Utf8FilterCtx::new(),
        }
    }
}

impl ContentFilter<'_> for Utf8Filter {
    fn name(&self) -> &'static str {
        &"Utf8Filter"
    }

    fn setup(&mut self, _editor: &Editor, _env: &mut LayoutEnv, _view: &Rc<RwLock<View>>) {
        self.ctx = Utf8FilterCtx::new();
    }

    fn run(
        &mut self,
        _view: &View,
        _env: &mut LayoutEnv,
        filter_in: &Vec<FilterIo>,
        mut filter_out: &mut Vec<FilterIo>,
    ) {
        // put in common
        if filter_in.is_empty() {
            dbg_println!("Utf8Filter : empty input !!!!");
            *filter_out = vec![];
            return;
        }

        self.ctx.out.clear();

        for d in filter_in {
            match &d.data {
                FilterData::ByteArray { vec } => {
                    filter_utf8_bytearray(&mut self.ctx, vec, &mut filter_out);
                    //filter_utf8_bytearray_shift_accum(&mut self.ctx, vec, &mut filter_out);
                }

                FilterData::EndOfStream | FilterData::StreamLimitReached => {
                    // NB: accumulated bytes means incomplete sequence
                    // flush each byte as invalid char
                    for _i in 0..self.ctx.cp_size {
                        // let io = utf8_default_codepoint(self.ctx.from_offset + i as u64, 1, 0xfffd);
                        // filter_out.push(io);
                        self.ctx.out.push(Unicode {
                            cp: 0xfffd,
                            size: 1,
                        });
                    }

                    // TODO(ceg): flush remaining bytes
                    filter_out.push(d.clone());
                }

                _ => {
                    /* unexpected */
                    dbg_println!("receive unexpected io {:?}", d.data);
                    panic!("");
                }
            }
        }
    }
}
