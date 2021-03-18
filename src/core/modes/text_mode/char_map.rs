use crate::core::view::layout::Filter;
use crate::core::view::layout::FilterData;
use crate::core::view::layout::FilterIo;
use crate::core::view::layout::LayoutEnv;
use std::collections::HashMap;

use super::TextModeContext;
use crate::core::view::View;

use crate::core::codec::text::u32_to_char;

use crate::core::codepointinfo::TextStyle;

pub struct CharMapFilter {
    char_map: Option<HashMap<char, String>>, // TODO: add CharMap type
    color_map: Option<HashMap<char, (u8, u8, u8)>>,
}

impl CharMapFilter {
    pub fn new() -> Self {
        CharMapFilter {
            char_map: None,
            color_map: None,
        }
    }
}

impl Filter<'_> for CharMapFilter {
    fn name(&self) -> &'static str {
        &"CharMapFilter"
    }

    fn setup(&mut self, _env: &LayoutEnv, view: &View) {
        let tm = view.mode_ctx::<TextModeContext>("text-mode");
        let char_map = tm.char_map.clone();
        let color_map = tm.color_map.clone();

        // TODO: reload only on view change ? ref ?
        self.char_map = char_map;
        self.color_map = color_map;
    }

    fn run(
        &mut self,
        _view: &View,
        _env: &mut LayoutEnv,
        filter_in: &Vec<FilterIo>,
        filter_out: &mut Vec<FilterIo>,
    ) {
        for io in filter_in.iter() {
            match io {
                FilterIo {
                    metadata,
                    style,
                    offset,
                    size,
                    data:
                        FilterData::Unicode {
                            real_cp,
                            displayed_cp,
                            ..
                        },
                    ..
                } => {
                    let v = transform_io_data(
                        self.char_map.as_ref(),
                        self.color_map.as_ref(),
                        u32_to_char(*real_cp),
                        u32_to_char(*displayed_cp),
                        *offset,
                        *size,
                        style.is_selected,
                        style.color,
                        style.bg_color,
                        *metadata,
                    );

                    for new_io in v {
                        filter_out.push(new_io.clone())
                    }
                }

                _ => filter_out.push(io.clone()),
            }
        }
    }
}

// TODO return array of CodePointInfo  0x7f -> <DEL>
pub fn transform_io_data(
    char_map: Option<&HashMap<char, String>>,
    color_map: Option<&HashMap<char, (u8, u8, u8)>>,
    real_cp: char,
    displayed_cp: char,
    offset: Option<u64>,
    size: usize,
    is_selected: bool,
    color: (u8, u8, u8),
    bg_color: (u8, u8, u8),
    metadata: bool,
) -> Vec<FilterIo> {
    let mut cp_vec = Vec::new();

    let orig_metadata = metadata;
    let orig_size = size;

    if metadata == true && size > 0 {
        dbg_println!(
            "real_cp = {}, displayed_cp = {}, size = {}, metadata = {}",
            real_cp,
            displayed_cp,
            size,
            metadata
        );
        panic!("");
    }

    if metadata == false && size == 0 {
        dbg_println!(
            "real_cp = {}, displayed_cp = {}, size = {}, metadata = {}",
            real_cp,
            displayed_cp,
            size,
            metadata
        );
        panic!("");
    }

    let fallback = |c: char, disp: char, color: (u8, u8, u8)| -> (String, (u8, u8, u8)) {
        match c {
            '\u{9}' => (" ".to_string(), color),
            '\u{7f}' => ("<DEL>".to_string(), (0x00, 0xff, 0xff)), // TODO: add user configuration for new-line representation
            '\r' => ("<CR>".to_string(), (0x00, 0xaa, 0xff)), // TODO: add user configuration for new-line representation
            '\n' => ('\u{2936}'.to_string(), color), // TODO: add user configuration for new-line representation
            //'\n' if c == disp => { (" ".to_string(), color) }, // TODO: add user configuration for new-line representation

            //            _ if real_cp < ' ' => (format!("<0x{:02}>", real_cp as u32), (0, 128, 0)), // TODO: change color/style '�',
            _ if real_cp < ' ' => (format!("."), (0, 128, 0)), // TODO: change color/style '�',

            // MOVE TO CODEC is valid cp
            // _ if c == '\u{7f}' => ('�'.to_string(), (0x00, 0xaa, 0xff)), // TODO: change color/style '�',
            _ => (c.to_string(), color),
        }
    };

    let (s, color) = fallback(real_cp, displayed_cp, color);

    let color = if let Some(color_map) = color_map {
        color_map.get(&real_cp).unwrap_or(&color).clone()
    } else {
        color
    };

    let (s, color) = if let Some(char_map) = char_map {
        if let Some(s) = char_map.get(&real_cp) {
            (s.to_string(), color)
        } else {
            (s.to_string(), color)
        }
    } else {
        let (s, color) = fallback(real_cp, displayed_cp, color);
        (s, color)
    };

    for (idx, displayed_cp) in s.chars().enumerate() {
        let size = if idx == 0 { orig_size } else { 0 };
        let metadata = if idx == 0 { orig_metadata } else { true };

        let mut style = TextStyle::new();
        style.is_selected = is_selected;
        style.is_inverse = false;
        style.color = color;
        style.bg_color = bg_color;

        cp_vec.push(FilterIo {
            metadata,
            style,
            offset,
            size,
            data: FilterData::Unicode {
                real_cp: real_cp as u32,
                displayed_cp: displayed_cp as u32,
                fragment_flag: false,
                fragment_count: 0,
            },
        });
    }

    return cp_vec;
}
