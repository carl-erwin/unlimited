use crate::core::Editor;
use parking_lot::RwLock;
use std::rc::Rc;

use crate::core::view::ContentFilter;
use crate::core::view::FilterData;
use crate::core::view::FilterIo;
use crate::core::view::LayoutEnv;
use std::collections::HashMap;

use super::TextModeContext;
use crate::core::view::View;

use crate::core::codec::text::u32_to_char;

use crate::core::codepointinfo::TextStyle;

pub struct CharMapFilter {
    char_map: Option<HashMap<char, String>>, // TODO(ceg): add CharMap type
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

impl ContentFilter<'_> for CharMapFilter {
    fn name(&self) -> &'static str {
        &"CharMapFilter"
    }

    fn setup(
        &mut self,
        _editor: &Editor,
        _env: &mut LayoutEnv,
        view: &Rc<RwLock<View>>,
        _parent_view: Option<&View<'static>>,
    ) {
        let v = view.read();
        let tm = v.mode_ctx::<TextModeContext>("text-mode");
        let char_map = tm.char_map.clone();
        let color_map = tm.color_map.clone();

        // TODO(ceg): reload only on view change ? ref ?
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
                        FilterData::TextInfo {
                            real_cp,
                            displayed_cp,
                            ..
                        },
                    ..
                } => {
                    // enable only for invisible ascii chars
                    let do_transform = if *real_cp < 0x9 {
                        true
                    } else if *real_cp >= 0xb && *real_cp <= 0x1f {
                        true
                    } else if *real_cp == 0x07f || *real_cp == 0x80 {
                        true
                    } else {
                        false
                    };

                    if !do_transform {
                        filter_out.push(io.clone());
                        continue;
                    }

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

    // debug
    if metadata && size > 0 {
        dbg_println!(
            "real_cp = {}, displayed_cp = {}, size = {}, metadata = {}",
            real_cp,
            displayed_cp,
            size,
            metadata
        );
        panic!("");
    }

    // debug
    if !metadata && size == 0 {
        dbg_println!(
            "real_cp = {}, displayed_cp = {}, size = {}, metadata = {}",
            real_cp,
            displayed_cp,
            size,
            metadata
        );
        panic!("");
    }

    let mut fg_color = color;
    if let Some(color_map) = color_map {
        fg_color = *color_map.get(&real_cp).unwrap_or(&color);
    }

    let mut style = TextStyle::new();
    style.is_selected = is_selected;
    style.is_inverse = false;
    style.color = fg_color;
    style.bg_color = bg_color;

    if char_map.is_none() || real_cp != displayed_cp {
        cp_vec.push(FilterIo {
            metadata,
            style,
            offset,
            size,
            data: FilterData::TextInfo {
                real_cp: real_cp as u32,
                displayed_cp: displayed_cp as u32,
            },
        });
        return cp_vec;
    }

    let char_map = char_map.unwrap();

    let s = char_map.get(&real_cp);
    if s.is_none() {
        cp_vec.push(FilterIo {
            metadata,
            style,
            offset,
            size,
            data: FilterData::TextInfo {
                real_cp: real_cp as u32,
                displayed_cp: displayed_cp as u32,
            },
        });
        return cp_vec;
    }

    let s = s.unwrap();

    for (idx, displayed_cp) in s.chars().enumerate() {
        let (size, metadata) = if idx == 0 {
            (orig_size, orig_metadata)
        } else {
            (0, true)
        };

        cp_vec.push(FilterIo {
            metadata,
            style,
            offset,
            size,
            data: FilterData::TextInfo {
                real_cp: real_cp as u32,
                displayed_cp: displayed_cp as u32,
            },
        });
    }

    return cp_vec;
}
