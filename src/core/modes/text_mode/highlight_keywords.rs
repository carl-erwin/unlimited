use parking_lot::RwLock;
use std::rc::Rc;

use crate::core::view::ContentFilter;
use crate::core::view::FilterData;
use crate::core::view::FilterIo;
use crate::core::Editor;

use crate::core::view::LayoutEnv;

use crate::core::codec::text::u32_to_char;
use crate::core::codec::text::utf8;
use crate::core::codec::text::TextCodec;

use crate::core::codepointinfo::TextStyle;

use crate::core::view::View;

//
static COLOR_DEFAULT: (u8, u8, u8) = (192, 192, 192);
static COLOR_RED: (u8, u8, u8) = (195, 75, 0);
static COLOR_GREEN: (u8, u8, u8) = (85, 170, 127);
static COLOR_ORANGE: (u8, u8, u8) = (247, 104, 38);
static COLOR_CYAN: (u8, u8, u8) = (86, 182, 185);
static COLOR_BLUE: (u8, u8, u8) = (35, 168, 242);
static COLOR_BRACE: (u8, u8, u8) = (0, 185, 163);
static COLOR_NUMBER: (u8, u8, u8) = (111, 100, 80);

#[derive(Debug, Clone, Copy, PartialEq)]
enum TokenType {
    Unknown,
    InvalidUnicode,
    Blank,            // ' ' | '\n' | '\t' : TODO(ceg): specific END_OF_LINE ?
    Identifier,       // _a-zA-Z unicode // default ?
    ParenOpen,        // (
    ParenClose,       // )
    BraceOpen,        // {
    BraceClose,       // }
    BracketOpen,      // [
    BracketClose,     // ]
    SingleQuote,      // '
    DoubleQuote,      // "
    Comma,            // ,
    Colon,            // :
    Semicolon,        // ;
    Ampersand,        // &
    VerticalBar,      // |
    Tilde,            // ~
    CircumflexAccent, // ^
    Dot,              // .
    ExclamationPoint, // !
    Equal,
    Plus,
    Minus,
    Mul,
    Div,
    Mod,
    LowerThan,
    GreaterThan,
}

pub struct HighlightFilter {
    token_io: Vec<FilterIo>,
    prev_token_type: TokenType,
    utf8_token: Vec<u8>,
    new_color: (u8, u8, u8),
    utf8_codec: Box<dyn TextCodec>, // internal token representation is utf8
    skip_filter: bool,
    max_token_size: usize,
}

impl HighlightFilter {
    pub fn new() -> Self {
        HighlightFilter {
            token_io: Vec::new(),
            prev_token_type: TokenType::Unknown,
            utf8_token: Vec::new(),
            new_color: TextStyle::default_color(),
            utf8_codec: Box::new(utf8::Utf8Codec::new()),
            skip_filter: false,
            max_token_size: 1024,
        }
    }

    fn colorize_token(&mut self) {
        // build token utf8 string
        for tok in self.token_io.iter() {
            match tok {
                &FilterIo {
                    data: FilterData::TextInfo { real_cp, .. },
                    ..
                } => {
                    let mut utf8_out: [u8; 4] = [0x00, 0x00, 0x00, 0x00];
                    let nr_bytes = self.utf8_codec.encode(real_cp, &mut utf8_out);
                    for b in utf8_out.iter().take(nr_bytes) {
                        self.utf8_token.push(*b);
                    }
                }
                _ => {
                    panic!();
                }
            }
        }

        self.new_color = match self.prev_token_type {
            TokenType::Unknown => COLOR_DEFAULT,
            TokenType::InvalidUnicode => COLOR_DEFAULT,
            TokenType::Blank => COLOR_DEFAULT, // ' ' | '\n' | '\t' : TODO(ceg): specific END_OF_LINE ?
            TokenType::ParenOpen => COLOR_GREEN, // (
            TokenType::ParenClose => COLOR_GREEN, // )
            TokenType::BraceOpen => COLOR_BRACE, // {
            TokenType::BraceClose => COLOR_BRACE, // }
            TokenType::BracketOpen => COLOR_BRACE, // [
            TokenType::BracketClose => COLOR_BRACE, // ]
            TokenType::SingleQuote => COLOR_ORANGE, // '
            TokenType::DoubleQuote => COLOR_ORANGE, // "
            TokenType::Comma => COLOR_GREEN,   // ,
            TokenType::Colon => COLOR_GREEN,   // :
            TokenType::Semicolon => COLOR_GREEN, // ;
            TokenType::Ampersand => COLOR_CYAN, // &
            TokenType::VerticalBar => COLOR_CYAN, // |
            TokenType::Tilde => COLOR_CYAN,    // ~
            TokenType::CircumflexAccent => COLOR_CYAN, // ^
            TokenType::Dot => COLOR_GREEN,     // .
            TokenType::ExclamationPoint => COLOR_GREEN, // !
            TokenType::Equal => COLOR_GREEN,
            TokenType::Plus => COLOR_GREEN,
            TokenType::Minus => COLOR_GREEN,
            TokenType::Mul => COLOR_GREEN,
            TokenType::Div => COLOR_GREEN,
            TokenType::Mod => COLOR_GREEN,
            TokenType::LowerThan => COLOR_GREEN,
            TokenType::GreaterThan => COLOR_GREEN,
            TokenType::Identifier => {
                self.set_indentifier_color();
                self.new_color
            }
        };
    }

    fn set_indentifier_color(&mut self) {
        // select color
        let token_str = if let Ok(s) = String::from_utf8(self.utf8_token.clone()) {
            s
        } else {
            "�".to_string()
        };

        // dbg_println!("TOKEN_STR = '{}'", token_str);
        self.new_color = match token_str.as_ref() {
            // some Rust keywords
            "use" | "crate" | "pub" => COLOR_RED,

            // some Rust keywords
            "let" | "ref" | "mut" | "fn" | "impl" | "trait" | "type" => (0, 128, 128),
            "Option" | "Some" | "None" | "Result" => (0, 128, 128),

            "unsafe" | "panic" => COLOR_RED,

            "borrow" | "unwrap" => (0, 128, 128),

            "str" | "u8" | "u16" | "u32" | "u64" | "u128" | "i8" | "i16" | "i32" | "i64"
            | "i128" | "f32" | "f64" => (0, 128, 128),

            // some C preprocessor tokens
            "#include" | "#if" | "#ifdef" | "#ifndef" | "#endif" | "#else" | "#undef"
            | "#define" | "#pragma" => COLOR_RED,

            // some C keywords
            "break" | "case" | "char" | "const" | "continue" | "default" | "do" | "double"
            | "enum" | "extern" | "float" | "for" | "int" | "long" | "register" | "short"
            | "signed" | "sizeof" | "static" | "struct" | "switch" | "typedef" | "union"
            | "unsigned" | "void" | "volatile" | "while" | "inline" => (0, 128, 128),

            "if" | "then" | "else" => COLOR_BRACE,

            // some C++ keywords
            "bool" | "class" | "template" | "namespace" | "auto" => (0, 128, 128),

            //
            "return" | "goto" | "true" | "false" => COLOR_BLUE,

            _ => {
                let mut non_alnum = 0;
                let mut digit_count = 0;

                let skip_n = if self.utf8_token.len() >= 2
                    && self.utf8_token[0] == b'0'
                    && self.utf8_token[1] == b'x'
                {
                    2
                } else {
                    0
                };

                for c in self.utf8_token.iter().skip(skip_n) {
                    if *c == b'_' {
                        continue;
                    }

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
                    COLOR_NUMBER
                } else {
                    self.new_color
                }
            }
        };
    }
}

fn get_token_type(c: char) -> TokenType {
    match c {
        '�' => TokenType::InvalidUnicode,
        ' ' | '\n' | '\t' => TokenType::Blank,
        '(' => TokenType::ParenOpen,
        ')' => TokenType::ParenClose,
        '{' => TokenType::BraceOpen,
        '}' => TokenType::BraceClose,
        '[' => TokenType::BracketOpen,
        ']' => TokenType::BracketClose,
        '\'' => TokenType::SingleQuote,
        '"' => TokenType::DoubleQuote,
        '=' => TokenType::Equal,
        '*' => TokenType::Mul,
        '+' => TokenType::Plus,
        '-' => TokenType::Minus,
        '/' => TokenType::Div,
        '<' => TokenType::LowerThan,
        '>' => TokenType::GreaterThan,
        ',' => TokenType::Comma,
        ':' => TokenType::Colon,
        ';' => TokenType::Semicolon,
        '&' => TokenType::Ampersand,
        '%' => TokenType::Mod,
        '|' => TokenType::VerticalBar,
        '~' => TokenType::Tilde,
        '^' => TokenType::CircumflexAccent,
        '.' => TokenType::Dot,
        '!' => TokenType::ExclamationPoint,

        // '0'...'9' => TokenType::NUM,
        _ => TokenType::Identifier,
    }
}

// TODO(ceg): monitor env.quit
// to flush
impl ContentFilter<'_> for HighlightFilter {
    fn name(&self) -> &'static str {
        &"HighlightFilter"
    }

    fn setup(
        &mut self,
        _editor: &Editor<'static>,
        env: &mut LayoutEnv,
        _view: &Rc<RwLock<View>>,
        _parent_view: Option<&View<'static>>,
    ) {
        self.token_io = Vec::new();
        self.prev_token_type = TokenType::Unknown;
        self.utf8_token = Vec::new();
        self.new_color = TextStyle::default_color();
        // self.utf8_codec =  Box::new(utf8::Utf8Codec::new());

        self.skip_filter = false;

        let p_input = crate::core::event::pending_input_event_count();
        if p_input > 255 {
            //dbg_println!("*** SKIP HIGHLIGHT *** p_input {}", p_input);
            self.skip_filter = true;
        }
        if env.screen.is_off_screen {
            self.skip_filter = true;
        }

        self.max_token_size = env.screen.width() * env.screen.height(); //
    }

    fn run(
        &mut self,
        _view: &View,
        _env: &mut LayoutEnv,
        filter_in: &[FilterIo],
        filter_out: &mut Vec<FilterIo>,
    ) {
        if self.skip_filter {
            // return NOP hand let the caller skip swap
            *filter_out = filter_in.to_vec();
            return;
        }

        // flush too big token
        if self.token_io.len() > self.max_token_size {
            for mut io in self.token_io.iter_mut() {
                io.style.color = self.new_color;
            }
            filter_out.append(&mut self.token_io);
        }

        for io in filter_in {
            match &*io {
                FilterIo {
                    data: FilterData::TextInfo { real_cp, .. },
                    ..
                } => {
                    let c = u32_to_char(*real_cp);

                    // dbg_println!("-----------");
                    // dbg_println!("parsing char : '{}'", c);
                    let token_type = get_token_type(c);

                    if token_type == TokenType::Identifier
                        && self.prev_token_type == TokenType::Identifier
                    {
                        self.token_io.push(io.clone());
                        self.prev_token_type = token_type;
                        continue;
                    }

                    // flush token: set color
                    self.colorize_token();
                    for mut io in self.token_io.iter_mut() {
                        io.style.color = self.new_color;
                    }
                    filter_out.append(&mut self.token_io);
                    // reset state
                    self.utf8_token.clear();
                    self.new_color = TextStyle::default_color();

                    // prepare next token
                    self.prev_token_type = token_type;
                    self.token_io.push(io.clone());
                }

                FilterIo {
                    data: FilterData::EndOfStream | FilterData::CustomLimitReached,
                    ..
                } => {
                    // flush pending token: set color
                    self.colorize_token();
                    for mut io in self.token_io.iter_mut() {
                        io.style.color = self.new_color;
                    }

                    filter_out.append(&mut self.token_io);

                    // forward tag
                    filter_out.push(io.clone());
                }

                _ => {
                    dbg_println!("unexpected {:?}", io);
                    panic!("");
                }
            }
        }
    }

    fn finish(&mut self, _view: &View, _env: &mut LayoutEnv) {
        // default
        if !self.token_io.is_empty() {
            // The parsing is incomplete
            // panic!("");
        }
    }
}
