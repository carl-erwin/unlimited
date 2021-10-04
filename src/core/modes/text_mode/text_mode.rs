/*
  TODO(ceg): split in function kind

  movements:
    up/down/forward/backward
    page-up/page-down
    move-to-start-of-doc
    move-to-end-of-doc
    move-to-start-of-line
    move-to-end-of-line
    move-to-next-token-start
    move-to-next-token-end
    move-to-previous-token-start
    move-to-previous-token-end
    move pointer to clicked-area

    goto-line
    goto-offset

  document modifications:
   insert/remove
   copy
   cut
   paste
   insert file (TODO)

   selection;
    pointer-selection
    selection with mark/point

   Fix all commands in multi-cursor context

    search/find

    instant search/find: search-as-you-type  case(in)sensitive
    form search/find:   fill-form-and-search
    reverse

    ////

    VIEW(1)    VIEW(2)

       TEXT-MODE(doc) -> doc.get_mode_shared_data("tex-mode")  -> dyn ?
           SHARED   should shared marks/sel between view / change cursor's shape when not focused

       TEXT-MODE(view) -> view.get_mode_private_data("tex-mode") -> dyn ?
           SHARED   should shared marks/sel between view / change cursor's shape when not focused

        we could do better: instead of storing data in doc
        store them in text-mode global struct/mutex etc..

        tm.get_doc_data(doc_id)  -> Option<>    destroy when doc is destroyed ? ...
        tm.get_view_data(view_id)  -> Option<>  destroy when view is destroyed ...

        in real world we have doc_ids + copy, no pointers

        struct TextModeDocumentData { ...
          marks
          selections
          buffer log here ?
          buffer index ?
        }

        struct TextModeViewData { ...
          filters
          data caches
          screen
        }

        type TextModeDocumentDataMap = HashMap<Document::Id, Arc<RwLock<TextModeDocumentData>> { ... }

        type TextModeViewDataMap     = HashMap<Document::Id, Arc<RwLock<TextModeViewData>> { ... }


*/

use parking_lot::RwLock;
use std::any::Any;
use std::rc::Rc;
use std::sync::Arc;

use std::collections::HashMap;
use std::time::Instant;

//
use crate::sort_pair;

use crate::core::editor::Editor;
use crate::core::editor::EditorEnv;

use crate::dbg_println;

use crate::core::screen::Screen;

use super::mark::Mark;

use crate::core::codec::text::utf8;
use crate::core::codec::text::SyncDirection; // TODO(ceg): remove
use crate::core::codec::text::TextCodec;

use crate::core::document::BufferOperation;
use crate::core::document::BufferOperationType;

use crate::core::event::ButtonEvent;
use crate::core::event::InputEvent;
use crate::core::event::Key;
use crate::core::event::KeyModifiers;
use crate::core::event::PointerEvent;

//
use crate::core::view::layout::run_compositing_stage_direct;
use crate::core::view::layout::LayoutPass;

use crate::core::editor;

use crate::core::editor::register_input_stage_action;
use crate::core::editor::InputStageActionMap;
use crate::core::editor::InputStageFunction;
use crate::core::view::View;

use crate::core::event::input_map::build_input_event_map;
use crate::core::event::input_map::DEFAULT_INPUT_MAP;

#[derive(Debug, Clone, Copy)]
pub enum Action {
    ScrollUp { n: usize },
    ScrollDown { n: usize },
    CenterAroundMainMark,
    CenterAroundMainMarkIfOffScreen,
    CenterAround { offset: u64 },
    MoveMarksToNextLine,
    MoveMarksToPreviousLine,
    MoveMarkToNextLine { idx: usize },
    MoveMarkToPreviousLine { idx: usize },
    ResetMarks,
    CheckMarks,
    DedupAndSaveMarks,
    SaveMarks,
    CancelSelection,
    UpdateReadCache,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum ActionType {
    MarksMove,
    ScreenMove,
    DocumentModification,
    Undo,
    Redo,
    DocumentSearch,
}

use super::super::Mode;

// Text mode content filters
use crate::core::modes::text_mode::CharMapFilter;
use crate::core::modes::text_mode::HighlightFilter;
use crate::core::modes::text_mode::HighlightSelectionFilter;
use crate::core::modes::text_mode::RawDataFilter;
use crate::core::modes::text_mode::ScreenFilter;
use crate::core::modes::text_mode::TabFilter;
use crate::core::modes::text_mode::TextCodecFilter;
use crate::core::modes::text_mode::UnicodeToTextFilter;
use crate::core::modes::text_mode::Utf8Filter;
use crate::core::modes::text_mode::WordWrapFilter;

// Text mode screen overlay filters
use crate::core::modes::text_mode::DrawMarks;

/// CopyData is used to implement the selection/cut/paste buffer
pub enum CopyData {
    BufferLogIndex(usize), // the data is in the document buffer log index see BufferLog
    Buffer(Vec<u8>),       // a standalone copy
}

pub struct TextModeContext {
    pub text_codec: Box<dyn TextCodec>,
    //
    pub center_on_mark_move: bool,
    pub scroll_on_mark_move: bool,

    pub prev_buffer_log_revision: usize, // use for tag save (in undo/redo context)
    pub prev_mark_revision: usize,       // use for tag save
    pub mark_revision: usize,            // use for tag save

    pub mark_index: usize, // move to text mode
    pub marks: Vec<Mark>,
    //
    pub select_point: Vec<Mark>,
    pub copy_buffer: Vec<CopyData>,
    pub button_state: [u32; 8],

    // TODO ? char_map_and_color HashMap<char, String, Option<(u8,u8,u8)>>,
    pub char_map: Option<HashMap<char, String>>,
    pub color_map: Option<HashMap<char, (u8, u8, u8)>>,
    pub display_word_wrap: bool,

    pub pre_compose_action: Vec<Action>,
    pub post_compose_action: Vec<Action>,

    pub prev_action: ActionType,
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

        char_map.insert('\u{0A}', " ".to_string()); //  '\n' (new line)
        char_map.insert('\u{7f}', "<DEL>".to_string());

        if true {
            // config toggle ?
            char_map.insert('\u{00}', "<NUL>".to_string()); // '\0' (null character)
            char_map.insert('\u{01}', "<SOH>".to_string()); // (start of heading)
            char_map.insert('\u{02}', "<STX>".to_string()); // (start of text)
            char_map.insert('\u{03}', "<ETX>".to_string()); // (end of text)
            char_map.insert('\u{04}', "<EOT>".to_string()); // (end of transmission)
            char_map.insert('\u{05}', "<ENQ>".to_string()); // (enquiry)
            char_map.insert('\u{06}', "<ACK>".to_string()); // (acknowledge)
            char_map.insert('\u{07}', "<BEL>".to_string()); // '\a' (bell)
            char_map.insert('\u{08}', "<BS>".to_string()); //  '\b' (backspace)
                                                           /* tab */
            char_map.insert('\u{09}', "<HT>".to_string()); //  '\t' (horizontal tab)
                                                           // do not do this if tab expansion is enabled
                                                           /* new line */
            // char_map.insert('\u{0A}', " ".to_string()); //  '\n' (new line)
            /* */
            char_map.insert('\u{0B}', "<VT>".to_string()); //  '\v' (vertical tab)
            char_map.insert('\u{0C}', "<FF>".to_string()); //  '\f' (form feed)
            char_map.insert('\u{0D}', "<CR>".to_string()); //  '\r' (carriage ret)
            char_map.insert('\u{0E}', "<SO>".to_string()); //  (shift out)
            char_map.insert('\u{0F}', "<SI>".to_string()); //  (shift in)
            char_map.insert('\u{10}', "<DLE>".to_string()); // (data link escape)
            char_map.insert('\u{11}', "<DC1>".to_string()); // (device control 1)
            char_map.insert('\u{12}', "<DC2>".to_string()); // (device control 2)
            char_map.insert('\u{13}', "<DC3>".to_string()); // (device control 3)
            char_map.insert('\u{14}', "<DC4>".to_string()); // (device control 4)
            char_map.insert('\u{15}', "<NAK>".to_string()); // (negative ack.)
            char_map.insert('\u{16}', "<SYN>".to_string()); // (synchronous idle)
            char_map.insert('\u{17}', "<ETB>".to_string()); // (end of trans. blk)
            char_map.insert('\u{18}', "<CAN>".to_string()); // (cancel)
            char_map.insert('\u{19}', "<EM>".to_string()); //  (end of medium)
            char_map.insert('\u{1A}', "<SUB>".to_string()); // (substitute)
            char_map.insert('\u{1B}', "<ESC>".to_string()); // (escape)
            char_map.insert('\u{1C}', "<FS>".to_string()); //  (file separator)
            char_map.insert('\u{1D}', "<GS>".to_string()); //  (group separator)
            char_map.insert('\u{1E}', "<RS>".to_string()); //  (record separator)
            char_map.insert('\u{1F}', "<US>".to_string()); //  (unit separator)
            char_map.insert('\u{7f}', "<DEL>".to_string());
        }

        let mut color_map = HashMap::new();
        for i in '\0'..' ' {
            color_map.insert(i as char, (0, 128, 0));
        }
        color_map.insert('\u{7f}', (0x00, 0xff, 0xff));
        color_map.insert('\r', (0x00, 0xaa, 0xff));

        let tab_color = (242, 71, 132);
        //  = if env.graphic_display {
        //    (242, 71, 132) // purple-like
        //} else {
        //    (128, 0, 128) // magenta
        //};
        color_map.insert('\t', tab_color);

        let ctx = TextModeContext {
            center_on_mark_move: false, // add movement enums and pass it to center fn
            scroll_on_mark_move: true,
            text_codec: Box::new(utf8::Utf8Codec::new()),
            //text_codec: Box::new(ascii::AsciiCodec::new()),
            prev_buffer_log_revision: 0,
            prev_mark_revision: 0,
            mark_revision: 0,
            marks,
            copy_buffer,
            mark_index: 0,
            select_point: vec![],
            button_state: [0; 8],
            char_map: Some(char_map),
            color_map: Some(color_map),
            display_word_wrap: false,
            pre_compose_action: vec![],
            post_compose_action: vec![],
            prev_action: ActionType::MarksMove,
        };

        Box::new(ctx)
    }

    fn configure_view(
        &mut self,
        editor: &mut Editor<'static>,
        _env: &mut EditorEnv<'static>,
        view: &mut View<'static>,
    ) {
        dbg_println!("config text-mode for VID {}", view.id);

        view.compose_priority = 256; // TODO: move to caller

        let doc_id = { view.document.as_ref().unwrap().clone().read().id };

        let doc = { editor.document_map.read().get(&doc_id).unwrap().clone() };

        let tm = view.mode_ctx_mut::<TextModeContext>("text-mode");

        // create first mark
        let marks_offsets: Vec<u64> = tm.marks.iter().map(|m| m.offset).collect();
        view.document
            .as_ref()
            .unwrap()
            .write()
            .tag(Instant::now(), 0, marks_offsets);

        // Config input map
        dbg_println!("DEFAULT_INPUT_MAP\n{}", DEFAULT_INPUT_MAP);
        // TODO(ceg): user define
        // let input_map = mode.build_input_map(); TODO
        {
            let input_map = build_input_event_map(DEFAULT_INPUT_MAP).unwrap();
            let mut input_map_stack = view.input_ctx.input_map.as_ref().borrow_mut();
            input_map_stack.push(input_map);
        }

        /*
        TODO(ceg): --set-key key1=val1,key2,..,key(n)
            no-highlight-keyword
            no-highlight-selection
            no-tab
            no-char-map
            no-marks
            no-utf8-filter
            no-word-wrap
        */

        //
        let use_utf8_codec = true;

        let use_highlight_keywords = true; // TODO(ceg): transform in overlay filter
        let use_highlight_selection = true; // mandatory
        let use_tabulation_exp = true;
        // TODO(ceg) filter '\r'
        let use_char_map = true; // mandatory very slow \r have side effects -> ' '
        let use_word_wrap = true;

        let use_draw_marks = true; // mandatory

        let skip_text_filters = if crate::core::raw_data_filter_to_screen() {
            true
        } else {
            // if use_raw_data_filter_to_screen
            false
        };

        // NB: Execution in push order

        // mandatory data reader
        view.compose_content_filters
            .borrow_mut()
            .push(Box::new(RawDataFilter::new()));

        if !skip_text_filters {
            if use_utf8_codec {
                //
                // DEBUG codec error
                view.compose_content_filters
                    .borrow_mut()
                    .push(Box::new(Utf8Filter::new()));
            } else {
                view.compose_content_filters
                    .borrow_mut()
                    .push(Box::new(TextCodecFilter::new()));
            }

            //
            view.compose_content_filters
                .borrow_mut()
                .push(Box::new(UnicodeToTextFilter::new()));

            //
            if use_highlight_keywords {
                //
                view.compose_content_filters
                    .borrow_mut()
                    .push(Box::new(HighlightFilter::new()));
            }

            if use_highlight_selection {
                //
                view.compose_content_filters
                    .borrow_mut()
                    .push(Box::new(HighlightSelectionFilter::new()));
                //
            }

            if use_tabulation_exp {
                view.compose_content_filters
                    .borrow_mut()
                    .push(Box::new(TabFilter::new()));
            }

            if use_char_map {
                // NB: Word Wrap after tab expansion
                view.compose_content_filters
                    .borrow_mut()
                    .push(Box::new(CharMapFilter::new()));
            }
            //

            if use_word_wrap {
                // NB: Word Wrap after tab expansion
                view.compose_content_filters
                    .borrow_mut()
                    .push(Box::new(WordWrapFilter::new()));
            }
        } // skip_text_filters ?
          //

        // Comment(s)

        // Folding

        // mandatory screen filler
        let mut screen_filter = ScreenFilter::new();
        screen_filter.display_eof = true;

        view.compose_content_filters
            .borrow_mut()
            .push(Box::new(screen_filter));

        if use_draw_marks {
            view.compose_screen_overlay_filters
                .borrow_mut()
                .push(Box::new(DrawMarks::new()));
        }

        // fix dedup marks, scrolling etc ...
        view.stage_actions
            .push((String::from("text-mode"), run_text_mode_actions));
    }
}

pub struct TextMode {
    // add common field
}

impl TextMode {
    pub fn new() -> Self {
        dbg_println!("TextMode");
        TextMode {}
    }

    pub fn register_input_stage_actions<'a>(mut map: &'a mut InputStageActionMap<'a>) {
        let v: Vec<(&str, InputStageFunction)> = vec![
            // tools
            ("text-mode:display-end-of-line", display_end_of_line),
            ("text-mode:display-word-wrap", display_word_wrap),
            // navigation
            // marks
            ("text-mode:move-marks-backward", move_marks_backward),
            ("text-mode:move-marks-forward", move_marks_forward),
            ("text-mode:move-marks-to-next-line", move_marks_to_next_line),
            (
                "text-mode:move-marks-to-previous-line",
                move_marks_to_previous_line,
            ),
            ("text-mode:move-to-token-start", move_to_token_start),
            ("text-mode:move-to-token-end", move_to_token_end),
            (
                "text-mode:move-marks-to-start-of-line",
                move_marks_to_start_of_line,
            ),
            (
                "text-mode:move-marks-to-end-of-line",
                move_marks_to_end_of_line,
            ),
            (
                "text-mode:move-marks-to-start-of-file",
                move_mark_to_start_of_file,
            ),
            (
                "text-mode:move-marks-to-end-of-file",
                move_mark_to_end_of_file,
            ),
            (
                "text-mode:clone-and-move-mark-to-previous-line",
                clone_and_move_mark_to_previous_line,
            ),
            (
                "text-mode:clone-and-move-mark-to-next-line",
                clone_and_move_mark_to_next_line,
            ),
            // selection
            (
                "text-mode:set-select-point-at-mark",
                set_selection_points_at_marks,
            ),
            ("text-mode:copy-selection", copy_selection),
            ("text-mode:cut-selection", cut_selection),
            // screen
            ("text-mode:page-up", scroll_to_previous_screen),
            ("text-mode:page-down", scroll_to_next_screen),
            ("text-mode:scroll-up", scroll_up),
            ("text-mode:scroll-down", scroll_down),
            //
            ("select-next-view", select_next_view),
            ("select-previous-view", select_previous_view),
            ("text-mode:center-around-mark", center_around_mark),
            ("text-mode:move-mark-to-clicked-area", button_press),
            // edition
            ("text-mode:self-insert", insert_codepoint_array),
            ("text-mode:remove-codepoint", remove_codepoint),
            (
                "text-mode:remove-previous-codepoint",
                remove_previous_codepoint,
            ),
            ("text-mode:paste", paste),
            ("text-mode:cut-to-end-of-line", cut_to_end_of_line),
            (
                "text-mode:remove-until-end-of-word",
                remove_until_end_of_word,
            ),
            // undo/redo
            ("text-mode:undo", undo),
            ("text-mode:redo", redo),
            // mouse handling
            ("text-mode:button-press", button_press),
            ("text-mode:button-release", button_release),
            ("text-mode:pointer-motion", pointer_motion),
            // TODO(ceg): usage not well defined
            ("editor:cancel", editor_cancel),
        ];

        for e in v {
            register_input_stage_action(&mut map, e.0, e.1);
        }
    }
}

pub fn run_text_mode_actions_vec(
    editor: &mut Editor<'static>,
    env: &mut EditorEnv<'static>,
    view: &Rc<RwLock<View<'static>>>,
    actions: &Vec<Action>,
) {
    let mut update_read_cache = true;

    for a in actions.iter() {
        match a {
            Action::ScrollUp { n } => {
                scroll_view_up(view, editor, env, *n);
                update_read_cache = true;
            }
            Action::ScrollDown { n } => {
                scroll_view_down(view, editor, env, *n);
                update_read_cache = true;
            }
            Action::CenterAroundMainMark => {
                center_around_mark(editor, env, &view);
            }
            Action::CenterAroundMainMarkIfOffScreen => {
                let center = {
                    let v = &mut view.write();

                    let tm = v.mode_ctx::<TextModeContext>("text-mode");
                    let mid = tm.mark_index;
                    let marks = &tm.marks;
                    if marks.len() > 0 {
                        let offset = marks[mid].offset;
                        let screen = v.screen.read();
                        !screen.contains_offset(offset)
                    } else {
                        false
                    }
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
            }
            Action::MoveMarkToPreviousLine { idx: _usize } => {}

            Action::ResetMarks => {
                let v = &mut view.write();
                let tm = v.mode_ctx_mut::<TextModeContext>("text-mode");
                let offset = tm.marks[tm.mark_index].offset;

                tm.mark_index = 0;
                tm.marks.clear();
                tm.marks.push(Mark { offset });
            }

            Action::CheckMarks => {
                let v = &mut view.write();
                let tm = v.mode_ctx_mut::<TextModeContext>("text-mode");
                tm.marks.dedup();
                tm.mark_index = tm.marks.len().saturating_sub(1);

                update_read_cache = true;
            }

            Action::UpdateReadCache => {
                update_read_cache = true;
            }

            Action::DedupAndSaveMarks => {
                let v = &mut view.write();
                let tm = v.mode_ctx_mut::<TextModeContext>("text-mode");

                //
                tm.marks.dedup();
                let marks_offsets: Vec<u64> = tm.marks.iter().map(|m| m.offset).collect();
                dbg_println!("MARKS {:?}", marks_offsets);

                //
                let doc = v.document().unwrap();
                let mut doc = doc.write();
                let max_offset = doc.size() as u64;
                doc.tag(env.current_time, max_offset, marks_offsets);

                dbg_println!("MARK DedupAndSaveMarks doc revision {}", doc.nr_changes());
            }

            Action::SaveMarks => {
                let v = &mut view.write();
                let tm = v.mode_ctx_mut::<TextModeContext>("text-mode");

                //
                let marks_offsets: Vec<u64> = tm.marks.iter().map(|m| m.offset).collect();
                dbg_println!("MARKS {:?}", marks_offsets);

                //
                let doc = v.document().unwrap();
                let mut doc = doc.write();
                let max_offset = doc.size() as u64;
                doc.tag(env.current_time, max_offset, marks_offsets);

                dbg_println!("MARK SaveMarks doc revision {}", doc.nr_changes());
            }

            Action::CancelSelection => {
                let v = &mut view.write();
                let tm = v.mode_ctx_mut::<TextModeContext>("text-mode");
                tm.select_point.clear();
            }
        }
    }

    // TODO(ceg): add offscreen support
    // marks ranges
    // screen scrolling * key freq
    // if main mark offscreen ignore cache ?
    if update_read_cache {
        let v = &mut view.write();
        let tm = v.mode_ctx_mut::<TextModeContext>("text-mode");
        tm.mark_index = tm.marks.len().saturating_sub(1);

        // TODO(ceg): Action::UpdateReadCache(s) vs multiple views
        // TODO(ceg): adjust with v.star_offset ..
        if tm.marks.len() > 0 {
            let mut min = tm.marks[0].offset;
            let mut max = tm.marks[tm.marks.len() - 1].offset;
            dbg_println!("min (mark) = {}", min);
            dbg_println!("max (mark) = {}", max);

            let doc = v.document().unwrap();
            let mut doc = doc.write();

            let (s, e) = doc.get_cache_range();

            // screen cache
            {
                let screen = v.screen.read();
                let w = screen.width();
                let h = screen.height();
                let nb_screens = 1;
                let codec_max_encode_size = 4;

                // use hints to adjust cache window
                let max_char = (w * h * nb_screens * codec_max_encode_size) as u64;
                // let max_char = 1024 * 1024 * 4;
                // as command line options to let the use change the settings on specific files

                let screen_start = screen.first_offset.unwrap_or(min);
                let screen_end = screen_start.saturating_add(max_char * 2);
                if screen_start > min {
                    min = screen_start;
                    dbg_println!("min (mark) = screen_start {}", min);
                }
                if screen_end < max {
                    max = screen_end;
                    dbg_println!("max (mark) = screen_end {}", max);
                }

                min = screen_start.saturating_sub(max_char);
                max = screen_end.saturating_add(max_char); // no eof checks

                // [min_mark, max_mark]
                // [screen start screen_end ]
                //min = std::cmp::min(min, screen_start);
                //max = std::cmp::max(max, screen_end);

                // cap size ... < 2m ?
                // TODO(ceg): add read cache for mark updates have multiple caches
            }

            if s <= min && e >= max {
                /* cache is up to date */
            } else {
                dbg_println!("UPDATE READ CACHE MIN={}, MAX={}", min, max);
                dbg_println!("UPDATE READ CACHE SIZE = {} bytes", (max - min));
                dbg_println!("UPDATE READ CACHE SIZE = {} Kib", (max - min) / 1024);
                dbg_println!(
                    "UPDATE READ CACHE SIZE = {} Mib",
                    (max - min) / (1024 * 1024)
                );
                doc.set_cache(min, max); // TODO(ceg): optimize read with discard cache + append

                let (s, e) = doc.get_cache_range();
                dbg_println!("UPDATE READ CACHE  MIN={}, MAX={}, diff={}", s, e, e - s);
            }
        }
    }
}

fn run_text_mode_actions(
    mut editor: &mut Editor<'static>,
    mut env: &mut EditorEnv<'static>,
    view: &Rc<RwLock<View<'static>>>,
    pos: editor::StagePosition,
    stage: editor::Stage,
) {
    dbg_println!("run_text_mode_actions stage {:?} pos {:?},", stage, pos);

    {
        let mut v = view.write();
        let tm = v.mode_ctx_mut::<TextModeContext>("text-mode");
        tm.pre_compose_action.push(Action::UpdateReadCache);
    }

    let actions: Vec<Action> = {
        match (stage, pos) {
            (editor::Stage::Input, editor::StagePosition::Pre) => {
                let mut v = view.write();
                let doc = v.document.clone();
                let doc = doc.unwrap();
                let doc = doc.read();

                let tm = v.mode_ctx_mut::<TextModeContext>("text-mode");

                // TODO(ceg): add selection in buffer log ?
                // ex: cut-line
                // undo must restore marks before cut
                tm.prev_buffer_log_revision = doc.buffer_log.data.len();

                // SAVE marks copy, slow fow now
                // add marks revision ?
                tm.prev_mark_revision = tm.mark_revision;
                //
                return;
            }

            (editor::Stage::Input, editor::StagePosition::Post) => {
                let mut v = view.write();
                let doc = v.document.clone();

                if let Some(doc) = doc {
                    let doc = doc.read();
                    let max_offset = doc.size() as u64;

                    // refresh view offset after user input
                    v.start_offset = std::cmp::min(v.start_offset, max_offset);

                    let mut save_marks = false;
                    // save marks if any change is detected
                    let tm = v.mode_ctx_mut::<TextModeContext>("text-mode");
                    if tm.prev_mark_revision != tm.mark_revision {
                        save_marks = true;
                    }

                    // save marks on document changes
                    if doc.buffer_log.pos > tm.prev_buffer_log_revision
                        && tm.prev_action == ActionType::MarksMove
                    {
                        // not undo/redo
                        save_marks = true;
                    }

                    if save_marks {
                        tm.pre_compose_action.push(Action::DedupAndSaveMarks);
                        tm.pre_compose_action.push(Action::CheckMarks);
                    }
                }

                return;
            }

            (editor::Stage::Compositing, editor::StagePosition::Pre) => {
                // clear
                let mut v = view.write();
                let tm = v.mode_ctx_mut::<TextModeContext>("text-mode");
                tm.pre_compose_action.drain(..).collect()
            }

            (editor::Stage::Compositing, editor::StagePosition::Post) => {
                // clear
                let mut v = view.write();
                let tm = v.mode_ctx_mut::<TextModeContext>("text-mode");
                tm.pre_compose_action.drain(..).collect()
            }

            _ => {
                // dbg_println!("NO action for {:?}::{:?}", pos, stage);
                return;
            }
        }
    };

    run_text_mode_actions_vec(&mut editor, &mut env, &view, &actions);

    // CEG: is this true after undo redo with multiple cursors ?
    // TODO(ceg): cut/paste
    if !true {
        let v = view.read();
        let doc = v.document().unwrap();
        let max_offset = doc.read().size() as u64;
        let tm = v.mode_ctx::<TextModeContext>("text-mode");
        let marks = &tm.marks;
        for m in marks.iter() {
            if m.offset > max_offset as u64 {
                //dbg_println!
                panic!(
                    "WARNING !!!!!! m.offset {} > max_offset {}, pos {:?}, stage {:?}",
                    m.offset, max_offset, pos, stage
                );
            }
        }
    }
}

///////////////////////////////////////////////////////////////////////////////////////////////////
//
// text mode functions

pub fn cancel_marks(_editor: &mut Editor, _env: &mut EditorEnv, view: &Rc<RwLock<View>>) {
    let v = &mut view.write();

    let tm = v.mode_ctx_mut::<TextModeContext>("text-mode");
    let offset = tm.marks[tm.mark_index].offset;

    tm.mark_index = 0;
    tm.marks.clear();
    tm.marks.push(Mark { offset });

    tm.pre_compose_action.push(Action::ResetMarks);
}

// text mode functions
pub fn cancel_selection(_editor: &mut Editor, _env: &mut EditorEnv, view: &Rc<RwLock<View>>) {
    let v = &mut view.write();
    let tm = v.mode_ctx_mut::<TextModeContext>("text-mode");

    tm.select_point.clear();
}

pub fn editor_cancel(editor: &mut Editor, env: &mut EditorEnv, view: &Rc<RwLock<View>>) {
    cancel_marks(editor, env, view);

    cancel_selection(editor, env, view);
}

pub fn scroll_up(_editor: &mut Editor, _env: &mut EditorEnv, view: &Rc<RwLock<View>>) {
    // TODO(ceg): 3 is from mode configuration
    // env["default-scroll-size"] -> int
    let v = &mut view.write();
    let tm = v.mode_ctx_mut::<TextModeContext>("text-mode");

    tm.pre_compose_action.push(Action::ScrollUp { n: 3 });
}

pub fn scroll_down(_editor: &mut Editor, _env: &mut EditorEnv, view: &Rc<RwLock<View>>) {
    // TODO(ceg): 3 is from mode configuration
    // env["default-scroll-size"] -> int
    let v = &mut view.write();
    let tm = v.mode_ctx_mut::<TextModeContext>("text-mode");

    tm.pre_compose_action.push(Action::ScrollDown { n: 3 });
}

// TODO(ceg): rename into handle_input_events
/// Insert an single element/array of unicode code points using hardcoded utf8 codec.<br/>
pub fn insert_codepoint_array(
    mut editor: &mut Editor<'static>,
    mut env: &mut EditorEnv<'static>,
    view: &Rc<RwLock<View<'static>>>,
) {
    // InputEvent -> Vec<char>
    let array = {
        let v = view.read();

        assert!(v.input_ctx.trigger.len() > 0);
        let idx = v.input_ctx.trigger.len() - 1;
        match &v.input_ctx.trigger[idx] {
            InputEvent::KeyPress {
                mods:
                    KeyModifiers {
                        ctrl: false,
                        alt: false,
                        shift: false,
                    },
                key: Key::UnicodeArray(ref v),
            } => v.clone(), // should move to Rc<> ?

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
                // unhandled event type
                return;
            }
        }
    };

    // doc read only ?
    {
        let v = view.read();
        let doc = v.document.as_ref().unwrap();
        let doc = doc.read();
        if doc.is_syncing {
            // TODO(ceg): send/display notification
            return;
        }
    }

    // check previous action:
    // if previous action was mark(s) move -> save current marks before modifying the buffer
    let save_marks = {
        let v = view.read();
        let tm = v.mode_ctx::<TextModeContext>("text-mode");

        tm.prev_action == ActionType::MarksMove
    };

    // TODO(ceg): find a way to remove this
    if save_marks {
        run_text_mode_actions_vec(
            &mut editor,
            &mut env,
            &view,
            &vec![Action::DedupAndSaveMarks],
        );
    }

    // delete selection before insert
    copy_maybe_remove_selection(editor, env, view, false, true);

    let center = {
        let mut v = view.write();
        let view_start = v.start_offset;
        let mut view_growth = 0;
        let mut offset: u64 = 0;
        {
            let mut doc = v.document.clone();
            let doc = doc.as_mut().unwrap();
            let mut doc = doc.write();

            let tm = v.mode_ctx_mut::<TextModeContext>("text-mode");
            tm.prev_action = ActionType::DocumentModification;

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

            let mut insert_ops = vec![];

            // build operations vector
            // while updating marks

            for m in tm.marks.iter_mut() {
                if m.offset < view_start {
                    view_growth += utf8.len() as u64;
                }

                m.offset += grow;
                doc.insert(m.offset, utf8.len(), &utf8);

                // track insert operation
                insert_ops.push(BufferOperation {
                    op_type: BufferOperationType::Insert,
                    data: Some(Arc::new(utf8.clone())),
                    offset: m.offset,
                });

                m.offset += utf8.len() as u64;

                offset = m.offset; // TODO(ceg): remove this merge

                grow += utf8.len() as u64;
            }

            // notify doc subscriberss of insert ops
            // cannot do this in doc.callback ?
            // and notify all users the current view should not touch the marks ?
            // struct DocumentId(u64)
            // struct DocumentClientId(u64)
            // view.doc_client_id = doc.add_client_cb(cb);
            // where cb = fn(DocumentId, DocumentClientId, [ops])
            // doc.notify_operations(view.doc_client_id, &insert_ops);
        }
        v.start_offset += view_growth;

        // mark off_screen ?
        let screen = v.screen.read();
        screen.contains_offset(offset) == false || array.len() > screen.width() * screen.height()
    };

    {
        let mut v = view.write();
        let tm = v.mode_ctx_mut::<TextModeContext>("text-mode");

        if center {
            tm.pre_compose_action.push(Action::CenterAroundMainMark);
        }

        tm.pre_compose_action.push(Action::CancelSelection);
    }
}

pub fn remove_previous_codepoint(
    mut editor: &mut Editor<'static>,
    mut env: &mut EditorEnv<'static>,
    view: &Rc<RwLock<View<'static>>>,
) {
    // check previous action: if previous action was a mark move -> tag new positions
    let save_marks = {
        let v = view.read();
        let tm = v.mode_ctx::<TextModeContext>("text-mode");

        tm.prev_action == ActionType::MarksMove
    };

    if save_marks {
        run_text_mode_actions_vec(
            &mut editor,
            &mut env,
            &view,
            &vec![Action::DedupAndSaveMarks],
        );
    }

    if copy_maybe_remove_selection(editor, env, view, false, true) > 0 {
        return;
    }

    let mut scroll_down = 0;
    let v = &mut view.write();
    let start_offset = v.start_offset;

    {
        let doc = v.document.clone();
        let doc = doc.clone().unwrap();
        let mut doc = doc.write();
        if doc.size() == 0 {
            return;
        }

        let tm = v.mode_ctx_mut::<TextModeContext>("text-mode");
        tm.prev_action = ActionType::DocumentModification;

        let codec = tm.text_codec.as_ref();

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
    }

    // schedule render actions
    {
        let tm = v.mode_ctx_mut::<TextModeContext>("text-mode");

        if scroll_down > 0 {
            tm.pre_compose_action.push(Action::ScrollUp { n: 1 });
        }
        tm.pre_compose_action.push(Action::CheckMarks);
    }
}

/// Undo the previous write operation and sync the screen around the main mark.<br/>
pub fn undo(
    mut editor: &mut Editor<'static>,
    mut env: &mut EditorEnv<'static>,
    view: &Rc<RwLock<View<'static>>>,
) {
    // check previous action:
    // if previous action was mark(s) move -> save current marks before modifying the buffer
    let save_marks = {
        let v = view.read();
        let tm = v.mode_ctx::<TextModeContext>("text-mode");

        tm.prev_action == ActionType::DocumentModification
    };

    // TODO(ceg): fin a way to remove this
    if save_marks {
        run_text_mode_actions_vec(
            &mut editor,
            &mut env,
            &view,
            &vec![Action::DedupAndSaveMarks],
        );
    }

    let v = &mut view.write();

    let mut doc = v.document.clone();
    let doc = doc.as_mut().unwrap();
    let mut doc = doc.write();

    let tm = v.mode_ctx_mut::<TextModeContext>("text-mode");
    let marks = &mut tm.marks;

    doc.undo_until_tag();

    if let Some(marks_offsets) = doc.get_tag_offsets() {
        dbg_println!("restore marks {:?}", marks_offsets);
        marks.clear();
        for offset in marks_offsets {
            marks.push(Mark { offset });
        }
    } else {
        dbg_println!("TAG not found");
    }

    tm.mark_index = 0; // ??

    tm.pre_compose_action
        .push(Action::CenterAroundMainMarkIfOffScreen);

    tm.pre_compose_action.push(Action::CancelSelection);

    tm.prev_action = ActionType::Undo;
}

/// Redo the previous write operation and sync the screen around the main mark.<br/>
pub fn redo(_editor: &mut Editor, _env: &mut EditorEnv, view: &Rc<RwLock<View>>) {
    let v = &mut view.write();

    let mut doc = v.document.clone();
    let doc = doc.as_mut().unwrap();
    let mut doc = doc.write();

    let tm = v.mode_ctx_mut::<TextModeContext>("text-mode");
    let marks = &mut tm.marks;

    tm.mark_index = 0;

    dbg_println!(
        "REDO: marks before redo {:?}, log pos {}",
        marks,
        doc.buffer_log_pos()
    );

    doc.redo_until_tag();

    dbg_println!(
        "REDO: marks after  redo {:?}, log pos {}",
        marks,
        doc.buffer_log_pos()
    );

    if let Some(marks_offsets) = doc.get_tag_offsets() {
        dbg_println!("restore marks {:?}", marks_offsets);
        dbg_println!("doc max size {:?}", doc.size());
        marks.clear();
        for offset in marks_offsets {
            marks.push(Mark { offset });
        }
    } else {
        dbg_println!("REDO: no marks ? doc size {:?}", doc.size());
    }

    tm.pre_compose_action
        .push(Action::CenterAroundMainMarkIfOffScreen);
    tm.pre_compose_action.push(Action::CancelSelection);

    tm.prev_action = ActionType::Redo;
    /*
    TODO(ceg): add this function pointer attr
    if ActionType::Modification -> save marks before exec, etc ...
    */
}

/// Remove the current utf8 encoded code point.<br/>
pub fn remove_codepoint(
    editor: &mut Editor<'static>,
    env: &mut EditorEnv<'static>,
    view: &Rc<RwLock<View<'static>>>,
) {
    if copy_maybe_remove_selection(editor, env, view, false, true) > 0 {
        return;
    }

    let v = &mut view.write();
    let view_start = v.start_offset;
    let mut view_shrink: u64 = 0;

    {
        let mut doc = v.document.clone();
        let doc = doc.as_mut().unwrap();
        let mut doc = doc.write();

        let tm = v.mode_ctx_mut::<TextModeContext>("text-mode");

        let codec = tm.text_codec.as_ref();

        if doc.size() == 0 {
            return;
        }

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

    let tm = v.mode_ctx_mut::<TextModeContext>("text-mode");
    tm.pre_compose_action.push(Action::CheckMarks);
    tm.pre_compose_action.push(Action::CancelSelection);
}

/// Skip blanks (if any) and remove until end of the word.
/// TODO(ceg): handle ',' | ';' | '(' | ')' | '{' | '}'
pub fn remove_until_end_of_word(
    _editor: &mut Editor,
    _env: &mut EditorEnv,

    view: &Rc<RwLock<View>>,
) {
    let v = &mut view.write();

    let mut doc = v.document.clone();
    let doc = doc.as_mut().unwrap();
    let mut doc = doc.write();

    let tm = v.mode_ctx_mut::<TextModeContext>("text-mode");

    let codec = tm.text_codec.as_ref();

    let size = doc.size() as u64;

    if size == 0 {
        return;
    }

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
                ' ' | '\t' | /*'\r' |*/ '\n' => {
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

    tm.pre_compose_action.push(Action::CheckMarks);
    tm.pre_compose_action.push(Action::CancelSelection); //TODO register last optype
                                                         // if doc changes cancel selection ?
}

// TODO(ceg): maintain main mark Option<(x,y)>
pub fn move_marks_backward(_editor: &mut Editor, _env: &mut EditorEnv, view: &Rc<RwLock<View>>) {
    let v = &mut view.write();

    let start_offset = v.start_offset;

    let doc = v.document().unwrap();
    let mut doc = doc.write();

    let tm = v.mode_ctx_mut::<TextModeContext>("text-mode");

    let codec = tm.text_codec.as_ref();

    let midx = tm.mark_index;

    // update read cache
    let nr_marks = tm.marks.len();
    if !nr_marks == 0 {
        return;
    }
    let min = tm.marks[0].offset;
    let max = tm.marks[nr_marks - 1].offset;

    doc.set_cache(min, max);

    let mut scroll_down = 0;
    for (idx, m) in tm.marks.iter_mut().enumerate() {
        if idx == midx && m.offset <= start_offset {
            scroll_down = 1;
        }

        m.move_backward(&doc, codec);
    }

    if scroll_down > 0 {
        tm.pre_compose_action.push(Action::ScrollUp { n: 1 });
    }

    tm.pre_compose_action.push(Action::CheckMarks);

    let tm = v.mode_ctx_mut::<TextModeContext>("text-mode");

    if tm.center_on_mark_move {
        tm.pre_compose_action.push(Action::CenterAroundMainMark);
    }

    tm.prev_action = ActionType::MarksMove;
}

pub fn move_marks_forward(_editor: &mut Editor, _env: &mut EditorEnv, view: &Rc<RwLock<View>>) {
    let mut scroll_down = 0;

    let v = &mut view.write();

    let screen_has_eof = v.screen.read().has_eof();
    let end_offset = v.end_offset;

    //
    let doc = v.document().unwrap();
    let mut doc = doc.write();

    dbg_println!("doc.size() {}", doc.size());

    let tm = v.mode_ctx_mut::<TextModeContext>("text-mode");
    let nr_marks = tm.marks.len();
    if nr_marks == 0 {
        return;
    }

    // set read cache between first and last mark (TODO: remove this, move cache updates to doc.read() + with size)
    let midx = tm.mark_index;
    let min = tm.marks[0].offset;
    let max = tm.marks[nr_marks - 1].offset;
    doc.set_cache(min, max);

    //
    let prev_main_mark = tm.marks[midx];

    /* TODO(ceg): check error */
    let codec = tm.text_codec.as_ref();
    for m in tm.marks.iter_mut() {
        m.move_forward(&doc, codec);
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

    // TODO(ceg):  tm.pre_compose_action.push(Action::SelectLastMark);
    let nr_marks = tm.marks.len();
    tm.mark_index = nr_marks.saturating_sub(1); // TODO(ceg): dedup ?

    //      move this check at post render to reschedule render ?
    //      if v.center_on_mark_move {
    //           tm.pre_compose_action.push(Action::CenterAroundMainMark);
    //      }

    if scroll_down > 0 {
        dbg_println!("schedule scroll down n = {}", scroll_down);

        tm.pre_compose_action
            .push(Action::ScrollDown { n: scroll_down });
    }

    tm.pre_compose_action.push(Action::CheckMarks);

    tm.prev_action = ActionType::MarksMove;
}

pub fn move_marks_to_start_of_line(
    _editor: &mut Editor,
    _env: &mut EditorEnv,

    view: &Rc<RwLock<View>>,
) {
    let v = &mut view.write();
    let screen = v.screen.clone();
    let screen = screen.read();

    let doc = v.document().unwrap();
    let doc = doc.read();

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
        tm.pre_compose_action.push(Action::CenterAroundMainMark);
    }
    tm.pre_compose_action.push(Action::CheckMarks);
    tm.prev_action = ActionType::MarksMove;
}

pub fn move_marks_to_end_of_line(
    _editor: &mut Editor,
    _env: &mut EditorEnv,

    view: &Rc<RwLock<View>>,
) {
    let mut v = view.write();
    let screen = v.screen.clone();
    let screen = screen.read();

    let doc = v.document().unwrap();
    let doc = doc.read();

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
        tm.pre_compose_action.push(Action::CenterAroundMainMark);
    }

    tm.pre_compose_action.push(Action::CheckMarks);
    tm.prev_action = ActionType::MarksMove;
}

/*
   - (TODO) if the view's lines cache is available check it. (TODO),
      the cache must be sync with the screen.width

   - We rewind W x H x 4 bytes (4 is max codec encode size in utf8/utf16/utf32)
   prev_start = m_offset - (W * H * 4)

   - scroll until m_offset is found

*/
fn move_offset_to_previous_line_index(
    mut editor: &mut Editor<'static>,
    mut env: &mut EditorEnv<'static>,
    view: &Rc<RwLock<View<'static>>>,
    offset: u64,
    line_index: usize,
) -> u64 {
    let t0 = Instant::now();

    // NOTE(ceg): this pattern begs for sub function: get_start_offset and dimension
    let (start_offset, width, height) = {
        let v = view.read();
        let screen = v.screen.clone();
        let screen = screen.read();
        let (width, height) = screen.dimension();
        let rewind = (width * height * 4) as u64;
        let start_offset = offset.saturating_sub(rewind);
        (start_offset, width, height)
    };

    // TODO(ceg): codec.sync_forward(off) -> off (start_offset)
    // if we are in the middle of a utf8 sequence we move max 4 bytes until a starting point is reached
    // for idx in 0..4 { if codec.is_sync(new_start) { break; } start_offset += codec.encode_min_size() }

    if offset - start_offset <= width as u64 {
        // document first start
        return 0;
    }

    dbg_println!(
        "MOVE OFFSET({}) W ({}) H  ({}) START ---------",
        offset,
        width,
        height
    );

    let _tmp = Mark::new(start_offset);

    dbg_println!(
        "   get lines [{} <--> {}], {} bytes",
        start_offset,
        offset,
        offset - start_offset
    );

    dbg_println!("line_index {}", line_index);

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

    dbg_println!("   get line index --------- lines.len() {}", lines.len());
    //    for (i, e) in lines.iter().enumerate() {
    //        dbg_println!("      line[{}] = {:?}", i, e);
    //    }

    // find "offset" line index
    let index = if line_index < lines.len() - 1 {
        lines.len() - (line_index + 1)
    } else {
        0
    };

    dbg_println!("found offset {} at index {}", offset, index);

    let line_start_off = lines[index].0;

    let t1 = Instant::now();
    let diff = (t1 - t0).as_millis();
    dbg_println!("MOVE OFFSET END --------- time {}", diff);

    return line_start_off; // return  (lines, index, start,end) ?
}

fn move_on_screen_mark_to_previous_line(
    _editor: &Editor,
    _env: &EditorEnv,
    v: &View,
    midx: usize,
    marks: &mut Vec<Mark>,
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
                    if cpi.metadata == false {
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

fn move_mark_to_previous_line(
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
        "MARK next position is offscreen ---------------- current m_offset = {}",
        m_offset
    );
    {
        let (start_offset, end_offset, width, height) = {
            let v = view.read();
            let screen = v.screen.clone();
            let screen = screen.read();
            let (width, height) = screen.dimension();

            let doc = v.document.clone();
            let doc = doc.unwrap();
            let doc = doc.read();
            let _doc_size = doc.size() as u64;

            // rewind at least "1 full" screen
            let max_encode_size = 4;
            let rewind = (width * height * max_encode_size) as u64;
            let start_offset = m_offset.saturating_sub(rewind);
            if start_offset == m_offset {
                return;
            }
            let end_offset = screen.last_offset.unwrap();

            (start_offset, end_offset, width, height)
        };

        //let end_offset = m_offset + width as u64 * 4;
        //let end_offset = std::cmp::min(end_offset, doc_size);
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
                lines.len() - 1
            }
            Some(0) => {
                dbg_println!("no previous line");
                return;
            }
            Some(i) => {
                dbg_println!("m_offset {} FOUND @ index {:?}", m_offset, i);
                i - 1
            }
        };

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
                // tm.pre_compose_action.push(Action::UpdateViewOnMainMarkMove { moveType: ToPreviousLine, before: prev_offset, after: new_offset });
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
        tm.pre_compose_action.push(Action::UpdateReadCache);
        tm.pre_compose_action.push(Action::CheckMarks);
        // save last op
        tm.prev_action = ActionType::MarksMove;
    }
}

pub fn move_on_screen_mark_to_next_line(
    m: &mut Mark,
    screen: &Screen,
) -> (bool, Option<(u64, u64)>, Option<Action>) {
    // TODO(ceg): add hints: check in screen range
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
    dbg_println!("new_y  {}", new_y);
    let l = screen.get_used_line(new_y);
    if l.is_none() {
        // new_y does not exist, return
        return (true, Some((m.offset, m.offset)), None);
    }
    let l = l.unwrap();

    dbg_println!("l.len  {}", l.len());

    if l.len() == 0 {
        // line is empty do nothing
        dbg_println!(" NEXT line is EMPTY do nothing ..........");
        return (true, Some((m.offset, m.offset)), None);
    }

    // l.len() > 0
    // get last line char
    let new_x = ::std::cmp::min(x, l.len() - 1);
    dbg_println!("new_x  {}", new_x);
    let cpi = screen.get_cpinfo(new_x, new_y);
    if cpi.is_none() {
        dbg_println!("HUMMMMM");
        return (false, None, None);
    }
    let cpi = cpi.unwrap();
    if cpi.offset.is_none() {
        dbg_println!("HUMMMMM");
        return (false, None, None);
    }

    let old_offset = m.offset;

    m.offset = cpi.offset.unwrap();

    dbg_println!("update mark : offset => {} -> {}", old_offset, m.offset);

    /*
     TODO(ceg):
     Our current data model does not support virtual characters.
     ie: if a filter fills the screen with meta info (not document's real data)
     The offset mechanism is broken
      ex: if a filter expands a cp on multiple lines

     To fix this the "injected" metadata span must be stored elsewhere.
     (internal, doc_id, offset, size)
     and use a portal like mechanism

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
        return (false, None, Some(Action::ScrollDown { n: 1 }));
    }

    // ok
    (true, Some((old_offset, m.offset)), None)
}

// remove multiple borrows
pub fn move_mark_to_next_line(
    editor: &mut Editor<'static>,
    env: &mut EditorEnv<'static>,
    view: &Rc<RwLock<View<'static>>>,
    mark_idx: usize,
) -> Option<(u64, u64)> {
    // TODO(ceg): m.on_buffer_end() ?

    let max_offset = {
        let v = view.read();
        v.document().unwrap().read().size() as u64
    };

    // off_screen ?
    let mut m_offset;
    let old_offset;

    {
        let (screen, mut m) = {
            let v = view.read();
            let screen = v.screen.clone();

            let tm = v.mode_ctx::<TextModeContext>("text-mode");
            let m = tm.marks[mark_idx].clone();
            (screen, m)
        };

        m_offset = m.offset;
        old_offset = m.offset;

        if m.offset == max_offset {
            return None;
        }

        dbg_println!("TRYING TO MOVE TO NEXT LINE MARK offset {}", m.offset);

        let screen = screen.read();
        let (ok, offsets, action) = move_on_screen_mark_to_next_line(&mut m, &screen);
        {
            let mut v = view.write();
            let tm = v.mode_ctx_mut::<TextModeContext>("text-mode");

            tm.marks[mark_idx] = m;
            if let Some(action) = action {
                // Add stage RenderStage :: PreRender PostRender
                // will be removed when the "scroll" update is implemented
                // ADD screen cache ?
                // screen[first mark -> last mark ] ? Ram usage ?
                // updated on resize -> slow
                tm.pre_compose_action.push(action);
            }
        }

        if ok == true {
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
            let doc = v.document().unwrap();
            let doc = doc.read();

            let tm = v.mode_ctx::<TextModeContext>("text-mode");
            let codec = tm.text_codec.as_ref();

            let m = &tm.marks[mark_idx];
            let mut tmp = Mark::new(m.offset);
            tmp.move_to_start_of_line(&doc, codec);
            tmp.offset
        };

        dbg_println!("MARK IS OFFSCREEN");

        // a codepoint can use 4 bytes the virtual end is
        // + 1 full line away
        let end_offset = ::std::cmp::min(m_offset + (4 * screen_width) as u64, max_offset);

        // get lines start, end offset
        // NB: run full layout code for one screen line ( folding etc ... )

        // TODO(ceg): return Vec<Box<screen>> ? update contenet
        // TODO(ceg): add perf view screen cache ? sorted by screens.start_offset
        // with same width/heigh as v.screen
        let lines = {
            get_lines_offsets_direct(
                view,
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
        let index = match lines.iter().position(|e| e.0 <= m_offset && m_offset < e.1) {
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
            let v = &view.read();
            let doc = v.document().unwrap();
            let doc = doc.read();

            let tm = v.mode_ctx::<TextModeContext>("text-mode");

            let codec = tm.text_codec.as_ref();

            // TODO(ceg): use codec.read(doc, n=width) until e.offset is reached
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

        let v = &view.read();
        let doc = v.document().unwrap();
        let doc = doc.read();

        let tm = v.mode_ctx::<TextModeContext>("text-mode");

        let codec = tm.text_codec.as_ref();

        // TODO(ceg): codec.skip_n(doc, 0..new_x)
        for _ in 0..new_x {
            tmp_mark.move_forward(&doc, codec); // TODO(ceg): pass n as arg
        }

        tmp_mark.offset = std::cmp::min(tmp_mark.offset, line_end_off);
        m_offset = tmp_mark.offset;
    }

    {
        let mut v = view.write();
        let tm = v.mode_ctx_mut::<TextModeContext>("text-mode");
        tm.marks[mark_idx].offset = m_offset;
    }

    Some((old_offset, m_offset))
}

///////////////////////////////////////////////////////////////////////////////////////////////////

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
    let max_offset = marks[idx_max - 1].offset;

    dbg_println!("max_offset {} - min_offset {}", max_offset, min_offset);

    (min_offset, idx_max)
}

fn sync_mark(view: &Rc<RwLock<View>>, m: &mut Mark) -> u64 {
    let v = view.read();
    let doc = v.document.clone();
    let doc = doc.unwrap();
    let doc = doc.read();

    // ctx
    let tm = v.mode_ctx::<TextModeContext>("text-mode");

    let _codec = tm.text_codec.as_ref();

    // get "real" line start
    // m.move_to_start_of_line(&doc, codec);

    let doc_size = doc.size() as u64;
    if doc_size > 0 {
        assert!(m.offset <= doc_size); // == EOF
    }

    doc_size
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
    // fast mode: 1 mark, on screen
    // check main mark, TODO(cef) proper function ?
    loop {
        let screen = {
            let v = view.read();
            v.screen.clone()
        };
        let mut screen = screen.as_ref().write();

        let mut mark = {
            let mut v = view.write();
            let tm = v.mode_ctx_mut::<TextModeContext>("text-mode");

            // 1 mark ?
            if tm.marks.len() != 1 {
                break;
            }

            tm.marks[0].clone()
        };

        if !screen.contains_offset(mark.offset) {
            dbg_println!("main mark is offscreen");
            break;
        }

        dbg_println!("main mark is onscreen");

        dbg_println!("screen.dimension = {:?}", screen.dimension());

        let last_line = screen.get_last_used_line();
        if last_line.is_none() {
            dbg_println!("no last line");
            panic!();
        }
        let last_line = last_line.unwrap();
        if last_line.len() == 0 {
            panic!(""); // empty line
        }

        // Go to next screen ?
        let cpi = &last_line[0].cpi; // will panic if invariant
        let last_line_first_offset = cpi.offset.unwrap(); // update next screen start offset
        dbg_println!("last_line_first_offset {}", last_line_first_offset);

        let has_eof = screen.has_eof();
        dbg_println!("has_eof = {}", has_eof);

        if mark.offset >= last_line_first_offset && !has_eof {
            // must scroll 1 line
            // NB: update the view's screen in place
            // by running "correct" compose passes

            //  use the first offset of the last line
            //  as the starting offset of the next screen
            dbg_println!("get line [1]");
            let line = screen.get_line(1).unwrap();
            let cpi = &line[0].cpi;
            let new_start = cpi.offset.unwrap();

            dbg_println!("line[1][0].cpi  = {:?}", cpi);

            dbg_println!("new_start = {}", new_start);

            let max_offset = screen.doc_max_offset;

            // build screen content
            screen.clear();
            run_compositing_stage_direct(
                editor,
                env,
                view,
                new_start,
                max_offset,
                &mut screen,
                LayoutPass::Content,
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
            if ret.0 == false {
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

            return;
        }
        // stop
        break;
    }

    dbg_println!("allocate tmp screen");

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
        let mut v = view.write();
        let tm = v.mode_ctx_mut::<TextModeContext>("text-mode");
        tm.marks.clone()
    };

    // TODO(ceg): add eof in conditions
    // find a way to transform while loops into iterator over screens
    // document_walk ? ...
    // ctx

    // update all marks
    {
        let mut idx_index = 0;
        while idx_index < idx_max {
            dbg_println!(" idx_index {} < idx_max {}", idx_index, idx_max);

            dbg_println!(
                "looking for marks[idx_index].offset = {}",
                marks[idx_index].offset
            );

            dbg_println!("compute layout from offset {}", m.offset);

            // update screen with configured filters
            screen.clear();

            run_compositing_stage_direct(
                editor,
                env,
                &view,
                m.offset,
                max_offset,
                &mut screen,
                LayoutPass::ContentAndScreenOverlay,
            );

            dbg_println!("\n\n\n---------");

            dbg_println!("screen first offset {:?}", screen.first_offset);
            dbg_println!("screen last offset {:?}", screen.last_offset);
            dbg_println!("screen current_line_index {:?}", screen.current_line_index);

            dbg_println!("max_offset {}", max_offset);

            if screen.push_count() == 0 {
                // screen is empty
                dbg_println!("screen.push_count() == 0");
                return;
            }
            assert_ne!(0, screen.push_count()); // at least EOF
            dbg_println!("screen.push_count() == {}", screen.push_count());
            // TODO(ceg): pass doc &doc to avoid double borrow
            // env.doc ?
            // env.view ? to avoid too many args

            //
            dbg_println!(
                "get_last_used_line_index = {:?}",
                screen.get_last_used_line_index()
            );
            let last_line = screen.get_last_used_line();
            if last_line.is_none() {
                dbg_println!("no last line");
                panic!();
            }
            let last_line = last_line.unwrap();

            if last_line.len() == 0 {
                panic!(""); // empty line
            }

            // go to next screen
            // using the first offset of the last line
            let cpi = &last_line[0].cpi;
            let last_line_first_offset = cpi.offset.unwrap(); // update next screen start offset
            dbg_println!("last_line_first_offset {}", last_line_first_offset);

            // idx_index not on screen  ? ...
            if !screen.contains_offset(marks[idx_index].offset) {
                dbg_println!(
                    "offset {} not found on screen go to next screen",
                    marks[idx_index].offset
                );

                if screen.has_eof() {
                    // EOF reached : stop
                    break;
                }

                // Go to next screen
                // use first offset of "current" screen's last line
                // as next screen start points
                let cpi = &last_line[0].cpi;
                m.offset = cpi.offset.unwrap(); // update next screen start offset
                continue;
            }

            dbg_println!("offset {} found on screen ", marks[idx_index].offset);

            // idx_index is on screen
            let mut idx_end = idx_index + 1;
            let next_screen_start_cpi = &last_line[0].cpi;
            while idx_end < idx_max {
                dbg_println!(
                    "check marks[{}].offset({}) >= next_screen_start_cpi({})",
                    idx_end,
                    marks[idx_end].offset,
                    next_screen_start_cpi.offset.unwrap()
                );

                if marks[idx_end].offset >= next_screen_start_cpi.offset.unwrap() {
                    break;
                }
                idx_end += 1;
            }

            dbg_println!("update marks[{}..{} / {}]", idx_index, idx_end, idx_max);

            for i in idx_index..idx_end {
                dbg_println!("update marks[{} / {}]", i, idx_max);

                // TODO(ceg): that use/match the returned action
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

            idx_index = idx_end; // next mark index

            let old_offset = m.offset;
            m.offset = next_screen_start_cpi.offset.unwrap(); // update next screen start
            dbg_println!("update marks {} -> {}]", old_offset, m.offset);
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

        dbg_println!("checking main mark index {}", idx);

        if !screen.contains_offset(tm.marks[idx].offset) {
            dbg_println!(
                "tm.marks[idx].offset {} NOT FOUND scroll down n=1",
                tm.marks[idx].offset
            );

            tm.pre_compose_action.push(Action::ScrollDown { n: 1 });
            // TODO ?  tm.pre_compose_action.push(Action::ScrollDownIfOffsetNotOnScreen { n: 1, offset: tm.marks[idx].offset });
            // TODO ?  tm.pre_compose_action.push(Action::ScrollDownIfMainMarkOffScreen { n: 1, offset: tm.marks[idx].offset });
        } else {
            dbg_println!(
                "tm.marks[idx].offset {} FOUND on screen",
                tm.marks[idx].offset
            );
        };
        tm.pre_compose_action.push(Action::CheckMarks);

        // save last op
        tm.prev_action = ActionType::MarksMove;
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
            tm.pre_compose_action.push(Action::ScrollUp { n: 1 });
        } else if !is_on_screen {
            tm.pre_compose_action.push(Action::CenterAroundMainMark);
        }

        // marks change
        tm.mark_revision += 1;
    }
}

pub fn clone_and_move_mark_to_next_line(
    editor: &mut Editor<'static>,
    env: &mut EditorEnv<'static>,

    view: &Rc<RwLock<View<'static>>>,
) {
    // refresh mark index
    let mark_len = {
        let mut v = view.write();
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
        mark_len
    };

    // NB: borrows: will use rendering pipeline to compute the marks_offset
    let offsets = move_mark_to_next_line(editor, env, view, mark_len - 1); // TODO return offset (old, new)
    if offsets.is_none() {
        dbg_println!(" cannot move mark to next line");

        run_text_mode_actions_vec(editor, env, &view, &vec![Action::CheckMarks]);
        return;
    }

    let offsets = offsets.unwrap();

    dbg_println!(" clone move down: offsets {:?}", offsets);

    let mut v = view.write();

    // no move ?
    if offsets.0 == offsets.1 {
        let was_on_screen = {
            let screen = v.screen.clone();
            let screen = screen.read();
            screen.contains_offset(offsets.0)
        };

        // destroy duplicated mark
        let tm = v.mode_ctx_mut::<TextModeContext>("text-mode");
        tm.mark_index = {
            let marks = &mut tm.marks;
            marks.pop();
            marks.len() - 1
        };

        if true || !was_on_screen {
            tm.pre_compose_action.push(Action::CenterAroundMainMark);
        }
        return;
    }

    dbg_println!(" clone move down: new_offset {}", offsets.1);
    // env.sort mark sync direction
    // update view.mark_index

    let (was_on_screen, is_on_screen) = {
        let screen = v.screen.read();
        let was_on_screen = screen.contains_offset(offsets.0);
        let is_on_screen = screen.contains_offset(offsets.1);
        dbg_println!(
            " was_on_screen {} , is_on_screen  {}",
            was_on_screen,
            is_on_screen
        );

        (was_on_screen, is_on_screen)
    };

    {
        let tm = v.mode_ctx_mut::<TextModeContext>("text-mode");

        if was_on_screen && !is_on_screen {
            tm.pre_compose_action.push(Action::ScrollDown { n: 1 });
        } else if !is_on_screen {
            tm.pre_compose_action.push(Action::CenterAroundMainMark);
        }

        // marks change
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

pub fn scroll_to_previous_screen(
    editor: &mut Editor<'static>,
    env: &mut EditorEnv<'static>,
    view: &Rc<RwLock<View<'static>>>,
) {
    {
        let nb = {
            let v = view.read();
            let screen = v.screen.read();
            let height = screen.height();
            ::std::cmp::max(height - 1, 1)
        };
        scroll_view_up(view, editor, env, nb);
    }

    // TODO(ceg): add hints to trigger mark moves
    move_mark_to_screen_end(editor, env, &view);
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

pub fn move_mark_to_end_of_file(
    _editor: &mut Editor,
    _env: &mut EditorEnv,

    view: &Rc<RwLock<View>>,
) {
    let mut v = view.write();

    let offset = {
        let doc = v.document().unwrap();
        let doc = doc.read();
        doc.size() as u64
    };
    v.start_offset = offset;

    let n = v.screen.read().height() / 2;

    let tm = v.mode_ctx_mut::<TextModeContext>("text-mode");
    tm.mark_index = 0;

    let marks = &mut tm.marks;
    marks.clear();
    marks.push(Mark { offset });

    tm.pre_compose_action.push(Action::ScrollUp { n });
}

pub fn scroll_to_next_screen(_editor: &mut Editor, _env: &mut EditorEnv, view: &Rc<RwLock<View>>) {
    let mut v = view.write();
    let n = ::std::cmp::max(v.screen.read().height() - 1, 1);

    let tm = v.mode_ctx_mut::<TextModeContext>("text-mode");
    tm.pre_compose_action.push(Action::ScrollDown { n });
}

/*
    TODO(ceg): with multi marks:
      add per mark cut/paste buffer
      and reuse it when pasting
      check behavior when the marks offset cross each other
      the buffer log is not aware of cut/paste/multicursor
*/
pub fn cut_to_end_of_line(
    mut editor: &mut Editor<'static>,
    mut env: &mut EditorEnv<'static>,
    view: &Rc<RwLock<View<'static>>>,
) {
    // doc read only ?
    {
        let v = view.read();
        let doc = v.document.clone();
        let doc = doc.unwrap();
        let doc = doc.read();
        if doc.is_syncing {
            return;
        }
    }

    // REMOVE THIS
    // save marks
    run_text_mode_actions_vec(
        &mut editor,
        &mut env,
        &view,
        &vec![Action::DedupAndSaveMarks],
    );

    let v = &mut view.write();

    let mut doc = v.document.clone();
    let doc = doc.as_mut().unwrap();
    let mut doc = doc.write();

    let tm = v.mode_ctx_mut::<TextModeContext>("text-mode");

    let codec = tm.text_codec.as_ref();

    tm.copy_buffer.clear();

    let mut remove_size = Vec::with_capacity(tm.marks.len());
    let single_mark = tm.marks.len() == 1;

    // this will join line with multi-marks
    let remove_eol = false && !single_mark; // && join_lines // TODO(ceg): use option join-cut-lines

    // TODO(ceg): compute range, check overlaps
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

    tm.pre_compose_action.push(Action::CheckMarks);
    tm.pre_compose_action.push(Action::CancelSelection);
}

pub fn paste(_editor: &mut Editor, _env: &mut EditorEnv, view: &Rc<RwLock<View>>) {
    let v = &mut view.write();

    // doc read only ?
    {
        let doc = v.document.clone();
        let doc = doc.unwrap();
        let doc = doc.read();
        if doc.is_syncing {
            return;
        }
    }

    let mut doc = v.document.clone();
    let doc = doc.as_mut().unwrap();
    let mut doc = doc.write();

    let tm = v.mode_ctx_mut::<TextModeContext>("text-mode");

    let marks = &mut tm.marks;
    let marks_len = marks.len();

    dbg_println!("mark_len {}", marks_len);

    dbg_println!("copy_buffer.len() {}", tm.copy_buffer.len());
    if tm.copy_buffer.len() == 0 {
        return;
    }

    let mut grow = 0;
    for (midx, m) in marks.iter_mut().enumerate() {
        m.offset += grow;

        if tm.copy_buffer.len() != marks_len {
            // TODO(ceg): insert each tm.copy_buffer transaction + '\n'
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

    tm.pre_compose_action.push(Action::CheckMarks);
    tm.pre_compose_action.push(Action::CancelSelection);

    // // mark off_screen ?
    // let screen = v.screen.read();
    // screen.contains_offset(offset) == false || array.len() > screen.width() * screen.height()
    // };
    //
    // if center {
    // tm.pre_compose_action.push(Action::CenterAroundMainMark);
    // };
}

pub fn move_to_token_start(_editor: &mut Editor, _env: &mut EditorEnv, view: &Rc<RwLock<View>>) {
    // TODO(ceg): factorize macrk action
    // mark.apply(fn); where fn=m.move_to_token_end(&doc, codec);
    //

    let mut center = false;

    let v = &mut view.write();
    let screen = v.screen.clone();
    let screen = screen.read();

    let doc = v.document.clone();
    let doc = doc.unwrap();
    let doc = doc.read();

    let tm = v.mode_ctx_mut::<TextModeContext>("text-mode");

    let codec = tm.text_codec.as_ref();

    let midx = tm.mark_index;

    let marks = &mut tm.marks;

    for (idx, m) in marks.iter_mut().enumerate() {
        m.move_to_token_start(&doc, codec);

        // main mark ?
        if idx == midx {
            if !screen.contains_offset(m.offset) {
                // TODO(ceg): push to post action queue
                // {SYNC_VIEW, CLEAR_VIEW, SCROLL_N }
                //
                center = true;
            }
        }
    }

    if center {
        tm.pre_compose_action.push(Action::CenterAroundMainMark);
    }
    tm.prev_action = ActionType::MarksMove;
}

pub fn move_to_token_end(_editor: &mut Editor, _env: &mut EditorEnv, view: &Rc<RwLock<View>>) {
    let mut sync = false;

    let mut v = view.write();
    let screen = v.screen.clone();
    let screen = screen.read();

    let doc = v.document.clone();
    let doc = doc.unwrap();
    let doc = doc.read();

    let tm = v.mode_ctx_mut::<TextModeContext>("text-mode");
    let codec = tm.text_codec.as_ref();

    let marks = &mut tm.marks;

    for m in marks.iter_mut() {
        m.move_to_token_end(&doc, codec);

        // main mark ?
        if !screen.contains_offset(m.offset) {
            // TODO(ceg): push to post action queue
            // {SYNC_VIEW, CLEAR_VIEW, SCROLL_N }
            //
            sync = true;
        }
    }

    if sync {
        tm.pre_compose_action.push(Action::CenterAroundMainMark);
    }

    tm.prev_action = ActionType::MarksMove;
}

fn _get_main_mark_offset(view: &View) -> u64 {
    let tm = view.mode_ctx::<TextModeContext>("text-mode");
    tm.marks[tm.mark_index].offset
}

pub fn set_selection_points_at_marks(
    _editor: &mut Editor,
    _env: &mut EditorEnv,
    view: &Rc<RwLock<View>>,
) {
    let sync = false;

    {
        let mut v = view.write();
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
        let mut v = view.write();
        let tm = v.mode_ctx_mut::<TextModeContext>("text-mode");
        tm.pre_compose_action.push(Action::CenterAroundMainMark);
    }
}

pub fn copy_maybe_remove_selection_symmetric(
    _editor: &mut Editor,
    _env: &mut EditorEnv,

    view: &Rc<RwLock<View>>,
    copy: bool,
    remove: bool,
) -> (usize, usize) {
    let v = &mut view.as_ref().clone().write();

    // doc
    let doc = v.document.clone();
    let doc = doc.clone().unwrap();
    let mut doc = doc.write();

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

        let (min, max) = sort_pair((m.offset, tm.select_point[idx].offset));
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

pub fn copy_maybe_remove_selection_non_symmetric(
    _editor: &mut Editor,
    _env: &mut EditorEnv,

    _view: &Rc<RwLock<View>>,
    _copy: bool,
    _remove: bool,
) -> (usize, usize) {
    (0, 0)
}

pub fn copy_maybe_remove_selection(
    mut editor: &mut Editor<'static>,
    mut env: &mut EditorEnv<'static>,
    view: &Rc<RwLock<View<'static>>>,
    copy: bool,
    remove: bool,
) -> usize {
    let symmetric = {
        let v = &mut view.as_ref().clone().write();
        let _start_offset = v.start_offset;
        let tm = v.mode_ctx_mut::<TextModeContext>("text-mode");
        let symmetric = tm.marks.len() == tm.select_point.len();
        symmetric
    };

    // TODO(ceg): always save marks and insert between transaction ?
    /*
      we could save marks in tmp vec
      save state log size
      apply operation
      if a change was done
      insert tag(marks) @ previous log size ?
      This will remove all custom save

        or

      always save marks
      save log index (prev_index)
      if log index <= prev_index  drop tag

    */
    if remove {
        // save marks before removal insert(s)
        run_text_mode_actions_vec(
            &mut editor,
            &mut env,
            &view,
            &vec![Action::DedupAndSaveMarks],
        );
    }

    // TODO(ceg): sync view(new_start, adjust_size)
    let (copied, removed) = if symmetric {
        copy_maybe_remove_selection_symmetric(editor, env, view, copy, remove)
    } else {
        copy_maybe_remove_selection_non_symmetric(editor, env, view, copy, remove)
    };

    // save marks: TODO(ceg): save marks before
    // cmp cur marks after and if changed save new marks
    {
        let v = &mut view.as_ref().clone().write();
        let _tm = v.mode_ctx_mut::<TextModeContext>("text-mode");
    }

    copied + removed
}

// TODO(ceg): add help, + flag , copy_maybe_remove_selection()
pub fn copy_selection(
    editor: &mut Editor<'static>,
    env: &mut EditorEnv<'static>,
    view: &Rc<RwLock<View<'static>>>,
) {
    copy_maybe_remove_selection(editor, env, view, true, false);
}

pub fn cut_selection(
    editor: &mut Editor<'static>,
    env: &mut EditorEnv<'static>,
    view: &Rc<RwLock<View<'static>>>,
) {
    copy_maybe_remove_selection(editor, env, view, true, true);
}

pub fn button_press(_editor: &mut Editor, _env: &mut EditorEnv, view: &Rc<RwLock<View>>) {
    let v = &mut view.write();

    let (button, x, y) = match v.input_ctx.trigger[0] {
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

    let screen = v.screen.clone();
    let screen = screen.read();

    let (w, h) = screen.dimension();

    let (x, y) = (x as usize, y as usize);

    dbg_println!(
        "VID {} : CLICK @ x({}) Y({})  W({}) H({})",
        v.id,
        x,
        y,
        w,
        h
    );
    // move cursor to (x,y)

    // check from right to left until some codepoint is found
    let tm = v.mode_ctx_mut::<TextModeContext>("text-mode");

    tm.select_point.clear();

    for i in (0..=x).rev() {
        if let Some(cpi) = screen.get_cpinfo(i, y) {
            // clear selection point
            // WARNING:

            if cpi.offset.is_none() {
                continue;
            }

            // reset main mark
            tm.mark_index = 0;
            tm.marks.clear();
            tm.marks.push(Mark {
                offset: cpi.offset.unwrap(),
            });

            dbg_println!(
                "VID {} : CLICK @ x({}) Y({}) set main mark at offset : {:?}",
                v.id,
                i,
                y,
                cpi.offset
            );

            break;
        }
    }

    // s // to internal view.write().state.s
}

pub fn button_release(_editor: &mut Editor, _env: &mut EditorEnv, view: &Rc<RwLock<View>>) {
    let v = &mut view.write();

    let (button, _x, _y) = match v.input_ctx.trigger[0] {
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

    if !v.check_mode_ctx::<TextModeContext>("text-mode") {
        return;
    }

    let tm = v.mode_ctx_mut::<TextModeContext>("text-mode");
    if (button as usize) < tm.button_state.len() {
        tm.button_state[button as usize] = 0;
    }
}

// TODO(ceg): add enter /leave clipped region detection
pub fn pointer_motion(_editor: &mut Editor, _env: &mut EditorEnv, view: &Rc<RwLock<View>>) {
    let v = &mut view.write();
    let screen = v.screen.clone();
    let screen = screen.read();

    if !v.check_mode_ctx::<TextModeContext>("text-mode") {
        return;
    }

    // TODO(ceg): match events
    match &v.input_ctx.trigger[0] {
        InputEvent::PointerMotion(PointerEvent { mods: _, x, y }) => {
            // TODO(ceg): change screen (x,y) to i32 ? and filter in functions ?

            let vid = v.id;
            dbg_println!("VID {} pointer motion x({}) y({})", vid, x, y);

            let x = std::cmp::max(0, *x) as usize;
            let y = std::cmp::max(0, *y) as usize;

            // get fist offset readline the line from right to left
            for i in (0..=x).rev() {
                if let Some(cpi) = screen.get_cpinfo(i, y) {
                    if cpi.offset.is_none() {
                        continue;
                    }

                    // update selection point
                    let tm = v.mode_ctx_mut::<TextModeContext>("text-mode");

                    // TODO(ceg): check focus
                    if let Some(offset) = cpi.offset {
                        if tm.button_state[0] == 1 {
                            tm.select_point.clear();
                            tm.select_point.push(Mark { offset });

                            // if on last line scroll down 1 line
                            if y + 1 >= screen.height() {
                                tm.pre_compose_action.push(Action::ScrollDown { n: 1 });
                            } else if y == 0 {
                                tm.pre_compose_action.push(Action::ScrollUp { n: 1 });
                            }
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

                    break;
                }
            }
        }

        _ => {}
    }
}

pub fn select_next_view(editor: &mut Editor, env: &mut EditorEnv, _view: &Rc<RwLock<View>>) {
    env.root_view_index = std::cmp::min(env.root_view_index + 1, editor.root_views.len() - 1);
    env.view_id = editor.root_views[env.root_view_index];
    dbg_println!("select view_id {}", env.view_id);
}

pub fn select_previous_view(editor: &mut Editor, env: &mut EditorEnv, _view: &Rc<RwLock<View>>) {
    env.root_view_index = env.root_view_index.saturating_sub(1);
    env.view_id = editor.root_views[env.root_view_index];
    dbg_println!("select view_id {}", env.view_id);
}

// TODO(ceg): view.center_around_offset()
pub fn center_around_mark(
    editor: &mut Editor<'static>,
    env: &mut EditorEnv<'static>,
    view: &Rc<RwLock<View<'static>>>,
) {
    let offset = {
        let v = view.read();
        let tm = v.mode_ctx::<TextModeContext>("text-mode");
        tm.marks[tm.mark_index].offset
    };
    dbg_println!("CENTER AROUND MAIN MARK OFFSET {}", offset);
    center_view_around_offset(view, editor, env, offset);
}

pub fn center_around_offset(
    editor: &mut Editor<'static>,
    env: &mut EditorEnv<'static>,
    view: &Rc<RwLock<View<'static>>>,
) {
    if let Some(center_offset) = env.center_offset {
        let offset = {
            let v = view.read();
            let doc = v.document().unwrap();
            let doc = doc.read();
            ::std::cmp::min(doc.size() as u64, center_offset)
        };

        center_view_around_offset(view, editor, env, offset); // TODO(ceg): enum { top center bottom } ? in text-mode
    }
}

pub fn display_end_of_line(_editor: &mut Editor, _env: &mut EditorEnv, view: &Rc<RwLock<View>>) {
    let mut v = view.write();
    let tm = v.mode_ctx_mut::<TextModeContext>("text-mode");

    let s = if let Some(s) = tm.char_map.as_mut().unwrap().get(&'\n') {
        if *s == " " {
            '\u{2936}'
        } else {
            ' '
        }
    } else {
        ' '
    };

    dbg_println!("\\n -> {}", s);

    tm.char_map.as_mut().unwrap().insert('\n', s.to_string());
}

pub fn display_word_wrap(_editor: &mut Editor, _env: &mut EditorEnv, view: &Rc<RwLock<View>>) {
    let mut v = view.write();
    let tm = v.mode_ctx_mut::<TextModeContext>("text-mode");

    tm.display_word_wrap = !tm.display_word_wrap;
}

/// This function computes start/end of lines between start_offset end_offset.<br/>
/// It (will) run the configured filters/plugins.<br/>
/// using the run_compositing_stage function until end_offset is reached.<br/>
/// It is up to the caller to synchronize the starting point
pub fn get_lines_offsets_direct(
    view: &Rc<RwLock<View<'static>>>,
    editor: &mut Editor<'static>,
    env: &mut EditorEnv<'static>,
    start_offset: u64,
    end_offset: u64,
    screen_width: usize,
    screen_height: usize,
) -> Vec<(u64, u64)> {
    let mut lines = Vec::<(u64, u64)>::new();
    let mut tmp = Mark::new(start_offset); // TODO(ceg): rename into screen_start

    // and build tmp screens until end_offset if found
    let screen_width = ::std::cmp::max(1, screen_width);
    let screen_height = ::std::cmp::max(4, screen_height);
    let mut screen = Screen::new(screen_width, screen_height);
    screen.is_off_screen = true; // NB: no marks highlights etc..

    let mut count = 0;
    let mut must_panic = false;

    dbg_println!(
        " get_lines_offsets_direct START -> tmp.offset {}, end_offset {}, screen_width {}, screen_height {}",
        tmp.offset,
        end_offset,
        screen_width,
        screen_height
    );

    let count_limit = 1000;
    // loop until  end_offset is found
    loop {
        count += 1;
        if count > count_limit {
            must_panic = true;
        }

        if count > count_limit {
            dbg_println!(
                "REMAIN to render : end_offset {} - tmp.offset {}= {}",
                end_offset,
                tmp.offset,
                end_offset - tmp.offset
            );
        }

        run_compositing_stage_direct(
            editor,
            env,
            &view,
            tmp.offset,
            end_offset,
            &mut screen,
            LayoutPass::Content,
        );

        if count > 0 {
            dbg_println!(
                    " loop({}) : get_lines_offsets_direct -> tmp.offset {}, end_offset {}, screen_width {}, screen_height {}",
                    count,
                    tmp.offset,
                    end_offset,
                    screen_width,
                    screen_height
                );

            dbg_print!(
                "lines {:?} + screen.line_offset {:?} = ",
                lines,
                screen.line_offset
            );
        }

        lines.append(&mut screen.line_offset.clone()); // move ?

        if count > count_limit {
            dbg_println!("{:?}", lines);
        }

        if must_panic {
            // break if start point is the same ?
            panic!("get_lines_offsets_direct: too many loops detected");
        }

        // we stop at end_offset-1
        if screen.contains_offset(end_offset.saturating_sub(1)) {
            return lines;
        }

        // eof reached ?
        if screen.has_eof() {
            return lines;
        }

        if screen.push_count == 0 {
            return lines;
        }

        // the next screen start is the offset past le last line last offset
        let l = screen.get_last_used_line().unwrap();
        tmp.offset = 1 + l[l.len() - 1].cpi.offset.unwrap();

        screen.clear(); // prepare next screen
    } // END LOOP
}

/*
 TODO(ceg): use nb_lines
 to compute previous screen height
 new_h = screen.weight + (nb_lines * screen.width * max_codec_encode_size)
*/
pub fn scroll_view_up(
    view: &Rc<RwLock<View<'static>>>,
    editor: &mut Editor<'static>,
    env: &mut EditorEnv<'static>,
    nb_lines: usize,
) {
    let off = {
        let start_offset = {
            let v = view.read();
            if v.start_offset == 0 || nb_lines == 0 {
                return;
            }

            dbg_println!(
                "SCROLL VIEW UP N={} START OFFSET {}",
                nb_lines,
                v.start_offset
            );
            v.start_offset
        };
        move_offset_to_previous_line_index(editor, env, &view, start_offset, nb_lines)
    };
    view.write().start_offset = off;
}

pub fn scroll_view_down(
    view: &Rc<RwLock<View<'static>>>,
    editor: &mut Editor<'static>,
    env: &mut EditorEnv<'static>,
    nb_lines: usize,
) {
    let mut view_start_offset = None;
    {
        let v = view.read();

        dbg_println!("SCROLL DOWN VID = {}", v.id);

        // nothing to do :-( ?
        if nb_lines == 0 {
            return;
        }

        let max_offset = {
            let doc = v.document().unwrap();
            let doc = doc.read();
            doc.size() as u64
        };

        // avoid useless scroll
        if v.screen.read().has_eof() {
            dbg_println!("SCROLLDOWN {} : view has EOF", nb_lines);
            return;
        }

        let h = v.screen.read().height();
        if nb_lines >= v.screen.read().height() {
            dbg_println!("SCROLLDOWN {} > view.H {}:  TRY OFFSCREEN", nb_lines, h);

            // slower : call layout builder to build  nb_lines - screen.height()
            let off = scroll_down_view_off_screen(&view, editor, env, max_offset, nb_lines);
            {
                view.write().start_offset = off;
            }

            return;
        }

        dbg_println!("SCROLLDOWN {} <= view.H {}:  TRY ONSCREEN", nb_lines, h);

        for idx in 0..=(h - nb_lines) {
            // just read the current screen
            if let Some(l) = v.screen.write().get_line(nb_lines + idx) {
                let cpi = &l[0].cpi;
                // set first offset of screen.line[nb_lines] as next screen start
                if let Some(offset) = cpi.offset {
                    if offset > v.start_offset {
                        view_start_offset = Some(offset);
                        break;
                    }
                }
            }
        }
    }

    if let Some(offset) = view_start_offset {
        view.write().start_offset = offset;
    }
}

fn scroll_down_view_off_screen(
    view: &Rc<RwLock<View<'static>>>,
    editor: &mut Editor<'static>,
    env: &mut EditorEnv<'static>,
    max_offset: u64,
    nb_lines: usize,
) -> u64 {
    // will be slower than just reading the current screen

    let v = view.read();
    let screen_width = v.screen.read().width();
    let screen_height = v.screen.read().height() + 32;

    let start_offset = v.start_offset;
    let end_offset = ::std::cmp::min(
        v.start_offset + (4 * nb_lines * screen_width) as u64,
        max_offset,
    );

    // will call all layout filters
    let lines = crate::core::modes::text_mode::get_lines_offsets_direct(
        view,
        editor,
        env,
        start_offset,
        end_offset,
        screen_width,
        screen_height,
    );

    // find line index and take lines[(index + nb_lines)].0 as new start of view
    let index = match lines
        .iter()
        .position(|e| e.0 <= start_offset && start_offset <= e.1)
    {
        None => 0,
        Some(i) => ::std::cmp::min(lines.len() - 1, i + nb_lines),
    };

    lines[index].0
}

pub fn center_view_around_offset(
    view: &Rc<RwLock<View<'static>>>,
    editor: &mut Editor<'static>,
    env: &mut EditorEnv<'static>,
    offset: u64,
) {
    view.write().start_offset = offset;
    let h = view.read().screen.read().height() / 2;
    scroll_view_up(view, editor, env, h);
}
