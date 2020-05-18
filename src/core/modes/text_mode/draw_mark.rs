use crate::core::view::layout::ScreenOverlayFilter;

use crate::core::view::layout::LayoutEnv;

use super::mark::Mark;
use crate::core::codepointinfo::TextStyle;

use crate::core::screen::Screen;
use crate::core::view::View;

use super::TextModeContext;

use crate::core::screen::screen_apply;

pub struct DrawMarks {
    skip_filter: bool,
}

impl DrawMarks {
    pub fn new() -> Self {
        DrawMarks { skip_filter: false }
    }
}

impl ScreenOverlayFilter<'_> for DrawMarks {
    fn name(&self) -> &'static str {
        &"DrawMarks"
    }

    fn finish(&mut self, view: &View, env: &mut LayoutEnv) -> () {
        if env.screen.is_off_screen == true {
            return;
        }

        let tm = view.mode_ctx::<TextModeContext>("text-mode");
        let marks = &tm.marks;

        let _draw_marks = true;
        refresh_screen_marks(&mut env.screen, marks, true);
    }
}

pub fn refresh_screen_marks(screen: &mut Screen, marks: &Vec<Mark>, set: bool) {
    dbg_println!(
        "DRAW MARKS TRY DRAW OFFSET : FIRST {:?}  LAST {:?}",
        screen.first_offset,
        screen.last_offset
    );

    let idx_max = marks.len();

    if idx_max == 1 {
        if !screen.contains_offset(marks[0].offset) {
            dbg_println!("MARK is offscreen");
            return;
        }
    }

    if !set {
        screen_apply(screen, |_, _, cpi| {
            cpi.style.is_inverse = false;
            true // continue
        });
        return;
    }

    let (first_offset, last_offset) = match (screen.first_offset, screen.last_offset) {
        (Some(first_offset), Some(last_offset)) => (first_offset, last_offset),
        _ => {
            dbg_println!("CANNOT DRAW MARKS");
            return;
        }
    };

    let mut idx = 0;
    // get first on screen mark index
    while idx < idx_max {
        if marks[idx].offset < first_offset {
            idx += 1;
            continue;
        }
        break;
    }

    let mut lines_with_marks = vec![];
    if idx < idx_max && marks[idx].offset <= last_offset {
        screen_apply(screen, |_c, l, cpi| {
            if let Some(offset) = cpi.offset {
                // get next mark
                while idx < idx_max {
                    if marks[idx].offset >= offset {
                        break;
                    }
                    idx += 1;
                }
                if idx == idx_max {
                    return false;
                }
                if marks[idx].offset > last_offset {
                    return false;
                }

                // check offset
                if offset == marks[idx].offset {
                    cpi.style.is_inverse = true;

                    // save line index
                    let mut save_line = true;
                    if let Some(last) = lines_with_marks.last() {
                        save_line = *last != l;
                    }
                    if save_line {
                        lines_with_marks.push(l);
                    }
                }
            }

            true
        });
    }

    // highlight mark-line
    for l in lines_with_marks {
        if let Some(line) = screen.get_line_mut(l) {
            for cell in line {
                if !cell.cpi.style.is_selected {
                    cell.cpi.style.bg_color = TextStyle::default_mark_line_bg_color();
                }
            }
        }
    }
}
