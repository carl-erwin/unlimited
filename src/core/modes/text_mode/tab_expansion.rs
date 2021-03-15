use crate::core::codec::text::u32_to_char;
use crate::core::view::layout::Filter;
use crate::core::view::layout::FilterData;
use crate::core::view::layout::FilterIoData;
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
                data: FilterData::Unicode { real_cp, .. },
                ..
            } = &*io
            {
                if
                /*self.prev_cp == '\r' ||*/
                self.prev_cp == '\n' {
                    self.column_count = 0;
                }

                match (self.prev_cp, u32_to_char(*real_cp)) {
                    (_, '\t') => {
                        self.prev_cp = '\t';

                        let tab_size = 8;
                        let padding = tab_size - (self.column_count % tab_size);

                        for (idx, _) in (0..padding).enumerate() {
                            let mut new_io = FilterIoData::replace_codepoint(io, ' ');
                            if env.graphic_display {
                                new_io.color = (242, 71, 132); // purple-like
                            } else {
                                new_io.color = (128, 0, 128); // magenta
                            }
                            new_io.size = if idx == 0 { io.size } else { 0 };
                            new_io.metadata = if idx == 0 { io.metadata } else { true };
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
