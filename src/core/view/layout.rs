// Copyright (c) Carl-Erwin Griffith

/* DO NOT SPLIT THIS FILE YET: the filter apis are not stable enough */

use core::panic;
use std::cell::RefCell;
use std::char;
use std::rc::Rc;

//
use crate::core::codec::text::u32_to_char;
use crate::core::codec::text::utf8;
use crate::core::codec::text::TextCodec;

use crate::dbg_println;

use crate::core::codepointinfo::CodepointInfo;

use crate::core::screen::Screen;

use crate::core::editor::EditorEnv;
use crate::core::mark::Mark;
use crate::core::view::View;

use crate::core::modes::text_mode::TextModeData; // TODO remove this impl details

//
pub struct LayoutEnv<'a> {
    pub graphic_display: bool,
    pub quit: bool,
    pub base_offset: u64,
    pub max_offset: u64,
    pub screen: &'a mut Screen,
    pub main_mark: Mark,
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
        _view: &View,
        _env: &mut LayoutEnv,
        input: &Vec<FilterIoData>,
        output: &mut Vec<FilterIoData>,
    ) -> () {
        // default
        *output = input.clone();
    }

    fn finish(&mut self, _view: &View, _env: &mut LayoutEnv) -> () {
        // default
    }
}

// content_type == unicode
#[derive(Debug, Clone)]
pub enum FilterData {
    EndOfStream,

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
    metadata: bool,

    is_selected: bool,
    color: (u8, u8, u8),
    bg_color: (u8, u8, u8),

    offset: Option<u64>,
    size: usize,

    data: FilterData,
    // TODO: add style infos ?
}

impl FilterIoData {
    pub fn replace_codepoint(io: &FilterIoData, new_cp: char) -> FilterIoData {
        if let &FilterIoData {
            // general info
            metadata,
            is_selected,
            color,
            bg_color,
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
                metadata,
                is_selected,
                offset: from_offset,
                color,
                bg_color,
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
    fn new(env: &LayoutEnv, _view: &View) -> Self {
        dbg_println!(
            "RawDataFilter w {} h {}",
            env.screen.width(),
            env.screen.height()
        );

        let screen_max_cp = env.screen.width() * env.screen.height() * 4; // 4: max utf8 encode size
        let read_size = std::cmp::min(env.max_offset as usize, screen_max_cp);

        RawDataFilter {
            pos: env.base_offset,
            //max: env.max_offset,
            read_size,
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

            let rd = doc
                .read()
                .unwrap()
                .read(self.pos, self.read_size, &mut raw_data);

            dbg_println!(
                "READ from offset({}) : {} / {} bytes",
                self.pos,
                rd,
                self.read_size
            );

            dbg_println!("BUFFER SIZE {}", doc.as_ref().read().unwrap().size());
            dbg_println!("POS {} + RD {}  = {}", self.pos, rd, self.pos + rd as u64);

            if rd > 0 {
                (*filter_out).push(FilterIoData {
                    metadata: false,
                    is_selected: false,
                    color: CodepointInfo::default_color(),
                    bg_color: CodepointInfo::default_bg_color(),
                    offset: Some(self.pos),
                    size: rd,
                    data: FilterData::ByteArray { vec: raw_data },
                });
            }

            if rd < self.read_size {
                env.quit = true;

                (*filter_out).push(FilterIoData {
                    metadata: false,
                    is_selected: false,
                    color: CodepointInfo::default_color(),
                    bg_color: CodepointInfo::default_bg_color(),
                    offset: Some(self.pos + rd as u64),
                    size: 0,
                    data: FilterData::EndOfStream,
                });

                dbg_println!("EOF");
            }

            self.pos += rd as u64;

            // increase read size at every call
            // TODO: find better default size
            if self.read_size < 1024 * 1024 {
                self.read_size += env.screen.width();
            }
        }
    }
}

// TODO: pass codec in env
struct Utf8FilterCtx {
    current_offset: u64,
    from_offset: u64,
    state: u32,
    codep: u32,
    cp_size: usize,
    cp_index: u64,
}

fn utf8_default_codepoint(offset: u64, size: usize, cp: u32, cp_index: u64) -> FilterIoData {
    assert!(size > 0);

    FilterIoData {
        // general info
        metadata: false,
        is_selected: false,
        color: CodepointInfo::default_color(),
        bg_color: CodepointInfo::default_bg_color(),
        offset: Some(offset),
        size,
        data: FilterData::Unicode {
            cp,
            real_cp: cp,
            cp_index, // be carefull used const u64 invalid_cp_index
            fragment_flag: false,
            fragment_count: 0,
        },
    }
}

fn filter_utf8_byte(ctx: &mut Utf8FilterCtx, val: u8, filter_out: &mut Vec<FilterIoData>) {
    let mut error_count = 0;

    loop {
        ctx.state = utf8::decode_byte(ctx.state, val, &mut ctx.codep);
        ctx.cp_size += 1;

        match ctx.state {
            utf8::UTF8_ACCEPT => {
                //                dbg_println!(
                //                "utf8 decode cp OK current_offset = {} from_offset = {} ctx.cp_size {} cp:u32 {}",
                //                ctx.current_offset,
                //                ctx.from_offset,
                //                ctx.cp_size, ctx.codep);

                let io =
                    utf8_default_codepoint(ctx.from_offset, ctx.cp_size, ctx.codep, ctx.cp_index);
                filter_out.push(io);

                ctx.cp_index += 1; // TODO: reset on new line ? cheap counter

                ctx.from_offset += ctx.cp_size as u64;

                // restart
                ctx.codep = 0;
                ctx.cp_size = 0;
                ctx.cp_index = 0;
                ctx.state = 0; // TODO: enum
                break;
            }

            utf8::UTF8_REJECT => {
                // dbg_println!(
                //     "utf8 decode cp ERROR current_offset = {} from_offset = {} cp_size {}",
                //     ctx.current_offset,
                //     ctx.from_offset,
                //     ctx.cp_size
                // );

                // decode error : invalid sequence
                let io = utf8_default_codepoint(ctx.from_offset, 1, 0xfffd, ctx.cp_index);
                filter_out.push(io);

                // restart @ next byte
                ctx.cp_index += 1;
                ctx.from_offset = ctx.current_offset + 1;

                // restart
                ctx.codep = 0;
                ctx.cp_size = 0;
                ctx.state = 0; // reset state on error

                error_count += 1;

                // nth byte of the utf8 sequence is bad : restart
                // TODO: we shoud retstart @ cxt.from_offset + 1
                // TODO: add litte ctx.cpi_buffer[4]
                // cpi.index
                if error_count == 1 && ctx.cp_size > 1 {
                    continue; // redecode with current byte
                }

                // 1st byte of the utf8 sequence is bad
                break;
            }

            _ => {
                /* need more data */
                //dbg_println!(
                //"utf8 decoder need more data , ctx.current_offset {} ctx.offset = {} ctx.cp_size {}",
                //ctx.current_offset,
                //ctx.from_offset,
                //ctx.cp_size
                //);

                break;
            }
        }
    }

    ctx.current_offset += 1;
}

pub struct Utf8Filter {
    // data
}

impl Utf8Filter {
    fn new(_env: &LayoutEnv, _view: &View) -> Self {
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

        let mut ctx = Utf8FilterCtx {
            current_offset: filter_in[0].offset.unwrap(), // start offset
            from_offset: filter_in[0].offset.unwrap(),    // start offset
            state: 0,
            codep: 0,
            cp_size: 0,
            cp_index: 0,
        };

        for d in filter_in {
            match &d.data {
                FilterData::ByteArray { vec } => {
                    for val in vec {
                        filter_utf8_byte(&mut ctx, *val, &mut filter_out);
                    }
                }

                FilterData::Byte { val } => {
                    filter_utf8_byte(&mut ctx, *val, &mut filter_out);
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

pub struct TabFilter {
    prev_cp: char,
    column_count: u64,
}

impl TabFilter {
    fn new(_env: &LayoutEnv, _view: &View) -> Self {
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
            } else {
                // unexpected
                filter_out.push(io.clone());
            }
        }
    }
}

pub struct WordWrapFilter {
    max_column: u64,
    column_count: u64,
    accum_count: u64,
    prev_cp: char,
    prev_offset: u64, // Option<u64> ?
    accum: Vec<FilterIoData>,
}

impl WordWrapFilter {
    fn new(env: &LayoutEnv, _view: &View) -> Self {
        WordWrapFilter {
            max_column: env.screen.width() as u64,
            column_count: 0,
            accum_count: 0,
            prev_cp: '\0',
            prev_offset: 0,
            accum: Vec::new(),
        }
    }
}

impl Filter<'_> for WordWrapFilter {
    fn name(&self) -> &'static str {
        &"WordWrapFilter"
    }

    fn run(
        &mut self,
        _view: &View,
        _env: &mut LayoutEnv,
        filter_in: &Vec<FilterIoData>,
        filter_out: &mut Vec<FilterIoData>,
    ) {
        dbg_println!("filter_in.len() {}", filter_in.len());

        for io in filter_in.iter() {
            if let FilterIoData {
                data: FilterData::Unicode { cp, .. },
                ..
            } = &*io
            {
                match (self.prev_cp, u32_to_char(*cp)) {
                    // TODO: split case
                    // blank separator
                    // new line separator
                    // should we consider '�' like normal char ?
                    (_, codepoint) if codepoint == ' ' || codepoint == '\n' /* || codepoint == '�' */ =>
                    {
                        // NB: ' ' at end of line acts like '\n'
                        if codepoint != ' ' /* && codepoint != '�' */
                            && self.column_count + self.accum_count >= self.max_column
                        {
                            // push artificial new line and flush: TODO update metadata flags
                            if self.column_count > 0 {
                                // insert new line only if previous data was seen
                                let mut new_io = FilterIoData::replace_codepoint(&io, '\n');
                                new_io.metadata = true;
                                new_io.color = (0, 255, 0);
                                new_io.offset = Some(self.prev_offset);
                                filter_out.push(new_io);
                                self.column_count = 0; // reset column counter
                            }

                            // flush accumulated data
                            let n = self.accum_count;
                            filter_out.append(&mut self.accum);
                            self.accum_count = 0;
                            self.column_count += n;
                            self.column_count %= self.max_column;

                        } else {
                            // current word fits
                            let n = self.accum_count;
                            filter_out.append(&mut self.accum);
                            self.accum_count = 0;
                            self.column_count += n;
                        }

                        // append current separator
                        self.prev_offset = io.offset.unwrap();
                        self.prev_cp = codepoint;
                        let new_io = io.clone();
                        filter_out.push(new_io);
                        if codepoint == '\n' {
                            self.column_count = 0;
                        } else {
                            self.column_count += 1;
                        }
                    }

                    (_, codepoint) => {
                        self.prev_cp = codepoint;
                        let new_io = io.clone();
                        self.accum.push(new_io);
                        self.accum_count += 1;
                    }
                }
            } else {
                //                TODO: use match else is ugly
                let new_io = io.clone();
                self.accum.push(new_io);
                self.accum_count += 1;
            }
        }

        dbg_println!("self.accum.len() {}", self.accum.len());

        // flush remaining accumulated data
        filter_out.append(&mut self.accum);
        self.accum_count = 0;
    }
}

pub struct HighlightSelectionFilter {
    sel_start_offset: u64,
    sel_end_offset: u64,
}

// TODO: move highlight filter to text mode
// must share selection point or
// declare var 'selection-point' : value  -> language level ...
// enum { type, value }
// a dynamic variables storage for view
// view.vars['selection-point'] -> &mut enum { int64, float64, string, Vec<u8> } | "C" api ...
// view.modes[''] -> std::any::Any
//
use crate::sort_tuple_pair;

impl HighlightSelectionFilter {
    fn new(env: &LayoutEnv, view: &View) -> Self {
        let tm = view.mode_ctx::<TextModeData>("text-mode");

        // TODO: compute selection ranges build vec[(min, max)] + index in selection ranges
        let min = env.main_mark.offset; // << remove this use tm.mark_index
        let max = if tm.select_point.len() == 1 {
            tm.select_point[0].offset
        } else {
            min
        };

        let (min, max) = sort_tuple_pair((min, max));
        HighlightSelectionFilter {
            sel_start_offset: min,
            sel_end_offset: max,
        }
    }
}

// TODO: monitor env.quit
// to flush
impl Filter<'_> for HighlightSelectionFilter {
    fn name(&self) -> &'static str {
        &"HighlightSelectionFilter"
    }

    fn run(
        &mut self,
        _view: &View,
        env: &mut LayoutEnv,
        filter_in: &Vec<FilterIoData>,
        filter_out: &mut Vec<FilterIoData>,
    ) {
        let _colors = [
            /* Black	     */ (0, 0, 0),
            /* Light_red	 */ (255, 0, 0),
            /* Light_green	 */ (0, 255, 0),
            /* Yellow	     */ (255, 255, 0),
            /* Light_blue	 */ (0, 0, 255),
            /* Light_magenta */ (255, 0, 255),
            /* Light_cyan	 */ (0, 255, 255),
            /* High_white	 */ (255, 255, 255),
            /* Gray	         */ (128, 128, 128),
            /* Red	         */ (128, 0, 0),
            /* Green	     */ (0, 128, 0),
            /* Brown	     */ (128, 128, 0),
            /* Blue	         */ (0, 0, 128),
            /* Magenta       */ (128, 0, 128),
            /* Cyan	         */ (0, 128, 128),
            /* White	     */ (192, 192, 192),
        ];

        for i in filter_in {
            match i.offset {
                Some(offset) if offset >= self.sel_start_offset && offset < self.sel_end_offset => {
                    let mut i = i.clone();
                    if env.graphic_display {
                        i.bg_color = CodepointInfo::default_selected_bg_color();
                    } else {
                        //let idx = offset as usize % _colors.len();
                        i.bg_color = (0, 0, 255);
                    }

                    filter_out.push(i);
                }

                _ => {
                    filter_out.push(i.clone());
                }
            }
        }
    }
}

#[derive(Debug, PartialEq)]
enum TokenType {
    Unknown,
    InvalidUnicode,
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
    utf8_codec: Box<dyn TextCodec>, // internal token representation is utf8
}
impl HighlightFilter {
    fn new(_env: &LayoutEnv, _view: &View) -> Self {
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
            match &*io {
                FilterIoData {
                    data: FilterData::Unicode { cp, .. },
                    ..
                } => {
                    let c = u32_to_char(*cp);

                    // dbg_println!("-----------");
                    // dbg_println!("parsing char : '{}'", c);

                    let token_type = match c {
                        '�' => TokenType::InvalidUnicode,
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
                        dbg_println!("self.token_io.len() == 0");

                        self.token_io.push(io.clone());
                        self.token_type = token_type;

                        continue;
                    }

                    if token_type == self.token_type && token_type != TokenType::InvalidUnicode {
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
                        "use" | "crate" | "pub" => (255, 0, 0),

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

                    // prepare next token
                    self.token_io.push(io.clone());

                    // reset state
                    self.utf8_token.clear();
                    self.new_color = CodepointInfo::default_color();
                }

                FilterIoData {
                    data: FilterData::EndOfStream,
                    ..
                } => {
                    // flush pending token: set color
                    for mut io in self.token_io.iter_mut() {
                        io.color = self.new_color;
                    }
                    filter_out.append(&mut self.token_io);

                    // push eof tag
                    filter_out.push(io.clone());
                }

                _ => {
                    dbg_println!("unexpected {:?}", io);
                    panic!("");
                }
            }
        }
    }
}

pub struct ScreenFilter {
    // data
    first_offset: Option<u64>,
    last_pushed_offset: Option<u64>,
    screen_is_full: bool,
}

impl ScreenFilter {
    fn new(_env: &LayoutEnv, _view: &View) -> Self {
        ScreenFilter {
            first_offset: None,
            last_pushed_offset: None,
            screen_is_full: false,
        }
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

        // here ?
        if self.first_offset.is_none() {
            env.screen.first_offset = base_offset.clone();
            self.first_offset = base_offset.clone();
        }

        let mut cpis_vec = Vec::new();

        dbg_println!("ScreenFilter : in len = {}", filter_in.len());

        dbg_println!(
            "screen.push_available({}) + screen.push_count({}) == screen.push_capacity({})",
            env.screen.push_available(),
            env.screen.push_count(),
            env.screen.push_capacity()
        );

        dbg_println!(
            "ScreenFilter :  env.screen.push_available(); {}",
            env.screen.push_available()
        );
        // env.quit = true;
        for io in filter_in.iter() {
            match io.data {
                FilterData::EndOfStream => {
                    // break ?
                    // eof handled in self.finish()
                }

                FilterData::Unicode { cp, .. } => {
                    // screen.push_available() + screen.push_count() == screen.push_capacity()
                    let cp = filter_codepoint(
                        u32_to_char(cp),
                        io.offset.clone(),
                        io.size,
                        io.is_selected,
                        io.color,
                        io.bg_color,
                        io.metadata,
                    );

                    cpis_vec.push(cp);
                }

                _ => {
                    panic!("invalid input type");
                }
            }
        }

        let (n, _size, last_offset) = env.screen.append(&cpis_vec);
        if n < cpis_vec.len() {
            env.quit = true;
            self.last_pushed_offset = last_offset;
            self.screen_is_full = true;
            dbg_println!("SCREEN is full");
        }

        // TODO: add filter.setup(env)
        // TODO: add filter.run(env)
    }

    fn finish(&mut self, _view: &View, env: &mut LayoutEnv) -> () {
        // default

        // EOF
        let eof_cpi = filter_codepoint(
            u32_to_char(' ' as u32),
            Some(env.max_offset),
            0,
            false,
            CodepointInfo::default_color(),
            CodepointInfo::default_bg_color(),
            false, // true ?
        );

        if self.screen_is_full == false {
            let (eof_pushed, _) = env.screen.push(eof_cpi);
            if eof_pushed {
                dbg_println!("EOF({}) pushed", env.max_offset);
            }

            env.screen.has_eof = eof_pushed;
        }

        env.screen.doc_max_offset = env.max_offset;
    }
}

// TODO return array of CodePointInfo  0x7f -> <ESC>
pub fn filter_codepoint(
    c: char,
    offset: Option<u64>,
    size: usize,
    is_selected: bool,
    color: (u8, u8, u8),
    bg_color: (u8, u8, u8),
    metadata: bool,
) -> CodepointInfo {
    let (displayed_cp, color) = match c {
        '\r' | '\n' => ('\u{2936}', color), // TODO: add user configuration for new-line representation
        //'\r' | '\n' => (' ', color),
        '\t' => (' ', color),

        _ if c < ' ' => ('.', (0, 128, 0)), // TODO: change color/style '�',

        _ if c == '\u{7f}' => ('�', color), // TODO: change color/style '�',

        _ => (c, color),
    };

    CodepointInfo {
        metadata,
        cp: c,
        displayed_cp,
        offset: offset.clone(),
        size,
        is_mark: false,
        is_selected,
        color,
        bg_color,
    }
}

pub fn run_view_render_filters(
    env: &EditorEnv,
    view: &Rc<RefCell<View>>,
    base_offset: u64,
    max_offset: u64,
    screen: &mut Screen,
    main_mark: Mark,
) {
    let view = view.as_ref().borrow();
    run_view_render_filters_direct(env, &view, base_offset, max_offset, screen, main_mark)
}

// This function can be considered as the core of the editor.<br/>
// It will run the configured filters until the screen is filled or eof is reached.<br/>
// the screen should be cleared first
// TODO: pass list of filter function to be applied
// 0 - allocate context for each configured plugin
// 1 - utf8 || hexa
// 2 - highlight (some) keywords
// 3 - highlight selection
//  4 - tabulation
//  5 - word wrap
pub fn run_view_render_filters_direct(
    editor_env: &EditorEnv,
    view: &View,
    base_offset: u64,
    max_offset: u64,
    screen: &mut Screen,
    main_mark: Mark,
) {
    let mut layout_env = LayoutEnv {
        graphic_display: editor_env.graphic_display,
        quit: false,
        base_offset,
        max_offset,
        screen,
        main_mark,
    };

    // NB: we allocate this at every screen rendering
    // For now it is simple/efficient enough
    //
    // TODO: move pipeline construction in Mode initialization
    // reserve/update io_vec size on screen dimension changes
    // this will obviously be mor efficient

    // screen must be clear by caller: we don't want
    assert_eq!(0, layout_env.screen.push_count());

    // move in mode init
    let mut filters: Vec<Box<dyn Filter>> = vec![];

    filters.push(Box::new(RawDataFilter::new(&layout_env, &view)));
    filters.push(Box::new(Utf8Filter::new(&layout_env, &view)));

    if layout_env.screen.is_off_screen == false
    /* && editor_env.pending_events <= 1 */
    {
        filters.push(Box::new(HighlightFilter::new(&layout_env, &view)));

        // TODO: find a way to unify filter signature and ActionMap callbacks
        let tm = view.mode_ctx::<TextModeData>("text-mode");
        if tm.select_point.len() > 0 {
            filters.push(Box::new(HighlightSelectionFilter::new(&layout_env, &view)));
        }
    }

    filters.push(Box::new(TabFilter::new(&layout_env, &view)));

    // TODO: disable word wrapping on non text input
    filters.push(Box::new(WordWrapFilter::new(&layout_env, &view)));

    filters.push(Box::new(ScreenFilter::new(&layout_env, &view)));

    // setup
    let mut filter_in = Vec::with_capacity(layout_env.screen.width() * layout_env.screen.height());
    let mut filter_out = Vec::with_capacity(layout_env.screen.width() * layout_env.screen.height());

    // TODO:
    //for f in &mut filters {
    //    f.setup(&view, &mut layout_env add offscreen flag);
    //}

    // is interactive rendering possible ?
    while layout_env.quit == false {
        dbg_println!("-------------------");
        for f in &mut filters {
            filter_out.clear();
            dbg_println!("running {} : in({})", f.name(), filter_in.len());
            f.run(&view, &mut layout_env, &filter_in, &mut filter_out);
            dbg_println!("        {} : out({})", f.name(), filter_out.len());
            std::mem::swap(&mut filter_in, &mut filter_out);
        }
    }

    for f in &mut filters {
        f.finish(&view, &mut layout_env);
    }
}
