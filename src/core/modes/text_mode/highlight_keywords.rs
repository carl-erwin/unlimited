use std::rc::Rc;
use std::sync::RwLock;

use crate::core::view::layout::ContentFilter;
use crate::core::view::layout::FilterData;
use crate::core::view::layout::FilterIo;
use crate::core::Editor;

use crate::core::view::layout::LayoutEnv;

use crate::core::codec::text::u32_to_char;
use crate::core::codec::text::utf8;
use crate::core::codec::text::TextCodec;

use crate::core::codepointinfo::TextStyle;

use crate::core::view::View;

#[derive(Debug, PartialEq)]
enum TokenType {
    Unknown,
    InvalidUnicode,
    Blank, // ' ' | '\n' | '\t' : TODO(ceg): specific END_OF_LINE ?
    // Num,
    Identifier,   // _a-zA-Z unicode // default ?
    ParenOpen,    // (
    ParenClose,   // )
    BraceOpen,    // {
    BraceClose,   // }
    BracketOpen,  // [
    BracketClose, // ]
    SingleQuote,  // '
    DoubleQuote,  // "
    Comma,        // ,
    Semicolon,    // ;
    Ampersand,
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
    token_type: TokenType,
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
            token_type: TokenType::Unknown,
            utf8_token: Vec::new(),
            new_color: TextStyle::default_color(),
            utf8_codec: Box::new(utf8::Utf8Codec::new()),
            skip_filter: false,
            max_token_size: 1024,
        }
    }
}

// TODO(ceg): monitor env.quit
// to flush
impl ContentFilter<'_> for HighlightFilter {
    fn name(&self) -> &'static str {
        &"HighlightFilter"
    }

    fn setup(&mut self, _editor: &Editor, env: &mut LayoutEnv, _view: &Rc<RwLock<View>>) {
        self.token_io = Vec::new();
        self.token_type = TokenType::Unknown;
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
        filter_in: &Vec<FilterIo>,
        filter_out: &mut Vec<FilterIo>,
    ) {
        if self.skip_filter == true {
            // return NOP hand let the caller skip swap
            *filter_out = filter_in.clone();
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

                    let token_type = match c {
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
                        ';' => TokenType::Semicolon,
                        '&' => TokenType::Ampersand,
                        '%' => TokenType::Mod,
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

                    // select color
                    let token_str = if let Ok(s) = String::from_utf8(self.utf8_token.clone()) {
                        s
                    } else {
                        "�".to_string()
                    };

                    // dbg_println!("TOKEN_STR = '{}'", token_str);

                    self.new_color = match token_str.as_ref() {
                        // some Rust keywords
                        "use" | "crate" | "pub" => (189, 35, 24),

                        // some Rust keywords
                        "let" | "mut" | "fn" | "impl" | "trait" => (0, 128, 128),

                        "str" | "u8" | "u16" | "u32" | "u64" | "u128" | "i8" | "i16" | "i32"
                        | "i64" | "i128" | "f32" | "f64" => (0, 128, 128),

                        // C preprocessor
                        "#include" | "#if" | "#ifdef" | "#ifndef" | "#endif" | "#define"
                        | "#pragma" => (255, 0, 0),

                        // C keywords
                        "if" | "auto" | "break" | "case" | "char" | "const" | "continue"
                        | "default" | "do" | "double" | "else" | "enum" | "extern" | "float"
                        | "for" | "goto" | "int" | "long" | "register" | "return" | "short"
                        | "signed" | "sizeof" | "static" | "struct" | "switch" | "typedef"
                        | "union" | "unsigned" | "void" | "volatile" | "while" | "inline" => {
                            (0, 128, 128)
                        }

                        // C operators
                        "(" | ")" | "." | "->" | "+" | "-" | "*" | "/" | "%" | "=" | "==" | "<"
                        | ">" | "<=" | ">=" | "!=" | "&&" | "||" | "~" | "^" => (0, 128, 0),

                        "/*" | "*/" => (255, 255, 255),
                        "//" => (255, 255, 255),

                        // C++ keywords
                        "class" | "template" | "namespace" => (0, 128, 128),

                        "\"" | "\"\"" | "'" | "''" => (247, 104, 38),

                        "," | ";" => (0, 128, 0),

                        "&" => (0, 128, 0),

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
                        io.style.color = self.new_color;
                    }
                    filter_out.append(&mut self.token_io);

                    // prepare next token
                    self.token_io.push(io.clone());

                    // reset state
                    self.utf8_token.clear();
                    self.new_color = TextStyle::default_color();
                }

                FilterIo {
                    data: FilterData::EndOfStream | FilterData::StreamLimitReached,
                    ..
                } => {
                    // flush pending token: set color
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

    fn finish(&mut self, _view: &View, _env: &mut LayoutEnv) -> () {
        // default
        if !self.token_io.is_empty() {
            // The parsing is incomplete
            // panic!("");
        }
    }
}
