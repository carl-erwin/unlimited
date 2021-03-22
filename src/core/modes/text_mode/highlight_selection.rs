use crate::core::view::layout::Filter;
use crate::core::view::layout::FilterIo;
use crate::core::view::layout::LayoutEnv;

use crate::core::codepointinfo::CodepointInfo;
use crate::core::codepointinfo::TextStyle;

use crate::core::view::View;

use super::TextModeContext;

pub struct HighlightSelectionFilter {
    sel_start_offset: u64,
    sel_end_offset: u64,
}

// TODO: move highlight filter to text mode
// must share selection point or
// declare var 'selection-point' : value  -> language level ...
// enum { type, value }
// a dynamic variables storage for view
// view.vars['selection-point'] -> &mut enum { int64, float64, string, Vec<u8> } | "C" api ...
// view.modes[''] -> std::any::Any
//
use crate::sort_tuple_pair;

impl HighlightSelectionFilter {
    pub fn new() -> Self {
        HighlightSelectionFilter {
            sel_start_offset: 0,
            sel_end_offset: 0,
        }
    }
}

// TODO: monitor env.quit
// to flush
impl Filter<'_> for HighlightSelectionFilter {
    fn name(&self) -> &'static str {
        &"HighlightSelectionFilter"
    }

    fn setup(&mut self, _env: &mut LayoutEnv, view: &View) {
        let tm = view.mode_ctx::<TextModeContext>("text-mode");

        // TODO: compute selection ranges build vec[(min, max)] + index in selection ranges
        let min = tm.marks[tm.mark_index].offset;
        let max = if tm.select_point.len() == 1 {
            tm.select_point[0].offset
        } else {
            min
        };

        let (min, max) = sort_tuple_pair((min, max));
        self.sel_start_offset = min;
        self.sel_end_offset = max;
    }

    fn run(
        &mut self,
        view: &View,
        env: &mut LayoutEnv,
        filter_in: &Vec<FilterIo>,
        filter_out: &mut Vec<FilterIo>,
    ) {
        if env.screen.is_off_screen == true {
            *filter_out = filter_in.clone();
            return;
        }

        let tm = view.mode_ctx::<TextModeContext>("text-mode");
        if tm.select_point.len() == 0 {
            *filter_out = filter_in.clone();
            return;
        }

        for (idx, io) in filter_in.iter().enumerate() {
            match io.offset {
                Some(offset) if offset >= self.sel_start_offset && offset < self.sel_end_offset => {
                    let mut io = io.clone();
                    if env.graphic_display {
                        io.style.bg_color = TextStyle::default_selected_bg_color();
                    } else {
                        io.style.bg_color = (0, 0, 255);
                    }
                    io.style.is_selected = true;

                    filter_out.push(io);
                }

                _ => {
                    filter_out.push(io.clone());
                }
            }
        }
    }
}
