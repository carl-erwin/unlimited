use crate::core::view::LayoutEnv;
use crate::core::view::ScreenOverlayFilter;

use crate::core::codepointinfo::TextStyle;

use crate::core::screen::Screen;
use crate::core::view::View;

use super::TextModeContext;
use crate::core::screen::screen_apply;
use crate::sort_pair;

pub struct HighlightSelectionOverlay {}

impl HighlightSelectionOverlay {
    pub fn new() -> Self {
        Self {}
    }
}

impl ScreenOverlayFilter<'_> for HighlightSelectionOverlay {
    fn name(&self) -> &'static str {
        &"HighlightSelectionOverlay"
    }

    fn finish(&mut self, view: &View, env: &mut LayoutEnv) {
        if env.screen.is_off_screen {
            return;
        }

        let tm = view.mode_ctx::<TextModeContext>("text-mode");

        if tm.marks.len() == tm.select_point.len() {
            let mut range = Vec::with_capacity(tm.marks.len());
            for i in 0..tm.marks.len() {
                let min = tm.marks[i].offset;
                let max = tm.select_point[i].offset;
                let (min, max) = sort_pair((min, max));
                range.push((min, max));
            }

            refresh_screen_selections(&mut env.screen, &range);
        }
    }
}

pub fn refresh_screen_selections(screen: &mut Screen, sel: &Vec<(u64, u64)>) {
    let idx_max = sel.len();
    let mut idx = 0;

    if idx_max == 0 {
        return;
    }

    // this is slow
    screen_apply(screen, |_c, _l, cpi| {
        if let Some(offset) = cpi.offset {
            // get next range
            while idx < idx_max {
                // offset < sel.min
                if offset < sel[idx].0 {
                    return true;
                }

                // offset is >= sel.min

                // offet is <= sel.max -> match
                if offset <= sel[idx].1 {
                    cpi.style.bg_color = TextStyle::default_selected_bg_color();
                    return true;
                }

                // offset is > sel.max -> out of range: select next range
                idx += 1;
                continue;
            }

            return false;
        }

        true
    });
}
