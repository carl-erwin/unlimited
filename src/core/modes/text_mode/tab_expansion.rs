use parking_lot::RwLock;
use std::rc::Rc;

use crate::core::codec::text::u32_to_char;
use crate::core::view::ContentFilter;
use crate::core::view::FilterData;
use crate::core::view::FilterIo;
use crate::core::view::LayoutEnv;
use crate::core::view::View;
use crate::core::Editor;

pub struct TabFilter {
    prev_cp: char,
    column_count: u64,
    tab_color: (u8, u8, u8),
}

impl TabFilter {
    pub fn new() -> Self {
        TabFilter {
            prev_cp: '\u{0}',
            column_count: 0,
            // tab_color: (242, 71, 132), // pink-like
            // tab_color: (40, 23, 51) // purple-like
            // tab_color: (59, 34, 76)
            tab_color: (128, 75, 165),
        }
    }
}

impl ContentFilter<'_> for TabFilter {
    fn name(&self) -> &'static str {
        &"TabFilter"
    }

    fn setup(
        &mut self,
        _editor: &Editor<'static>,
        env: &mut LayoutEnv,
        _view: &Rc<RwLock<View>>,
        _parent_view: Option<&View<'static>>,
    ) {
        self.prev_cp = '\u{0}';
        self.column_count = 0;
        if env.graphic_display {
            // self.tab_color = (242, 71, 132); // purple-like
        } else {
            self.tab_color = (128, 0, 128); // magenta
        }
    }

    fn run(
        &mut self,
        _view: &View,
        _env: &mut LayoutEnv,
        filter_in: &[FilterIo],
        filter_out: &mut Vec<FilterIo>,
    ) {
        for io in filter_in.iter() {
            if let FilterIo {
                data: FilterData::TextInfo { real_cp, .. },
                ..
            } = &*io
            {
                let codepoint = u32_to_char(*real_cp);
                match codepoint {
                    '\t' => {
                        self.prev_cp = '\t';
                        // TODO(ceg): setup
                        let tab_size = 8;
                        let padding = tab_size - (self.column_count % tab_size);

                        //dbg_println!(" TAB column count = {}, padding = {}", self.column_count, padding);

                        for (idx, _) in (0..padding).enumerate() {
                            // \t -> ' '
                            let mut new_io = FilterIo::replace_displayed_codepoint(io, ' ');
                            new_io.style.color = self.tab_color;
                            new_io.size = if idx == 0 { io.size } else { 0 };
                            new_io.metadata = if idx == 0 { io.metadata } else { true };
                            filter_out.push(new_io);
                        }
                        self.column_count += padding;
                    }

                    _ => {
                        if codepoint == '\n' {
                            self.column_count = 0;
                        } else {
                            self.column_count += 1;
                        }
                        self.prev_cp = codepoint;
                        filter_out.push(io.clone());
                    }
                }
            } else {
                // not unicode
                // dbg_println!(" TAB EXP no unicode: {:?}", io);
                filter_out.push(io.clone());
            }
        }
    }
}
