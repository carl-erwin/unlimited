use crate::core::view::layout::ContentFilter;
use crate::core::view::layout::FilterData;
use crate::core::view::layout::FilterIo;
use crate::core::view::layout::LayoutEnv;

use crate::core::view::View;

use super::TextModeContext;
use crate::core::codec::text::u32_to_char;

use crate::core::codepointinfo::TextStyle;

pub struct WordWrapFilter {
    max_column: u64,
    column_count: u64,
    accum: Vec<FilterIo>,
    display_wrap: bool,
}

impl WordWrapFilter {
    pub fn new() -> Self {
        WordWrapFilter {
            max_column: 0,
            column_count: 0,
            accum: vec![],
            display_wrap: false,
        }
    }
}

impl ContentFilter<'_> for WordWrapFilter {
    fn name(&self) -> &'static str {
        &"WordWrapFilter"
    }

    fn setup(&mut self, env: &mut LayoutEnv, view: &View) {
        self.max_column = env.screen.width() as u64;
        self.column_count = 0;
        self.accum = Vec::new();

        if view.check_mode_ctx::<TextModeContext>("text-mode") {
            let tm = view.mode_ctx::<TextModeContext>("text-mode");
            self.display_wrap = tm.display_word_wrap;
        }
    }

    /*
        TODO: filters dependencies: check in view's filter_array that
        dep.index < cur_filter.index or (and WARN)
        we can push multiple times new instance of a filter :-)

        prerequisite:
        - tab expansion before: ('\t' -> ' ' should be done before)

    display offset in status

    */
    fn run(
        &mut self,
        _view: &View,
        _env: &mut LayoutEnv,
        filter_in: &Vec<FilterIo>,
        filter_out: &mut Vec<FilterIo>,
    ) {
        if self.max_column <= 2 {
            *filter_out = filter_in.clone();
            return;
        }

        for io in filter_in.iter() {
            if let FilterIo {
                data: FilterData::Unicode { real_cp, .. },
                ..
            } = &*io
            {
                let c = u32_to_char(*real_cp);
                self.column_count += 1;

                if self.column_count > self.max_column {
                    // flush accum

                    // look for first space backward
                    let mut blank_idx = 0; // option ?
                    let mut blank_offset = None; // option ?

                    for (idx, accum_io) in self.accum.iter().rev().enumerate() {
                        if let FilterIo {
                            data: FilterData::Unicode { real_cp, .. },
                            ..
                        } = &*accum_io
                        {
                            if *real_cp == '\n' as u32 || *real_cp == ' ' as u32 {
                                blank_idx = (self.accum.len() - idx) - 1;
                                blank_offset = accum_io.offset;
                                break;
                            }
                        }
                    }

                    if blank_idx > 0 && blank_idx + 1 != self.max_column as usize {
                        let mut fnl = FilterIo {
                            // general info
                            metadata: true,
                            style: TextStyle::new(), // TODO: customize
                            offset: blank_offset,
                            size: 0,
                            data: FilterData::Unicode {
                                displayed_cp: '\\' as u32,
                                real_cp: '\n' as u32,
                                fragment_flag: false,
                                fragment_count: 0,
                            },
                        };

                        if true || self.display_wrap {
                            fnl.style.color = (255, 255, 0);
                            //fnl.style.bg_color = (0, 255, 0);
                        }

                        let mut line: Vec<FilterIo> = self.accum.drain(0..blank_idx + 1).collect();
                        line.push(fnl);

                        filter_out.append(&mut line);
                    } else {
                        let drain_size = if self.accum.len() < self.max_column as usize {
                            self.accum.len()
                        } else {
                            self.max_column as usize
                        };

                        let mut line: Vec<FilterIo> = self.accum.drain(0..drain_size).collect();
                        filter_out.append(&mut line);
                    }

                    self.column_count = self.accum.len() as u64 % self.max_column;
                }

                match c {
                    '\n' => {
                        // flush accum
                        filter_out.append(&mut self.accum);

                        // flush new line
                        let mut nl = io.clone();
                        if self.display_wrap {
                            nl.style.is_selected = true;
                            nl.style.bg_color = (255, 0, 0);
                        }
                        filter_out.push(nl);

                        self.column_count = 0;
                    }

                    _ => {
                        // accum
                        self.accum.push(io.clone());
                    }
                }
            } else {
                self.accum.push(io.clone());
                filter_out.append(&mut self.accum);
            }
        }
    }

    fn finish(&mut self, _view: &View, _env: &mut LayoutEnv) -> () {
        // default
        // filter_out.append(&mut self.accum);
    }
}
