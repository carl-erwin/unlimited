use crate::core::view::layout::Filter;
use crate::core::view::layout::FilterData;
use crate::core::view::layout::FilterIo;
use crate::core::view::layout::LayoutEnv;

use crate::core::view::View;

use super::TextModeContext;
use crate::core::codec::text::u32_to_char;

use crate::core::codepointinfo::CodepointInfo;

pub struct WordWrapFilter {
    max_column: u64,
    column_count: u64,
    accum: Vec<FilterIo>,
    display_wrap: bool,
    flush_count: usize,
    last_pushed_offset: u64,
}

impl WordWrapFilter {
    pub fn new() -> Self {
        WordWrapFilter {
            max_column: 0,
            column_count: 0,
            accum: vec![],
            display_wrap: false,
            flush_count: 0,
            last_pushed_offset: 0,
        }
    }
}

impl Filter<'_> for WordWrapFilter {
    fn name(&self) -> &'static str {
        &"WordWrapFilter"
    }

    fn setup(&mut self, env: &LayoutEnv, view: &View) {
        self.max_column = env.screen.width() as u64;
        self.column_count = 0;
        self.accum = Vec::new();
        let tm = view.mode_ctx::<TextModeContext>("text-mode");
        self.display_wrap = tm.display_word_wrap;
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
        // dbg_println!("filter_in.len() {}", filter_in.len());
        for io in filter_in.iter() {
            if let FilterIo {
                data: FilterData::Unicode { real_cp, .. },
                ..
            } = &*io
            {
                if self.max_column <= 2 {
                    filter_out.push(io.clone());
                    continue;
                }

                // dbg_println!("     WORD WRAP cur IO {:?}", io);
                //self.display_wrap = true;

                let c = u32_to_char(*real_cp);

                // flush ?
                if self.column_count == self.max_column {
                    // "inject" fake "line separator"
                    if !self.accum.is_empty() && (c != '\n' && c != ' ') && self.flush_count > 0 {
                        let off = self.accum[0].offset.unwrap();
                        if off > 0 {
                            let mut fnl = FilterIo {
                                // general info
                                metadata: true,
                                is_selected: false,
                                offset: Some(off),
                                color: CodepointInfo::default_color(), // TODO: customize
                                bg_color: CodepointInfo::default_bg_color(), // TODO: customize
                                size: 0,
                                data: FilterData::Unicode {
                                    displayed_cp: '\\' as u32,
                                    real_cp: '\\' as u32,
                                    fragment_flag: false,
                                    fragment_count: 0,
                                },
                            };

                            let fscp = FilterIo {
                                metadata: true,
                                is_selected: false,
                                offset: Some(off),
                                color: CodepointInfo::default_color(),
                                bg_color: CodepointInfo::default_bg_color(),
                                size: 0,
                                data: FilterData::Unicode {
                                    displayed_cp: ' ' as u32,
                                    real_cp: ' ' as u32,
                                    fragment_flag: false,
                                    fragment_count: 0,
                                },
                            };

                            if true || self.display_wrap {
                                fnl.color = (255, 255, 0);
                                //fnl.bg_color = (0, 255, 0);
                            }
                            // dbg_println!("WORD WRAP FAKE NEW LINE @OFFSET {:?}", fnl.offset);
                            // dbg_println!(
                            //     "WORD ACCUM START offset =  @OFFSET {:?}",
                            //     self.accum[0].offset
                            // );
                            // dbg_println!("ACCUM LEN {}", self.accum.len());

                            filter_out.push(fnl);
                            //
                            // fill remain spaces with 'fake spaces'
                            for _ in 0..self.accum.len() {
                                filter_out.push(fscp.clone());
                            }

                            //
                            self.column_count = 0;
                            self.flush_count = 0;
                        }
                    }
                    let n = self.accum.len() as u64;
                    self.column_count = n % self.max_column;
                }

                self.column_count += 1;

                match c {
                    '\n' => {
                        let mut nl = io.clone();
                        if self.display_wrap {
                            nl.is_selected = true;
                            nl.color = (255, 0, 0);
                        }
                        self.accum.push(nl);
                        //
                        // dbg_println!(
                        //     " > (NewLine) @OFFSET {:?} FLUSH ACCUM LEN {}",
                        //     io.offset,
                        //     self.accum.len()
                        // );
                        filter_out.append(&mut self.accum);
                        self.column_count = 0;
                        self.flush_count = 0;
                        self.last_pushed_offset = io.offset.unwrap() + 1;
                    }
                    ' ' => {
                        // flush "word"
                        let mut space = io.clone();
                        if self.display_wrap {
                            space.is_selected = true;
                            space.color = (0, 0, 255);
                        }
                        self.last_pushed_offset = io.offset.unwrap() + 1;
                        self.accum.push(space);
                        //
                        // dbg_println!(
                        //     " > (SPACE) @OFFSET {:?} FLUSH ACCUM LEN {}",
                        //     io.offset,
                        //     self.accum.len()
                        // );
                        filter_out.append(&mut self.accum);
                        self.flush_count += 1;
                    }
                    _ => {
                        // dbg_println!(" > ACCUM io OFFSET {:?}", io.offset);
                        self.accum.push(io.clone());
                    }
                }
            } else {
                // dbg_println!(" > (Non Unicode) ACCUM io OFFSET {:?}", io.offset);
                self.accum.push(io.clone());
                filter_out.append(&mut self.accum);
            }
        }

        if !self.accum.is_empty() {
            // finish ?
        }
    }
}
