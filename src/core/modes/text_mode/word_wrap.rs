use parking_lot::RwLock;
use std::rc::Rc;

use crate::core::view::ContentFilter;
use crate::core::view::FilterData;
use crate::core::view::FilterIo;
use crate::core::view::LayoutEnv;
use crate::core::Editor;

use crate::core::view::View;

use super::TextModeContext;
use crate::core::codec::text::u32_to_char;

use crate::core::codepointinfo::TextStyle;

pub struct WordWrapFilter {
    max_column: u64,
    column_count: u64,
    accum: Vec<FilterIo>,
    display_wrap: bool,

    blank_column_idx: u64,
    blank_accum_idx: u64,
    blank_offset: Option<u64>,
}

impl WordWrapFilter {
    pub fn new() -> Self {
        WordWrapFilter {
            max_column: 0,
            column_count: 0,
            accum: vec![],
            display_wrap: false,
            blank_column_idx: 0,
            blank_accum_idx: 0,
            blank_offset: None,
        }
    }
}

fn set_io_color(io: &FilterIo, color: (u8, u8, u8)) -> FilterIo {
    // flush new line
    let mut new_io = FilterIo {
        // general info
        metadata: false,
        style: TextStyle::new(),
        offset: io.offset,
        size: io.size,
        data: io.data.clone(),
    };

    {
        new_io.style.is_blinking = true;
        new_io.style.is_selected = true;
        new_io.style.bg_color = color;
    }

    new_io
}

fn set_first_column_color(io: &FilterIo) -> FilterIo {
    set_io_color(io, (0, 0, 255))
}

fn build_wrap_point_io(blank_offset: Option<u64>) -> FilterIo {
    FilterIo {
        // general info
        metadata: true,
        style: TextStyle::new(), // TODO(ceg): customize
        offset: blank_offset,
        size: 0,
        data: FilterData::TextInfo {
            real_cp: '\n' as u32,
            displayed_cp: '\\' as u32,
        },
    }
}

impl ContentFilter<'_> for WordWrapFilter {
    fn name(&self) -> &'static str {
        &"WordWrapFilter"
    }

    fn setup(
        &mut self,
        _editor: &Editor,
        env: &mut LayoutEnv,
        view: &Rc<RwLock<View>>,
        _parent_view: Option<&View<'static>>,
    ) {
        self.max_column = env.screen.width() as u64;
        self.column_count = 0;
        self.accum = Vec::new();

        // the screen is the final output
        // TODO(ceg): ask env.screen.cp_width(cp) -> nb_cells
        // or embed cp_width in FilterIo meta ?

        let v = view.read();
        if v.check_mode_ctx::<TextModeContext>("text-mode") {
            let tm = v.mode_ctx::<TextModeContext>("text-mode");
            self.display_wrap = tm.display_word_wrap;
        }
    }

    /*
            TODO(ceg): filters dependencies: check in view's filter_array that
            dep.index < cur_filter.index or (and WARN)
            we can push multiple times new instance of a filter :-)

            prerequisite:
            - tab expansion before: ('\t' -> ' ' should be done before)

        display offset in status


            NB: accum.len() != column count

    BEFORE:
            |  0    1    2    3    4    5    6    7    8 |  9  |        max_col = 10
            | xx |    |   ww    | xx | xx | xx | xx | xx | xx  |
                   ^      ^                                       ^
                   |      |_ wide char (2 cols)                   |
                   |                                              |
             blank_col_idx                                    colum_count = 10


            |  0    1    2    3    4    5    6    7    8 |  9  | max_col = 10
            | xx |    |   ww    | xx | xx | xx | xx | xx | xx  | <ADD>
                   ^      ^
                   |      |_ wide char (2 cols)                   ^
                   |                                              |
             blank_col_idx                                    colum_count


            column_count + width(<ADD>) >= max_col  : compute wrap


        */

    fn run(
        &mut self,
        _view: &View,
        env: &mut LayoutEnv,
        filter_in: &Vec<FilterIo>,
        filter_out: &mut Vec<FilterIo>,
    ) {
        if self.max_column <= 2
        /* TODO(ceg) screen.max_char_width() */
        {
            *filter_out = filter_in.clone();
            return;
        }

        for io in filter_in.iter() {
            if let FilterIo {
                data: FilterData::TextInfo { real_cp, .. },
                ..
            } = &*io
            {
                let c = u32_to_char(*real_cp);
                let width = env.screen.char_width(c) as u64;

                // width overflow  ?
                if self.column_count + width > self.max_column {
                    // not blank and accum > max column

                    // have previous blank ? => split accum after blank, insert '\' wrap point
                    if self.blank_offset.is_some()
                        && c != ' '
                        && c != '\n' // user option ?
                        && self.blank_column_idx > 0
                        && self.blank_column_idx + 1 != self.max_column
                    {
                        let mut fnl = build_wrap_point_io(self.blank_offset);

                        // TODO(ceg): add use option
                        if true || self.display_wrap {
                            fnl.style.color = (255, 255, 0); // yellow '\'
                        }

                        let mut new = self.accum.split_off(self.blank_accum_idx as usize + 1);

                        if self.display_wrap {
                            // replace front/back
                            if !new.is_empty() {
                                let nio = new.remove(0);
                                new.insert(0, set_io_color(&nio, (255, 0, 0)));
                            }
                        }

                        filter_out.append(&mut self.accum);
                        self.accum = new;

                        filter_out.push(fnl);

                        // "current word" size
                        self.column_count = self.max_column - self.blank_column_idx - 1;

                        self.blank_accum_idx = 0;
                        self.blank_offset = None;
                        self.blank_column_idx = 0;
                    } else {
                        if self.display_wrap {
                            // replace  last char
                            if !self.accum.is_empty() {
                                let nio = self.accum.pop().unwrap();
                                self.accum.push(set_io_color(&nio, (0, 255, 0)));
                            }
                        }

                        // FLUSH
                        filter_out.append(&mut self.accum);
                        self.column_count = 0;
                    }
                    /* FALLTHROUGH  */
                }

                // APPEND
                if self.display_wrap && self.column_count == 0 {
                    self.accum.push(set_first_column_color(&io));
                } else {
                    self.accum.push(io.clone());
                }

                // self.column_count + width < self.max_column
                self.column_count += width; // char fits

                // check
                // new line before max column
                if c == '\n' {
                    if self.display_wrap {
                        self.accum.pop();
                        self.accum.push(set_io_color(&io, (128, 255, 255)));
                    }
                    // restart
                    filter_out.append(&mut self.accum);
                    self.column_count = 0;
                    self.blank_accum_idx = 0;
                    self.blank_offset = None;
                    self.blank_column_idx = 0;
                }

                if c == ' ' {
                    // remember blank idx/offset to build wrap point
                    self.blank_column_idx = self.column_count - width;
                    self.blank_accum_idx = self.accum.len() as u64 - 1;
                    self.blank_offset = io.offset;

                    if self.display_wrap && self.blank_column_idx > 0 {
                        self.accum.pop();
                        self.accum.push(set_io_color(&io, (255, 255, 0)));
                    }
                }
            } else {
                /*  unhandled input type */
                self.accum.push(io.clone());
                filter_out.append(&mut self.accum);
            }
        }
    }

    fn finish(&mut self, _view: &View, _env: &mut LayoutEnv) {
        dbg_println!("WRAP: FINISH");
        // TODO finish count ...
        // self.finish_count += 1;
    }
}
