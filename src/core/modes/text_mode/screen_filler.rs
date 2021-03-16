use std::collections::HashMap;

use crate::core::view::layout::Filter;
use crate::core::view::layout::FilterData;
use crate::core::view::layout::FilterIo;
use crate::core::view::layout::LayoutEnv;
use crate::core::view::View;

use crate::core::codepointinfo::CodepointInfo;

use super::TextModeContext;

use crate::core::codec::text::u32_to_char;

use crate::core::screen::Screen;

///////////////////////////////////////////////////////////////////////////////////////////////////

// TRANSFORM into filter pass char_map_filter before word wrap

///////////////////////////////////////////////////////////////////////////////////////////////////

pub struct ScreenFilter {
    // data
    first_offset: Option<u64>,
    screen_is_full: bool,
    char_map: Option<HashMap<char, String>>, // TODO: add CharMap type
    color_map: Option<HashMap<char, (u8, u8, u8)>>,
}

impl ScreenFilter {
    pub fn new() -> Self {
        ScreenFilter {
            // data
            first_offset: None,
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
        self.screen_is_full = false;

        // TODO: reload only on view change ? ref ?
        self.char_map = char_map;
        self.color_map = color_map;
    }

    fn run(
        &mut self,
        _view: &View,
        env: &mut LayoutEnv,
        filter_in: &Vec<FilterIo>,
        _filter_out: &mut Vec<FilterIo>,
    ) {
        if filter_in.is_empty() {
            return;
        }

        // start offset
        let base_offset = filter_in[0].offset;

        // here ?
        if self.first_offset.is_none() {
            env.screen.clear();
            env.screen.first_offset = base_offset.clone();
            self.first_offset = base_offset.clone();
        }

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
        if env.screen.push_available() == 0 {
            env.quit = true;
        }

        env.screen.check_invariants();

        for io in filter_in.iter() {
            if env.quit {
                //    break;
            }

            match &io {
                &FilterIo {
                    data: FilterData::EndOfStream,
                    ..
                } => {
                    let eof_cpi = CodepointInfo {
                        metadata: true,
                        cp: u32_to_char('$' as u32),
                        displayed_cp: u32_to_char('$' as u32),
                        offset: Some(env.max_offset),
                        size: 0,
                        is_mark: false,
                        is_selected: false,
                        color: (255, 255, 0),
                        bg_color: CodepointInfo::default_bg_color(),
                    };
                    dbg_println!("add EOF to stream {:?}", io.offset);
                    let ret = env.screen.push(eof_cpi.clone());
                    env.screen.check_invariants();
                    if !ret.0 {
                        env.quit = true;
                        break;
                    }
                    env.screen.set_has_eof();
                }

                &FilterIo {
                    data:
                        FilterData::Unicode {
                            real_cp,
                            displayed_cp,
                            ..
                        },
                    ..
                } => {
                    let mut cpi = CodepointInfo {
                        metadata: io.metadata,
                        cp: u32_to_char(*real_cp),
                        displayed_cp: u32_to_char(*displayed_cp),
                        offset: io.offset.clone(),
                        size: io.size,
                        is_mark: false,
                        is_selected: io.is_selected,
                        color: io.color,
                        bg_color: io.bg_color,
                    };

                    let real_cp = u32_to_char(*real_cp);
                    let displayed_cp = u32_to_char(*displayed_cp);

                    // always transform displayed '\n' in ' '
                    // (fix redraw if char map filter is disabled)
                    if displayed_cp == '\n' {
                        cpi.displayed_cp = ' ';
                    }

                    let ret = env.screen.push(cpi);
                    if !ret.0 {
                        env.quit = true;
                        break;
                    }

                    if real_cp == '\n' || displayed_cp == '\n' {
                        // fill line with same offset to allow simple mouse selection
                        //
                        let mut cpi_fill = CodepointInfo::new();
                        cpi_fill.offset = io.offset.clone(); // mandatory
                        cpi_fill.cp = real_cp;
                        //cpi_fill.displayed_cp = displayed_cp;
                        //cpi_fill.displayed_cp = '_';
                        //cpi_fill.displayed_cp = '\u{21c0}';

                        // special display for this new line ?
                        if displayed_cp != real_cp {
                            cpi_fill.displayed_cp = displayed_cp;
                            cpi_fill.cp = displayed_cp;
                        }

                        cpi_fill.metadata = true;
                        cpi_fill.size = 0;
                        cpi_fill.is_selected = false;
                        cpi_fill.bg_color = CodepointInfo::default_bg_color();
                        cpi_fill.color = (115, 115, 115);

                        //env.screen.fill_with_cpi_until_eol(cpi_fill);
                        env.screen.select_next_line_index();
                    }
                }

                _ => {
                    panic!("unexpected io");
                }
            }
        }
    }

    fn finish(&mut self, _view: &View, env: &mut LayoutEnv) -> () {
        return;

        // fill with invisible eof ?
        let eof_cpi = CodepointInfo {
            metadata: true,
            cp: u32_to_char(' ' as u32),
            displayed_cp: u32_to_char(' ' as u32),
            offset: Some(env.max_offset),
            size: 0,
            is_mark: false,
            is_selected: false,
            color: (255, 255, 0),
            bg_color: CodepointInfo::default_bg_color(),
        };
        //
        loop {
            let ret = env.screen.push(eof_cpi.clone());
            env.screen.check_invariants();
            if !ret.0 {
                break;
            }
            env.screen.set_has_eof();
        }

        env.screen.check_invariants();
        env.screen.doc_max_offset = env.max_offset;
    }
}
