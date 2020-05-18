use crate::core::codec::text::u32_to_char;
use crate::core::view::layout::ContentFilter;
use crate::core::view::layout::FilterData;
use crate::core::view::layout::FilterIo;
use crate::core::view::layout::LayoutEnv;
use crate::core::view::View;

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

    fn setup(&mut self, _env: &mut LayoutEnv, _view: &View) {
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
                data: FilterData::Unicode { real_cp, .. },
                ..
            } = &*io
            {
                if
                /*self.prev_cp == '\r' ||*/
                self.prev_cp == '\n' {
                    //dbg_println!(" TAB LF at col {}, reset col", self.column_count);
                    self.column_count = 0;
                }

                match (self.prev_cp, u32_to_char(*real_cp)) {
                    (_, '\t') => {
                        self.prev_cp = '\t';
                        // TODO: setup
                        let tab_size = 8;
                        let padding = tab_size - (self.column_count % tab_size);

                        //dbg_println!(" TAB column count = {}", self.column_count);
                        //dbg_println!(" TAB padding = {}", padding);

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
                            //dbg_println!("  TAB push spc");
                            self.column_count += 1;
                        }
                    }

                    (_, codepoint) => {
                        // dbg_println!(" TAB char({}) at col {}", codepoint, self.column_count);
                        self.prev_cp = codepoint;
                        filter_out.push(io.clone());
                        self.column_count += 1;
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
