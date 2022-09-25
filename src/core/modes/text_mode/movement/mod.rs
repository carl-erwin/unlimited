use parking_lot::RwLock;
use std::rc::Rc;
use std::sync::Arc;

//use std::time::Instant;

use super::*;

use super::mark::Mark;
use crate::core::editor::Editor;
use crate::core::editor::EditorEnv;
use crate::core::screen::Screen;
use crate::core::view::View;

use crate::core::view::layout::run_compositing_stage_direct;
use crate::core::view::LayoutPass;

// Helpers

// allocate and sets screen.first_offset
fn allocate_temporary_screen_and_start_offset(view: &Rc<RwLock<View>>) -> (Screen, u64) {
    let (width, height, first_offset) = {
        let v = view.read();
        let screen = v.screen.clone();
        let screen = screen.read();
        let first_offset = screen.first_offset.unwrap();

        let width = screen.width();
        /*
          NB : the virtual screen MUST but big enough to compute the marks on the the last line
        */
        let height = screen.height() + 1;
        dbg_println!("current screen : {} x {}", screen.width(), screen.height());
        dbg_println!("new virtual screen : {} x {}", width, height);
        (width, height, first_offset)
    };
    let screen = Screen::new(width, height);
    (screen, first_offset)
}

// min offset, max index
fn get_marks_min_offset_and_max_idx(view: &Rc<RwLock<View>>) -> (u64, usize) {
    let mut v = view.write();

    let tm = v.mode_ctx_mut::<TextModeContext>("text-mode");
    let idx_max = tm.marks.len();
    assert!(idx_max > 0);

    let marks = &mut tm.marks;
    let min_offset = marks[0].offset;

    (min_offset, idx_max)
}

// TODO(ceg): add option to always sync marks
// use byte_index
fn sync_mark(view: &Rc<RwLock<View>>, m: &mut Mark) -> u64 {
    let v = view.read();
    let buffer = v.buffer().unwrap();
    let buffer = buffer.read();

    // ctx
    let tm = v.mode_ctx::<TextModeContext>("text-mode");

    let _codec = tm.text_codec.as_ref();

    // get "real" line start
    // m.move_to_start_of_line(&buffer, codec);

    let buffer_size = buffer.size() as u64;
    if buffer_size > 0 {
        assert!(m.offset <= buffer_size); // == EOF
    }

    buffer_size
}

//
pub fn cancel_marks(_editor: &mut Editor, _env: &mut EditorEnv, view: &Rc<RwLock<View>>) {
    let v = &mut view.write();

    let tm = v.mode_ctx_mut::<TextModeContext>("text-mode");
    let offset = tm.marks[tm.mark_index].offset;

    tm.mark_index = 0;
    tm.marks.clear();
    tm.marks.push(Mark { offset });

    tm.pre_compose_action.push(PostInputAction::ResetMarks);
}

// TODO(ceg): maintain main mark Option<(x,y)>
pub fn move_marks_backward(_editor: &mut Editor, _env: &mut EditorEnv, view: &Rc<RwLock<View>>) {
    let v = &mut view.write();

    let start_offset = v.start_offset;

    let buffer = v.buffer().unwrap();
    let mut buffer = buffer.write();

    let tm = v.mode_ctx_mut::<TextModeContext>("text-mode");

    let codec = tm.text_codec.as_ref();

    let midx = tm.mark_index;

    // update read cache
    let nr_marks = tm.marks.len();
    if nr_marks == 0 {
        return;
    }

    let min = tm.marks[0].offset;
    let max = tm.marks[nr_marks - 1].offset;

    buffer.set_cache(min, max);

    let mut scroll_down = 0;
    for (idx, m) in tm.marks.iter_mut().enumerate() {
        if idx == midx && m.offset <= start_offset {
            scroll_down = 1;
        }

        m.move_backward(&buffer, codec);
    }

    if scroll_down > 0 {
        tm.pre_compose_action
            .push(PostInputAction::ScrollUp { n: 1 });
    }

    tm.pre_compose_action.push(PostInputAction::CheckMarks);

    let tm = v.mode_ctx_mut::<TextModeContext>("text-mode");

    if tm.center_on_mark_move {
        tm.pre_compose_action
            .push(PostInputAction::CenterAroundMainMark);
    }

    tm.prev_action = TextModeAction::MarksMove;
}

pub fn move_marks_forward(_editor: &mut Editor, _env: &mut EditorEnv, view: &Rc<RwLock<View>>) {
    let mut scroll_down = 0;

    let v = &mut view.write();

    let screen_has_eof = v.screen.read().has_eof();
    let end_offset = v.end_offset;

    //
    let buffer = v.buffer().unwrap();
    let mut buffer = buffer.write();

    dbg_println!("buffer.size() {}", buffer.size());

    let tm = v.mode_ctx_mut::<TextModeContext>("text-mode");
    let nr_marks = tm.marks.len();
    if nr_marks == 0 {
        return;
    }

    // set read cache between first and last mark (TODO: remove this, move cache updates to buffer.read() + with size)
    let midx = tm.mark_index;
    let min = tm.marks[0].offset;
    let max = tm.marks[nr_marks - 1].offset;
    buffer.set_cache(min, max);

    //
    let prev_main_mark = tm.marks[midx];

    /* TODO(ceg): check error */
    let codec = tm.text_codec.as_ref();
    for m in tm.marks.iter_mut() {
        m.move_forward(&buffer, codec);
    }

    // mark move off_screen ? scroll down 1 line
    // TODO(ceg): end_offset is not set properly at startup
    // main mark + on screen ?
    let main_mark = tm.marks[midx];
    if prev_main_mark.offset > 0
        && main_mark.offset != prev_main_mark.offset
        && main_mark.offset > end_offset
        && !screen_has_eof
    {
        dbg_println!(
            "main_mark.offset {} > v.end_offset {}",
            main_mark.offset,
            end_offset
        );
        scroll_down = 1;
    }

    // TODO(ceg):  tm.pre_compose_action.push(PostInputAction::SelectLastMark);
    let nr_marks = tm.marks.len();
    tm.mark_index = nr_marks.saturating_sub(1); // TODO(ceg): dedup ?

    //      move this check at post render to reschedule render ?
    //      if v.center_on_mark_move {
    //           tm.pre_compose_action.push(PostInputAction::CenterAroundMainMark);
    //      }

    if scroll_down > 0 {
        dbg_println!("schedule scroll down n = {}", scroll_down);

        tm.pre_compose_action
            .push(PostInputAction::ScrollDown { n: scroll_down });
    }

    tm.pre_compose_action.push(PostInputAction::CheckMarks);

    tm.prev_action = TextModeAction::MarksMove;
}

pub fn move_marks_to_start_of_line(
    _editor: &mut Editor,
    _env: &mut EditorEnv,

    view: &Rc<RwLock<View>>,
) {
    let v = &mut view.write();
    let screen = v.screen.clone();
    let screen = screen.read();

    let buffer = v.buffer().unwrap();
    let buffer = buffer.read();

    let tm = v.mode_ctx_mut::<TextModeContext>("text-mode");
    let codec = tm.text_codec.as_ref();
    //
    let midx = tm.mark_index;

    let mut center = false;
    for (idx, m) in tm.marks.iter_mut().enumerate() {
        m.move_to_start_of_line(&buffer, codec);

        if idx == midx && !screen.contains_offset(m.offset) {
            center = true;
        }
    }

    if center {
        tm.pre_compose_action
            .push(PostInputAction::CenterAroundMainMark);
    }
    tm.pre_compose_action.push(PostInputAction::CheckMarks);
    tm.prev_action = TextModeAction::MarksMove;
}

pub fn move_marks_to_end_of_line(
    _editor: &mut Editor,
    _env: &mut EditorEnv,

    view: &Rc<RwLock<View>>,
) {
    let mut v = view.write();
    let screen = v.screen.clone();
    let screen = screen.read();

    let buffer = v.buffer().unwrap();
    let buffer = buffer.read();

    let tm = v.mode_ctx_mut::<TextModeContext>("text-mode");

    let codec = tm.text_codec.as_ref();

    let midx = tm.mark_index;

    let mut center = false;
    for (idx, m) in tm.marks.iter_mut().enumerate() {
        m.move_to_end_of_line(&buffer, codec);

        if idx == midx && !screen.contains_offset(m.offset) {
            center = true;
        }
    }

    if center {
        tm.pre_compose_action
            .push(PostInputAction::CenterAroundMainMark);
    }

    tm.pre_compose_action.push(PostInputAction::CheckMarks);
    tm.prev_action = TextModeAction::MarksMove;
}

/*
   - (TODO) if the view's lines cache is available check it. (TODO),
      the cache must be sync with the screen.width

   - We rewind W x H x 4 bytes (4 is max codec encode size in utf8/utf16/utf32)
   prev_start = m_offset - (W * H * 4)

   - scroll until m_offset is found

*/
pub fn scroll_screen_up(
    editor: &mut Editor<'static>,
    env: &mut EditorEnv<'static>,
    view: &Rc<RwLock<View<'static>>>,
    offset: u64,
    line_index: usize,
) -> u64 {
    let mut rewind = 0;
    let mut loop_count = 0;
    loop {
        loop_count += 1;

        // NOTE(ceg): this pattern begs for sub function: get_start_offset and dimension
        let (start_offset, width, height) = {
            let v = view.read();
            let screen = v.screen.clone();
            let screen = screen.read();
            let (width, height) = screen.dimension();

            rewind += (width * height / 2) as u64; // rewind max is width * height * 4, but this is slower

            let start_offset = offset.saturating_sub(rewind);
            (start_offset, width, height)
        };

        // TODO(ceg): codec.sync_forward(start_offset) -> start_offset
        // if we are in the middle of a utf8 sequence we move max 4 bytes until a starting point is reached
        // for idx in 0..4 { if codec.is_sync(new_start) { break; } start_offset += codec.encode_min_size() }

        let _tmp = Mark::new(start_offset);

        dbg_println!(
            "   get line index --------- offset - start_offset = {}",
            offset - start_offset
        );

        let lines = {
            crate::core::modes::text_mode::get_lines_offsets_direct(
                &view,
                editor,
                env,
                start_offset,
                offset,
                width,
                height,
            )
        };

        dbg_println!("   get line index --------- loop {}, offset {}, start_offset {}, rewind {} , lines.len() {}", loop_count, offset, start_offset, rewind, lines.len());

        if lines.len() == 0 && offset > 0 {
            // and when folding large block(s) ?
            rewind += 1024 * loop_count;
            continue;
        }

        if lines.len() < line_index {
            if start_offset == 0 {
                return 0;
            }

            if rewind as usize >= width * height {
                return start_offset;
            }

            // we need more previous lines
            rewind += (width * height / 4) as u64;
            continue;
        }

        // lines[] contains start_offset (and maybe more)
        let mut index = lines.len().saturating_sub(1);
        while index > 0 {
            if lines[index].0 <= offset {
                // found current screen first line index
                break;
            }
            index -= 1;
        }

        let next_start_index = index.saturating_sub(line_index);
        let line_start_off = lines[next_start_index].0;
        return line_start_off; // return  (lines, index, start,end) ?
    }
}

fn move_on_screen_mark_to_previous_line(
    _editor: &Editor,
    _env: &EditorEnv,
    v: &View,
    midx: usize,
    marks: &mut [Mark],
) -> (u64, bool) {
    let mut mark_moved = false;

    let screen = v.screen.clone();
    let screen = screen.read();
    let mut m = &mut marks[midx];

    // TODO(ceg): if v.is_mark_on_screen(m) -> (bool, x, y) + (prev/new offset)?
    match screen.find_cpi_by_offset(m.offset) {
        // off_screen
        (None, _, _) => {
            dbg_println!("MARK offscreen");
        }
        // mark on first line
        (Some(_), x, y) if y == 0 => {
            dbg_println!("MARK on screen @ ({},{})", x, y);
            dbg_println!("MARK on first line -> scroll needed");
        }

        // onscreen
        (Some(_), x, y) if y > 0 => {
            dbg_println!("MARK on screen @ ({},{})", x, y);

            // TODO(ceg): refactor code to support screen cell metadata
            let new_y = y - 1; // select previous line
            let l = screen.get_used_line(new_y).unwrap(); // TODO(ceg):  get_line_last_used_cpi()
            dbg_println!("MARK  line {} : len {} ", new_y, l.len());
            // previous line is filled ?
            if l.len() > 0 {
                let mut new_x = ::std::cmp::min(x, l.len() - 1);
                dbg_println!("MARK  look for last non metadata cell");
                while new_x > 0 {
                    let cpi = screen.get_cpinfo(new_x, new_y).unwrap();
                    if !cpi.metadata {
                        break;
                    }
                    new_x -= 1;
                }

                let cpi = screen.get_cpinfo(new_x, new_y).unwrap();
                dbg_println!("MARK  found cpi ( x={},  y={} ) : {:?}", new_x, new_y, cpi);
                m.offset = cpi.offset.unwrap();
                mark_moved = true;
            } else {
                panic!(";")
            }
        }

        // internal error
        _ => {
            panic!();
        }
    }

    (m.offset, mark_moved)
}

pub fn move_mark_to_previous_line(
    editor: &mut Editor<'static>,
    env: &mut EditorEnv<'static>,
    view: &Rc<RwLock<View<'static>>>,
    midx: usize,
    mut marks: &mut Vec<Mark>,
) {
    // first try on screen mark move (except first line)
    let (m_offset, mark_moved) = {
        let v = view.read();
        move_on_screen_mark_to_previous_line(&editor, &env, &v, midx, &mut marks)
    };
    if mark_moved {
        return;
    }

    // TODO(ceg):
    // offset = move_off_screen_mark_to_previous_line(&editor, &env, &v, midx, &mut marks);

    dbg_println!(
        "MARK next position is offscreen ---------------- current mark offset = {}",
        m_offset
    );
    {
        let (start_offset, end_offset, width, height) = {
            let v = view.read();
            let screen = v.screen.clone();
            let screen = screen.read();
            let (width, height) = screen.dimension();

            let buffer = v.buffer().unwrap();
            let buffer = buffer.read();
            let buffer_size = buffer.size() as u64;

            // rewind at least "1 full line" bytes
            let max_encode_size = 4;
            let rewind = (width * max_encode_size) as u64;

            let start_offset = m_offset.saturating_sub(rewind);
            dbg_println!(
                "MARK next position is offscreen ---------------- rewind offset = {}",
                start_offset
            );

            if start_offset == m_offset {
                return;
            }

            let end_offset = std::cmp::min(buffer_size, m_offset + rewind);

            dbg_println!(
                "MARK next position is offscreen ---------------- end offset = {}",
                end_offset
            );

            assert!(end_offset >= start_offset);

            (start_offset, end_offset, width, height)
        };

        //let end_offset = m_offset + width as u64 * 4;
        //let end_offset = std::cmp::min(end_offset, buffer_size);
        //let end_offset = m_offset + width as u64 * 4;

        // TODO(ceg): return last screen, ie screen that contains
        //  the last offset
        // and then use screen_find_offset
        // to compute correct column
        let lines = {
            crate::core::modes::text_mode::get_lines_offsets_direct(
                &view,
                editor,
                env,
                start_offset,
                end_offset,
                width,
                height,
            )
        };

        dbg_println!("*** LINES = {:?}", lines);
        dbg_println!("*** looking for m_offset = {}", m_offset);
        dbg_println!(" lines.len() = {}", lines.len());

        // find "previous" line index
        let index = match lines
            .iter()
            .position(|e| e.0 <= m_offset && m_offset <= e.1)
        {
            None => {
                // return last index
                lines.len().saturating_sub(1)
            }
            Some(0) => {
                dbg_println!("no previous line");
                return;
            }
            Some(i) => {
                dbg_println!("m_offset {} FOUND @ index {:?}", m_offset, i);
                i.saturating_sub(1)
            }
        };

        if index >= lines.len() {
            // BUG!
            // offscreen marks not correctly handled
            // return;
        }

        let line_start_off = lines[index].0;

        dbg_println!("*** INDEX {}", index);

        dbg_println!("line_start_off {}", line_start_off);

        // TODO(ceg): update view only if there is only one mark
        // or a specific flag is passed like screen-follow-mark
        //if !screen.contains_offset(m_offset) {
        {
            let mut v = view.write();
            v.start_offset = line_start_off;
        }

        // TODO(ceg): we can avoid a redraw if the last screen in get_lines_offsets_direct is sync
        {
            marks[midx].offset = line_start_off;
        }
    }
}

pub fn move_mark_to_end_of_file(
    _editor: &mut Editor,
    _env: &mut EditorEnv,

    view: &Rc<RwLock<View>>,
) {
    let mut v = view.write();

    let offset = {
        let buffer = v.buffer().unwrap();
        let buffer = buffer.read();
        buffer.size() as u64
    };
    v.start_offset = offset;

    let n = v.screen.read().height() / 2;

    let tm = v.mode_ctx_mut::<TextModeContext>("text-mode");
    tm.mark_index = 0;

    let marks = &mut tm.marks;
    marks.clear();
    marks.push(Mark { offset });

    tm.pre_compose_action.push(PostInputAction::ScrollUp { n });
}

pub fn move_mark_to_start_of_file(
    _editor: &mut Editor,
    _env: &mut EditorEnv,

    view: &Rc<RwLock<View>>,
) {
    let mut v = view.write();
    v.start_offset = 0;

    let tm = v.mode_ctx_mut::<TextModeContext>("text-mode");

    tm.mark_index = 0;

    tm.marks.clear();
    tm.marks.push(Mark { offset: 0 });
}

pub fn move_mark_to_screen_end(
    _editor: &mut Editor,
    _env: &mut EditorEnv,

    view: &Rc<RwLock<View>>,
) {
    let mut v = view.write();
    let (start_offset, end_offset) = (v.start_offset, v.end_offset);

    let tm = v.mode_ctx_mut::<TextModeContext>("text-mode");
    let marks = &mut tm.marks;

    for m in marks.iter_mut() {
        // TODO(ceg): add main mark check
        if m.offset < start_offset || m.offset > end_offset {
            m.offset = end_offset;
        }
    }
}

pub fn clone_and_move_mark_to_next_line(
    editor: &mut Editor<'static>,
    env: &mut EditorEnv<'static>,

    view: &Rc<RwLock<View<'static>>>,
) {
    // refresh mark index
    let (mut m, midx) = {
        let mut v = view.write();
        let tm = v.mode_ctx_mut::<TextModeContext>("text-mode");
        let marks = &mut tm.marks;
        (marks[marks.len() - 1], marks.len() - 1)
    };

    // NB: move_mark_to_next_line: will use rendering pipeline and borrow view etc..
    let (prev, next) = match move_mark_to_next_line(editor, env, view, &mut m) {
        Some((prev, next)) if prev != next => (prev, next),
        _ => return,
    };

    {
        let save = {
            let mut v = view.write();
            let buffer = v.buffer().unwrap();
            let buffer = buffer.read();
            let last_pos = buffer.buffer_log_pos() >= buffer.buffer_log_count().saturating_sub(1);
            let tm = v.mode_ctx_mut::<TextModeContext>("text-mode");

            !last_pos
                || tm.prev_action == TextModeAction::MarksMove
                || tm.prev_action == TextModeAction::BufferModification
        };

        if save {
            run_text_mode_actions_vec(
                editor,
                env,
                view,
                &vec![PostInputAction::SaveMarks {
                    caller: &"clone_and_move_mark_to_next_line",
                }],
            );
        }

        let mut v = view.write();
        let tm = v.mode_ctx_mut::<TextModeContext>("text-mode");
        tm.marks.push(m);
        tm.mark_index = midx + 1;
    }

    let mut v = view.write();

    // env.sort mark sync direction
    // update view.mark_index
    let (was_on_screen, is_on_screen) = {
        let screen = v.screen.read();
        (screen.contains_offset(prev), screen.contains_offset(next))
    };

    {
        let tm = v.mode_ctx_mut::<TextModeContext>("text-mode");
        if was_on_screen && !is_on_screen {
            tm.pre_compose_action
                .push(PostInputAction::ScrollDown { n: 1 });
        } else if !is_on_screen {
            tm.pre_compose_action
                .push(PostInputAction::CenterAroundMainMark);
        }

        // marks change
        tm.prev_action = TextModeAction::CreateMarks;
        tm.mark_revision += 1;
    }
}

pub fn move_mark_to_screen_start(
    _editor: &mut Editor,
    _env: &mut EditorEnv,

    view: &Rc<RwLock<View>>,
) {
    let mut v = view.write();
    let (start_offset, end_offset) = (v.start_offset, v.end_offset);

    let tm = v.mode_ctx_mut::<TextModeContext>("text-mode");
    let marks = &mut tm.marks;

    for m in marks.iter_mut() {
        // TODO(ceg): add main mark check
        if m.offset < start_offset || m.offset > end_offset {
            m.offset = start_offset;
        }
    }
}

// TODO(ceg): change implementation
//  alloc tmp screen
//  set first mark as current mark
//  1 - if mark offset offscreen, sync tmp screen : fn (offset) -> screen
//      and rewind until the screen contains at least contains 2 lines
//      use get_lines_offsets_direct (use current view plugins)
//  2 - move mark to next line
//  3 - select next mark
//  4 - restart @1
//
pub fn move_marks_to_previous_line(
    editor: &mut Editor<'static>,
    env: &mut EditorEnv<'static>,

    view: &Rc<RwLock<View<'static>>>,
) {
    let (mut marks, idx_max) = {
        let mut v = view.write();
        let tm = v.mode_ctx_mut::<TextModeContext>("text-mode");

        // TODO(ceg): maintain env.mark_index_max ?
        let idx_max = tm.marks.len() - 1;
        (tm.marks.clone(), idx_max)
    };

    let mut mark_index = None;

    {
        for idx in 0..=idx_max {
            let prev_offset = marks[idx].offset;
            move_mark_to_previous_line(editor, env, view, idx, &mut marks);

            // TODO(ceg): move this to pre/post render
            if idx == 0 && idx_max == 0 {
                // tm.pre_compose_action.push(PostInputAction::UpdateViewOnMainMarkMove { moveType: ToPreviousLine, before: prev_offset, after: new_offset });
                let new_offset = marks[idx].offset;

                if new_offset != prev_offset {
                    mark_index = Some(0); // reset main mark
                }
            }
        }
    }

    {
        // copy back
        let mut v = view.write();

        let tm = v.mode_ctx_mut::<TextModeContext>("text-mode");
        tm.marks = marks;
        if let Some(idx) = mark_index {
            tm.mark_index = idx;
        }

        // schedule actions
        tm.pre_compose_action.push(PostInputAction::UpdateReadCache);
        tm.pre_compose_action.push(PostInputAction::CheckMarks);
        // save last op
        tm.prev_action = TextModeAction::MarksMove;
    }
}

pub fn move_on_screen_mark_to_next_line(
    m: &mut Mark,
    screen: &Screen,
) -> (bool, Option<(u64, u64)>, Option<PostInputAction>) {
    // TODO(ceg): add hints: check in screen range
    if !screen.contains_offset(m.offset) {
        return (false, None, None);
    }

    // get offset coordinates
    let (_, x, y) = screen.find_cpi_by_offset(m.offset);
    let screen_height = screen.height();

    // mark on last line -> must scroll
    let new_y = y + 1;
    if new_y >= screen_height {
        // mark on last screen line cannot be updated
        assert_eq!(y, screen_height - 1);
        return (false, None, Some(PostInputAction::ScrollDown { n: 1 }));
    }

    // new_y < screen_height
    let l = screen.get_used_line(new_y);
    if l.is_none() {
        // new_y does not exist, return
        return (true, Some((m.offset, m.offset)), None);
    }
    let l = l.unwrap();

    if l.is_empty() {
        // line is empty do nothing
        return (true, Some((m.offset, m.offset)), None);
    }

    // l.len() > 0
    // get last line char
    let new_x = ::std::cmp::min(x, l.len() - 1);
    let cpi = screen.get_cpinfo(new_x, new_y);
    if cpi.is_none() {
        return (false, None, None);
    }
    let cpi = cpi.unwrap();
    if cpi.offset.is_none() {
        return (false, None, None);
    }

    let old_offset = m.offset;

    m.offset = cpi.offset.unwrap();

    /*
     TODO(ceg):
     Our current data model does not support virtual characters.
     ie: if a filter fills the screen with meta info (not buffer's real data)
     The offset mechanism is broken
      ex: if a filter expands a cp on multiple lines

     To fix this, the "injected" metadata span must be stored elsewhere.
     (internal, buffer_id, offset, size)
    */
    if old_offset == m.offset {
        if let Some(l) = screen.get_used_line(new_y + 1) {
            // TODO(cef) : remove other screen.get_cpinfo(0, new_y + 1);

            // same offset detected: bug to fix in line wrapping
            // a line cannot start with a wrap
            // when line wrapping is enabled
            if !l.is_empty() {
                let cpi = &l[std::cmp::min(x, l.len() - 1)].cpi;
                m.offset = cpi.offset.unwrap();
                return (true, Some((old_offset, m.offset)), None);
            } else {
                // ???? panic!
            }
        }
        return (false, None, Some(PostInputAction::ScrollDown { n: 1 }));
    }

    // ok
    (true, Some((old_offset, m.offset)), None)
}

// remove multiple borrows
pub fn move_mark_index_to_next_line(
    editor: &mut Editor<'static>,
    env: &mut EditorEnv<'static>,
    view: &Rc<RwLock<View<'static>>>,
    mark_idx: usize,
) -> Option<(u64, u64)> {
    let mut m = { view.read().mode_ctx::<TextModeContext>("text-mode").marks[mark_idx] };
    let offsets = move_mark_to_next_line(editor, env, view, &mut m);
    {
        view.write()
            .mode_ctx_mut::<TextModeContext>("text-mode")
            .marks[mark_idx] = m;
    };

    offsets
}

// remove multiple borrows
pub fn move_mark_to_next_line(
    editor: &mut Editor<'static>,
    env: &mut EditorEnv<'static>,
    view: &Rc<RwLock<View<'static>>>,
    mut m: &mut Mark,
) -> Option<(u64, u64)> {
    // TODO(ceg): m.on_buffer_end() ?

    let max_offset = { view.read().buffer()?.read().size() as u64 };

    // off_screen ?
    let (mut m_offset, old_offset) = (m.offset, m.offset);
    if m.offset == max_offset {
        return None;
    }

    {
        let screen = { Arc::clone(&view.read().screen) };

        dbg_println!("TRYING TO MOVE TO NEXT LINE MARK offset {}", m.offset);

        let screen = screen.read();
        let (ok, offsets, action) = move_on_screen_mark_to_next_line(&mut m, &screen);
        {
            if let Some(action) = action {
                // Add stage RenderStage :: PreRender PostRender
                // will be removed when the "scroll" update is implemented
                // ADD screen cache ?
                // screen[first mark -> last mark ] ? Ram usage ?
                // updated on resize -> slow
                view.write()
                    .mode_ctx_mut::<TextModeContext>("text-mode")
                    .pre_compose_action
                    .push(action);
            }
        }

        if ok {
            dbg_println!("MARK FOUND ON SCREEN");
            return offsets;
        }
    }

    if true {
        dbg_println!("MARK IS OFFSCREEN");

        // mark is off_screen
        let (screen_width, screen_height) = {
            let view = view.write();
            let screen = view.screen.read();
            (screen.width(), screen.height())
        };

        // get start_of_line(m.offset) -> u64
        let start_offset = {
            let v = &view.read();
            let buffer = v.buffer().unwrap();
            let buffer = buffer.read();

            let tm = v.mode_ctx::<TextModeContext>("text-mode");
            let codec = tm.text_codec.as_ref();

            let mut tmp = Mark::new(m.offset);
            tmp.move_to_start_of_line(&buffer, codec);
            tmp.offset
        };

        dbg_println!("MARK IS OFFSCREEN");

        // a codepoint can use 4 bytes the virtual end is
        // + 1 full line away
        let end_offset = ::std::cmp::min(m_offset + (4 * screen_width) as u64, max_offset);

        // get lines start, end offset
        // NB: run full layout code for one screen line ( folding etc ... )

        // TODO(ceg): return Vec<Box<screen>> ? update content
        // TODO(ceg): add perf view screen cache ? sorted by screens.start_offset
        // with same width/heigh as v.screen
        let lines = get_lines_offsets_direct(
            view,
            editor,
            env,
            start_offset,
            end_offset,
            screen_width,
            screen_height,
        );

        dbg_println!("GET {} lines ", lines.len());

        // find the cursor index
        let index = match lines.iter().position(|e| e.0 <= m_offset && m_offset < e.1) {
            Some(i) if i != lines.len() - 1 => i,
            _ => return None,
        };

        // compute column
        let new_x = {
            let v = &view.read();
            let buffer = v.buffer().unwrap();
            let buffer = buffer.read();

            let tm = v.mode_ctx::<TextModeContext>("text-mode");

            let codec = tm.text_codec.as_ref();

            // TODO(ceg): use codec.read(buffer, n=width) until e.offset is reached
            let mut s = Mark::new(lines[index].0);
            let e = Mark::new(lines[index].1);
            let mut count = 0;
            while s.offset < e.offset {
                if s.offset == m_offset {
                    break;
                }

                s.move_forward(&buffer, codec);
                count += 1;
            }
            count
        };

        // get next line start/end offsets
        let next_index = index + 1;
        let line_start_off = lines[next_index].0;
        let line_end_off = lines[next_index].1;

        let mut tmp_mark = Mark::new(line_start_off);

        let v = &view.read();
        let buffer = v.buffer().unwrap();
        let buffer = buffer.read();

        let tm = v.mode_ctx::<TextModeContext>("text-mode");

        let codec = tm.text_codec.as_ref();

        // TODO(ceg): codec.skip_n(buffer, 0..new_x)
        for _ in 0..new_x {
            tmp_mark.move_forward(&buffer, codec); // TODO(ceg): pass n as arg
        }

        tmp_mark.offset = std::cmp::min(tmp_mark.offset, line_end_off);
        m_offset = tmp_mark.offset;
    }

    {
        m.offset = m_offset;
    }

    Some((old_offset, m_offset))
}

fn move_onscreen_single_mark_to_next_line(
    editor: &mut Editor<'static>,
    env: &mut EditorEnv<'static>,
    view: &Rc<RwLock<View<'static>>>,
) -> bool {
    // fast mode: 1 mark, on screen
    // check main mark, TODO(ceg) proper function ?
    // fast mode: 1 mark, on screen
    // check main mark, TODO(ceg) proper function ?

    let screen = {
        let v = view.read();

        v.screen.clone()
    };
    let mut screen = screen.write();
    if screen.height() <= 1 {
        // cannot compute next line
        return false;
    }

    let mut mark = {
        let mut v = view.write();
        let tm = v.mode_ctx_mut::<TextModeContext>("text-mode");
        // 1 mark ?
        if tm.marks.len() != 1 {
            return false;
        }

        tm.marks[0].clone()
    };

    if !screen.contains_offset(mark.offset) {
        dbg_println!("main mark is offscreen");
        return false;
    }

    dbg_println!("main mark is onscreen");
    dbg_println!("screen.dimension = {:?}", screen.dimension());

    let last_line = screen.get_last_used_line();
    if last_line.is_none() {
        dbg_println!("no last line");
        panic!();
    }
    let last_line = last_line.unwrap();
    if last_line.is_empty() {
        panic!(""); // empty line
    }

    // Go to next screen ?
    let cpi = &last_line[0].cpi; // will panic if invariant is broken
    let last_line_first_offset = cpi.offset.unwrap(); // update next screen start offset
    let has_eof = screen.has_eof();

    dbg_println!("mark.offset = {}", mark.offset);
    dbg_println!("last_line_first_offset {}", last_line_first_offset);
    dbg_println!("has_eof = {}", has_eof);

    if mark.offset < last_line_first_offset {
        dbg_println!("mark is on screen (not on last line)");

        let ret = move_on_screen_mark_to_next_line(&mut mark, &screen);
        if !ret.0 {
            dbg_println!(
                " cannot update marks[{}], offset {} : {:?}",
                0,
                mark.offset,
                ret.2
            );

            return false;
        }

        // update mark
        let mut v = view.write();
        let tm = v.mode_ctx_mut::<TextModeContext>("text-mode");
        tm.marks[0].offset = ret.1.unwrap().1;
        return true;
    }

    //
    if mark.offset >= last_line_first_offset && !has_eof {
        // must scroll 1 line
        // NB: update the view's screen in place
        // by running "correct" compose passes

        //  use the first offset of the last line
        //  as the starting offset of the next screen
        dbg_println!("screen dimension {:?}", screen.dimension());
        dbg_println!("get line [1]");
        let line = screen.get_line(1).unwrap();
        let cpi = &line[0].cpi;
        let new_start = cpi.offset.unwrap();

        dbg_println!("line[1][0].cpi  = {:?}", cpi);

        dbg_println!("new_start = {}", new_start);

        let max_offset = screen.buffer_max_offset;

        // build screen content
        screen.clear();
        run_compositing_stage_direct(
            editor,
            env,
            view,
            new_start,
            max_offset,
            &mut screen,
            LayoutPass::ScreenContent,
        );

        // NB: update view after scroll
        {
            let mut v = view.write();
            v.start_offset = new_start;
            if let Some(last_offset) = screen.last_offset {
                v.end_offset = last_offset; // DO NOT REMOVE
            }
        }

        // TODO(ceg) : sync_view_from_screen(screen)
        /* replace pass_mask -> struct CompositingParameters {
             base_offset
             pass_mask
             update_view,
             layout_filters { vec, vec }
            }

            add a struct CompositingResults {
                start, end, of screen
            }

            view.apply_compositing_result(res);
        */

        // TODO(ceg): that use/match the returned action
        let ret = move_on_screen_mark_to_next_line(&mut mark, &screen);
        if !ret.0 {
            dbg_println!(
                " cannot update marks[{}], offset {} : {:?}",
                0,
                mark.offset,
                ret.2
            );
        }

        // build screen overlay
        {
            {
                let mut v = view.write();
                let tm = v.mode_ctx_mut::<TextModeContext>("text-mode");
                tm.marks[0] = mark; // save update
                dbg_println!("main mark updated (fast path) {:?}", tm.marks[0]);

                if !tm.select_point.is_empty() {
                    tm.pre_compose_action
                        .push(PostInputAction::DeduplicateAndSaveMarks {
                            caller: &"move_marks_to_next_line",
                        });
                    // save last op
                    tm.prev_action = TextModeAction::MarksMove;
                }
            }
            // do not update screen twice
            env.skip_compositing = true;

            run_compositing_stage_direct(
                editor,
                env,
                view,
                new_start,
                max_offset,
                &mut screen,
                LayoutPass::ScreenOverlay,
            );
        }

        return true;
    }

    false
}

/*
    TODO(ceg): we use a virtual screen to compute OFFSCREEN marks

    We should reuse the temporary screen if possible and skip composing pass
    or better allocate a virtual screen with 1 + height + 1
    and clip header/footer + swap view screen
*/
pub fn move_marks_to_next_line(
    editor: &mut Editor<'static>,
    env: &mut EditorEnv<'static>,
    view: &Rc<RwLock<View<'static>>>,
) {
    if move_onscreen_single_mark_to_next_line(editor, env, view) {
        return;
    }

    let (mut screen, start_offset) = allocate_temporary_screen_and_start_offset(&view);
    let mut m = Mark::new(start_offset);
    let (first_mark_offset, idx_max) = get_marks_min_offset_and_max_idx(&view);
    // set screen start
    screen.is_off_screen = true;
    m.offset = std::cmp::min(m.offset, first_mark_offset);

    let max_offset = sync_mark(&view, &mut m); // codec in name ?

    // let mut marks = clone_marks(view);
    let mut marks = {
        let v = view.read();
        let tm = v.mode_ctx::<TextModeContext>("text-mode");
        tm.marks.clone()
    };

    // update all marks
    // screen fill + cb(editor, env, view, cb_ctx)
    {
        let mut idx = 0;
        while idx < idx_max {
            screen.clear();
            run_compositing_stage_direct(
                editor,
                env,
                &view,
                m.offset,
                max_offset,
                &mut screen,
                LayoutPass::ScreenContent,
            );

            if screen.push_count() == 0 {
                return;
            }
            let last_line = screen.get_last_used_line();
            if last_line.is_none() {
                panic!();
            }
            let last_line = last_line.unwrap();
            if last_line.is_empty() {
                panic!(""); // empty line
            }

            if !screen.contains_offset(marks[idx].offset) {
                if screen.has_eof() {
                    break;
                }
                let cpi = &last_line[0].cpi;
                m.offset = cpi.offset.unwrap(); // update next screen start offset
                continue;
            }

            let next_screen_start_cpi = &last_line[0].cpi;
            // idx is on screen
            // find next onscreen index
            let mut idx_end = idx + 1;
            while idx_end < idx_max {
                if marks[idx_end].offset >= next_screen_start_cpi.offset.unwrap() {
                    break;
                }
                idx_end += 1;
            }

            for i in idx..idx_end {
                move_on_screen_mark_to_next_line(&mut marks[i], &screen);
            }

            idx = idx_end; // next mark index
            m.offset = next_screen_start_cpi.offset.unwrap(); // update next screen start
        }
    }

    // check main mark
    {
        let mut v = view.write();
        let screen = v.screen.clone();
        let screen = screen.as_ref().read();

        let tm = v.mode_ctx_mut::<TextModeContext>("text-mode");

        // set back
        tm.marks = marks;
        let idx = tm.mark_index;

        if !screen.contains_offset(tm.marks[idx].offset) {
            tm.pre_compose_action
                .push(PostInputAction::ScrollDown { n: 1 });
            // TODO ?  tm.pre_compose_action.push(PostInputAction::ScrollDownIfOffsetNotOnScreen { n: 1, offset: tm.marks[idx].offset });
            // TODO ?  tm.pre_compose_action.push(PostInputAction::ScrollDownIfMainMarkOffScreen { n: 1, offset: tm.marks[idx].offset });
        } else {
        };

        if !tm.select_point.is_empty() {
            tm.pre_compose_action
                .push(PostInputAction::DeduplicateAndSaveMarks {
                    caller: &"move_marks_to_next_line",
                });
        }

        // save last op
        tm.prev_action = TextModeAction::MarksMove;
    }
}

pub fn clone_and_move_mark_to_previous_line(
    editor: &mut Editor<'static>,
    env: &mut EditorEnv<'static>,
    view: &Rc<RwLock<View<'static>>>,
) {
    let (mut marks, prev_off) = {
        let v = view.read();
        let tm = v.mode_ctx::<TextModeContext>("text-mode");
        (tm.marks.clone(), tm.marks[0].offset)
    };

    dbg_println!(" clone move up: prev_offset {}", prev_off);

    {
        move_mark_to_previous_line(editor, env, view, 0, &mut marks);
        if marks[0].offset == prev_off {
            // no change
            return;
        }
    }

    if marks.len() == 1 {
        run_text_mode_actions_vec(
            editor,
            env,
            view,
            &vec![PostInputAction::SaveMarks {
                caller: &"clone_and_move_mark_to_previous_line",
            }],
        );
    }

    let mut v = view.write();
    let screen = v.screen.clone();
    let screen = screen.read();

    let tm = v.mode_ctx_mut::<TextModeContext>("text-mode");
    tm.marks = marks;

    if tm.marks[0].offset != prev_off {
        let tm = v.mode_ctx_mut::<TextModeContext>("text-mode");
        tm.mark_index = 0;

        // insert mark @ m_offset + pa
        tm.marks.insert(
            0,
            Mark {
                offset: tm.marks[0].offset,
            },
        );
        tm.marks[1].offset = prev_off;
        // env.sort mark sync direction
        // update view.mark_index

        let was_on_screen = screen.contains_offset(prev_off);
        let is_on_screen = screen.contains_offset(tm.marks[0].offset);
        if was_on_screen && !is_on_screen {
            tm.pre_compose_action
                .push(PostInputAction::ScrollUp { n: 1 });
        } else if !is_on_screen {
            tm.pre_compose_action
                .push(PostInputAction::CenterAroundMainMark);
        }

        // marks change
        tm.prev_action = TextModeAction::CreateMarks;
        tm.mark_revision += 1;
    }
}

pub fn move_to_token_start(_editor: &mut Editor, _env: &mut EditorEnv, view: &Rc<RwLock<View>>) {
    // TODO(ceg): factorize mark action
    // mark.apply(fn); where fn=m.move_to_token_end(&buffer, codec);
    //

    let mut center = false;

    let v = &mut view.write();
    let screen = v.screen.clone();
    let screen = screen.read();

    let buffer = v.buffer().unwrap();
    let buffer = buffer.read();

    let tm = v.mode_ctx_mut::<TextModeContext>("text-mode");

    let codec = tm.text_codec.as_ref();

    let midx = tm.mark_index;

    let marks = &mut tm.marks;

    for (idx, m) in marks.iter_mut().enumerate() {
        m.move_to_token_start(&buffer, codec);

        // main mark ?
        if idx == midx && !screen.contains_offset(m.offset) {
            // TODO(ceg): push to post action queue
            // {SYNC_VIEW, CLEAR_VIEW, SCROLL_N }
            //
            center = true;
        }
    }

    if center {
        tm.pre_compose_action
            .push(PostInputAction::CenterAroundMainMark);
    }
    tm.prev_action = TextModeAction::MarksMove;
}

pub fn move_to_token_end(_editor: &mut Editor, _env: &mut EditorEnv, view: &Rc<RwLock<View>>) {
    let mut sync = false;

    let mut v = view.write();
    let screen = v.screen.clone();
    let screen = screen.read();

    let buffer = v.buffer().unwrap();
    let buffer = buffer.read();

    let tm = v.mode_ctx_mut::<TextModeContext>("text-mode");
    let codec = tm.text_codec.as_ref();

    let marks = &mut tm.marks;

    for m in marks.iter_mut() {
        m.move_to_token_end(&buffer, codec);

        // main mark ?
        if !screen.contains_offset(m.offset) {
            // TODO(ceg): push to post action queue
            // {SYNC_VIEW, CLEAR_VIEW, SCROLL_N }
            //
            sync = true;
        }
    }

    if sync {
        tm.pre_compose_action
            .push(PostInputAction::CenterAroundMainMark);
    }

    tm.prev_action = TextModeAction::MarksMove;
}
