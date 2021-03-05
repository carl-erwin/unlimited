// Copyright (c) Carl-Erwin Griffith

use std::rc::Rc;
use std::{any::Any, cell::RefCell};

use std::collections::HashMap;
use std::time::Instant;

//
use crate::sort_tuple_pair;

use crate::core::editor::Editor;

use crate::core::editor::EditorEnv;

use crate::dbg_println;

use crate::core::screen::Screen;

use crate::core::mark::Mark;

use crate::core::codec::text::utf8;
use crate::core::codec::text::SyncDirection; // TODO: remove
use crate::core::codec::text::TextCodec;

use crate::core::event::ButtonEvent;
use crate::core::event::InputEvent;
use crate::core::event::Key;
use crate::core::event::KeyModifiers;
use crate::core::event::PointerEvent;

//
use crate::core::view::layout::run_compositing_stage_direct;

use crate::core::editor;
use crate::core::editor::register_input_stage_action;
use crate::core::editor::InputStageActionMap;
use crate::core::view::View;

// TODO: move to TextMode post actions
use crate::core::view::Action;

use super::Mode;

//
use crate::core::view::layout::LayoutEnv;

use crate::core::view::layout::DrawMarks;
use crate::core::view::layout::HighlightFilter;
use crate::core::view::layout::HighlightSelectionFilter;
use crate::core::view::layout::RawDataFilter;
use crate::core::view::layout::ScreenFilter;
use crate::core::view::layout::TabFilter;
use crate::core::view::layout::Utf8Filter;
use crate::core::view::layout::WordWrapFilter;

pub type Id = u64;

// save transaction's index
pub enum CopyData {
    BufferLogIndex(usize),
    Buffer(Vec<u8>),
}

pub struct TextModeContext {
    pub center_on_mark_move: bool,
    pub scroll_on_mark_move: bool,
    pub text_codec: Box<dyn TextCodec>,
    pub mark_index: usize, // move to text mode
    pub marks: Vec<Mark>,
    pub select_point: Vec<Mark>,
    pub copy_buffer: Vec<CopyData>,
    pub button_state: [u32; 8],

    pub char_map: Option<HashMap<char, char>>,
    pub color_map: Option<HashMap<char, (u8, u8, u8)>>,
}

impl<'a> Mode for TextMode {
    fn name(&self) -> &'static str {
        &"text-mode"
    }

    fn build_action_map(&self) -> InputStageActionMap<'static> {
        let mut map = InputStageActionMap::new();
        Self::register_input_stage_actions(&mut map);
        map
    }

    fn alloc_ctx(&self) -> Box<dyn Any> {
        dbg_println!("allocate text-mode ctx");

        let marks = vec![Mark { offset: 0 }];
        let copy_buffer = vec![];

        let mut char_map = HashMap::new();

        for _c in '\0'..' ' {
            // char_map.insert(c, '.');
        }

        //
        char_map.insert('\u{7f}', '�');

        char_map.insert('\r', ' ');
        char_map.insert('\r', '\u{2190}');

        char_map.insert('\n', ' ');

        char_map.insert('\t', ' ');

        let ctx = TextModeContext {
            center_on_mark_move: false, // add movement enums and pass it to center fn
            scroll_on_mark_move: true,
            text_codec: Box::new(utf8::Utf8Codec::new()),
            marks,
            copy_buffer,
            mark_index: 0,
            select_point: vec![],
            button_state: [0; 8],
            char_map: Some(char_map),
            color_map: None,
        };

        Box::new(ctx)
    }

    fn configure_view(&self, view: &mut View) {
        view.compose_filters
            .borrow_mut()
            .push(Box::new(RawDataFilter::new()));
        view.compose_filters
            .borrow_mut()
            .push(Box::new(Utf8Filter::new()));
        view.compose_filters
            .borrow_mut()
            .push(Box::new(HighlightFilter::new()));
        view.compose_filters
            .borrow_mut()
            .push(Box::new(HighlightSelectionFilter::new()));
        view.compose_filters
            .borrow_mut()
            .push(Box::new(TabFilter::new()));
        view.compose_filters
            .borrow_mut()
            .push(Box::new(WordWrapFilter::new()));
        view.compose_filters
            .borrow_mut()
            .push(Box::new(ScreenFilter::new()));
        view.compose_filters
            .borrow_mut()
            .push(Box::new(DrawMarks::new()));
    }
}

pub struct TextMode {
    // add common filed
}

impl TextMode {
    pub fn new() -> Self {
        dbg_println!("TextMode");
        TextMode {}
    }

    pub fn register_input_stage_actions<'a>(mut map: &'a mut InputStageActionMap<'a>) {
        register_input_stage_action(
            &mut map,
            "text-mode:display-end-of-line",
            display_end_of_line,
        );

        register_input_stage_action(&mut map, "text-mode:self-insert", insert_codepoint_array);

        register_input_stage_action(
            &mut map,
            "text-mode:move-marks-backward",
            move_marks_backward,
        );

        register_input_stage_action(&mut map, "text-mode:move-marks-forward", move_marks_forward);
        register_input_stage_action(
            &mut map,
            "text-mode:move-marks-to-next-line",
            move_marks_to_next_line,
        );
        register_input_stage_action(
            &mut map,
            "text-mode:move-marks-to-previous-line",
            move_marks_to_previous_line,
        );

        register_input_stage_action(
            &mut map,
            "text-mode:move-to-token-start",
            move_to_token_start,
        );

        register_input_stage_action(&mut map, "text-mode:move-to-token-end", move_to_token_end);

        register_input_stage_action(&mut map, "text-mode:page-up", scroll_to_previous_screen);
        register_input_stage_action(&mut map, "text-mode:page-down", scroll_to_next_screen);

        register_input_stage_action(&mut map, "text-mode:scroll-up", scroll_up);
        register_input_stage_action(&mut map, "text-mode:scroll-down", scroll_down);

        register_input_stage_action(
            &mut map,
            "text-mode:move-marks-to-start-of-line",
            move_marks_to_start_of_line,
        );
        register_input_stage_action(
            &mut map,
            "text-mode:move-marks-to-end-of-line",
            move_marks_to_end_of_line,
        );

        register_input_stage_action(
            &mut map,
            "text-mode:move-marks-to-start-of-file",
            move_mark_to_start_of_file,
        );
        register_input_stage_action(
            &mut map,
            "text-mode:move-marks-to-end-of-file",
            move_mark_to_end_of_file,
        );

        register_input_stage_action(&mut map, "text-mode:undo", undo);
        register_input_stage_action(&mut map, "text-mode:redo", redo);
        register_input_stage_action(&mut map, "text-mode:remove-codepoint", remove_codepoint);
        register_input_stage_action(
            &mut map,
            "text-mode:remove-previous-codepoint",
            remove_previous_codepoint,
        );

        register_input_stage_action(&mut map, "text-mode:button-press", button_press);
        register_input_stage_action(&mut map, "text-mode:button-release", button_release);
        register_input_stage_action(
            &mut map,
            "text-mode:move-mark-to-clicked-area",
            button_press,
        );

        register_input_stage_action(&mut map, "text-mode:center-around-mark", center_around_mark);
        register_input_stage_action(&mut map, "text-mode:cut-to-end-of-line", cut_to_end_of_line);

        register_input_stage_action(&mut map, "text-mode:paste", paste);
        register_input_stage_action(
            &mut map,
            "text-mode:remove-until-end-of-word",
            remove_until_end_of_word,
        );
        register_input_stage_action(&mut map, "scroll-to-next-screen", scroll_to_next_screen);
        register_input_stage_action(
            &mut map,
            "scroll-to-previous-screen",
            scroll_to_previous_screen,
        );

        register_input_stage_action(&mut map, "select-next-view", select_next_view);

        register_input_stage_action(&mut map, "select-previous-view", select_previous_view);

        register_input_stage_action(
            &mut map,
            "text-mode:clone-and-move-mark-to-previous-line",
            clone_and_move_mark_to_previous_line,
        );
        register_input_stage_action(
            &mut map,
            "text-mode:clone-and-move-mark-to-next-line",
            clone_and_move_mark_to_next_line,
        );

        register_input_stage_action(&mut map, "text-mode:pointer-motion", pointer_motion);

        register_input_stage_action(
            &mut map,
            "text-mode:set-select-point-at-mark",
            set_selection_points_at_marks,
        );

        register_input_stage_action(&mut map, "text-mode:copy-selection", copy_selection);

        register_input_stage_action(&mut map, "text-mode:cut-selection", cut_selection);

        register_input_stage_action(&mut map, "editor:cancel", editor_cancel);
    }
}

pub fn run_text_mode_actions(
    editor: &mut Editor,
    env: &mut EditorEnv,
    view: &Rc<RefCell<View>>,
    stage: editor::Stage,
    pos: editor::StagePosition,
) {
    let actions: Vec<Action> = {
        match (stage, pos) {
            (editor::Stage::Compositing, editor::StagePosition::Pre) => {
                view.borrow_mut().pre_compose_action.drain(..).collect()
            }

            (editor::Stage::Compositing, editor::StagePosition::Post) => {
                view.borrow_mut().post_compose_action.drain(..).collect()
            }

            _ => {
                panic!();
            }
        }
    };

    for a in actions.iter() {
        match a {
            Action::ScrollUp { n } => {
                let v = &mut view.borrow_mut();

                v.scroll_up(editor, env, *n);
            }
            Action::ScrollDown { n } => {
                let v = &mut view.borrow_mut();

                v.scroll_down(editor, env, *n);
            }
            Action::CenterAroundMainMark => {
                center_around_mark(editor, env, &view);
            }
            Action::CenterAroundMainMarkIfOffScreen => {
                let center = {
                    let v = &mut view.borrow_mut();

                    let tm = v.mode_ctx::<TextModeContext>("text-mode");
                    let mid = tm.mark_index;
                    let marks = &tm.marks;
                    let offset = marks[mid].offset;
                    let screen = v.screen.read().unwrap();
                    !screen.contains_offset(offset)
                };
                if center {
                    center_around_mark(editor, env, &view);
                }
            }
            Action::CenterAround { offset } => {
                env.center_offset = Some(*offset);
                center_around_mark(editor, env, &view);
            }
            Action::MoveMarksToNextLine => {
                move_marks_to_next_line(editor, env, &view);
            }
            Action::MoveMarksToPreviousLine => {}
            Action::MoveMarkToNextLine { idx } => {
                move_mark_to_next_line(editor, env, view, *idx);
                env.cur_mark_index = None;
            }
            Action::MoveMarkToPreviousLine { idx: _usize } => {}

            Action::ResetMarks => {
                let v = &mut view.borrow_mut();
                let tm = v.mode_ctx_mut::<TextModeContext>("text-mode");
                let offset = tm.marks[tm.mark_index].offset;

                tm.mark_index = 0;
                tm.marks.clear();
                tm.marks.push(Mark { offset });
            }

            Action::CheckMarks => {
                let v = &mut view.borrow_mut();
                let tm = v.mode_ctx_mut::<TextModeContext>("text-mode");
                tm.marks.dedup();
                tm.mark_index = tm.marks.len().saturating_sub(1);

                // TODO: Action::UpdateReadCache(s) vs multiple views
                // TODO: adjust with v.star_offset ..
                if tm.marks.len() > 0 {
                    let min = tm.marks[0].offset;
                    let max = tm.marks[tm.marks.len() - 1].offset;
                    let doc = v.document.clone();
                    let doc = doc.as_ref().unwrap();
                    let mut doc = doc.as_ref().write().unwrap();
                    doc.set_cache(min, max);
                }
            }

            Action::SaveCurrentMarks => {
                let v = &mut view.borrow_mut();
                let doc = v.document.clone();
                let doc = doc.as_ref().unwrap();
                let mut doc = doc.as_ref().write().unwrap();
                let tm = v.mode_ctx_mut::<TextModeContext>("text-mode");

                let max_offset = doc.size() as u64;
                let marks_offsets: Vec<u64> = tm.marks.iter().map(|m| m.offset).collect();
                doc.tag(env.current_time, max_offset, marks_offsets);
            }

            Action::DedupAndSaveMarks => {
                let v = &mut view.borrow_mut();
                let tm = v.mode_ctx_mut::<TextModeContext>("text-mode");

                //
                tm.marks.dedup();
                let marks_offsets: Vec<u64> = tm.marks.iter().map(|m| m.offset).collect();

                //
                let doc = v.document.as_ref().unwrap();
                let mut doc = doc.as_ref().write().unwrap();
                let max_offset = doc.size() as u64;
                doc.tag(env.current_time, max_offset, marks_offsets);
            }

            Action::CancelSelection => {
                let v = &mut view.borrow_mut();
                let tm = v.mode_ctx_mut::<TextModeContext>("text-mode");
                tm.select_point.clear();
            }
        }
    }
}

///////////////////////////////////////////////////////////////////////////////////////////////////
//
// text mode functions

pub fn save_marks(_editor: &mut Editor, _env: &mut EditorEnv, view: &Rc<RefCell<View>>) {
    let v = &mut view.borrow_mut();
    v.pre_compose_action.push(Action::SaveCurrentMarks);
}

pub fn cancel_marks(_editor: &mut Editor, _env: &mut EditorEnv, view: &Rc<RefCell<View>>) {
    let v = &mut view.borrow_mut();

    v.pre_compose_action.push(Action::ResetMarks);

    let tm = v.mode_ctx_mut::<TextModeContext>("text-mode");
    let offset = tm.marks[tm.mark_index].offset;

    tm.mark_index = 0;
    tm.marks.clear();
    tm.marks.push(Mark { offset });
}

// text mode functions
pub fn cancel_selection(_editor: &mut Editor, _env: &mut EditorEnv, view: &Rc<RefCell<View>>) {
    let v = &mut view.borrow_mut();
    let tm = v.mode_ctx_mut::<TextModeContext>("text-mode");

    tm.select_point.clear();
}

pub fn editor_cancel(editor: &mut Editor, env: &mut EditorEnv, view: &Rc<RefCell<View>>) {
    cancel_marks(editor, env, view);

    cancel_selection(editor, env, view);
}

pub fn scroll_up(_editor: &mut Editor, _env: &mut EditorEnv, view: &Rc<RefCell<View>>) {
    // TODO: 3 is from mode configuration
    // env["default-scroll-size"] -> int
    let v = &mut view.borrow_mut();
    v.pre_compose_action.push(Action::ScrollUp { n: 3 });
}

pub fn scroll_down(_editor: &mut Editor, _env: &mut EditorEnv, view: &Rc<RefCell<View>>) {
    // TODO: 3 is from mode configuration
    // env["default-scroll-size"] -> int
    let v = &mut view.borrow_mut();
    v.pre_compose_action.push(Action::ScrollDown { n: 3 });
}

// TODO: rename into handle_input_events
/// Insert an single element/array of unicode code points using hardcoded utf8 codec.<br/>
pub fn insert_codepoint_array(editor: &mut Editor, env: &mut EditorEnv, view: &Rc<RefCell<View>>) {
    let array = {
        assert!(env.trigger.len() > 0);
        let idx = env.trigger.len() - 1;
        match &env.trigger[idx] {
            InputEvent::KeyPress {
                mods:
                    KeyModifiers {
                        ctrl: false,
                        alt: false,
                        shift: false,
                    },
                key: Key::UnicodeArray(ref v),
            } => v.clone(), // should move Rc<> ?

            InputEvent::KeyPress {
                key: Key::Unicode(c),
                mods:
                    KeyModifiers {
                        ctrl: false,
                        alt: false,
                        shift: false,
                    },
            } => {
                vec![*c]
            }

            _ => {
                return;
            }
        }
    };

    // doc read only ?
    {
        let v = view.borrow();
        let doc = v.document.clone();
        let doc = doc.as_ref().unwrap();
        let doc = doc.as_ref().read().unwrap();
        if doc.is_syncing {
            return;
        }
    }

    // delete selection before insert
    copy_maybe_remove_selection(editor, env, view, false, true);

    let center = {
        let mut v = view.borrow_mut();
        let view_start = v.start_offset;
        let mut view_growth = 0;
        let mut offset: u64 = 0;
        {
            let mut doc = v.document.clone();
            let doc = doc.as_mut().unwrap();
            let mut doc = doc.as_ref().write().unwrap();

            let tm = v.mode_ctx_mut::<TextModeContext>("text-mode");

            let codec = tm.text_codec.as_ref();
            let mut utf8 = Vec::with_capacity(array.len());

            for codepoint in &array {
                let mut data: &mut [u8] = &mut [0, 0, 0, 0];
                let data_size = codec.encode(*codepoint as u32, &mut data);
                for d in data.iter().take(data_size) {
                    utf8.push(*d);
                }
            }

            let mut grow: u64 = 0;

            let marks_offsets: Vec<u64> = tm.marks.iter().map(|m| m.offset).collect();

            let max_offset = doc.size() as u64;
            doc.tag(env.current_time, max_offset, marks_offsets);

            for m in tm.marks.iter_mut() {
                if m.offset < view_start {
                    view_growth += utf8.len() as u64;
                }

                m.offset += grow;
                doc.insert(m.offset, utf8.len(), &utf8);
                m.offset += utf8.len() as u64;

                offset = m.offset; // TODO: remove this merge

                grow += utf8.len() as u64;
            }

            let max_offset = doc.size() as u64;
            //
            let marks_offsets: Vec<u64> = tm.marks.iter().map(|m| m.offset).collect();
            doc.tag(env.current_time, max_offset, marks_offsets);
        }
        v.start_offset += view_growth;

        dbg_println!("view_growth = {}", view_growth);

        // mark off_screen ?
        let screen = v.screen.read().unwrap();
        screen.contains_offset(offset) == false || array.len() > screen.width() * screen.height()
    };

    {
        let mut v = view.borrow_mut();
        if center {
            v.pre_compose_action.push(Action::CenterAroundMainMark);
        };

        v.pre_compose_action.push(Action::CancelSelection);
    }
}

pub fn remove_previous_codepoint(
    editor: &mut Editor,
    env: &mut EditorEnv,

    view: &Rc<RefCell<View>>,
) {
    if copy_maybe_remove_selection(editor, env, view, false, true) > 0 {
        return;
    }

    let mut scroll_down = 0;
    let v = &mut view.borrow_mut();
    let start_offset = v.start_offset;

    {
        let doc = v.document.clone();
        let doc = doc.as_ref().clone().unwrap();
        let mut doc = doc.as_ref().write().unwrap();
        if doc.size() == 0 {
            return;
        }

        let tm = v.mode_ctx_mut::<TextModeContext>("text-mode");
        let codec = tm.text_codec.as_ref();

        let marks_offsets: Vec<u64> = tm.marks.iter().map(|m| m.offset).collect();
        let max_offset = doc.size() as u64;
        doc.tag(env.current_time, max_offset, marks_offsets);

        let mut shrink = 0;
        for m in tm.marks.iter_mut() {
            dbg_println!("before shrink m.offset= {}", m.offset);
            m.offset = m.offset.saturating_sub(shrink);
            dbg_println!("after shrink m.offset= {}", m.offset);

            if m.offset == 0 {
                continue;
            }

            m.move_backward(&doc, codec);
            dbg_println!("after move.backward m.offset= {}", m.offset);

            let mut data = vec![];
            doc.read(m.offset, 4, &mut data);
            let (_, _, size) = codec.decode(SyncDirection::Forward, &data, 0);
            dbg_println!("read {} bytes", size);

            let nr_removed = doc.remove(m.offset, size, None);
            dbg_println!("nr_removed {} bytes", nr_removed);

            dbg_println!(
                "shrink({}) + nr_rm({}) = {}",
                shrink,
                nr_removed,
                shrink + nr_removed as u64
            );
            shrink += nr_removed as u64;

            if m.offset < start_offset {
                scroll_down = 1;
            }
        }

        let max_offset = doc.size() as u64;

        let marks_offsets = tm.marks.iter().map(|m| m.offset).collect();
        doc.tag(env.current_time, max_offset, marks_offsets);
    }

    // schedule render actions
    {
        if scroll_down > 0 {
            v.pre_compose_action.push(Action::ScrollUp { n: 1 });
        }
        v.pre_compose_action.push(Action::CheckMarks);
    }
}

/// Undo the previous write operation and sync the screen around the main mark.<br/>
pub fn undo(_editor: &mut Editor, _env: &mut EditorEnv, view: &Rc<RefCell<View>>) {
    let v = &mut view.borrow_mut();

    let mut doc = v.document.clone();
    let doc = doc.as_mut().unwrap();
    let mut doc = doc.as_ref().write().unwrap();

    let tm = v.mode_ctx_mut::<TextModeContext>("text-mode");
    let marks = &mut tm.marks;

    doc.undo_until_tag();
    doc.undo_until_tag();
    if let Some(marks_offsets) = doc.get_tag_offsets() {
        //dbg_println!("restore marks {:?}", marks_offsets);
        marks.clear();
        for offset in marks_offsets {
            marks.push(Mark { offset });
        }
    }

    tm.mark_index = 0;

    v.pre_compose_action
        .push(Action::CenterAroundMainMarkIfOffScreen);

    v.pre_compose_action.push(Action::CancelSelection);
}

/// Redo the previous write operation and sync the screen around the main mark.<br/>
pub fn redo(_editor: &mut Editor, _env: &mut EditorEnv, view: &Rc<RefCell<View>>) {
    let v = &mut view.borrow_mut();

    let mut doc = v.document.clone();
    let doc = doc.as_mut().unwrap();
    let mut doc = doc.as_ref().write().unwrap();

    let tm = v.mode_ctx_mut::<TextModeContext>("text-mode");
    let marks = &mut tm.marks;

    tm.mark_index = 0;

    doc.redo_until_tag();
    doc.redo_until_tag();
    if let Some(marks_offsets) = doc.get_tag_offsets() {
        //dbg_println!("restore marks {:?}", marks_offsets);
        marks.clear();
        for offset in marks_offsets {
            marks.push(Mark { offset });
        }
    }

    v.pre_compose_action
        .push(Action::CenterAroundMainMarkIfOffScreen);
    v.pre_compose_action.push(Action::CancelSelection);
}

/// Remove the current utf8 encoded code point.<br/>
pub fn remove_codepoint(editor: &mut Editor, env: &mut EditorEnv, view: &Rc<RefCell<View>>) {
    if copy_maybe_remove_selection(editor, env, view, false, true) > 0 {
        return;
    }

    let v = &mut view.borrow_mut();
    let view_start = v.start_offset;
    let mut view_shrink: u64 = 0;

    {
        let mut doc = v.document.clone();
        let doc = doc.as_mut().unwrap();
        let mut doc = doc.as_ref().write().unwrap();

        let tm = v.mode_ctx_mut::<TextModeContext>("text-mode");

        let codec = tm.text_codec.as_ref();

        if doc.size() == 0 {
            return;
        }

        let marks_offsets: Vec<u64> = tm.marks.iter().map(|m| m.offset).collect();
        let max_offset = doc.size() as u64;
        doc.tag(env.current_time, max_offset, marks_offsets);

        let mut shrink = 0;

        for m in tm.marks.iter_mut() {
            if m.offset >= shrink {
                m.offset -= shrink;
            }

            let mut data = Vec::with_capacity(4);
            doc.read(m.offset, data.capacity(), &mut data);
            let (_, _, size) = codec.decode(SyncDirection::Forward, &data, 0);

            if m.offset < view_start {
                view_shrink += size as u64;
            }

            let nr_removed = doc.remove(m.offset, size as usize, None);
            shrink += nr_removed as u64;
        }

        let _max_offset = doc.size() as u64;
    }
    v.start_offset -= view_shrink;

    v.pre_compose_action.push(Action::CheckMarks);
    v.pre_compose_action.push(Action::DedupAndSaveMarks);
    v.pre_compose_action.push(Action::CancelSelection);
}

/// Skip blanks (if any) and remove until end of the word.
/// TODO: handle ',' | ';' | '(' | ')' | '{' | '}'
pub fn remove_until_end_of_word(
    _editor: &mut Editor,
    env: &mut EditorEnv,

    view: &Rc<RefCell<View>>,
) {
    let v = &mut view.borrow_mut();

    let mut doc = v.document.clone();
    let doc = doc.as_mut().unwrap();
    let mut doc = doc.as_ref().write().unwrap();

    let tm = v.mode_ctx_mut::<TextModeContext>("text-mode");

    let codec = tm.text_codec.as_ref();

    let size = doc.size() as u64;

    if size == 0 {
        return;
    }

    let marks_offsets: Vec<u64> = tm.marks.iter().map(|m| m.offset).collect();
    doc.tag(env.current_time, size, marks_offsets);

    let mut shrink: u64 = 0;

    for m in tm.marks.iter_mut() {
        if m.offset >= shrink {
            m.offset -= shrink;
        }

        let start = m.clone();

        let mut data = Vec::with_capacity(4);

        // skip blanks until any char or end-of-line
        loop {
            data.clear();
            doc.read(m.offset, data.capacity(), &mut data);
            let (cp, _, size) = codec.decode(SyncDirection::Forward, &data, 0);

            if size == 0 {
                break;
            }

            match cp {
                ' ' | '\t' => {
                    m.offset += size as u64;
                    continue;
                }

                _ => break,
            }
        }

        // skip until blank or end-of-line
        loop {
            data.clear();
            doc.read(m.offset, data.capacity(), &mut data);
            let (cp, _, size) = codec.decode(SyncDirection::Forward, &data, 0);

            if size == 0 {
                break;
            }

            match cp {
                ' ' | '\t' | '\r' | '\n' => {
                    break;
                }

                _ => {
                    m.offset += size as u64;
                    continue;
                }
            }
        }

        // remove [start, m[
        let nr_removed = doc.remove(start.offset, (m.offset - start.offset) as usize, None);

        shrink += nr_removed as u64;

        m.offset = start.offset;
    }

    let marks_offsets: Vec<u64> = tm.marks.iter().map(|m| m.offset).collect();

    let max_offset = doc.size() as u64;
    doc.tag(env.current_time, max_offset, marks_offsets);

    v.pre_compose_action.push(Action::CheckMarks);
    v.pre_compose_action.push(Action::CancelSelection); //TODO register last optype
                                                        // if doc changes cancel selection ?
}

// TODO: maintain main mark Option<(x,y)>
pub fn move_marks_backward(_editor: &mut Editor, _env: &mut EditorEnv, view: &Rc<RefCell<View>>) {
    let v = &mut view.borrow_mut();

    let start_offset = v.start_offset;

    let doc = v.document.clone();
    let doc = doc.as_ref().unwrap();
    let doc = doc.as_ref().read().unwrap();

    let tm = v.mode_ctx_mut::<TextModeContext>("text-mode");

    let codec = tm.text_codec.as_ref();

    let midx = tm.mark_index;

    let mut scroll_down = 0;
    for (idx, m) in tm.marks.iter_mut().enumerate() {
        if idx == midx && m.offset <= start_offset {
            scroll_down = 1;
        }

        m.move_backward(&doc, codec);
    }

    if scroll_down > 0 {
        v.pre_compose_action.push(Action::ScrollUp { n: 1 });
    }

    v.pre_compose_action.push(Action::CheckMarks);

    let tm = v.mode_ctx::<TextModeContext>("text-mode");

    if tm.center_on_mark_move {
        v.pre_compose_action.push(Action::CenterAroundMainMark);
    }
}

pub fn move_marks_forward(_editor: &mut Editor, _env: &mut EditorEnv, view: &Rc<RefCell<View>>) {
    let mut scroll_down = 0;

    {
        let v = &mut view.borrow_mut();

        let screen_has_eof = v.screen.read().unwrap().has_eof();
        let end_offset = v.end_offset;

        //
        let doc = v.document.clone();
        let doc = doc.as_ref().unwrap();
        let doc = doc.as_ref().read().unwrap();

        let tm = v.mode_ctx_mut::<TextModeContext>("text-mode");

        let codec = tm.text_codec.as_ref();

        let midx = tm.mark_index;

        let nr_marks = {
            for (idx, m) in tm.marks.iter_mut().enumerate() {
                // mark move off_screen ? scroll down 1 line
                m.move_forward(&doc, codec);

                if idx == midx && m.offset >= end_offset && !screen_has_eof {
                    scroll_down = 1;
                }
            }

            // update main mark index
            tm.marks.len()
        };

        // TODO:  v.pre_compose_action.push(Action::SelectLastMark);
        tm.mark_index = nr_marks.saturating_sub(1); // TODO: dedup ?
    }

    //      move this check at post render to reschedule render ?
    //      if v.center_on_mark_move {
    //           v.pre_compose_action.push(Action::CenterAroundMainMark);
    //      }
    {
        let v = &mut view.borrow_mut();

        if scroll_down > 0 {
            v.pre_compose_action
                .push(Action::ScrollDown { n: scroll_down });
        }

        v.pre_compose_action.push(Action::CheckMarks);
    }
}

pub fn move_marks_to_start_of_line(
    _editor: &mut Editor,
    _env: &mut EditorEnv,

    view: &Rc<RefCell<View>>,
) {
    let v = &mut view.borrow_mut();
    let screen = v.screen.clone();
    let screen = screen.read().unwrap();

    let doc = v.document.clone();
    let doc = doc.as_ref().unwrap().read().unwrap();

    let tm = v.mode_ctx_mut::<TextModeContext>("text-mode");
    let codec = tm.text_codec.as_ref();
    //
    let midx = tm.mark_index;

    let mut center = false;
    for (idx, m) in tm.marks.iter_mut().enumerate() {
        m.move_to_start_of_line(&doc, codec);

        if idx == midx && screen.contains_offset(m.offset) == false {
            center = true;
        }
    }

    if center {
        v.pre_compose_action.push(Action::CenterAroundMainMark);
    }
    v.pre_compose_action.push(Action::CheckMarks);
}

pub fn move_marks_to_end_of_line(
    _editor: &mut Editor,
    _env: &mut EditorEnv,

    view: &Rc<RefCell<View>>,
) {
    let mut v = view.borrow_mut();
    let screen = v.screen.clone();
    let screen = screen.read().unwrap();

    let doc = v.document.clone();
    let doc = doc.as_ref().unwrap().read().unwrap();

    let tm = v.mode_ctx_mut::<TextModeContext>("text-mode");

    let codec = tm.text_codec.as_ref();

    let midx = tm.mark_index;

    let mut center = false;
    for (idx, m) in tm.marks.iter_mut().enumerate() {
        m.move_to_end_of_line(&doc, codec);

        if idx == midx && screen.contains_offset(m.offset) == false {
            center = true;
        }
    }

    if center {
        v.pre_compose_action.push(Action::CenterAroundMainMark);
    }

    v.pre_compose_action.push(Action::CheckMarks);
}

fn move_mark_to_previous_line(
    editor: &mut Editor,
    env: &mut EditorEnv,

    v: &mut View,
    midx: usize,
    marks: &mut Vec<Mark>,
) {
    let mut mark_moved = false;

    let m_offset = {
        let screen = v.screen.clone();
        let screen = screen.read().unwrap();
        let mut m = &mut marks[midx];

        // TODO: if v.is_mark_on_screen(m) -> (bool, x, y) + (prev/new offset)?
        match screen.find_cpi_by_offset(m.offset) {
            // off_screen
            (None, _, _) => {}
            // mark on first line
            (Some(_), _, y) if y == 0 => {}

            // onscreen
            (Some(_), x, y) if y > 0 => {
                // TODO: refactor code to support screen cell metadata
                let new_y = y - 1; // select previous line
                let l = screen.get_line(new_y).unwrap();
                // previous line is filled ?
                if l.nb_cells > 0 {
                    let new_x = ::std::cmp::min(x, l.nb_cells - 1);
                    let cpi = screen.get_cpinfo(new_x, new_y).unwrap();
                    if !cpi.metadata {
                        m.offset = cpi.offset.unwrap();
                        mark_moved = true;
                    }
                } else {
                    // ???
                }
            }

            // impossible
            _ => {}
        }

        m.offset
    };

    // off_screen
    if !mark_moved {
        // mark is off_screen

        let end_offset = m_offset;
        let (start_offset, screen_width, screen_height) = {
            let start_offset = {
                let doc = v.document.as_ref().unwrap();
                let doc = doc.as_ref().read().unwrap();

                let tm = v.mode_ctx::<TextModeContext>("text-mode");

                let codec = tm.text_codec.as_ref();

                // todo: set marks codecs
                let mut tmp = Mark { offset: m_offset };

                // goto start of current line (mar is on first line of screen)
                tmp.move_to_start_of_line(&doc, codec);
                // goto end of previous line
                tmp.move_backward(&doc, codec);
                // goto start of previous line
                tmp.move_to_start_of_line(&doc, codec);
                tmp.offset

                /*
                if m.offset - tmp.offset > (screen.width * screen.height)
                {
                   long line mode
                }
                else {

                }

                */
            };

            let width = v.screen.read().unwrap().width();

            let add_height = if width > 0 {
                (m_offset - start_offset) as usize / width
            } else {
                1
            };
            let height = v.screen.read().unwrap().height() + (add_height * 4); // 4 is utf8 max encode size

            (start_offset, width, height)
        };

        // TODO: loop until m.offset is on screen

        let lines = {
            v.get_lines_offsets_direct(
                editor,
                env,
                start_offset,
                end_offset,
                screen_width,
                screen_height,
            )
        };

        // find "previous" line index
        let index = match lines
            .iter()
            .position(|e| e.0 <= end_offset && end_offset <= e.1)
        {
            None | Some(0) => return, // error
            Some(i) => i - 1,
        };

        let line_start_off = lines[index].0;
        let line_end_off = lines[index].1;

        let mut tmp_mark = Mark::new(line_start_off);

        // compute column
        let new_x = {
            let doc = v.document.as_ref().unwrap();
            let doc = doc.as_ref().read().unwrap();

            let tm = v.mode_ctx::<TextModeContext>("text-mode");

            let codec = tm.text_codec.as_ref();

            let mut s = Mark::new(lines[index + 1].0);
            let e = Mark::new(lines[index + 1].1);
            let mut count = 0;
            while s.offset != e.offset {
                if s.offset == m_offset {
                    break;
                }

                s.move_forward(&doc, codec);
                count += 1;
            }
            count
        };

        {
            let doc = v.document.as_ref().unwrap();
            let doc = doc.as_ref().read().unwrap();
            let tm = v.mode_ctx::<TextModeContext>("text-mode");

            let codec = tm.text_codec.as_ref();

            for _ in 0..new_x {
                tmp_mark.move_forward(&doc, codec);
            }

            tmp_mark.offset = std::cmp::min(tmp_mark.offset, line_end_off);
        }
        // TODO: add some post processing after screen moves
        // this will avoid custom code in pageup/down
        // if m.offset < screen.start -> m.offset = start_offset
        // if m.offset > screen.end -> m.offset = screen.line[last_index].start_offset

        // resync mark to "new" first line offset
        if tmp_mark.offset < m_offset {
            marks[midx].offset = tmp_mark.offset;
        }
    }
}

pub fn move_marks_to_previous_line(
    editor: &mut Editor,
    env: &mut EditorEnv,

    view: &Rc<RefCell<View>>,
) {
    let (mut marks, idx_max) = {
        let mut v = view.borrow_mut();
        let tm = v.mode_ctx_mut::<TextModeContext>("text-mode");

        // TODO: maintain env.mark_index_max ?
        let idx_max = tm.marks.len() - 1;
        (tm.marks.clone(), idx_max)
    };

    let mut mark_index = None;

    {
        let mut v = view.borrow_mut();
        let screen = v.screen.clone();
        let screen = screen.read().unwrap();

        for idx in 0..=idx_max {
            let prev_offset = marks[idx].offset;
            move_mark_to_previous_line(editor, env, &mut v, idx, &mut marks);

            // TODO: move this to pre/post render
            if idx == 0 {
                // v.pre_compose_action.push(Action::UpdateViewOnMainMarkMove { moveType: ToPreviousLine, before: prev_offset, after: new_offset });
                let new_offset = marks[idx].offset;

                if new_offset != prev_offset {
                    mark_index = Some(0); // reset main mark

                    let was_on_screen = screen.contains_offset(prev_offset);
                    let is_on_screen = screen.contains_offset(new_offset);
                    if was_on_screen && !is_on_screen {
                        v.pre_compose_action.push(Action::ScrollUp { n: 1 });
                    } else if !is_on_screen {
                        v.pre_compose_action.push(Action::CenterAroundMainMark);
                    }
                }
            }
        }
    }

    {
        // copy back
        let mut v = view.borrow_mut();

        let tm = v.mode_ctx_mut::<TextModeContext>("text-mode");
        tm.marks = marks;
        if let Some(idx) = mark_index {
            tm.mark_index = idx;
        }

        // schedule actions
        v.pre_compose_action.push(Action::CheckMarks);
    }
}

pub fn move_on_screen_mark_to_next_line(
    m: &mut Mark,
    screen: &Screen,
) -> (bool, Option<(u64, u64)>, Option<Action>) {
    // TODO: add hints: check in screen range
    if !screen.contains_offset(m.offset) {
        dbg_println!(" offset {} not found in screen", m.offset);

        return (false, None, None);
    }

    // get offset coordinates
    let (_, x, y) = screen.find_cpi_by_offset(m.offset);
    let screen_height = screen.height();

    dbg_println!("FOUND m.offset @ (X({}), Y({}))", x, y);
    dbg_println!("screen_height {}", screen_height);

    // mark on last line -> must scroll
    let new_y = y + 1;
    if new_y >= screen_height {
        // mark on last screen line cannot be updated
        assert_eq!(y, screen_height - 1);

        dbg_println!(" next line off_screen MUST scroll to compute");

        return (false, None, Some(Action::ScrollDown { n: 1 }));
    }

    // new_y < screen_height
    let l = screen.get_line(new_y).unwrap();
    if l.nb_cells == 0 {
        // line is empty do nothing
        dbg_println!(" NEXT line is EMPTY do nothing ..........");

        return (true, Some((m.offset, m.offset)), None);
    }

    // l.nb_cells > 0
    let new_x = ::std::cmp::min(x, l.nb_cells - 1);
    let cpi = screen.get_cpinfo(new_x, new_y).unwrap();

    let old_offset = m.offset;
    m.offset = cpi.offset.unwrap();

    dbg_println!("update mark : offset => {} -> {}", old_offset, m.offset);

    // ok
    (true, Some((old_offset, m.offset)), None)
}

// remove multiple borrows
pub fn move_mark_to_next_line(
    editor: &Editor,
    env: &mut EditorEnv,
    view: &Rc<RefCell<View>>,
    mark_idx: usize,
) -> Option<(u64, u64)> {
    // TODO: m.on_buffer_end() ?

    let max_offset = {
        let v = view.borrow();
        v.document().as_ref().unwrap().read().unwrap().size() as u64
    };

    // off_screen ?
    let mut m_offset;
    let old_offset;

    {
        let mut v = view.borrow_mut();
        let screen = v.screen.clone();
        let screen = screen.read().unwrap();
        let tm = v.mode_ctx_mut::<TextModeContext>("text-mode");
        let marks = &mut tm.marks;
        let mut m = &mut marks[mark_idx];
        m_offset = m.offset;
        old_offset = m.offset;

        if m.offset == max_offset {
            return None;
        }

        let (ok, offsets, action) = move_on_screen_mark_to_next_line(&mut m, &screen);
        if let Some(action) = action {
            // Add stage RenderStage :: PreRender PostRender
            // will be removed when the "scroll" update is implemented
            // ADD screen cache ?
            // screen[first mark -> last mark ] ? Ram usage ?
            // updated on resize -> slow
            v.pre_compose_action.push(action);
        }

        if ok == true {
            return offsets;
        }
    }

    if true {
        // mark is off_screen
        let (screen_width, screen_height) = {
            let view = view.borrow_mut();
            let screen = view.screen.read().unwrap();
            (screen.width(), screen.height())
        };

        // get start_of_line(m.offset) -> u64
        let start_offset = {
            let v = &view.borrow();
            let doc = v.document.as_ref().unwrap();
            let doc = doc.as_ref().read().unwrap();

            let tm = v.mode_ctx::<TextModeContext>("text-mode");
            let codec = tm.text_codec.as_ref();

            let m = &tm.marks[mark_idx];
            let mut tmp = Mark::new(m.offset);
            tmp.move_to_start_of_line(&doc, codec);
            tmp.offset
        };

        // a codepoint can use 4 bytes the virtual end is
        // + 1 full line away
        let end_offset = ::std::cmp::min(m_offset + (4 * screen_width) as u64, max_offset);

        // get lines start, end offset
        // NB: run full layout code for one screen line ( folding etc ... )

        // TODO: return Vec<Box<screen>> ? update contenet
        // TODO: add perf view screen cache ? sorted by screens.start_offset
        // with same width/heigh as v.screen
        let lines = {
            let mut view = view.borrow_mut();
            view.get_lines_offsets_direct(
                editor,
                env,
                start_offset,
                end_offset,
                screen_width,
                screen_height,
            )
        };

        dbg_println!("GET {} lines ", lines.len());

        // find the cursor index
        let index = match lines
            .iter()
            .position(|e| e.0 <= m_offset && m_offset <= e.1)
        {
            None => return None,
            Some(i) => {
                if i == lines.len() - 1 {
                    return None;
                } else {
                    i
                }
            }
        };

        // compute column
        let new_x = {
            let v = &view.borrow();
            let doc = v.document.as_ref().unwrap();
            let doc = doc.as_ref().read().unwrap();

            let tm = v.mode_ctx::<TextModeContext>("text-mode");

            let codec = tm.text_codec.as_ref();

            // TODO: use codec.read(doc, n=width) until e.offset is reached
            let mut s = Mark::new(lines[index].0);
            let e = Mark::new(lines[index].1);
            let mut count = 0;
            while s.offset < e.offset {
                if s.offset == m_offset {
                    break;
                }

                s.move_forward(&doc, codec);
                count += 1;
            }
            count
        };

        // get next line start/end offsets
        let next_index = index + 1;
        let line_start_off = lines[next_index].0;
        let line_end_off = lines[next_index].1;

        let mut tmp_mark = Mark::new(line_start_off);

        let v = &view.borrow();
        let doc = v.document.as_ref().unwrap();
        let doc = doc.as_ref().read().unwrap();

        let tm = v.mode_ctx::<TextModeContext>("text-mode");

        let codec = tm.text_codec.as_ref();

        // TODO: codec.skip_n(doc, 0..new_x)
        for _ in 0..new_x {
            tmp_mark.move_forward(&doc, codec); // TODO: pass n as arg
        }

        tmp_mark.offset = std::cmp::min(tmp_mark.offset, line_end_off);

        m_offset = tmp_mark.offset;
    }

    {
        let mut v = view.borrow_mut();
        let tm = v.mode_ctx_mut::<TextModeContext>("text-mode");
        tm.marks[mark_idx].offset = m_offset;
    }

    Some((old_offset, m_offset))
}

///////////////////////////////////////////////////////////////////////////////////////////////////

// allocate and sets screen.first_offset
fn allocate_temporary_screen_and_start_offset(view: &Rc<RefCell<View>>) -> (Screen, u64) {
    let (width, height, first_offset) = {
        let v = view.borrow();
        let screen = v.screen.clone();
        let screen = screen.read().unwrap();
        let first_offset = screen.first_offset.unwrap();

        let width = screen.width();
        /*
          NB : the virtual screen MUST but big enough to compute the marks on the the last line
        */
        let height = screen.height() * 2;
        dbg_println!("current screen : {} x {}", screen.width(), screen.height());
        dbg_println!("new virtual screen : {} x {}", width, height);
        (width, height, first_offset)
    };
    let screen = Screen::new(width, height);
    (screen, first_offset)
}

// min offset, max index
fn get_marks_min_offset_and_max_idx(view: &Rc<RefCell<View>>) -> (u64, usize) {
    let mut v = view.borrow_mut();

    let tm = v.mode_ctx_mut::<TextModeContext>("text-mode");
    let idx_max = tm.marks.len();
    assert!(idx_max > 0);

    let marks = &mut tm.marks;
    let min_offset = marks[0].offset;
    let max_offset = marks[idx_max - 1].offset;

    dbg_println!("max_offset {} - min_offset {}", max_offset, min_offset);

    (min_offset, idx_max)
}

fn sync_mark(view: &Rc<RefCell<View>>, m: &mut Mark) -> u64 {
    let v = view.borrow();
    let doc = v.document.clone();
    let doc = doc.as_ref().unwrap();
    let doc = doc.as_ref().read().unwrap();

    // ctx
    let tm = v.mode_ctx::<TextModeContext>("text-mode");

    let codec = tm.text_codec.as_ref();

    // get "real" line start
    m.move_to_start_of_line(&doc, codec);

    let doc_size = doc.size() as u64;
    if doc_size > 0 {
        assert!(m.offset < doc_size);
    }

    doc_size
}

/*

*/
pub fn move_marks_to_next_line(editor: &mut Editor, env: &mut EditorEnv, view: &Rc<RefCell<View>>) {
    // allocate temporary screen
    let (mut screen, start_offset) = allocate_temporary_screen_and_start_offset(&view);
    screen.is_off_screen = true;
    let mut m = Mark::new(start_offset);

    // min offset, max index
    let (first_mark_offset, idx_max) = get_marks_min_offset_and_max_idx(&view);

    // set screen start
    m.offset = std::cmp::min(m.offset, first_mark_offset);

    let max_offset = sync_mark(&view, &mut m); // codec in name ?

    // copy all marks
    let mut marks = {
        let mut v = view.borrow_mut();
        let tm = v.mode_ctx_mut::<TextModeContext>("text-mode");
        tm.marks.clone()
    };

    // TODO: add eof in conditions
    // find a way to transform while loops into iterator over screens
    // document_walk ? ...
    // ctx

    // update all marks
    {
        let v = view.borrow();
        let mut idx_start = 0;
        while idx_start < idx_max {
            dbg_println!(" idx_start {} < idx_max {}", idx_start, idx_max);

            dbg_println!(
                "looking for marks[idx_start].offset = {}",
                marks[idx_start].offset
            );

            // update screen with configure filters
            screen.clear();

            dbg_println!("compute layout from offset {}", m.offset);

            run_compositing_stage_direct(editor, env, &v, m.offset, max_offset, &mut screen);

            dbg_println!("screen first offset {:?}", screen.first_offset);
            dbg_println!("screen last offset {:?}", screen.last_offset);
            dbg_println!("max_offset {}", max_offset);

            assert_ne!(0, screen.push_count()); // at least EOF

            // TODO: pass doc &doc to avoid double borrow
            // env.doc ?
            // env.view ? to avoid too many args

            //
            let last_line = screen.get_last_used_line();
            if last_line.is_none() {
                dbg_println!("no last line");
                panic!();
                //break;
            }
            let last_line = last_line.unwrap();

            // go to next screen
            // using the firt offset of the last line
            if let Some(cpi) = last_line.get_first_cpi() {
                let last_line_first_offset = cpi.offset.unwrap(); // update next screen start offset
                dbg_println!("last_line_first_offset {}", last_line_first_offset);
            } else {
                panic!();
            }

            // idx_start not on screen  ? ...
            if !screen.contains_offset(marks[idx_start].offset) {
                dbg_println!(
                    "offset {} not found on screen go to next screen",
                    marks[idx_start].offset
                );

                if screen.has_eof() {
                    // EOF reached : stop
                    break;
                }

                // Go to next screen
                // use first offset of "current" screen's last line
                // as next screen start points
                if let Some(cpi) = last_line.get_first_cpi() {
                    m.offset = cpi.offset.unwrap(); // update next screen start offset
                    continue;
                }
                panic!();
            }

            // idx_start is on screen
            let mut idx_end = idx_start + 1;
            let next_screen_start_cpi = last_line.get_first_cpi().unwrap();
            while idx_end < idx_max {
                if marks[idx_end].offset >= next_screen_start_cpi.offset.unwrap() {
                    break;
                }
                idx_end += 1;
            }

            dbg_println!("update marks[{}..{} / {}]", idx_start, idx_end, idx_max);

            for i in idx_start..idx_end {
                dbg_println!("update marks[{} / {}]", i, idx_max);

                // TODO: that use/match the returned action
                let ret = move_on_screen_mark_to_next_line(&mut marks[i], &screen);
                if ret.0 == false {
                    dbg_println!(
                        " cannot update marks[{}], offset {} : {:?}",
                        i,
                        marks[i].offset,
                        ret.2
                    );
                }
            }

            idx_start = idx_end; // next mark index

            m.offset = next_screen_start_cpi.offset.unwrap(); // update next screen start
        }
    }

    // check main mark
    {
        let mut v = view.borrow_mut();
        let screen = v.screen.clone();
        let screen = screen.as_ref().read().unwrap();

        let tm = v.mode_ctx_mut::<TextModeContext>("text-mode");

        // set back
        tm.marks = marks;
        let idx = tm.mark_index;

        if !screen.contains_offset(tm.marks[idx].offset) {
            v.pre_compose_action.push(Action::ScrollDown { n: 1 });
            // TODO ?  v.pre_compose_action.push(Action::ScrollDownIfOffsetNotOnScreen { n: 1, offset: tm.marks[idx].offset });
            // TODO ?  v.pre_compose_action.push(Action::ScrollDownIfMainMarkOffScreen { n: 1, offset: tm.marks[idx].offset });
        }

        v.pre_compose_action.push(Action::CheckMarks);
    }
}

pub fn clone_and_move_mark_to_previous_line(
    editor: &mut Editor,
    env: &mut EditorEnv,

    view: &Rc<RefCell<View>>,
) {
    let (mut marks, prev_off) = {
        let v = view.borrow();
        let tm = v.mode_ctx::<TextModeContext>("text-mode");
        (tm.marks.clone(), tm.marks[0].offset)
    };

    dbg_println!(" clone move up: prev_offset {}", prev_off);

    {
        let mut v = view.borrow_mut();
        move_mark_to_previous_line(editor, env, &mut v, 0, &mut marks);
    }

    let mut v = view.borrow_mut();
    let screen = v.screen.clone();
    let screen = screen.read().unwrap();

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
            v.pre_compose_action.push(Action::ScrollUp { n: 1 });
        } else if !is_on_screen {
            v.pre_compose_action.push(Action::CenterAroundMainMark);
        }
    }
}

pub fn clone_and_move_mark_to_next_line(
    editor: &mut Editor,
    env: &mut EditorEnv,

    view: &Rc<RefCell<View>>,
) {
    // refresh mark index
    let mark_len = {
        let mut v = view.borrow_mut();
        let tm = v.mode_ctx_mut::<TextModeContext>("text-mode");

        let mark_len = {
            let marks = &mut tm.marks;
            let midx = marks.len() - 1;
            let offset = marks[midx].offset;
            // duplicated last mark + select
            marks.push(Mark { offset });
            marks.len()
        };

        tm.mark_index = mark_len - 1;
        env.cur_mark_index = Some(tm.mark_index);

        // doc
        let doc = v.document.as_ref().unwrap();
        let doc = doc.as_ref().read().unwrap();
        let _max_offset = doc.size() as u64;

        mark_len
    };

    // NB: borrows: will use rendering pipeline to compute the marks_offset
    let offsets = move_mark_to_next_line(editor, env, view, mark_len - 1); // TODO return offset (old, new)
    if offsets.is_none() {
        dbg_println!(" cannot move mark to next line");
        return;
    }

    let offsets = offsets.unwrap();

    dbg_println!(" clone move down: offsets {:?}", offsets);

    let mut v = view.borrow_mut();
    let tm = v.mode_ctx_mut::<TextModeContext>("text-mode");

    // no move ?
    if offsets.0 == offsets.1 {
        // destroy duplicated mark
        tm.mark_index = {
            let marks = &mut tm.marks;
            marks.pop();
            marks.len() - 1
        };

        let was_on_screen = {
            let screen = v.screen.read().unwrap();
            screen.contains_offset(offsets.0)
        };

        if !was_on_screen {
            v.pre_compose_action.push(Action::CenterAroundMainMark);
        }
        return;
    }

    dbg_println!(" clone move down: new_offset {}", offsets.1);
    // env.sort mark sync direction
    // update view.mark_index

    let (was_on_screen, is_on_screen) = {
        let screen = v.screen.read().unwrap();
        let was_on_screen = screen.contains_offset(offsets.0);
        let is_on_screen = screen.contains_offset(offsets.1);
        dbg_println!(
            " was_on_screen {} , is_on_screen  {}",
            was_on_screen,
            is_on_screen
        );

        (was_on_screen, is_on_screen)
    };

    if was_on_screen && !is_on_screen {
        v.pre_compose_action.push(Action::ScrollDown { n: 1 });
    } else if !is_on_screen {
        v.pre_compose_action.push(Action::CenterAroundMainMark);
    }
}

pub fn move_mark_to_screen_start(
    _editor: &mut Editor,
    _env: &mut EditorEnv,

    view: &Rc<RefCell<View>>,
) {
    let mut v = view.borrow_mut();
    let (start_offset, end_offset) = (v.start_offset, v.end_offset);

    let tm = v.mode_ctx_mut::<TextModeContext>("text-mode");
    let marks = &mut tm.marks;

    for m in marks.iter_mut() {
        // TODO: add main mark check
        if m.offset < start_offset || m.offset > end_offset {
            m.offset = start_offset;
        }
    }
}

pub fn move_mark_to_screen_end(
    _editor: &mut Editor,
    _env: &mut EditorEnv,

    view: &Rc<RefCell<View>>,
) {
    let mut v = view.borrow_mut();
    let (start_offset, end_offset) = (v.start_offset, v.end_offset);

    let tm = v.mode_ctx_mut::<TextModeContext>("text-mode");
    let marks = &mut tm.marks;

    for m in marks.iter_mut() {
        // TODO: add main mark check
        if m.offset < start_offset || m.offset > end_offset {
            m.offset = end_offset;
        }
    }
}

pub fn scroll_to_previous_screen(
    editor: &mut Editor,
    env: &mut EditorEnv,

    view: &Rc<RefCell<View>>,
) {
    {
        let mut v = view.borrow_mut();
        let nb = ::std::cmp::max(v.screen.read().unwrap().height() - 1, 1);
        v.scroll_up(editor, env, nb);
    }

    // TODO: add hints to trigger mar moves
    move_mark_to_screen_end(editor, env, &view);
}

pub fn move_mark_to_start_of_file(
    _editor: &mut Editor,
    _env: &mut EditorEnv,

    view: &Rc<RefCell<View>>,
) {
    let mut v = view.borrow_mut();
    v.start_offset = 0;

    let tm = v.mode_ctx_mut::<TextModeContext>("text-mode");

    tm.mark_index = 0;

    tm.marks.clear();
    tm.marks.push(Mark { offset: 0 });
}

pub fn move_mark_to_end_of_file(
    _editor: &mut Editor,
    _env: &mut EditorEnv,

    view: &Rc<RefCell<View>>,
) {
    let mut v = view.borrow_mut();

    let offset = {
        let doc = v.document.as_ref().unwrap();
        let doc = doc.as_ref().read().unwrap();
        doc.size() as u64
    };
    v.start_offset = offset;

    let tm = v.mode_ctx_mut::<TextModeContext>("text-mode");
    tm.mark_index = 0;

    let marks = &mut tm.marks;
    marks.clear();
    marks.push(Mark { offset });

    //
    let n = v.screen.read().unwrap().height() / 2;
    v.pre_compose_action.push(Action::ScrollUp { n })
}

pub fn scroll_to_next_screen(_editor: &mut Editor, _env: &mut EditorEnv, view: &Rc<RefCell<View>>) {
    let mut v = view.borrow_mut();
    let n = ::std::cmp::max(v.screen.read().unwrap().height() - 1, 1);
    v.pre_compose_action.push(Action::ScrollDown { n });
}

/*
    TODO: with multi marks:
      add per mark cut/paste buffer
      and reuse it when pasting
      check behavior when the marks offset cross each other
      the buffer log is not aware of cut/paste/multicursor
*/
pub fn cut_to_end_of_line(_editor: &mut Editor, env: &mut EditorEnv, view: &Rc<RefCell<View>>) {
    let v = &mut view.borrow_mut();

    // doc read only ?
    {
        let doc = v.document.clone();
        let doc = doc.as_ref().unwrap();
        let doc = doc.as_ref().read().unwrap();
        if doc.is_syncing {
            return;
        }
    }

    let mut doc = v.document.clone();
    let doc = doc.as_mut().unwrap();
    let mut doc = doc.as_ref().write().unwrap();

    let tm = v.mode_ctx_mut::<TextModeContext>("text-mode");

    let codec = tm.text_codec.as_ref();

    tm.copy_buffer.clear();

    let marks_offsets: Vec<u64> = tm.marks.iter().map(|m| m.offset).collect();

    let max_offset = doc.size() as u64;
    doc.tag(env.current_time, max_offset, marks_offsets);
    // TODO: doc.tag(env.current_time, max_offset, marks_offsets, selections);

    let mut remove_size = Vec::with_capacity(tm.marks.len());
    let single_mark = tm.marks.len() == 1;

    // this will join line whith multi-marks
    let remove_eol = false && !single_mark; // && join_lines // TODO: use option join-cut-lines

    // TODO: compute range, check overlaps
    // remove marks in other ranges
    // and cut
    for m in tm.marks.iter_mut().rev() {
        let offset0 = m.offset;

        let mut end = m.clone();
        end.move_to_end_of_line(&doc, codec);
        let offset1 = end.offset;

        // remove end-of-line (\n) ?
        if offset0 == offset1 && single_mark || remove_eol {
            end.move_forward(&doc, codec);
        }

        // remove data
        let size = (end.offset - m.offset) as usize;
        doc.remove(m.offset, size, None);
        remove_size.insert(0, size);

        // save transaction's index
        tm.copy_buffer
            .insert(0, CopyData::BufferLogIndex(doc.buffer_log.pos - 1));
    }

    // update marks offsets
    let mut shrink = 0;
    for (idx, m) in tm.marks.iter_mut().skip(1).enumerate() {
        shrink += remove_size[idx] as u64;
        m.offset -= shrink;
    }

    // invariants
    let mlen = tm.marks.len();
    assert!(tm.copy_buffer.len() == mlen);

    v.pre_compose_action.push(Action::SaveCurrentMarks);
    v.pre_compose_action.push(Action::CheckMarks);
    v.pre_compose_action.push(Action::CancelSelection);
}

pub fn paste(_editor: &mut Editor, env: &mut EditorEnv, view: &Rc<RefCell<View>>) {
    let v = &mut view.borrow_mut();

    // doc read only ?
    {
        let doc = v.document.clone();
        let doc = doc.as_ref().unwrap();
        let doc = doc.as_ref().read().unwrap();
        if doc.is_syncing {
            return;
        }
    }

    let mut doc = v.document.clone();
    let doc = doc.as_mut().unwrap();
    let mut doc = doc.as_ref().write().unwrap();

    let tm = v.mode_ctx_mut::<TextModeContext>("text-mode");

    let marks = &mut tm.marks;
    let marks_len = marks.len();

    dbg_println!("mark_len {}", marks_len);

    dbg_println!("copy_buffer.len() {}", tm.copy_buffer.len());
    if tm.copy_buffer.len() == 0 {
        return;
    }

    // TODO: post_eval stage(editor, env, view, action as member of mode);
    // view::run_text_mode_actions(_editor, env, view, vec![]);
    {
        // TODO: run_action(Action::SaveCurrentMarks);
        // save marks: TODO helper functions
        let marks_offsets: Vec<u64> = marks.iter().map(|m| m.offset).collect();
        let max_offset = doc.size() as u64;
        doc.tag(env.current_time, max_offset, marks_offsets);
    }

    let mut grow = 0;
    for (midx, m) in marks.iter_mut().enumerate() {
        m.offset += grow;

        if tm.copy_buffer.len() != marks_len {
            // TODO: insert each tm.copy_buffer transaction + '\n'
            // grow += each tr
        } else {
            let copy = &tm.copy_buffer[midx];
            let data = match copy {
                CopyData::BufferLogIndex(tridx) => {
                    let tr = doc.buffer_log.data[*tridx].clone();
                    if let Some(ref data) = tr.data {
                        data.as_ref().clone()
                    } else {
                        panic!("wrong transaction index");
                    }
                }
                CopyData::Buffer(data) => data.clone(),
            };

            dbg_println!("paste @ offset {} data.len {}", m.offset, data.len());

            let nr_in = doc.insert(m.offset, data.len(), data.as_slice());
            assert_eq!(nr_in, data.len());
            grow += nr_in as u64;
            m.offset += nr_in as u64;
        }
    }

    v.pre_compose_action.push(Action::SaveCurrentMarks);
    v.pre_compose_action.push(Action::CheckMarks);
    v.pre_compose_action.push(Action::CancelSelection);

    // // mark off_screen ?
    // let screen = v.screen.read().unwrap();
    // screen.contains_offset(offset) == false || array.len() > screen.width() * screen.height()
    // };
    //
    // if center {
    // v.pre_compose_action.push(Action::CenterAroundMainMark);
    // };
}

pub fn move_to_token_start(_editor: &mut Editor, _env: &mut EditorEnv, view: &Rc<RefCell<View>>) {
    // TODO: factorize macrk action
    // mark.apply(fn); where fn=m.move_to_token_end(&doc, codec);
    //

    let mut center = false;

    let v = &mut view.borrow_mut();
    let screen = v.screen.clone();
    let screen = screen.read().unwrap();

    let doc = v.document.clone();
    let doc = doc.as_ref().unwrap();
    let doc = doc.as_ref().read().unwrap();

    let tm = v.mode_ctx_mut::<TextModeContext>("text-mode");

    let codec = tm.text_codec.as_ref();

    let midx = tm.mark_index;

    let marks = &mut tm.marks;

    for (idx, m) in marks.iter_mut().enumerate() {
        m.move_to_token_start(&doc, codec);

        // main mark ?
        if idx == midx {
            if !screen.contains_offset(m.offset) {
                // TODO: push to post action queue
                // {SYNC_VIEW, CLEAR_VIEW, SCROLL_N }
                //
                center = true;
            }
        }
    }

    if center {
        v.pre_compose_action.push(Action::CenterAroundMainMark);
    }
}

pub fn move_to_token_end(_editor: &mut Editor, _env: &mut EditorEnv, view: &Rc<RefCell<View>>) {
    let mut sync = false;

    let mut v = view.borrow_mut();
    let screen = v.screen.clone();
    let screen = screen.read().unwrap();

    let doc = v.document.clone();
    let doc = doc.as_ref().unwrap();
    let doc = doc.as_ref().read().unwrap();

    let tm = v.mode_ctx_mut::<TextModeContext>("text-mode");
    let codec = tm.text_codec.as_ref();

    let marks = &mut tm.marks;

    for m in marks.iter_mut() {
        m.move_to_token_end(&doc, codec);

        // main mark ?
        if !screen.contains_offset(m.offset) {
            // TODO: push to post action queue
            // {SYNC_VIEW, CLEAR_VIEW, SCROLL_N }
            //
            sync = true;
        }
    }

    if sync {
        v.pre_compose_action.push(Action::CenterAroundMainMark);
    }
}

fn _get_main_mark_offset(view: &View) -> u64 {
    let tm = view.mode_ctx::<TextModeContext>("text-mode");
    tm.marks[tm.mark_index].offset
}

pub fn set_selection_points_at_marks(
    _editor: &mut Editor,
    _env: &mut EditorEnv,
    view: &Rc<RefCell<View>>,
) {
    let sync = false;

    {
        let mut v = view.borrow_mut();
        let vid = v.id;

        let tm = v.mode_ctx_mut::<TextModeContext>("text-mode");

        // update selection point
        tm.select_point.clear();
        for m in tm.marks.iter() {
            dbg_println!("VID {} set point @ offset {}", vid, m.offset);
            tm.select_point.push(m.clone());
        }
    }

    if sync
    /* always center ? */
    {
        let mut v = view.borrow_mut();
        v.pre_compose_action.push(Action::CenterAroundMainMark);
    }
}

pub fn copy_maybe_remove_selection_symetric(
    _editor: &mut Editor,
    _env: &mut EditorEnv,

    view: &Rc<RefCell<View>>,
    copy: bool,
    remove: bool,
) -> (usize, usize) {
    let v = &mut view.as_ref().clone().borrow_mut();

    // doc
    let doc = v.document.clone();
    let doc = doc.as_ref().clone().unwrap();
    let mut doc = doc.as_ref().write().unwrap();

    let tm = v.mode_ctx_mut::<TextModeContext>("text-mode");

    let mut nr_bytes_copied = 0;
    let mut nr_bytes_removed = 0;

    if copy {
        tm.copy_buffer.clear();
    }

    let mut shrink = 0;
    for (idx, m) in tm.marks.iter_mut().enumerate() {
        dbg_println!(
            " m.offset({}), tm.select_point[{}].offset {}",
            m.offset,
            idx,
            tm.select_point[idx].offset
        );

        m.offset -= shrink;
        tm.select_point[idx].offset -= shrink;

        let (min, max) = sort_tuple_pair((m.offset, tm.select_point[idx].offset));
        let data_size = (max - min) as usize;
        if copy {
            let mut data = Vec::with_capacity(data_size);
            let nr_read = doc.read(min, data_size, &mut data);
            dbg_println!(
                "nr copied from min({}) -> max({}) = {}",
                min,
                max,
                data_size
            );

            assert_eq!(nr_read, data.len());
            tm.copy_buffer.push(CopyData::Buffer(data));

            nr_bytes_copied += nr_read;
        }
        if remove {
            let nr_removed = doc.remove(min, data_size, None);
            assert_eq!(nr_removed, data_size);
            shrink += data_size as u64;
            nr_bytes_removed += data_size;

            if m.offset > tm.select_point[idx].offset {
                m.offset = m.offset.saturating_sub(nr_bytes_removed as u64);
            }
        }
    }

    if nr_bytes_copied + nr_bytes_removed > 0 {
        tm.select_point.clear();
    }

    (nr_bytes_copied, nr_bytes_removed)
}

pub fn copy_maybe_remove_selection_non_symetric(
    _editor: &mut Editor,
    _env: &mut EditorEnv,

    _view: &Rc<RefCell<View>>,
    _copy: bool,
    _remove: bool,
) -> (usize, usize) {
    (0, 0)
}

pub fn copy_maybe_remove_selection(
    editor: &mut Editor,
    env: &mut EditorEnv,

    view: &Rc<RefCell<View>>,
    copy: bool,
    remove: bool,
) -> usize {
    let symetric = {
        let v = &mut view.as_ref().clone().borrow_mut();
        let _start_offset = v.start_offset;
        let tm = v.mode_ctx_mut::<TextModeContext>("text-mode");
        let symetric = tm.marks.len() == tm.select_point.len();
        symetric
    };

    // todo: sync view(new_start, adjust_size)
    let (copied, removed) = if symetric {
        copy_maybe_remove_selection_symetric(editor, env, view, copy, remove)
    } else {
        copy_maybe_remove_selection_non_symetric(editor, env, view, copy, remove)
    };

    copied + removed
}

// TODO: add help, + flag , copy_maybe_remove_selection()
pub fn copy_selection(editor: &mut Editor, env: &mut EditorEnv, view: &Rc<RefCell<View>>) {
    copy_maybe_remove_selection(editor, env, view, true, false);
}

pub fn cut_selection(editor: &mut Editor, env: &mut EditorEnv, view: &Rc<RefCell<View>>) {
    copy_maybe_remove_selection(editor, env, view, true, true);
}

pub fn button_press(_editor: &mut Editor, env: &mut EditorEnv, view: &Rc<RefCell<View>>) {
    let v = &mut view.borrow_mut();

    let (button, x, y) = match env.trigger[0] {
        InputEvent::ButtonPress(ref button_event) => match button_event {
            ButtonEvent {
                mods:
                    KeyModifiers {
                        ctrl: _,
                        alt: _,
                        shift: _,
                    },
                x,
                y,
                button,
            } => (*button, *x, *y),
        },

        _ => {
            return;
        }
    };

    if !v.check_mode_ctx::<TextModeContext>("text-mode") {
        return;
    }

    let tm = v.mode_ctx_mut::<TextModeContext>("text-mode");

    if (button as usize) < tm.button_state.len() {
        tm.button_state[button as usize] = 1;
    }

    match button {
        0 => {}
        _ => {
            return;
        }
    }

    // TODO: new function clip screen.coordinates(x,y) -> (x, y)
    /*
      (0,0) --------------------- max_width
                 clip.x
             -------------------
        |    | | | | | | | | | | |
        |    | |   clip.width  | |
      clip.y | | |_|_|_|_|_|_| h |
        |    | | | | | | | | | e |
        |    | | | | | | | | | i |
        |    | | | | | | | | | g |
        |    | | |_|_|_|_|_|_| h |
        |    | | | | | | | | | t |
        |    | | | | | | | | | | |
        |    -------------------
    max_height

    */

    let screen = v.screen.clone();
    let screen = screen.read().unwrap();

    let (x, y) = (x as usize, y as usize);

    dbg_println!("VID {} : CLICK @ x({}) Y({})", v.id, x, y);
    // move cursor to (x,y)

    /*
         TOD: retest this

        // 0 <= x < screen.width()
        if x < screen.clip_rect().x {
            x = 0;
        } else if x >= screen.clip_rect().x + screen.clip_rect().width {
            x = screen.clip_rect().width - 1;
        } else {
            x -= screen.clip_rect().x;
        }

        // 0 <= y < screen.height()
        if y < screen.clip_rect().y {
            y = 0;
        } else if y > screen.clip_rect().y + screen.clip_rect().height {
            y = screen.clip_rect().height - 1;
        } else {
            y -= screen.clip_rect().y;
        }

        //
        let _max_offset = screen.doc_max_offset;

        let last_li = screen.get_last_used_line_index();
        if y >= last_li {
            if last_li >= screen.height() {
                y = screen.height() - 1;
            } else {
                y = last_li;
            }
        }

        if let Some(l) = screen.get_line(y) {
            if l.nb_cells > 0 && x > l.nb_cells {
                x = l.nb_cells - 1;
            } else if l.nb_cells == 0 {
                x = 0;
            }
        } else {
        }
    */

    // check from right to left until some codepoint is found
    let tm = v.mode_ctx_mut::<TextModeContext>("text-mode");

    tm.select_point.clear();

    let mut i = x + 1;
    while i > 0 {
        if let Some(cpi) = screen.get_used_cpinfo(x, y) {
            // clear selection point
            // WARNING:

            // reset main mark
            tm.mark_index = 0;
            tm.marks.clear();
            tm.marks.push(Mark {
                offset: cpi.offset.unwrap(),
            });

            dbg_println!(
                "VID {} : CLICK @ x({}) Y({}) set main mark at offset : {:?}",
                v.id,
                x,
                y,
                cpi.offset
            );

            break;
        }

        i -= 1;
    }

    // s // to internal view.borrow_mut().state.s
}

pub fn button_release(_editor: &mut Editor, env: &mut EditorEnv, view: &Rc<RefCell<View>>) {
    let v = &mut view.borrow_mut();

    let (button, _x, _y) = match env.trigger[0] {
        InputEvent::ButtonRelease(ref button_event) => match button_event {
            ButtonEvent {
                mods:
                    KeyModifiers {
                        ctrl: _,
                        alt: _,
                        shift: _,
                    },
                x,
                y,
                button,
            } => (*button, *x, *y),
        },

        _ => {
            return;
        }
    };

    let tm = v.mode_ctx_mut::<TextModeContext>("text-mode");
    if (button as usize) < tm.button_state.len() {
        tm.button_state[button as usize] = 0;
    }
}

// TODO: add enter /leave clipped region detection
pub fn pointer_motion(_editor: &mut Editor, env: &mut EditorEnv, view: &Rc<RefCell<View>>) {
    let v = &mut view.borrow_mut();
    let screen = v.screen.clone();
    let screen = screen.read().unwrap();

    // TODO: match events
    match &env.trigger[0] {
        InputEvent::PointerMotion(PointerEvent { mods: _, x, y }) => {
            // TODO: change screen (x,y) to i32 ? and filter in functions ?

            let vid = v.id;
            dbg_println!("VID {} pointer motion x({}) y({})", vid, x, y);

            let x = std::cmp::max(0, *x) as usize;
            let y = std::cmp::max(0, *y) as usize;

            if let Some(cpi) = screen.get_cpinfo(x, y) {
                {
                    // update selection point
                    let tm = v.mode_ctx_mut::<TextModeContext>("text-mode");

                    // TODO: check focus
                    if let Some(offset) = cpi.offset {
                        if tm.button_state[0] == 1 {
                            tm.select_point.clear();
                            tm.select_point.push(Mark { offset });
                        }
                    }

                    dbg_println!(
                        "VID {} @{:?} : pointer motion x({}) y({}) | select offset({:?})",
                        vid,
                        Instant::now(),
                        x,
                        y,
                        cpi.offset
                    );
                }
            }
        }

        _ => {}
    }
}

pub fn select_next_view(editor: &mut Editor, env: &mut EditorEnv, _view: &Rc<RefCell<View>>) {
    env.view_id = std::cmp::min(env.view_id + 1, editor.view_map.len() - 1);
}

pub fn select_previous_view(_editor: &mut Editor, env: &mut EditorEnv, _view: &Rc<RefCell<View>>) {
    env.view_id = std::cmp::max(env.view_id - 1, 1);
}

// TODO: view.center_arrout_offset()
pub fn center_around_mark(editor: &mut Editor, env: &mut EditorEnv, view: &Rc<RefCell<View>>) {
    let mut v = view.borrow_mut();
    let tm = v.mode_ctx::<TextModeContext>("text-mode");
    let offset = tm.marks[tm.mark_index].offset;
    v.center_around_offset(editor, env, offset);
}

pub fn center_around_offset(editor: &mut Editor, env: &mut EditorEnv, view: &Rc<RefCell<View>>) {
    if let Some(center_offset) = env.center_offset {
        let mut v = view.borrow_mut();
        let offset = {
            let doc = v.document.as_ref().unwrap();
            let doc = doc.as_ref().read().unwrap();
            ::std::cmp::min(doc.size() as u64, center_offset)
        };

        v.center_around_offset(editor, env, offset); // TODO: enum { top center bottom } ? in text-mode
    }
}

pub fn display_end_of_line(_editor: &mut Editor, _env: &mut EditorEnv, view: &Rc<RefCell<View>>) {
    let mut v = view.borrow_mut();
    let tm = v.mode_ctx_mut::<TextModeContext>("text-mode");

    let c = if let Some(c) = tm.char_map.as_mut().unwrap().get(&'\n') {
        if *c == ' ' {
            '\u{2936}'
        } else {
            ' '
        }
    } else {
        ' '
    };

    dbg_println!("\\n -> {}", c);

    tm.char_map.as_mut().unwrap().insert('\n', c);
}
