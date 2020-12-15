// Copyright (c) Carl-Erwin Griffith

/*
  The Dream:

    interactive edition of (de)mux pipeline
    text
    audio
    video
    output device
    a special way to describe user input event ? :
     button/click
     keypress/ IoData utf-32/16/8utf8
               special keys combination

    define: simple unit to be used
       byte
       pixel
       audio sample
       codepoint

    provide basic decoders
    bits accumlator: bits strings of arbitrary type
    (u)int{8,16,32,64} f32/f64

    (de)multiplexing

    ex:

    setup:
        raw | detect type (select out type)
                                                       ______ audio/ogg
    decode:                                           /
        raw | "type decoder" -> vec[IoData] | demux |
                                                      \_______ video/ogm

    ex:
     setup:
        raw | detect type (select out type)
                                                       audio/ogg  ___________
    decode:                                          /                        \
        raw | "type decoder" -> vec[IoData] | demux |                          \_______ mux ___ container
                                                    \                         /
                                                      video/ogm  ____________/

    virtually
    we could create a nes emulator using the right comination of decoder

    user input _____
                     \
                      \
                        \
                         \               ______ audio
                          \             /
    raw | detect(rom) | nes_enum | demux
                                        \
                                         \____  video


    find a way to script state machine in a decoder/encoder :-)

    -------------------------------------------------------
    use C abi for ffi


    DataType {
        type : u64, //  crc64 ("text/utf8") ?
    }

    struct IoData {
      mime_tpe: MimeType,
        mime/type u64 crc64 ("text/utf8") ?
      }
    }

    binary/raw {
        vec: Vec<u8>,
    },


    TextCodec (UTF8){
                // unit: codepoint utf32
                // frame: array of codepoints
                // format: impl def

            r_data &u8[4]
            w_data &u8[4]
interface:
            fn read_forward(buffer: , offset)
            fn read_backward(buffer, offset)
            fn write_backward(buffer, offset)
            fn sync_forward(buffer, offset));
            fn sync_backward((buffer, offset));
        }

    ImageCodec (PNG){
            r_data &u8[4]
            w_data &u8[4]

                // unit: pixel
                // frame: array of pixel
                // format:
    interface:
            fn read_forward(buffer: , offset)
            fn read_backward(buffer, offset)
            fn write_backward(buffer, offset)
            fn sync_forward(buffer, offset));
            fn sync_backward((buffer, offset));
        }

  The Realty:
      broken utf8 handling
      no left-to-right , etc ...

*/

use std::cell::RefCell;
use std::rc::Rc;

//
use crate::core::codec::text::u32_to_char;
use crate::core::codec::text::utf8;
use crate::dbg_println;

use crate::core::codepointinfo::CodepointInfo;

use crate::core::screen::Screen;

use crate::core::server::EditorEnv;
use crate::core::view::View;

pub struct FilterContext {}

pub struct LayoutEnv<'a> {
    pub quit: bool,
    pub base_offset: u64,
    pub max_offset: u64,
    pub screen: &'a mut Screen,
}

// TODO: add ?
//        doc,
//        view

pub trait Filter<'a> {
    fn name(&self) -> &'static str;

    fn run_managed(
        &mut self,
        view: &Rc<RefCell<View>>,
        mut env: &mut LayoutEnv,
        input: &Vec<FilterIoData>,
        output: &mut Vec<FilterIoData>,
    ) -> () {
        let mut view = view.borrow();
        self.run(&mut view, &mut env, input, output);
    }

    fn run(
        &mut self,
        view: &View,
        env: &mut LayoutEnv,
        input: &Vec<FilterIoData>,
        output: &mut Vec<FilterIoData>,
    ) -> ();
}

// content_type == unicode
#[derive(Debug, Clone)]
pub enum FilterData {
    ByteArray {
        vec: Vec<u8>,
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
pub struct FilterIoData {
    // general info
    is_valid: bool,
    end_of_pipe: bool, // skip
    quit: bool,        // close pipeline

    is_selected: bool,
    color: (u8, u8, u8),

    offset: u64,
    size: usize,

    data: FilterData,
    // TODO: add style infos ?
}

impl FilterIoData {
    pub fn replace_codepoint(io: &FilterIoData, new_cp: char) -> FilterIoData {
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

#[derive(Debug, Clone)]
struct _LayoutPlugin {
    plugin_id: u32,
    context_id: u32,
}

pub struct RawDataFilter {
    // data
    pos: u64,
    //max: u64,
    read_size: usize,
}

impl RawDataFilter {
    fn new(env: &LayoutEnv) -> Self {
        dbg_println!(
            "RawDataFilter w {} h {}",
            env.screen.width(),
            env.screen.height()
        );

        RawDataFilter {
            pos: env.base_offset,
            //max: env.max_offset,
            read_size: env.screen.width() * env.screen.height(),
        }
    }
}

impl Filter<'_> for RawDataFilter {
    fn name(&self) -> &'static str {
        &"RawDataFilter"
    }

    fn run(
        &mut self,
        view: &View,
        env: &mut LayoutEnv,
        _noinput: &Vec<FilterIoData>,
        filter_out: &mut Vec<FilterIoData>,
    ) {
        // There is no input HERE
        // we convert the document buffer to ouput

        // we read screen.width() bytes // TODO: width * codec_max_encode_size() for now
        let doc = view.document.clone();
        if let Some(ref doc) = doc {
            // 1st pass raw_data_filter
            let mut raw_data = vec![];

            let rd = doc.borrow().read(self.pos, self.read_size, &mut raw_data);

            dbg_println!("READ {} / {} bytes", rd, self.read_size);

            if rd > 0 {
                (*filter_out).push(FilterIoData {
                    is_valid: true,
                    end_of_pipe: false,
                    quit: false, // close pipeline
                    is_selected: false,
                    color: CodepointInfo::default_color(),
                    offset: self.pos,
                    size: 1,
                    data: FilterData::ByteArray { vec: raw_data },
                });
            }

            if rd < self.read_size {
                env.quit = true;

                // TODO: only text-mode add special tag end-of-stream, add filter-end-of-stream -> ' '
                // for now eof handling -> fake ' ' @ end of stream

                (*filter_out).push(FilterIoData {
                    is_valid: true,
                    end_of_pipe: true,
                    quit: false, // close pipeline
                    is_selected: true,
                    color: CodepointInfo::default_color(),
                    offset: self.pos + rd as u64,
                    size: 1,
                    data: FilterData::Byte { val: b' ' },
                });
            }

            self.pos += rd as u64;

            // increase read size at every call
            if self.read_size < 1024 * 1024 {
                self.read_size += env.screen.width();
            }
        }
    }
}

struct Utf8FilterCtx {
    from_offset: u64,
    state: u32,
    codep: u32,
    cp_size: usize,
    cp_index: u64,
    end_of_pipe: bool,
}

fn filter_utf8_byte(ctx: &mut Utf8FilterCtx, filter_out: &mut Vec<FilterIoData>) {
    match ctx.state {
        utf8::UTF8_ACCEPT => {
            let io = FilterIoData {
                // general info
                is_valid: true,
                end_of_pipe: ctx.end_of_pipe,
                quit: false, // close pipeline
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

            filter_out.push(io);

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
                end_of_pipe: ctx.end_of_pipe,
                quit: false, // close pipeline
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
            filter_out.push(io);

            // restart @ next byte
            ctx.cp_index += 1;
            ctx.from_offset += 1 as u64;

            ctx.codep = 0;
            ctx.cp_size = 0;
        }
        _ => { /* need more data */ }
    }
}

pub struct Utf8Filter {
    // data
}

impl Utf8Filter {
    fn new(_env: &LayoutEnv) -> Self {
        Utf8Filter {}
    }
}

impl Filter<'_> for Utf8Filter {
    fn name(&self) -> &'static str {
        &"Utf8Filter"
    }

    fn run(
        &mut self,
        _view: &View,
        _env: &mut LayoutEnv,
        filter_in: &Vec<FilterIoData>,
        mut filter_out: &mut Vec<FilterIoData>,
    ) {
        if filter_in.is_empty() {
            *filter_out = vec![];
            return;
        }

        let mut ctx = Utf8FilterCtx {
            from_offset: filter_in[0].offset, // start offset
            state: 0,
            codep: 0,
            cp_size: 0,
            cp_index: 0,
            end_of_pipe: false,
        };

        for d in filter_in {
            match &d.data {
                FilterData::ByteArray { vec } => {
                    for val in vec {
                        ctx.cp_size += 1;
                        ctx.state = utf8::decode_byte(ctx.state, *val, &mut ctx.codep);
                        filter_utf8_byte(&mut ctx, &mut filter_out);
                    }
                }

                // TODO: add special type for end on stream ?
                FilterData::Byte { val } => {
                    ctx.end_of_pipe = d.end_of_pipe;
                    ctx.cp_size += 1;
                    ctx.state = utf8::decode_byte(ctx.state, *val, &mut ctx.codep);
                    filter_utf8_byte(&mut ctx, &mut filter_out);
                }

                _ => { /* unexpected */ }
            }
        }

        //       view.clone()
    }
}

pub struct TabFilter {
    prev_cp: char,
    column_count: u64,
}

impl TabFilter {
    fn new(_env: &LayoutEnv) -> Self {
        TabFilter {
            prev_cp: ' ',
            column_count: 0,
        }
    }
}

impl Filter<'_> for TabFilter {
    fn name(&self) -> &'static str {
        &"TabFilter"
    }

    fn run(
        &mut self,
        _view: &View,
        _env: &mut LayoutEnv,
        filter_in: &Vec<FilterIoData>,
        filter_out: &mut Vec<FilterIoData>,
    ) {
        for io in filter_in.iter() {
            if let FilterIoData {
                data: FilterData::Unicode { cp, .. },
                ..
            } = &*io
            {
                if self.prev_cp == '\r' || self.prev_cp == '\n' {
                    self.column_count = 0;
                }

                match (self.prev_cp, u32_to_char(*cp)) {
                    (_, '\t') => {
                        self.prev_cp = '\t';

                        let tab_size = 8;
                        let padding = tab_size - (self.column_count % tab_size);

                        for _ in 0..padding {
                            let new_io = FilterIoData::replace_codepoint(io, ' ');
                            filter_out.push(new_io);
                            self.column_count += 1;
                        }
                    }

                    (_, codepoint) => {
                        self.prev_cp = codepoint;
                        filter_out.push(io.clone());
                        self.column_count += 1;
                    }
                }
            }
        }
    }
}

#[derive(Debug, PartialEq)]
enum TokenType {
    Unknown,
    Blank, // ' ' | '\n' | '\t' : TODO: sepcific END_OF_LINE ?
    // Num,
    Identifier,   // _a-zA-Z unicode // default ?
    ParenOpen,    // (
    ParenClose,   // )
    BraceOpen,    // {
    BraceClose,   // }
    BracketOpen,  // [
    BracketClose, // ]
    Comma,        // ,
    Semicolon,    // ,
                  // Eof,           // End
                  // TODO: QUOTE SINGLE_QUOTE
}

pub struct HighlightFilter {
    token_io: Vec<FilterIoData>,
    token_type: TokenType,
    utf8_token: Vec<u8>,
    new_color: (u8, u8, u8),
    utf8_codec: Box<dyn utf8::TextCodec>, // internal token representation is utf8
}
impl HighlightFilter {
    fn new(_env: &LayoutEnv) -> Self {
        HighlightFilter {
            token_io: Vec::new(),
            token_type: TokenType::Unknown,
            utf8_token: Vec::new(),
            new_color: CodepointInfo::default_color(),
            utf8_codec: Box::new(utf8::Utf8Codec::new()),
        }
    }
}

// TODO: monitor env.quit
// to flush
impl Filter<'_> for HighlightFilter {
    fn name(&self) -> &'static str {
        &"HighlightFilter"
    }

    fn run(
        &mut self,
        _view: &View,
        _env: &mut LayoutEnv,
        filter_in: &Vec<FilterIoData>,
        filter_out: &mut Vec<FilterIoData>,
    ) {
        for io in filter_in {
            let eof = io.end_of_pipe;

            match &*io {
                FilterIoData {
                    data: FilterData::Unicode { cp, .. },
                    ..
                } => {
                    let c = u32_to_char(*cp);

                    //                    dbg_println!("-----------");
                    //                    dbg_println!("parsing char : '{}'", c);

                    let token_type = match c {
                        ' ' | '\n' | '\t' => TokenType::Blank,
                        '(' => TokenType::ParenOpen,
                        ')' => TokenType::ParenClose,
                        '{' => TokenType::BraceOpen,
                        '}' => TokenType::BraceClose,
                        '[' => TokenType::BracketOpen,
                        ']' => TokenType::BracketClose,
                        ',' => TokenType::Comma,
                        ';' => TokenType::Semicolon,
                        // '0'...'9' => TokenType::NUM,
                        _ => TokenType::Identifier,
                    };

                    // need more or accumulae same class
                    if self.token_io.len() == 0 {
                        if eof {
                            filter_out.push(io.clone());
                        } else {
                            self.token_io.push(io.clone());
                            self.token_type = token_type;
                        }
                        continue;
                    }

                    if !eof && token_type == self.token_type {
                        self.token_io.push(io.clone());
                        continue;
                    }

                    // flush token
                    // dbg_println!("FLUSH prev TOKEN");

                    // build token utf8 string
                    for tok in self.token_io.iter() {
                        match tok {
                            &FilterIoData {
                                data: FilterData::Unicode { cp, .. },
                                ..
                            } => {
                                let mut utf8_out: [u8; 4] = [0x00, 0x00, 0x00, 0x00];
                                let nr_bytes = self.utf8_codec.encode(cp, &mut utf8_out);
                                for b in utf8_out.iter().take(nr_bytes) {
                                    self.utf8_token.push(*b);
                                }
                            }
                            _ => {
                                panic!();
                            }
                        }
                    }

                    // select color
                    let token_str = if let Ok(s) = String::from_utf8(self.utf8_token.clone()) {
                        s
                    } else {
                        "�".to_string()
                    };

                    // dbg_println!("TOKEN_STR = '{}'", token_str);

                    self.new_color = match token_str.as_ref() {
                        // some Rust keywords
                        "use" | "crate" => (255, 0, 0),

                        // some Rust keywords
                        "let" | "mut" | "fn" | "impl" | "trait" => (0, 128, 128),

                        "u8" | "u16" | "u32" | "u64" | "u128" | "i8" | "i16" | "i32" | "i64"
                        | "i128" | "f32" | "f64" => (0, 128, 128),

                        // C preprocessor
                        "#include" | "#if" | "#ifdef" | "#ifndef" | "#endif" | "#define" => {
                            (255, 0, 0)
                        }

                        // C keywords
                        "if" | "auto" | "break" | "case" | "char" | "const" | "continue"
                        | "default" | "do" | "double" | "else" | "enum" | "extern" | "float"
                        | "for" | "goto" | "int" | "long" | "register" | "return" | "short"
                        | "signed" | "sizeof" | "static" | "struct" | "switch" | "typedef"
                        | "union" | "unsigned" | "void" | "volatile" | "while" => (0, 128, 128),

                        // C operators
                        "." | "->" | "=" | "==" | "!=" | "&&" | "||" | "~" | "^" => (0, 128, 0),

                        "," | ";" => (0, 128, 0),

                        _ => {
                            let mut non_alnum = 0;
                            let mut digit_count = 0;

                            for c in self.utf8_token.iter() {
                                //dbg_println!("*c = {} b'0' {}", *c as u32, b'0' as u32);
                                //dbg_println!("*c = {} b'9' {}", *c as u32, b'9' as u32);
                                if *c >= b'0' && *c <= b'9' {
                                    digit_count += 1;
                                    continue;
                                }

                                if *c >= b'a' && *c <= b'f' {
                                    continue;
                                }

                                if *c >= b'A' && *c <= b'F' {
                                    continue;
                                }

                                non_alnum += 1;
                                break;
                            }

                            if non_alnum == 0 && digit_count != 0 {
                                (111, 100, 80)
                            } else {
                                self.new_color
                            }
                        }
                    };

                    self.token_type = token_type;

                    // flush token: set color
                    for mut io in self.token_io.iter_mut() {
                        io.color = self.new_color;
                    }
                    filter_out.append(&mut self.token_io);

                    if eof {
                        filter_out.push(io.clone());
                    } else {
                        // prepare next token
                        self.token_io.push(io.clone());

                        // reset state
                        self.utf8_token.clear();
                        self.new_color = CodepointInfo::default_color();
                    }
                }

                _ => {}
            }
        }
    }
}

pub struct ScreenFilter {
    // data
    first_offset: Option<u64>,
}

impl ScreenFilter {
    fn new(_env: &LayoutEnv) -> Self {
        ScreenFilter { first_offset: None }
    }
}

impl Filter<'_> for ScreenFilter {
    fn name(&self) -> &'static str {
        &"ScreenFilter"
    }

    fn run(
        &mut self,
        _view: &View,
        env: &mut LayoutEnv,
        filter_in: &Vec<FilterIoData>,
        _filter_out: &mut Vec<FilterIoData>,
    ) {
        if filter_in.is_empty() {
            return;
        }

        // start offset
        let base_offset = filter_in[0].offset;

        if self.first_offset.is_none() {
            env.screen.first_offset = base_offset;
            self.first_offset = Some(base_offset);
        }

        let mut cpis_vec = Vec::new();

        dbg_println!("ScreenFilter : in len = {}", filter_in.len());

        dbg_println!(
            "screen.push_available({}) + screen.push_count({}) == screen.push_capacity({})",
            env.screen.push_available(),
            env.screen.push_count(),
            env.screen.push_capacity()
        );

        let remain = env.screen.push_available();
        dbg_println!(
            "ScreenFilter :  env.screen.push_available(); {}",
            env.screen.push_available()
        );
        // env.quit = true;
        for (idx, io) in filter_in.iter().enumerate() {
            if let FilterIoData {
                offset,
                data: FilterData::Unicode { cp, .. },
                is_selected,
                color,
                ..
            } = &*io
            {
                // screen.push_available() + screen.push_count() == screen.push_capacity()
                let cp = filter_codepoint(u32_to_char(*cp), *offset, *is_selected, *color);

                cpis_vec.push(cp);

                if idx == remain {
                    dbg_println!("ScreenFilter : screen eof reached !!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!");
                    break;
                }
            }
        }

        let (n, _, last_offset) = env.screen.append(&cpis_vec);
        if n < cpis_vec.len() {
            env.quit = true;
        }

        // TODO: add filter.setup(env)
        // TODO: add filter.run(env)
        // TODO: add filter.finish(env)

        // remove this: add filter.finish() pass
        env.screen.doc_max_offset = env.max_offset;
    }
}

// TODO return array of CodePointInfo  0x7f -> <ESC>
pub fn filter_codepoint(
    c: char,
    offset: u64,
    is_selected: bool,
    color: (u8, u8, u8),
) -> CodepointInfo {
    let (displayed_cp, color) = match c {

        '\r' | '\n'  => ('\u{2936}', (0,0,0xCE)),
        
        '\t' => (' ', color),

        _ if c < ' ' => ('�', color), // TODO: change color/style '�',

        _ if c == '\u{7f}' =>  ('�', color), // TODO: change color/style '�',

        _ => (c, color),
    };

    CodepointInfo {
        metadata: false,
        cp: c,
        displayed_cp,
        offset,
        is_selected,
        color,
    }
}

/// This function can be considered as the core of the editor.<br/>
/// It will run the configured filters until the screen is filled or eof is reached.<br/>
/// the screen is clear first
/// TODO: pass list of filter function to be applied
/// 0 - allocate context for each configurred plugin
/// 1 - utf8 || hexa
/// 2 - tabulation
pub fn run_view_layout_filters(
    env: &EditorEnv,
    view: &Rc<RefCell<View>>,
    base_offset: u64,
    max_offset: u64,
    screen: &mut Screen,
) {
    let view = view.as_ref().borrow();
    run_view_layout_filters_direct(env, &view, base_offset, max_offset, screen)
}

/// This function can be considered as the core of the editor.<br/>
/// It will run the configured filters until the screen is filled or eof is reached.<br/>
/// the screen is clear first
/// TODO: pass list of filter function to be applied
/// 0 - allocate context for each configurred plugin
/// 1 - utf8 || hexa
/// 2 - tabulation
pub fn run_view_layout_filters_direct(
    editor_env: &EditorEnv,
    view: &View,
    base_offset: u64,
    max_offset: u64,
    screen: &mut Screen,
) {
    let mut layout_env = LayoutEnv {
        quit: false,
        base_offset,
        max_offset,
        screen,
    };

    // move in mode init
    let mut filters: Vec<Box<dyn Filter>> = vec![];

    filters.push(Box::new(RawDataFilter::new(&layout_env)));
    filters.push(Box::new(Utf8Filter::new(&layout_env)));

    if layout_env.screen.is_off_screen == false {
        /* || editor_env.pending_events <= 1 || */
        // TODO: schedule refresh on idle
        filters.push(Box::new(HighlightFilter::new(&layout_env)));
    }

    filters.push(Box::new(TabFilter::new(&layout_env)));
    filters.push(Box::new(ScreenFilter::new(&layout_env)));

    // setup
    let mut filter_in = Vec::with_capacity(layout_env.screen.width() * layout_env.screen.height());
    let mut filter_out = Vec::with_capacity(layout_env.screen.width() * layout_env.screen.height());

    // for f in filters { f.setup(); }

    while layout_env.quit == false {
        for f in &mut filters {
            filter_out.clear();
            // dbg_println!("running {} : in({})", f.name(), filter_in.len());
            f.run(&view, &mut layout_env, &filter_in, &mut filter_out);
            // dbg_println!("        {} : out({})", f.name(), filter_out.len());
            std::mem::swap(&mut filter_in, &mut filter_out);
        }
    }

    // for f in filters { f.finish(); }
}
