use std::collections::HashMap;

use crate::core::view::layout::Filter;
use crate::core::view::layout::FilterData;
use crate::core::view::layout::FilterIo;
use crate::core::view::layout::LayoutEnv;
use crate::core::view::View;

use crate::core::codepointinfo::CodepointInfo;
use crate::core::codepointinfo::TextStyle;

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
}

impl ScreenFilter {
    pub fn new() -> Self {
        ScreenFilter {
            // data
            first_offset: None,
            screen_is_full: false,
        }
    }
}

impl Filter<'_> for ScreenFilter {
    fn name(&self) -> &'static str {
        &"ScreenFilter"
    }

    fn setup(&mut self, _env: &LayoutEnv, _view: &View) {
        self.first_offset = None;
        self.screen_is_full = false;
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
                    let mut style = TextStyle::new();
                    style.color = (255, 255, 0);

                    let eof_cpi = CodepointInfo {
                        metadata: true,
                        cp: u32_to_char('$' as u32),
                        displayed_cp: u32_to_char('$' as u32),
                        offset: Some(env.max_offset),
                        size: 0,
                        style,
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
                        style: io.style,
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

                        cpi_fill.size = 0;
                        cpi_fill.style.color = (115, 115, 115);

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
        env.screen.check_invariants();
        env.screen.doc_max_offset = env.max_offset;
    }
}
