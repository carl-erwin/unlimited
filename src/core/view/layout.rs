// Copyright (c) Carl-Erwin Griffith

/* DO NOT SPLIT THIS FILE YET: the filter apis are not stable enough */

use core::panic;
use std::cell::RefCell;
use std::char;
use std::collections::HashMap;
use std::rc::Rc;
use std::sync::Arc;
use std::sync::RwLock;

//
use crate::core::codec::text::u32_to_char;
use crate::core::codec::text::utf8;
use crate::core::codec::text::TextCodec;

use crate::dbg_println;

use crate::core::codepointinfo::CodepointInfo;

use crate::core::screen::Screen;

use crate::core::editor::Editor;
use crate::core::editor::EditorEnv;

use crate::core::mark::Mark;
use crate::core::view;
use crate::core::view::View;

use crate::core::modes::text_mode::TextModeContext; // TODO remove this impl details

//
pub struct LayoutEnv<'a> {
    pub graphic_display: bool,
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

    fn setup(&mut self, env: &LayoutEnv, _view: &View);

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
    pub fn new() -> Self {
        RawDataFilter {
            pos: 0,
            read_size: 0,
        }
    }
}

impl Filter<'_> for RawDataFilter {
    fn name(&self) -> &'static str {
        &"RawDataFilter"
    }

    fn setup(&mut self, env: &LayoutEnv, _view: &View) {
        dbg_println!(
            "RawDataFilter w {} h {}",
            env.screen.width(),
            env.screen.height()
        );
        //      let screen_max_cp = env.screen.width() * env.screen.height() * 4; // 4: max utf8 encode size
        //      self.read_size = std::cmp::min(env.max_offset as usize, screen_max_cp);
        self.read_size = 1024 * 4;
        self.pos = env.base_offset;
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

            /*

                        dbg_println!(
                            "READ from offset({}) : {} / {} bytes",
                            self.pos,
                            rd,
                            self.read_size
                        );

                        dbg_println!("BUFFER SIZE {}", doc.as_ref().read().unwrap().size());
                        dbg_println!("POS {} + RD {}  = {}", self.pos, rd, self.pos + rd as u64);

            */
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
    pub fn new() -> Self {
        Utf8Filter {}
    }
}

impl Filter<'_> for Utf8Filter {
    fn name(&self) -> &'static str {
        &"Utf8Filter"
    }

    fn setup(&mut self, _env: &LayoutEnv, _view: &View) {}

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
    pub fn new() -> Self {
        TabFilter {
            prev_cp: '\u{0}',
            column_count: 0,
        }
    }
}

impl Filter<'_> for TabFilter {
    fn name(&self) -> &'static str {
        &"TabFilter"
    }

    fn setup(&mut self, _env: &LayoutEnv, _view: &View) {
        self.prev_cp = '\u{0}';
        self.column_count = 0;
    }

    fn run(
        &mut self,
        _view: &View,
        env: &mut LayoutEnv,
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
                            let mut new_io = FilterIoData::replace_codepoint(io, ' ');
                            if env.graphic_display {
                                new_io.color = (242, 71, 132); // purple-like
                            } else {
                                new_io.color = (128, 0, 128); // magenta
                            }

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
                // not unicode
                filter_out.push(io.clone());
            }
        }
    }
}

pub struct WordWrapFilter {
    max_column: u64,
    column_count: u64,
    accum: Vec<FilterIoData>,
    display_wrap: bool,
}

impl WordWrapFilter {
    pub fn new() -> Self {
        WordWrapFilter {
            max_column: 0,
            column_count: 0,
            accum: vec![],
            display_wrap: false,
        }
    }
}

impl Filter<'_> for WordWrapFilter {
    fn name(&self) -> &'static str {
        &"WordWrapFilter"
    }

    fn setup(&mut self, env: &LayoutEnv, view: &View) {
        self.max_column = env.screen.width() as u64;
        self.column_count = 0;
        self.accum = Vec::new();
        let tm = view.mode_ctx::<TextModeContext>("text-mode");
        self.display_wrap = tm.display_word_wrap;
    }

    /*
        TODO: filters dependencies: check in view's filter_array that
        dep.index < cur_filter.index or (and WARN)
        we can push multiple times new instance of a filter :-)

        prerequisite:
        - tab expansion before: ('\t' -> ' ' should be done before)

        a line can hold max_column chars.
        We accumulate non blank characters, ie ! '\n' ' '
          -> accum

        if ' ' -> flush | accum

        if '\n' -> flush | accum | and jump

        _ => accum


    */
    fn run(
        &mut self,
        _view: &View,
        _env: &mut LayoutEnv,
        filter_in: &Vec<FilterIoData>,
        filter_out: &mut Vec<FilterIoData>,
    ) {
        dbg_println!("filter_in.len() {}", filter_in.len());

        let mut flush_count = 0;

        for io in filter_in.iter() {
            if let FilterIoData {
                data: FilterData::Unicode { cp, .. },
                ..
            } = &*io
            {
                let c = u32_to_char(*cp);

                // flush ?
                if self.column_count == self.max_column {
                    // "inject" fake new line
                    if !self.accum.is_empty() && (c != '\n' && c != ' ') && flush_count > 0 {
                        let mut fnl = FilterIoData::replace_codepoint(&io, '\n');
                        if self.display_wrap {
                            fnl.color = (0, 255, 0);
                            fnl.is_selected = true;
                        }
                        fnl.offset = self.accum[0].offset; // align offset
                        filter_out.push(fnl);
                        self.column_count = 0;
                        flush_count = 0;
                    }
                    let n = self.accum.len() as u64;
                    self.column_count = n % self.max_column;
                }

                self.column_count += 1;

                match c {
                    '\n' => {
                        let mut nl = io.clone();
                        if self.display_wrap {
                            nl.is_selected = true;
                            nl.color = (255, 0, 0);
                        }
                        self.accum.push(nl);
                        filter_out.append(&mut self.accum);
                        self.column_count = 0;
                        flush_count = 0;
                    }
                    ' ' => {
                        // flush "word"
                        let mut space = io.clone();
                        if self.display_wrap {
                            space.is_selected = true;
                            space.color = (0, 0, 255);
                        }
                        self.accum.push(space);
                        filter_out.append(&mut self.accum);
                        flush_count += 1;
                    }
                    _ => {
                        self.accum.push(io.clone());
                    }
                }
            } else {
                //  TODO: use match else is ugly
                let new_io = io.clone();
                self.accum.push(new_io);
                filter_out.append(&mut self.accum);
            }
        }

        if !self.accum.is_empty() {
            // EOF, etc ..
            filter_out.append(&mut self.accum);
        }
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
    pub fn new() -> Self {
        HighlightSelectionFilter {
            sel_start_offset: 0,
            sel_end_offset: 0,
        }
    }
}

// TODO: monitor env.quit
// to flush
impl Filter<'_> for HighlightSelectionFilter {
    fn name(&self) -> &'static str {
        &"HighlightSelectionFilter"
    }

    fn setup(&mut self, _env: &LayoutEnv, view: &View) {
        let tm = view.mode_ctx::<TextModeContext>("text-mode");

        // TODO: compute selection ranges build vec[(min, max)] + index in selection ranges
        let min = tm.marks[tm.mark_index].offset;
        let max = if tm.select_point.len() == 1 {
            tm.select_point[0].offset
        } else {
            min
        };

        let (min, max) = sort_tuple_pair((min, max));
        self.sel_start_offset = min;
        self.sel_end_offset = max;
    }

    fn run(
        &mut self,
        view: &View,
        env: &mut LayoutEnv,
        filter_in: &Vec<FilterIoData>,
        filter_out: &mut Vec<FilterIoData>,
    ) {
        if env.screen.is_off_screen == true {
            *filter_out = filter_in.clone();
            return;
        }

        let tm = view.mode_ctx::<TextModeContext>("text-mode");
        if tm.select_point.len() == 0 {
            *filter_out = filter_in.clone();
            return;
        }

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
    pub fn new() -> Self {
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

    fn setup(&mut self, _env: &LayoutEnv, _view: &View) {
        self.token_io = Vec::new();
        self.token_type = TokenType::Unknown;
        self.utf8_token = Vec::new();
        self.new_color = CodepointInfo::default_color();
        // self.utf8_codec =  Box::new(utf8::Utf8Codec::new());
    }

    fn run(
        &mut self,
        _view: &View,
        env: &mut LayoutEnv,
        filter_in: &Vec<FilterIoData>,
        filter_out: &mut Vec<FilterIoData>,
    ) {
        if env.screen.is_off_screen == true {
            *filter_out = filter_in.clone();
            return;
        }

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
    char_map: Option<HashMap<char, char>>,
    color_map: Option<HashMap<char, (u8, u8, u8)>>,
}

impl ScreenFilter {
    pub fn new() -> Self {
        ScreenFilter {
            // data
            first_offset: None,
            last_pushed_offset: None,
            screen_is_full: false,
            char_map: None,
            color_map: None,
        }
    }
}

impl Filter<'_> for ScreenFilter {
    fn name(&self) -> &'static str {
        &"ScreenFilter"
    }

    fn setup(&mut self, _env: &LayoutEnv, view: &View) {
        let tm = view.mode_ctx::<TextModeContext>("text-mode");
        let char_map = tm.char_map.clone();
        let color_map = tm.color_map.clone();

        self.first_offset = None;
        self.last_pushed_offset = None;
        self.screen_is_full = false;

        // TODO: reload only on view change ? ref ?
        self.char_map = char_map;
        self.color_map = color_map;
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
                        self.char_map.as_ref(),
                        self.color_map.as_ref(),
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
            None,
            None,
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
    char_map: Option<&HashMap<char, char>>,
    color_map: Option<&HashMap<char, (u8, u8, u8)>>,
    c: char,
    offset: Option<u64>,
    size: usize,
    is_selected: bool,
    color: (u8, u8, u8),
    bg_color: (u8, u8, u8),
    metadata: bool,
) -> CodepointInfo {
    let new_color = if let Some(color_map) = color_map {
        if let Some(new_color) = color_map.get(&c) {
            Some(new_color.clone())
        } else {
            Some(color)
        }
    } else {
        Some(color)
    };

    let new_displayed_cp = if let Some(char_map) = char_map {
        if let Some(disp) = char_map.get(&c) {
            Some(*disp)
        } else {
            None
        }
    } else {
        None
    };

    let fallback = |c: char, color: (u8, u8, u8)| -> (char, (u8, u8, u8)) {
        match c {
            '\r' | '\n' => ('\u{2936}', color), // TODO: add user configuration for new-line representation
            //'\r' | '\n' => (' ', color),
            '\t' => (' ', color),
            _ if c < ' ' => ('.', (0, 128, 0)), // TODO: change color/style '�',
            _ if c == '\u{7f}' => ('�', color), // TODO: change color/style '�',
            _ => (c, color),
        }
    };

    let (filtered_cp, filtered_color) = match (new_displayed_cp, new_color) {
        (Some(cp), Some(cl)) => (cp, cl),
        (None, Some(_cl)) => fallback(c, color),
        (Some(cp), None) => (cp, color),
        _ => fallback(c, color),
    };

    CodepointInfo {
        metadata,
        cp: c,
        displayed_cp: filtered_cp,
        offset: offset.clone(),
        size,
        is_mark: false,
        is_selected,
        color: filtered_color,
        bg_color,
    }
}

//////////////////////////////////////////////////////////////////////////////////////////////////////////////////

pub struct DrawMarks {}

impl DrawMarks {
    pub fn new() -> Self {
        DrawMarks {}
    }
}

impl Filter<'_> for DrawMarks {
    fn name(&self) -> &'static str {
        &"DrawMarks"
    }

    fn setup(&mut self, _env: &LayoutEnv, _view: &View) {}

    fn finish(&mut self, view: &View, env: &mut LayoutEnv) -> () {
        let tm = view.mode_ctx::<TextModeContext>("text-mode");
        let marks = &tm.marks;
        let draw_marks = env.screen.is_off_screen == false;
        refresh_screen_marks(&mut env.screen, marks, draw_marks);
    }
}

// SLOW
// we should iterate over the screen
// find the first mark
fn refresh_screen_marks(screen: &mut Screen, marks: &Vec<Mark>, set: bool) {
    if !set {
        screen_apply(screen, |_, _, cpi| {
            cpi.is_mark = false;
            true // continue
        });
        return;
    }

    let (first_offset, last_offset) = match (screen.first_offset, screen.last_offset) {
        (Some(first_offset), Some(last_offset)) => (first_offset, last_offset),
        _ => {
            return;
        }
    };

    for m in marks.iter() {
        match screen.find_cpi_by_offset(m.offset) {
            (Some(&_cpi), x, y) => {
                screen.get_mut_cpinfo(x, y).unwrap().is_mark = true;
            }
            _ => {}
        }
    }

    if true {
        return;
    }

    // incremental mark rendering
    // draw marks
    let mut mark_offset: u64 = 0xFFFFFFFFFFFFFFFF; // replace by max u64
    let mut fetch_mark = true;
    let mut mark_it = marks.iter();
    screen_apply(screen, |_, _, cpi| {
        if let Some(cpi_offset) = cpi.offset {
            if fetch_mark {
                // get 1st  mark >= current cpi_offset
                loop {
                    let m = mark_it.next();
                    if m.is_none() {
                        return false;
                    }

                    let m = m.unwrap();
                    if m.offset < first_offset {
                        continue;
                    }

                    if m.offset > last_offset {
                        return false;
                    }

                    if m.offset >= cpi_offset {
                        mark_offset = m.offset;
                        break;
                    }
                }
                fetch_mark = false;
            }

            if cpi_offset == mark_offset {
                cpi.is_mark = !cpi.metadata;
            } else {
                //
                if mark_offset < cpi_offset {
                    fetch_mark = true;
                }
            }
        }

        true
    });
}

// move to screen module , rename walk/map ?
fn screen_apply<F: FnMut(usize, usize, &mut CodepointInfo) -> bool>(
    screen: &mut Screen,
    mut on_cpi: F,
) {
    for l in 0..screen.height() {
        if let Some(line) = screen.get_mut_line(l) {
            for c in 0..line.nb_cells {
                if let Some(cpi) = line.get_mut_cpi(c) {
                    if on_cpi(c, l, cpi) == false {
                        return;
                    }
                }
            }
        }
    }
}

pub fn run_compositing_stage(
    editor: &Editor,
    env: &EditorEnv,
    view: &Rc<RefCell<View>>,
    base_offset: u64, // default view.start_offset start -> Option<u64>
    max_offset: u64,  // default view.doc.size()   end  -> Option<u64>
    screen: &mut Screen,
) {
    let view = view.borrow();
    run_compositing_stage_direct(editor, env, &view, base_offset, max_offset, screen)
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
fn compose_children(
    editor: &Editor,
    editor_env: &EditorEnv,
    view: &View,
    _base_offset: u64, // default view.start_offset start -> Option<u64>
    max_offset: u64,   // default view.doc.size()   end  -> Option<u64>
    screen: &mut Screen,
) -> bool {
    if view.children.len() == 0 {
        return false;
    }

    dbg_println!("COMPOSE CHILDREN OF VID {}", view.id);

    // vertically
    let split_is_vertical = view.layout_direction == view::LayoutDirection::Vertical;

    let (width, height) = (screen.width(), screen.height());
    if width == 0 || height == 0 {
        return false;
    }

    // cache size ?
    let sizes = if split_is_vertical {
        view::compute_layout_sizes(width, &view.layout_ops)
    } else {
        view::compute_layout_sizes(height, &view.layout_ops)
    };

    dbg_println!(
        "ITER over VID {}, CHILDREN {:?}, size {:?}",
        view.id,
        view.children,
        sizes
    );

    assert_eq!(view.children.len(), sizes.len());

    let mut compose_idx = vec![];
    // 1 - compute position and size
    // 2 - compose based on sibling dependencies/priority
    let mut x = 0;
    let mut y = 0;
    for (idx, vid) in view.children.iter().enumerate() {
        let mut child_v = editor.view_map.get(vid).unwrap().borrow_mut();
        child_v.x = x;
        child_v.y = y;
        let (w, h) = if split_is_vertical {
            (sizes[idx], height)
        } else {
            (width, sizes[idx])
        };

        compose_idx.push((idx, (x, y), (w, h))); // to sort later

        // TODO: resize instead of replace
        let child_screen = Screen::new(w, h);
        child_v.screen = Arc::new(RwLock::new(Box::new(child_screen)));

        if split_is_vertical {
            x += w;
        } else {
            y += h;
        }
    }

    // TODO: sort based on deps/prio
    compose_idx.sort_by(|idxa, idxb| {
        let vida = view.children[idxa.0];
        let vidb = view.children[idxb.0];

        let va = Rc::clone(editor.view_map.get(&vida).unwrap());
        let vb = Rc::clone(editor.view_map.get(&vidb).unwrap());

        let pa = vb.borrow().compose_priority;
        let pb = va.borrow().compose_priority;
        pb.cmp(&pa)
    });
    //

    dbg_println!("COMPOSE sub VIDs {:?}, ", compose_idx);

    for info in &compose_idx {
        let idx = info.0;
        let (x, y) = info.1;
        let (w, h) = info.2;
        if sizes[idx] == 0 {
            continue;
        }

        let vid = view.children[idx];

        let mut child_v = editor.view_map.get(&vid).unwrap().borrow_mut();
        {
            child_v.x = x;
            child_v.y = y;
            let (w, h) = if split_is_vertical {
                (sizes[idx], height)
            } else {
                (width, sizes[idx])
            };

            assert!(w > 0);
            assert!(h > 0);

            let mut child_screen = child_v.screen.write().unwrap();
            run_compositing_stage_direct(
                editor,
                editor_env,
                &child_v,
                child_v.start_offset,
                max_offset, // TODO take child doc size
                &mut child_screen,
            );
        }

        let child_screen = child_v.screen.as_ref().read().unwrap();

        if idx == 0 {
            screen.first_offset = child_screen.first_offset.clone();
        }

        // composition copy child to (parent's) output screen
        screen.copy_to(x, y, &child_screen);
    }

    true
}

// core
pub fn run_compositing_stage_direct(
    editor: &Editor,
    editor_env: &EditorEnv,
    view: &View,
    base_offset: u64, // default view.start_offset start -> Option<u64>
    max_offset: u64,  // default view.doc.size()   end  -> Option<u64>
    mut screen: &mut Screen,
) {
    // check screen size
    if screen.width() == 0 || screen.height() == 0 {
        return;
    }

    // (recursive) children compositing
    let draw = compose_children(
        &editor,
        &editor_env,
        &view,
        base_offset,
        max_offset,
        &mut screen,
    );
    if draw {
        return;
    }

    dbg_println!("COMPOSE VID {}", view.id);

    // Draw Leaf View
    let mut layout_env = LayoutEnv {
        graphic_display: editor_env.graphic_display,
        quit: false,
        base_offset,
        max_offset,
        screen,
    };

    // screen must be cleared by caller
    assert_eq!(0, layout_env.screen.push_count());

    // setup
    let mut compose_filters = view.compose_filters.borrow_mut();

    let mut filter_in = Vec::with_capacity(layout_env.screen.width() * layout_env.screen.height());
    let mut filter_out = Vec::with_capacity(layout_env.screen.width() * layout_env.screen.height());

    // TODO
    for f in compose_filters.iter_mut() {
        f.setup(&mut layout_env, &view);
    }

    if compose_filters.len() == 0 {
        layout_env.quit = true;
    }

    // is interactive rendering possible ?
    while layout_env.quit == false {
        for f in compose_filters.iter_mut() {
            filter_out.clear();
            f.run(&view, &mut layout_env, &filter_in, &mut filter_out);
            dbg_println!(
                "running {:32} : in({}) out({})",
                f.name(),
                filter_in.len(),
                filter_out.len()
            );
            std::mem::swap(&mut filter_in, &mut filter_out);
        }
    }

    for f in compose_filters.iter_mut() {
        f.finish(&view, &mut layout_env);
    }

    // update/return screen start offset
}
