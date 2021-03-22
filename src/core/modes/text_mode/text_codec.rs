use crate::core::codec::text::utf8;
use crate::core::codepointinfo::CodepointInfo;
use crate::core::codepointinfo::TextStyle;

use crate::core::view::layout::Filter;
use crate::core::view::layout::FilterData;
use crate::core::view::layout::FilterIo;
use crate::core::view::layout::LayoutEnv;
use crate::core::view::View;

use super::TextModeContext;

const DEBUG: bool = false;

pub struct TextCodecFilter {
    // data
}

impl TextCodecFilter {
    pub fn new() -> Self {
        TextCodecFilter {}
    }
}

fn text_codec_default_codepoint(offset: u64, size: usize, cp: u32) -> FilterIo {
    assert!(size > 0);

    FilterIo {
        // general info
        metadata: false,
        style: TextStyle::new(),
        offset: Some(offset),
        size,
        data: FilterData::Unicode {
            real_cp: cp,
            displayed_cp: cp,
            fragment_flag: false,
            fragment_count: 0,
        },
    }
}

impl Filter<'_> for TextCodecFilter {
    fn name(&self) -> &'static str {
        &"TextCodecFilter"
    }

    fn setup(&mut self, _env: &mut LayoutEnv, _view: &View) {}

    fn run(
        &mut self,
        view: &View,
        _env: &mut LayoutEnv,
        filter_in: &Vec<FilterIo>,
        filter_out: &mut Vec<FilterIo>,
    ) {
        // ref ? in setup
        let _tm = view.mode_ctx::<TextModeContext>("text-mode");

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
                    for (idx, val) in vec.iter().enumerate() {
                        // tm.text_codec.decode_byte // TODO
                        let io =
                            text_codec_default_codepoint(base_offset + idx as u64, 1, *val as u32);
                        filter_out.push(io);
                    }
                }

                FilterData::Byte { val } => {
                    let io = text_codec_default_codepoint(base_offset, 1, *val as u32);
                    filter_out.push(io);
                }

                FilterData::EndOfStream => {
                    filter_out.push(d.clone());
                }

                _ => {
                    /* unexpected */
                    dbg_println!("receive unexpedted io {:?}", d.data);
                    panic!("");
                }
            }
        }
    }
}

///////////////////////////////////////////////////////////////////////////////////////////////////

// TODO: pass codec in env
struct Utf8FilterCtx {
    current_offset: Option<u64>,
    from_offset: Option<u64>,
    state: u32,
    codep: u32,
    cp_size: usize,
    accum: [u8; 4],
    accum_size: usize,
}

impl Utf8FilterCtx {
    pub fn new() -> Self {
        Utf8FilterCtx {
            current_offset: None,
            from_offset: None,
            state: 0,
            codep: 0,
            cp_size: 0,
            accum: [0; 4],
            accum_size: 0,
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
        data: FilterData::Unicode {
            real_cp: cp,
            displayed_cp: cp,
            fragment_flag: false,
            fragment_count: 0,
        },
    }
}

fn filter_utf8_byte(ctx: &mut Utf8FilterCtx, val: u8, filter_out: &mut Vec<FilterIo>) {
    ctx.accum[ctx.cp_size] = val;
    ctx.accum_size += 1;
    loop {
        ctx.state = utf8::decode_byte(ctx.state, ctx.accum[ctx.cp_size], &mut ctx.codep);
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
            utf8::UTF8_ACCEPT => {
                if DEBUG {
                    dbg_println!(
                ">>> utf8 decode cp OK current_offset = {:?} from_offset = {:?} ctx.cp_size {} cp:u32 {}",
                ctx.current_offset,
                ctx.from_offset,
                ctx.cp_size, ctx.codep);
                }

                let io = utf8_default_codepoint(ctx.from_offset.unwrap(), ctx.cp_size, ctx.codep);
                filter_out.push(io);

                ctx.from_offset = Some(ctx.from_offset.unwrap() + ctx.cp_size as u64);

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

            utf8::UTF8_REJECT => {
                if DEBUG {
                    dbg_println!(
                        "utf8 decode cp ERROR current_offset = {:?} from_offset = {:?} cp_size {}",
                        ctx.current_offset,
                        ctx.from_offset,
                        ctx.cp_size
                    );
                }

                // decode error : invalid sequence
                let io = utf8_default_codepoint(ctx.from_offset.unwrap(), 1, 0xfffd);
                filter_out.push(io);

                // restart @ next byte
                ctx.from_offset = Some(ctx.from_offset.unwrap() + 1);

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

    ctx.current_offset = Some(ctx.current_offset.unwrap() + 1); // ext ?
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

impl Filter<'_> for Utf8Filter {
    fn name(&self) -> &'static str {
        &"Utf8Filter"
    }

    fn setup(&mut self, _env: &mut LayoutEnv, _view: &View) {
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

        dbg_println!(
            "Utf8Filter : start @ offset {}",
            filter_in[0].offset.unwrap()
        );

        if self.ctx.current_offset.is_none() {
            self.ctx.current_offset = filter_in[0].offset;
            self.ctx.from_offset = filter_in[0].offset;
        }

        for d in filter_in {
            match &d.data {
                FilterData::ByteArray { vec } => {
                    if DEBUG {
                        dbg_println!("decode buffer {:x?}", vec);
                    }
                    for val in vec {
                        filter_utf8_byte(&mut self.ctx, *val, &mut filter_out);
                    }
                }

                FilterData::Byte { val } => {
                    filter_utf8_byte(&mut self.ctx, *val, &mut filter_out);
                }

                FilterData::EndOfStream => {
                    // NB: accumulated bytes means incomplete sequence
                    // flush each byte as invalid char
                    for i in 0..self.ctx.cp_size {
                        let io = utf8_default_codepoint(
                            self.ctx.from_offset.unwrap() + i as u64,
                            1,
                            0xfffd,
                        );
                        filter_out.push(io);
                    }

                    // TODO: flush remaining bytes
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
