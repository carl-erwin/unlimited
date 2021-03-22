use crate::core::view::layout::Filter;
use crate::core::view::layout::LayoutEnv;

use super::mark::Mark;
use crate::core::codepointinfo::CodepointInfo;
use crate::core::codepointinfo::TextStyle;

use crate::core::screen::Screen;
use crate::core::view::View;

use super::TextModeContext;

pub struct DrawMarks {}

impl DrawMarks {
    pub fn new() -> Self {
        DrawMarks {}
    }
}

impl Filter<'_> for DrawMarks {
    fn name(&self) -> &'static str {
        &"DrawMarks"
    }

    fn setup(&mut self, _env: &mut LayoutEnv, _view: &View) {}

    fn finish(&mut self, view: &View, env: &mut LayoutEnv) -> () {
        if env.screen.is_off_screen == true {
            return;
        }

        let tm = view.mode_ctx::<TextModeContext>("text-mode");
        let marks = &tm.marks;
        let draw_marks = env.screen.is_off_screen == false;
        refresh_screen_marks(&mut env.screen, marks, draw_marks);
    }
}


fn refresh_screen_marks(screen: &mut Screen, marks: &Vec<Mark>, set: bool) {
    if !set {
        screen_apply(screen, |_, _, cpi| {
            cpi.style.is_inverse = false;
            true // continue
        });
        return;
    }

    dbg_println!(
        "DRAW MARKS TRY DRAW OFFSET : FIRST {:?}  LAST {:?}",
        screen.first_offset,
        screen.last_offset
    );

    let (first_offset, last_offset) = match (screen.first_offset, screen.last_offset) {
        (Some(first_offset), Some(last_offset)) => (first_offset, last_offset),
        _ => {
            dbg_println!("CANNOT DRAW MARKS");
            return;
        }
    };

    let idx_max = marks.len();
    let mut idx = 0;
    while idx < idx_max {
        if marks[idx].offset < first_offset {
            idx += 1;
            continue;
        }
        break;
    }

    let doc_max_offset = screen.doc_max_offset;
    let mut lines_with_marks = vec![];
    if idx < idx_max && marks[idx].offset <= last_offset {
        screen_apply(screen, |_, l, cpi| {
            if let Some(offset) = cpi.offset {
                if *&marks[idx].offset > last_offset {
                    return false;
                }

                if offset == *&marks[idx].offset {
                    // save line index
                    if let Some(last) = (*&lines_with_marks).last() {
                        if *last != l {
                            &lines_with_marks.push(l);
                        }
                    } else {
                        &lines_with_marks.push(l);
                    }

                    cpi.style.is_inverse = true;
                    if cpi.cp == '\n' {
                        // stop at first new line // line is filled with same offsets
                        return true;
                    }
                    if offset > doc_max_offset {
                        // EOF: the screen can be filled with EOFs
                        return false;
                    }
                } else {
                    // get next mark
                    while idx < idx_max {
                        if marks[idx].offset > offset {
                            break;
                        }
                        idx += 1;
                    }
                    if idx == idx_max {
                        return false;
                    }
                }
            }

            true
        });
    }

    // highlight mark-line
    for l in lines_with_marks {
        if let Some(l) = screen.get_line_mut(l) {
            for cell in &mut l.cells {
                if !cell.cpi.style.is_selected {
                    cell.cpi.style.bg_color = TextStyle::default_mark_line_bg_color();
                }
            }
        }
    }
}

// move to screen module , rename walk/map ?
fn screen_apply<F: FnMut(usize, usize, &mut CodepointInfo) -> bool>(
    screen: &mut Screen,
    mut on_cpi: F,
) {
    for l in 0..screen.height() {
        if let Some(line) = screen.get_line_mut(l) {
            for c in 0..line.nb_cells {
                if let Some(cpi) = line.get_mut_cpi(c) {
                    if on_cpi(c, l, cpi) == false {
                        return;
                    }
                }
            }
        }
    }
}
