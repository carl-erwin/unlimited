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

fn set_first_column_color(io: &FilterIo) -> FilterIo {
    // flush new line
    let mut new_io = FilterIo {
        // general info
        metadata: false,
        style: TextStyle::new(),
        offset: io.offset,
        size: 1, // io.size,
        data: io.data.clone(),
    };

    {
        new_io.style.is_blinking = true;
        new_io.style.is_selected = true;
        new_io.style.bg_color = (0, 0, 255);
    }

    new_io
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

        dbg_println!("------------------");

        for io in filter_in.iter() {
            if let FilterIo {
                data: FilterData::Unicode { real_cp, .. },
                ..
            } = &*io
            {
                self.column_count += 1; // TODO: char width here ?

                let c = u32_to_char(*real_cp);

                if self.display_wrap && self.column_count == 1 {
                    self.accum.push(set_first_column_color(&io));
                } else {
                    self.accum.push(io.clone());
                }

                if self.display_wrap {
                    dbg_println!(
                        "column_count/max {}/{} , cp '{}', real_cp {}, accum.len() {}",
                        self.column_count,
                        self.max_column,
                        c,
                        real_cp,
                        self.accum.len()
                    );
                }

                // line is full ?
                if self.column_count > self.max_column {
                    dbg_println!("line is full");

                    // blank ?
                    if c == ' ' || c == '\n' {
                        dbg_println!("last char is blank, ignore wrapping");
                        self.column_count = 0;
                        filter_out.append(&mut self.accum);
                        self.column_count = 0;
                        continue;
                    }

                    dbg_println!("last char is NOT blank");

                    if self.accum.len() > self.max_column as usize {
                        dbg_println!("accum.len() > max_column");

                        // middle of a word
                        let mut blank_idx = 0; // option ?
                        let mut blank_offset = None; // option ?

                        for (idx, accum_io) in self.accum.iter().rev().enumerate() {
                            if let FilterIo {
                                data: FilterData::Unicode { real_cp, .. },
                                ..
                            } = &*accum_io
                            {
                                if *real_cp == ' ' as u32 {
                                    blank_idx = (self.accum.len() - idx) - 1;
                                    dbg_println!("found wrap blank @ idx {}", blank_idx);
                                    blank_offset = accum_io.offset;
                                    break;
                                }
                            }
                        }

                        if blank_offset.is_some() && blank_idx + 1 != self.max_column as usize {
                            // fake new line
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
                            }

                            let mut line: Vec<FilterIo> =
                                self.accum.drain(0..blank_idx + 1).collect();
                            line.push(fnl);
                            dbg_println!("line.len() {}", line.len());

                            filter_out.append(&mut line);
                            self.column_count = self.accum.len() as u64 % self.max_column;
                            dbg_println!("after wrap , new column count {}", self.column_count);

                            continue;
                        } else {
                            dbg_println!("NOP");
                            let mut line: Vec<FilterIo> =
                                self.accum.drain(0..(self.max_column) as usize).collect();

                            dbg_println!("line.len() {}", line.len());
                            filter_out.append(&mut line);

                            dbg_println!("accum.len() {}", self.accum.len());

                            if self.display_wrap {
                                self.accum.pop();
                                self.accum.push(set_first_column_color(&io));
                            }
                            self.column_count = 1;
                            dbg_println!("new column count {}", self.column_count);
                        }
                    } else {
                        dbg_println!("accum.len() <= max_column");

                        self.column_count = self.accum.len() as u64 % self.max_column;
                        dbg_println!("new column count {}", self.column_count);
                        panic!("");
                    }

                    continue;
                }

                // new line before max column
                if c == '\n' {
                    if self.display_wrap {
                        self.accum.pop();

                        // flush new line
                        let mut new_io = FilterIo {
                            // general info
                            metadata: false,
                            style: TextStyle::new(),
                            offset: io.offset,
                            size: 1, // io.size,
                            data: FilterData::Unicode {
                                displayed_cp: '\n' as u32,
                                real_cp: *real_cp,
                                fragment_flag: false,
                                fragment_count: 0,
                            },
                        };

                        {
                            new_io.style.is_blinking = true;
                            new_io.style.is_selected = true;
                            new_io.style.bg_color = (0, 255, 0);
                        }
                        self.accum.push(new_io);
                    }
                    filter_out.append(&mut self.accum);
                    self.column_count = 0;
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
