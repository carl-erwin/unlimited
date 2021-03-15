use crate::core::view::layout::Filter;
use crate::core::view::layout::LayoutEnv;

use super::mark::Mark;
use crate::core::codepointinfo::CodepointInfo;
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

    fn setup(&mut self, _env: &LayoutEnv, _view: &View) {}

    fn finish(&mut self, view: &View, env: &mut LayoutEnv) -> () {
        let tm = view.mode_ctx::<TextModeContext>("text-mode");
        let marks = &tm.marks;
        let draw_marks = env.screen.is_off_screen == false;
        refresh_screen_marks(&mut env.screen, marks, draw_marks);
    }
}

// SLOW
// we should iterate over the screen
fn refresh_screen_marks(screen: &mut Screen, marks: &Vec<Mark>, set: bool) {
    if !set {
        screen_apply(screen, |_, _, cpi| {
            cpi.is_mark = false;
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

    for m in marks.iter() {
        if m.offset < first_offset {
            continue;
        }
        if m.offset > last_offset {
            break;
        }

        let doc_max_offset = screen.doc_max_offset;
        // TODO: screen iterator
        screen_apply(screen, |_, _, cpi| {
            if let Some(offset) = cpi.offset {
                if offset == m.offset {
                    cpi.is_mark = true;
                    if cpi.cp == '\n' {
                        // stop at first new line // line is filled with same offsets
                        return false;
                    }
                    if offset == doc_max_offset {
                        // EOF: the screen can be filled with EOFs
                        return false;
                    }
                }
            }
            true // continue
        });
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
