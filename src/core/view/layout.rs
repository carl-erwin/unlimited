// Copyright (c) Carl-Erwin Griffith

use std::cell::RefCell;
use std::rc::Rc;

//
use crate::core::codec::text::u32_to_char;
use crate::core::codec::text::utf8;
use crate::dbg_println;

use crate::core::codepointinfo::CodepointInfo;

use crate::core::screen::Screen;

use crate::core::view::View;

pub struct FilterContext {}

pub struct LayoutEnv<'a> {
    pub quit: bool,
    pub base_offset: u64,
    pub max_offset: u64,
    pub screen: &'a mut Screen,
}

pub trait Filter<'a> {
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
    max: u64,
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
            max: env.max_offset,
            read_size: env.screen.width() * env.screen.height() / 8,
        }
    }
}

impl Filter<'_> for RawDataFilter {
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

            dbg_println!("READ {} bytes", rd);

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
    UNKNOWN,
    BLANK, // ' ' | '\n' | '\t' : TODO: sepcific END_OF_LINE ?
    NUM,
    IDENTIFIER,    // _a-zA-Z unicode // default ?
    PAREN_OPEN,    // (
    PAREN_CLOSE,   // )
    BRACE_OPEN,    // {
    BRACE_CLOSE,   // }
    BRACKET_OPEN,  // [
    BRACKET_CLOSE, // ]
    COMMA,         // ,
    SEMICOLON,     // ,
    EOF,           // END
                   // TODO: QUOTE SINGLE_QUOTE
}

pub struct HighlightFilter {
    token_io: Vec<FilterIoData>,
    token_type: TokenType,
    utf8_token: Vec<u8>,
    new_color: (u8, u8, u8),
}
impl HighlightFilter {
    fn new(_env: &LayoutEnv) -> Self {
        HighlightFilter {
            token_io: Vec::new(),
            token_type: TokenType::UNKNOWN,
            utf8_token: Vec::new(),
            new_color: CodepointInfo::default_color(),
        }
    }
}

// TODO: monitor env.quit
// to flush
impl Filter<'_> for HighlightFilter {
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
                        ' ' | '\n' | '\t' => TokenType::BLANK,
                        '(' => TokenType::PAREN_OPEN,
                        ')' => TokenType::PAREN_CLOSE,
                        '{' => TokenType::BRACE_OPEN,
                        '}' => TokenType::BRACE_CLOSE,
                        '[' => TokenType::BRACKET_OPEN,
                        ']' => TokenType::BRACKET_CLOSE,
                        ',' => TokenType::COMMA,
                        ';' => TokenType::SEMICOLON,
                        // '0'...'9' => TokenType::NUM,
                        _ => TokenType::IDENTIFIER,
                    };

                    // need more or accumulae same class
                    if self.token_io.len() == 0 {
                        self.token_io.push(io.clone());
                        self.token_type = token_type;
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
                                let nr_bytes = utf8::encode(cp, &mut utf8_out);
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
                            // identifier
                            let mut is_digit = true;
                            for c in self.utf8_token.iter() {
                                //dbg_println!("*c = {} b'0' {}", *c as u32, b'0' as u32);
                                //dbg_println!("*c = {} b'9' {}", *c as u32, b'9' as u32);
                                if *c < b'0' || *c > b'9' {
                                    is_digit = false;
                                    break;
                                }
                            }

                            if is_digit {
                                (111, 100, 80)
                            } else {
                                self.new_color
                            }
                        }
                    };

                    self.token_type = token_type;

                    // set color
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
    last_pushed_offset: u64,
}

impl ScreenFilter {
    fn new(_env: &LayoutEnv) -> Self {
        ScreenFilter {
            first_offset: None,
            last_pushed_offset: 0,
        }
    }
}

impl Filter<'_> for ScreenFilter {
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

        self.last_pushed_offset = base_offset;

        for io in filter_in.iter() {
            if let FilterIoData {
                offset,
                data: FilterData::Unicode { cp, .. },
                color,
                ..
            } = &*io
            {
                let (push_ok, _) =
                    env.screen
                        .push(filter_codepoint(u32_to_char(*cp), *offset, *color));
                if !push_ok {
                    env.quit = true;
                    break;
                }

                self.last_pushed_offset = *offset;
            }
        }

        // TODO: add filter.setup(env)
        // TODO: add filter.run(env)
        // TODO: add filter.finish(env)

        env.screen.doc_max_offset = env.max_offset;
        env.screen.last_offset = self.last_pushed_offset;
    }
}

//    struct LayoutEnv {
//        doc,
//        view
//        base_offset: u64,
//        max_offset: u64,
//        screen: &Screen, // for width height
//    };

// TODO return array of CodePointInfo  0x7f -> <ESC>
pub fn filter_codepoint(c: char, offset: u64, color: (u8, u8, u8)) -> CodepointInfo {
    let displayed_cp: char = match c {
        '\r' | '\n' | '\t' => ' ',

        _ if c < ' ' => '�', // TODO: change color/style '�',

        _ if c == 0x7f as char => '�', // TODO: change color/style '�',

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

/// This function can be considered as the core of the editor.<br/>
/// It will run the configured filters until the screen is filled or eof is reached.<br/>
/// the screen is clear first
/// TODO: pass list of filter function to be applied
/// 0 - allocate context for each configurred plugin
/// 1 - utf8 || hexa
/// 2 - tabulation
pub fn run_view_layout_filters(
    view: &Rc<RefCell<View>>,
    base_offset: u64,
    max_offset: u64,
    screen: &mut Screen,
) -> u64 {
    let view = view.as_ref().borrow();
    run_view_layout_filters_direct(&view, base_offset, max_offset, screen)
}

/// This function can be considered as the core of the editor.<br/>
/// It will run the configured filters until the screen is filled or eof is reached.<br/>
/// the screen is clear first
/// TODO: pass list of filter function to be applied
/// 0 - allocate context for each configurred plugin
/// 1 - utf8 || hexa
/// 2 - tabulation
pub fn run_view_layout_filters_direct(
    view: &View,
    base_offset: u64,
    max_offset: u64,
    screen: &mut Screen,
) -> u64 {
    let mut env = LayoutEnv {
        quit: false,
        base_offset,
        max_offset,
        screen,
    };

    let mut filters: Vec<Box<dyn Filter>> = vec![
        Box::new(RawDataFilter::new(&env)),
        Box::new(Utf8Filter::new(&env)),
        Box::new(TabFilter::new(&env)),
        Box::new(HighlightFilter::new(&env)),
        Box::new(ScreenFilter::new(&env)),
    ];

    // setup
    let mut filter_in = Vec::new();
    let mut filter_out = Vec::new();

    while env.quit == false {
        for f in &mut filters {
            filter_out.clear();
            f.run(&view, &mut env, &filter_in, &mut filter_out);
            std::mem::swap(&mut filter_in, &mut filter_out);
        }
    }

    env.screen.last_offset
}
