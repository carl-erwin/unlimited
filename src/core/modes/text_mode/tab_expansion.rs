use parking_lot::RwLock;
use std::rc::Rc;

use crate::core::codec::text::u32_to_char;
use crate::core::view::layout::ContentFilter;
use crate::core::view::layout::FilterData;
use crate::core::view::layout::FilterIo;
use crate::core::view::layout::LayoutEnv;
use crate::core::view::View;
use crate::core::Editor;

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

impl ContentFilter<'_> for TabFilter {
    fn name(&self) -> &'static str {
        &"TabFilter"
    }

    fn setup(&mut self, _editor: &Editor, _env: &mut LayoutEnv, _view: &Rc<RwLock<View>>) {
        self.prev_cp = '\u{0}';
        self.column_count = 0;
    }

    fn run(
        &mut self,
        _view: &View,
        env: &mut LayoutEnv,
        filter_in: &Vec<FilterIo>,
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
                            if env.graphic_display {
                                new_io.style.color = (242, 71, 132); // purple-like
                            } else {
                                new_io.style.color = (128, 0, 128); // magenta
                            }
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
