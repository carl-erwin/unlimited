use crate::core::view::layout::Filter;
use crate::core::view::layout::FilterIoData;
use crate::core::view::layout::LayoutEnv;

use crate::core::codepointinfo::CodepointInfo;
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

    fn setup(&mut self, _env: &LayoutEnv, view: &View) {
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
        filter_in: &Vec<FilterIoData>,
        filter_out: &mut Vec<FilterIoData>,
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

        let _colors = [
            /* Black	     */ (0, 0, 0),
            /* Light_red	 */ (255, 0, 0),
            /* Light_green	 */ (0, 255, 0),
            /* Yellow	     */ (255, 255, 0),
            /* Light_blue	 */ (0, 0, 255),
            /* Light_magenta */ (255, 0, 255),
            /* Light_cyan	 */ (0, 255, 255),
            /* High_white	 */ (255, 255, 255),
            /* Gray	         */ (128, 128, 128),
            /* Red	         */ (128, 0, 0),
            /* Green	     */ (0, 128, 0),
            /* Brown	     */ (128, 128, 0),
            /* Blue	         */ (0, 0, 128),
            /* Magenta       */ (128, 0, 128),
            /* Cyan	         */ (0, 128, 128),
            /* White	     */ (192, 192, 192),
        ];

        for i in filter_in {
            match i.offset {
                Some(offset) if offset >= self.sel_start_offset && offset < self.sel_end_offset => {
                    let mut i = i.clone();
                    if env.graphic_display {
                        i.bg_color = CodepointInfo::default_selected_bg_color();
                    } else {
                        //let idx = offset as usize % _colors.len();
                        i.bg_color = (0, 0, 255);
                    }

                    filter_out.push(i);
                }

                _ => {
                    filter_out.push(i.clone());
                }
            }
        }
    }
}
