use parking_lot::RwLock;
use std::rc::Rc;

use crate::core::codec::text::u32_to_char;
use crate::core::view::ContentFilter;
use crate::core::view::FilterData;
use crate::core::view::FilterIo;
use crate::core::view::LayoutEnv;
use crate::core::view::View;
use crate::core::Editor;

pub struct ShowTrailingSpaces {
    accum: Vec<FilterIo>,
    trailing_color: (u8, u8, u8),
}

impl ShowTrailingSpaces {
    pub fn new() -> Self {
        ShowTrailingSpaces {
            accum: vec![],
            trailing_color: (255, 0, 0), // red
        }
    }

    pub fn colorize_accum(&mut self) {
        for io in self.accum.iter_mut() {
            io.style.bg_color = self.trailing_color;
        }
    }

    pub fn flush_io(&mut self, filter_out: &mut Vec<FilterIo>, io: FilterIo) {
        filter_out.append(&mut self.accum);
        filter_out.push(io);
    }
}

impl ContentFilter<'_> for ShowTrailingSpaces {
    fn name(&self) -> &'static str {
        &"ShowTrailingSpaces"
    }

    fn setup(
        &mut self,
        _editor: &Editor,
        _env: &mut LayoutEnv,
        _view: &Rc<RwLock<View>>,
        _parent_view: Option<&View<'static>>,
    ) {
        // TODO(ceg): define and load user preferences
        // modes["core.showTrailingSpaces"]["trailing_color"]" = "(255,0,255,0)"

        // reset previous
        self.accum.clear();
    }

    fn run(
        &mut self,
        _view: &View,
        _env: &mut LayoutEnv,
        filter_in: &Vec<FilterIo>,
        filter_out: &mut Vec<FilterIo>,
    ) {
        for io in filter_in.iter() {
            match io {
                &FilterIo {
                    data: FilterData::EndOfStream | FilterData::StreamLimitReached,
                    ..
                } => {
                    self.colorize_accum();
                    self.flush_io(filter_out, io.clone());
                }

                FilterIo {
                    data: FilterData::TextInfo { real_cp, .. },
                    ..
                } => match u32_to_char(*real_cp) {
                    '\n' => {
                        self.colorize_accum();
                        self.flush_io(filter_out, io.clone());
                    }

                    ' ' => {
                        self.accum.push(io.clone());
                    }

                    _ => {
                        self.flush_io(filter_out, io.clone());
                    }
                },

                _ => {
                    // TextInfo
                    self.flush_io(filter_out, io.clone());
                }
            }
        }
    }
}
