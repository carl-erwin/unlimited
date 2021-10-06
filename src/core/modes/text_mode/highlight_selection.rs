use crate::core::view::ContentFilter;
use crate::core::view::FilterIo;
use crate::core::view::LayoutEnv;
use crate::core::Editor;
use parking_lot::RwLock;
use std::rc::Rc;

use crate::core::codepointinfo::TextStyle;

use crate::core::view::View;

use super::TextModeContext;

pub struct HighlightSelectionFilter {
    sel_start_offset: u64,
    sel_end_offset: u64,
    skip_filter: bool,
}

// TODO(ceg): move highlight filter to text mode
// must share selection point or
// declare var 'selection-point' : value  -> language level ...
// enum { type, value }
// a dynamic variables storage for view
// view.vars['selection-point'] -> &mut enum { int64, float64, string, Vec<u8> } | "C" api ...
// view.modes[''] -> std::any::Any
//
use crate::sort_pair;

impl HighlightSelectionFilter {
    pub fn new() -> Self {
        HighlightSelectionFilter {
            sel_start_offset: 0,
            sel_end_offset: 0,
            skip_filter: false,
        }
    }
}

// TODO(ceg): monitor env.quit
// to flush
impl ContentFilter<'_> for HighlightSelectionFilter {
    fn name(&self) -> &'static str {
        &"HighlightSelectionFilter"
    }

    fn setup(
        &mut self,
        _editor: &Editor,
        env: &mut LayoutEnv,
        view: &Rc<RwLock<View>>,
        _parent_view: Option<&View<'static>>,
    ) {
        let v = view.read();
        let tm = v.mode_ctx::<TextModeContext>("text-mode");

        // TODO(ceg): compute selection ranges build vec[(min, max)] + index in selection ranges
        let min = tm.marks[tm.mark_index].offset;
        let max = if tm.select_point.len() == 1 {
            tm.select_point[0].offset
        } else {
            min
        };

        let (min, max) = sort_pair((min, max));
        self.sel_start_offset = min;
        self.sel_end_offset = max;

        self.skip_filter = false;
        if env.screen.is_off_screen == true {
            self.skip_filter = true;
        }
    }

    fn run(
        &mut self,
        view: &View,
        env: &mut LayoutEnv,
        filter_in: &Vec<FilterIo>,
        filter_out: &mut Vec<FilterIo>,
    ) {
        if self.skip_filter == true {
            *filter_out = filter_in.clone();
            return;
        }

        let tm = view.mode_ctx::<TextModeContext>("text-mode");
        if tm.select_point.is_empty() {
            *filter_out = filter_in.clone();
            return;
        }

        for (_idx, io) in filter_in.iter().enumerate() {
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
