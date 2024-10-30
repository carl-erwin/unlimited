/*
    TODO(ceg): filters dependencies: check in view's filter_array that
            dep.index < cur_filter.index or (and WARN)
            we can push multiple times new instance of a filter :-)

            prerequisite:
            - tab/words/"invisible chars" expansion before: ('\t' -> ' ' should be done before)

            NB: accum.len() != column count

    NB: some plugins can expand a given byte to multiple chars with the same offset
        We must accumulate until the offset changes and move the whole "group" to the next line
        and update the column count accordingly.
*/

use parking_lot::RwLock;
use std::rc::Rc;

use crate::core::bench_to_eof;
use crate::core::view::ContentFilter;
use crate::core::view::FilterData;
use crate::core::view::FilterIo;
use crate::core::view::LayoutEnv;
use crate::core::Editor;
use crate::core::EditorEnv;

use crate::core::view::View;

use crate::core::codec::text::u32_to_char;

use crate::core::codepointinfo::TextStyle;

#[derive(Debug, Clone, Copy)]
enum WordWrapState {
    Init,
    InWord,
    Blank,
    EndOfLine,
}

#[derive(Debug, Clone)]
pub struct WordWrapElement {
    io: FilterIo,
    c: char,
    char_width: u32,
    offset: Option<u64>,
}

impl WordWrapElement {
    pub fn new(io: FilterIo, c: char, char_width: u32, offset: &Option<u64>) -> Self {
        WordWrapElement {
            io,
            c,
            char_width,
            offset: offset.clone(),
        }
    }

    pub fn from_elm(elm: &WordWrapElement) -> Self {
        WordWrapElement {
            io: elm.io.clone(),
            c: elm.c,
            char_width: elm.char_width,
            offset: elm.offset.clone(),
        }
    }
}

pub struct WordWrapFilter {
    quit: bool,
    bench: bool,
    state: WordWrapState,

    max_column: u64,
    max_row: u64,
    column_count: u64,
    accum: Vec<FilterIo>,

    prev_offset: Option<u64>,

    lines: Vec<Vec<WordWrapElement>>,
    line_index: usize,
}

impl WordWrapFilter {
    pub fn new() -> Self {
        WordWrapFilter {
            quit: false,
            bench: false,
            state: WordWrapState::Init,
            max_column: 0,
            max_row: 0,
            column_count: 0,
            accum: vec![],
            prev_offset: None,
            lines: vec![Vec::new()],
            line_index: 0,
        }
    }

    pub fn reset(&mut self, env: &LayoutEnv) -> &mut Self {
        self.quit = false;

        self.state = WordWrapState::Init;
        self.max_column = env.screen.width() as u64;
        self.max_row = env.screen.height() as u64;
        self.column_count = 0;
        self.accum = Vec::new();

        self.prev_offset = None;

        self.lines = vec![Vec::new()];
        self.line_index = 0;

        self.bench = bench_to_eof();
        self
    }
}

fn build_wrap_point_new_line(blank_offset: Option<u64>) -> FilterIo {
    FilterIo {
        metadata: true,
        style: TextStyle::new(),
        offset: blank_offset,
        size: 0,
        data: FilterData::TextInfo {
            real_cp: '\n' as u32,
            displayed_cp: ' ' as u32,
        },
    }
}

fn build_wrap_point_delimiter(blank_offset: Option<u64>) -> FilterIo {
    let mut io = FilterIo {
        metadata: true,
        style: TextStyle::new(), // TODO(ceg): customize
        offset: blank_offset,
        size: 0,
        data: FilterData::TextInfo {
            real_cp: '\\' as u32,
            displayed_cp: '\\' as u32,
        },
    };

    io.style.color = (255, 255, 0);

    io
}

impl ContentFilter<'_> for WordWrapFilter {
    fn name(&self) -> &'static str {
        &"WordWrapFilter"
    }

    fn setup(
        &mut self,
        _editor: &mut Editor<'static>,
        _editor_env: &mut EditorEnv<'static>,
        env: &mut LayoutEnv,
        _view: &Rc<RwLock<View>>,
        _parent_view: Option<&View<'static>>,
    ) {
        dbg_println!(
            "WORD WRAP (max col {}, max row {})--------------------",
            env.screen.width(),
            env.screen.height(),
        );

        self.reset(&env);
    }

    fn run(
        &mut self,
        _view: &View,
        env: &mut LayoutEnv,
        filter_in: &[FilterIo],
        filter_out: &mut Vec<FilterIo>,
    ) {
        if self.max_column <= 2
        /* TODO(ceg) screen.max_char_width() */
        {
            *filter_out = filter_in.to_vec();
            return;
        }

        if self.quit && !self.bench {
            self.quit = false; // only bench
            return;
        }

        for io in filter_in.iter() {
            if self.quit && !self.bench {
                dbg_println!("WORD WRAP: env.quit detected");
                break;
            }

            match io {
                FilterIo {
                    offset,
                    data:
                        FilterData::TextInfo {
                            real_cp,
                            displayed_cp,
                            ..
                        },
                    ..
                } => {
                    match (self.prev_offset, *offset) {
                        (Some(prev_off), Some(e_off)) => {
                            assert!(prev_off <= e_off);
                        }
                        _ => {}
                    }

                    // prepare element info
                    let c = u32_to_char(*displayed_cp);
                    let real_c = u32_to_char(*real_cp);
                    let c_width = env.screen.char_width(c) as u64;
                    let io = io.clone();

                    let e = WordWrapElement::new(io, c, c_width as u32, offset);

                    self.column_count += c_width;
                    self.lines[self.line_index].push(e);

                    let next_state = match real_c {
                        '\n' => WordWrapState::EndOfLine,
                        ' ' | '\t' => {
                            // TODO(ceg): save last_blank idx, offset and reuse
                            WordWrapState::Blank
                        }
                        _ => WordWrapState::InWord,
                    };

                    self.state = match (self.state, next_state) {
                        (_, WordWrapState::EndOfLine) => {
                            //  dbg_println!("WORD WRAP: flush line self.line_index {}", self.line_index);

                            // flush current line
                            for e in &self.lines[self.line_index] {
                                filter_out.push(e.io.clone());
                            }
                            self.column_count = 0;
                            self.line_index += 1;
                            self.lines.push(vec![]);

                            if self.line_index >= self.max_row as usize {
                                self.quit = true;
                            }

                            WordWrapState::Init
                        }

                        _ => next_state,
                    };

                    if self.column_count > self.max_column {
                        // need wrap

                        let mut split_index = None;

                        // wrap offset ?
                        if true {
                            let l = &self.lines[self.line_index];
                            // if prev offset == cur_offset

                            match (self.prev_offset, *offset) {
                                (Some(prev_off), Some(e_off)) => {
                                    if prev_off == e_off {
                                        // need wrap

                                        // find first != offset
                                        for (idx, e) in l.iter().rev().enumerate() {
                                            if e.offset.unwrap() != e_off && idx > 1 {
                                                // found split point
                                                split_index = Some(l.len() - idx);
                                                break;
                                            }
                                        }
                                    }
                                }
                                _ => {}
                            }
                        }

                        // wrap word ?
                        if split_index.is_none() {
                            let l = &self.lines[self.line_index];
                            // look for first space (word start)
                            for (idx, e) in l.iter().rev().enumerate() {
                                if e.c == ' ' {
                                    // char is space and not on last column
                                    if idx > 1 {
                                        // char is not on last column

                                        // set split point
                                        split_index = Some(l.len() - idx);
                                    }
                                    break;
                                }
                            }
                        }

                        // need wrap
                        if split_index.is_some() {
                            let split_index = split_index.unwrap();

                            let l = &mut self.lines[self.line_index];
                            // cut
                            let (left, right) = l.split_at(split_index);
                            let split_offset = if let Some(l) = left.last() {
                                l.offset
                            } else {
                                None
                            };

                            let mut next_line = vec![];
                            let next_elms = right;
                            for e in next_elms {
                                next_line.push(WordWrapElement::from_elm(&e));
                            }

                            // erase after split
                            let mut restart_col = 0;
                            let n = l.len() - split_index;
                            for _ in 0..n {
                                if let Some(e) = l.pop() {
                                    restart_col += e.char_width as u64;
                                }
                            }

                            if l.len() <= self.max_column as usize {
                                //
                                let arrow = build_wrap_point_delimiter(split_offset);
                                let e = WordWrapElement::new(arrow, c, 1, &split_offset);
                                l.push(e);

                                // padding ?
                                if l.len() < self.max_column as usize {
                                    let fnl = build_wrap_point_new_line(split_offset);
                                    let e = WordWrapElement::new(fnl, c, 1, &split_offset);
                                    l.push(e);
                                }
                            }

                            // flush current line
                            for e in l.iter() {
                                filter_out.push(e.io.clone());
                            }

                            self.line_index += 1;
                            self.lines.push(next_line);

                            self.column_count = restart_col;

                            if self.line_index >= self.max_row as usize {
                                self.quit = true;
                            }
                        }

                        if split_index.is_none() {
                            // long word
                            let last_e = self.lines[self.line_index].pop().unwrap();

                            // self.flush_current_line(&mut filter_out, vec![last_e]);
                            for e in &self.lines[self.line_index] {
                                filter_out.push(e.io.clone());
                            }
                            // flush
                            self.column_count = c_width;
                            self.lines.push(vec![last_e]);
                            self.line_index += 1;

                            if self.line_index >= self.max_row as usize {
                                self.quit = true;
                            }
                        }
                    }

                    self.prev_offset = *offset;
                }

                FilterIo {
                    data: FilterData::EndOfStream,
                    ..
                }
                | FilterIo {
                    data: FilterData::CustomLimitReached,
                    ..
                } => {
                    /*  eof  */
                    dbg_println!("WORD WRAP:  {:?}", io);
                    if !self.quit {
                        // flush current line
                        for e in &self.lines[self.line_index] {
                            filter_out.push(e.io.clone());
                        }

                        filter_out.push(io.clone());
                    }

                    self.quit = true;
                }

                _ => {
                    /*  unhandled input type */
                    dbg_println!(
                        "WORD WRAP:  unhandled input type  self.column_count {} , self.max_column {}",
                        self.column_count,
                        self.max_column
                    );
                    // flush current line
                    // filter_out.push(io.clone());
                }
            }
        }

        if self.quit {
            // NB: in benchmark mode this will allocate until EOF ...
            self.lines = vec![Vec::new()];
            self.line_index = 0;

            self.accum.clear();
        }
    }

    fn finish(&mut self, _view: &View, _env: &mut LayoutEnv) {
        dbg_println!("WRAP: FINISH");
    }
}
