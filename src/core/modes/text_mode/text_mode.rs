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
use crate::core::view::LayoutPass;

use crate::core::editor;

use crate::core::editor::register_input_stage_action;
use crate::core::editor::InputStageActionMap;
use crate::core::editor::InputStageFunction;
use crate::core::view::View;

use crate::core::event::input_map::build_input_event_map;
use crate::core::event::input_map::DEFAULT_INPUT_MAP;

use super::movement::*;

#[derive(Debug, Clone, Copy)]
pub enum PostInputAction {
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
    DeduplicateMarks { caller: &'static str },
    DeduplicateAndSaveMarks { caller: &'static str },
    SaveMarks { caller: &'static str },
    CancelSelection,
    UpdateReadCache,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum TextModeAction {
    Ignore,
    CreateMarks,
    MarksMove,
    ScreenMove,
    DocumentModification,
    Undo,
    Redo,
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
use crate::core::modes::text_mode::ShowTrailingSpaces;

use crate::core::view::ContentFilter;
use crate::core::view::ScreenOverlayFilter;

struct ContentFilterInfo<'a> {
    allocator: fn() -> Box<dyn ContentFilter<'a>>,
}

struct ScreenOverlayFilterInfo<'a> {
    allocator: fn() -> Box<dyn ScreenOverlayFilter<'a>>,
}

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

    pub pre_compose_action: Vec<PostInputAction>,
    pub post_compose_action: Vec<PostInputAction>,

    pub prev_action: TextModeAction,
}

/* TODO(ceg): command line option opt-in/opt-out ?

  per file extension config file

{
  ".txt" : "text/plain",
  .c   -> { content filter [], screen overlay filter }
  .h   -> { content filter [], screen overlay filter }
  .rs  -> { content filter [], screen overlay filter }
  "default" : "text/plain",
}

text_filters.json
{

"text/plain" -> {
    "content filter": [ "binary/raw",
                        "text/utf8",
                        "text/unicode-to-text",
                        "text/highlight-keywords",
                        "text/highlight-selection",
                        "text/tab-expansion",
                        "text/highlight-keywords",
                        "text/char-map",
                        "text/show-trailing-spaces",
                        "text/word-wrap",
                        "text/screen"
                       ]

    "screen overlay filter": [] }
}

"text/rust" -> {
    "content filter": [ "name1", "name2" ],
    "screen overlay filter": [] }
}

}

*/
fn build_text_mode_content_filters_map() -> HashMap<&'static str, ContentFilterInfo<'static>> {
    let mut content_filter_map = HashMap::new();

    content_filter_map
        .entry("binary/raw")
        .or_insert(ContentFilterInfo {
            allocator: || Box::new(RawDataFilter::new()),
        });
    content_filter_map
        .entry("text/utf8-to-unicode")
        .or_insert(ContentFilterInfo {
            allocator: || Box::new(Utf8Filter::new()),
        });
    content_filter_map
        .entry("text/codec")
        .or_insert(ContentFilterInfo {
            allocator: || Box::new(TextCodecFilter::new()),
        });

    content_filter_map
        .entry("text/unicode-to-text")
        .or_insert(ContentFilterInfo {
            allocator: || Box::new(UnicodeToTextFilter::new()),
        });

    content_filter_map
        .entry("text/highlight-keywords")
        .or_insert(ContentFilterInfo {
            allocator: || Box::new(HighlightFilter::new()),
        });

    content_filter_map
        .entry("text/highlight-selection")
        .or_insert(ContentFilterInfo {
            allocator: || Box::new(HighlightSelectionFilter::new()),
        });

    content_filter_map
        .entry("text/tab-expansion")
        .or_insert(ContentFilterInfo {
            allocator: || Box::new(TabFilter::new()),
        });

    content_filter_map
        .entry("text/char-map")
        .or_insert(ContentFilterInfo {
            allocator: || Box::new(CharMapFilter::new()),
        });

    content_filter_map
        .entry("text/show-trailing-spaces")
        .or_insert(ContentFilterInfo {
            allocator: || Box::new(ShowTrailingSpaces::new()),
        });

    content_filter_map
        .entry("text/word-wrap")
        .or_insert(ContentFilterInfo {
            allocator: || Box::new(WordWrapFilter::new()),
        });

    content_filter_map
        .entry("text/screen")
        .or_insert(ContentFilterInfo {
            allocator: || Box::new(ScreenFilter::new()),
        });

    content_filter_map
}

fn build_text_mode_screen_overlay_filters_map(
) -> HashMap<&'static str, ScreenOverlayFilterInfo<'static>> {
    let mut screen_overlay_filter_map = HashMap::new();

    screen_overlay_filter_map
        .entry("text/draw-marks")
        .or_insert(ScreenOverlayFilterInfo {
            allocator: || Box::new(DrawMarks::new()),
        });

    screen_overlay_filter_map
}

fn build_text_mode_char_map() -> HashMap<char, String> {
    let mut char_map = HashMap::new();

    char_map.insert('\u{0A}', " ".to_string()); //  '\n' (new line)
    char_map.insert('\u{7f}', "<DEL>".to_string());

    if true {
        for i in 0..0x9 {
            let fmt = format!("\\x{i:02x}");

            let c = unsafe { char::from_u32_unchecked(i) };
            char_map.insert(c, fmt);
        }
        for i in 0xb..0x1f {
            let fmt = format!("\\x{i:02x}");
            let c = unsafe { char::from_u32_unchecked(i) };
            char_map.insert(c, fmt);
        }
        for i in 0x07f..0x80 {
            let fmt = format!("\\x{i:02x}");
            let c = unsafe { char::from_u32_unchecked(i) };
            char_map.insert(c, fmt);
        }
    }

    if !true {
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

    char_map
}

fn build_text_mode_color_map() -> HashMap<char, (u8, u8, u8)> {
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

    color_map
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

        let char_map = build_text_mode_char_map();
        let color_map = build_text_mode_color_map();

        let ctx = TextModeContext {
            center_on_mark_move: false, // add movement enums and pass it to center fn
            scroll_on_mark_move: true,
            text_codec: Box::new(utf8::Utf8Codec::new()),
            //text_codec: Box::new(ascii::AsciiCodec::new()),
            prev_buffer_log_revision: 0,
            prev_mark_revision: 0,
            mark_revision: 0,
            marks: vec![Mark { offset: 0 }],
            copy_buffer: vec![],
            mark_index: 0,
            select_point: vec![],
            button_state: [0; 8],
            char_map: Some(char_map),
            color_map: Some(color_map),
            pre_compose_action: vec![],
            post_compose_action: vec![],
            prev_action: TextModeAction::MarksMove,
        };

        Box::new(ctx)
    }

    fn configure_view(
        &mut self,
        _editor: &mut Editor<'static>,
        _env: &mut EditorEnv<'static>,
        view: &mut View<'static>,
    ) {
        dbg_println!("config text-mode for  {:?}", view.id);

        view.ignore_focus = false;

        view.compose_priority = 256; // TODO: move to caller

        let tm = view.mode_ctx_mut::<TextModeContext>("text-mode");

        // create first mark
        let marks_offsets: Vec<u64> = tm.marks.iter().map(|m| m.offset).collect();
        let selections_offsets: Vec<u64> = tm.select_point.iter().map(|m| m.offset).collect();

        view.document.as_ref().unwrap().write().tag(
            Instant::now(),
            0,
            marks_offsets,
            selections_offsets,
        );

        // Config input map
        dbg_println!("DEFAULT_INPUT_MAP\n{}", DEFAULT_INPUT_MAP);
        // TODO(ceg): user define
        // let input_map = mode.build_input_map(); TODO
        {
            let input_map = build_input_event_map(DEFAULT_INPUT_MAP).unwrap();
            let mut input_map_stack = view.input_ctx.input_map.as_ref().borrow_mut();
            input_map_stack.push((self.name(), input_map));
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

            --filter word-wrap,tab // respect order ?
        */

        let content_filter_map = build_text_mode_content_filters_map();
        let screen_overlay_filter_map = build_text_mode_screen_overlay_filters_map();

        // NB: build pipeline in this strict order
        let content_filters_pipeline = if crate::core::raw_data_filter_to_screen() {
            vec![
                "binary/raw",  // mandatory
                "text/screen", // mandatory
            ]
        } else {
            vec![
                "binary/raw",
                "text/utf8-to-unicode", // TODO(ceg) update/remove TextCodecFilter
                "text/unicode-to-text",
                "text/highlight-keywords",
                "text/highlight-selection",
                "text/tab-expansion",
                "text/char-map",
                "text/show-trailing-spaces",
                "text/word-wrap",
                "text/screen", // mandatory
            ]
        };

        for f in content_filters_pipeline {
            if let Some(info) = content_filter_map.get(&f) {
                view.compose_content_filters
                    .borrow_mut()
                    .push((info.allocator)());
            } else {
            }
        }

        let use_draw_marks = true; // mandatory

        let screen_overlay_filters_pipeline = if use_draw_marks {
            vec!["text/draw-marks"]
        } else {
            vec![]
        };

        for f in screen_overlay_filters_pipeline {
            if let Some(info) = screen_overlay_filter_map.get(&f) {
                view.compose_screen_overlay_filters
                    .borrow_mut()
                    .push((info.allocator)());
            } else {
            }
        }

        // setup view action for text mode

        // fix dedup marks, scrolling etc ...
        view.stage_actions
            .push((String::from("text-mode"), run_text_mode_actions));
    }
}

pub struct TextMode {
    // add common fields
}

impl TextMode {
    pub fn new() -> Self {
        dbg_println!("TextMode");
        TextMode {}
    }

    pub fn register_input_stage_actions<'a>(map: &'a mut InputStageActionMap<'a>) {
        let v: Vec<(&str, InputStageFunction)> = vec![
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
            ("text-mode:print-buffer-log", print_buffer_log),
        ];

        for e in v {
            register_input_stage_action(map, e.0, e.1);
        }
    }
}

pub fn run_text_mode_actions_vec(
    editor: &mut Editor<'static>,
    env: &mut EditorEnv<'static>,
    view: &Rc<RwLock<View<'static>>>,
    actions: &Vec<PostInputAction>,
) {
    let mut update_read_cache = true;

    for a in actions.iter() {
        match a {
            PostInputAction::ScrollUp { n } => {
                scroll_view_up(view, editor, env, *n);
                update_read_cache = true;
            }
            PostInputAction::ScrollDown { n } => {
                scroll_view_down(view, editor, env, *n);
                update_read_cache = true;
            }
            PostInputAction::CenterAroundMainMark => {
                center_around_mark(editor, env, &view);
            }
            PostInputAction::CenterAroundMainMarkIfOffScreen => {
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
            PostInputAction::CenterAround { offset } => {
                env.center_offset = Some(*offset);
                center_around_mark(editor, env, &view);
            }
            PostInputAction::MoveMarksToNextLine => {
                move_marks_to_next_line(editor, env, &view);
            }
            PostInputAction::MoveMarksToPreviousLine => {}
            PostInputAction::MoveMarkToNextLine { idx } => {
                move_mark_index_to_next_line(editor, env, view, *idx);
            }
            PostInputAction::MoveMarkToPreviousLine { idx: _usize } => {}

            PostInputAction::ResetMarks => {
                let v = &mut view.write();
                let tm = v.mode_ctx_mut::<TextModeContext>("text-mode");
                let offset = tm.marks[tm.mark_index].offset;

                tm.mark_index = 0;
                tm.marks.clear();
                tm.marks.push(Mark { offset });
            }

            PostInputAction::CheckMarks => {
                let v = &mut view.write();
                let tm = v.mode_ctx_mut::<TextModeContext>("text-mode");
                tm.marks.dedup();
                tm.mark_index = tm.marks.len().saturating_sub(1);

                update_read_cache = true;
            }

            PostInputAction::UpdateReadCache => {
                update_read_cache = true;
            }

            PostInputAction::DeduplicateMarks { caller } => {
                dbg_println!("PostInputAction::DeduplicateAndSaveMarks {}", caller);

                let v = &mut view.write();
                let tm = v.mode_ctx_mut::<TextModeContext>("text-mode");
                tm.marks.dedup();
                tm.select_point.dedup();
            }

            PostInputAction::DeduplicateAndSaveMarks { caller } => {
                dbg_println!("PostInputAction::DeduplicateAndSaveMarks {}", caller);

                let v = &mut view.write();
                let tm = v.mode_ctx_mut::<TextModeContext>("text-mode");

                //
                tm.marks.dedup();
                tm.select_point.dedup();

                let marks_offsets: Vec<u64> = tm.marks.iter().map(|m| m.offset).collect();
                let selections_offsets: Vec<u64> =
                    tm.select_point.iter().map(|m| m.offset).collect();
                dbg_println!(
                    "PostInputAction::DeduplicateAndSaveMarks MARKS {:?}",
                    marks_offsets
                );

                //
                let doc = v.document().unwrap();
                let mut doc = doc.write();
                let max_offset = doc.size() as u64;

                let n = doc.buffer_log_count();
                dbg_println!(
                    "save MARKS PostInputAction::DeduplicateAndSaveMarks doc.buffer_log_count() {}",
                    n
                );
                if n > 0 {
                    doc.tag(
                        env.current_time,
                        max_offset,
                        marks_offsets,
                        selections_offsets,
                    );

                    dbg_println!(
                        "MARK PostInputAction::DeduplicateAndSaveMarks doc revision {}",
                        doc.nr_changes()
                    );
                } else {
                    dbg_println!("MARK PostInputAction::DeduplicateAndSaveMarks nothing to do");
                }
            }

            PostInputAction::SaveMarks { caller } => {
                dbg_println!("marks SaveMarks {}", caller);

                let v = &mut view.write();
                let tm = v.mode_ctx_mut::<TextModeContext>("text-mode");

                //
                let marks_offsets: Vec<u64> = tm.marks.iter().map(|m| m.offset).collect();
                let selections_offsets: Vec<u64> =
                    tm.select_point.iter().map(|m| m.offset).collect();

                dbg_println!("MARKS {:?}", marks_offsets);

                //
                let doc = v.document().unwrap();
                let mut doc = doc.write();
                let max_offset = doc.size() as u64;
                doc.tag(
                    env.current_time,
                    max_offset,
                    marks_offsets,
                    selections_offsets,
                );

                dbg_println!("MARK SaveMarks doc revision {}", doc.nr_changes());
            }

            PostInputAction::CancelSelection => {
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

        // TODO(ceg): PostInputAction::UpdateReadCache(s) vs multiple views
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
    editor: &mut Editor<'static>,
    env: &mut EditorEnv<'static>,
    view: &Rc<RwLock<View<'static>>>,
    pos: editor::StagePosition,
    stage: editor::Stage,
) {
    dbg_println!("run_text_mode_actions stage {:?} pos {:?},", stage, pos);

    {
        let mut v = view.write();
        let tm = v.mode_ctx_mut::<TextModeContext>("text-mode");
        tm.pre_compose_action.push(PostInputAction::UpdateReadCache);
    }

    let actions: Vec<PostInputAction> = {
        match (stage, pos) {
            (editor::Stage::Input, editor::StagePosition::Pre) => {
                let mut v = view.write();
                let doc = v.document().unwrap();
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
                        && tm.prev_action == TextModeAction::MarksMove
                    {
                        // not undo/redo
                        save_marks = true;
                    }

                    if save_marks {
                        tm.pre_compose_action
                            .push(PostInputAction::DeduplicateAndSaveMarks {
                                caller: &"run_text_mode_actions",
                            });
                        tm.pre_compose_action.push(PostInputAction::CheckMarks);
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
                tm.post_compose_action.drain(..).collect()
            }

            _ => {
                // dbg_println!("NO action for {:?}::{:?}", pos, stage);
                return;
            }
        }
    };

    run_text_mode_actions_vec(editor, env, view, &actions);

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

    tm.pre_compose_action
        .push(PostInputAction::ScrollUp { n: 3 });
}

pub fn scroll_down(_editor: &mut Editor, _env: &mut EditorEnv, view: &Rc<RwLock<View>>) {
    // TODO(ceg): 3 is from mode configuration
    // env["default-scroll-size"] -> int
    let v = &mut view.write();
    let tm = v.mode_ctx_mut::<TextModeContext>("text-mode");

    tm.pre_compose_action
        .push(PostInputAction::ScrollDown { n: 3 });
}

// TODO(ceg): rename into handle_input_events
/// Insert an single element/array of unicode code points using hardcoded utf8 codec.<br/>
pub fn insert_codepoint_array(
    editor: &mut Editor<'static>,
    env: &mut EditorEnv<'static>,
    view: &Rc<RwLock<View<'static>>>,
) {
    // InputEvent -> Vec<char>
    let array = {
        let v = view.read();

        assert!(!v.input_ctx.trigger.is_empty());
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
        let doc = v.document.as_ref().unwrap();
        let doc = doc.read();
        let last_pos = doc.buffer_log_pos() >= doc.buffer_log_count().saturating_sub(1);
        dbg_println!(
            "buffer log {} / {} | doc.changed {}, doc.nr_changes() {}",
            doc.buffer_log_pos(),
            doc.buffer_log_count(),
            doc.changed, // NEW != changed -> need save
            doc.nr_changes()
        );
        !last_pos
            || tm.prev_action == TextModeAction::MarksMove
            || tm.prev_action == TextModeAction::DocumentModification
    };

    // TODO(ceg): find a way to remove this
    if save_marks {
        // if undo/redo pos is last do nothing
        dbg_println!("insert_codepoint_array: SAVING marks");
        run_text_mode_actions_vec(
            editor,
            env,
            view,
            &vec![PostInputAction::DeduplicateAndSaveMarks {
                caller: &"insert_code_point_array",
            }],
        );
    } else {
        dbg_println!("skip buff lod save marks");
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
            tm.prev_action = TextModeAction::DocumentModification;

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
        let mut tm = v.mode_ctx_mut::<TextModeContext>("text-mode");

        tm.prev_action = TextModeAction::DocumentModification;

        if center {
            tm.pre_compose_action
                .push(PostInputAction::CenterAroundMainMark);
        }

        tm.pre_compose_action.push(PostInputAction::CancelSelection);
    }
}

pub fn remove_previous_codepoint(
    editor: &mut Editor<'static>,
    env: &mut EditorEnv<'static>,
    view: &Rc<RwLock<View<'static>>>,
) {
    // check previous action: if previous action was a mark move -> tag new positions
    let save_marks = {
        let v = view.read();
        let tm = v.mode_ctx::<TextModeContext>("text-mode");

        tm.prev_action == TextModeAction::MarksMove
    };

    if save_marks {
        run_text_mode_actions_vec(
            editor,
            env,
            view,
            &vec![PostInputAction::DeduplicateAndSaveMarks {
                caller: &"remove_previous_codepoint",
            }],
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
        tm.prev_action = TextModeAction::DocumentModification;

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
            tm.pre_compose_action
                .push(PostInputAction::ScrollUp { n: 1 });
        }
        tm.pre_compose_action.push(PostInputAction::CheckMarks);
    }
}

/// Undo the previous write operation and sync the screen around the main mark.<br/>
pub fn undo(
    editor: &mut Editor<'static>,
    env: &mut EditorEnv<'static>,
    view: &Rc<RwLock<View<'static>>>,
) {
    // check previous action:
    let save_marks = {
        let v = view.read();
        let tm = v.mode_ctx::<TextModeContext>("text-mode");
        tm.prev_action == TextModeAction::DocumentModification
    };
    // TODO(ceg): fin a way to remove this
    if save_marks {
        run_text_mode_actions_vec(
            editor,
            env,
            view,
            &vec![PostInputAction::DeduplicateAndSaveMarks { caller: &"undo" }],
        );
    }

    let v = &mut view.write();

    let mut doc = v.document.clone();
    let doc = doc.as_mut().unwrap();
    let mut doc = doc.write();

    let tm = v.mode_ctx_mut::<TextModeContext>("text-mode");
    let marks = &mut tm.marks;
    let select_point = &mut tm.select_point;

    dbg_println!("undo: doc.buffer_log_count {:?}", doc.buffer_log_count());

    doc.undo_until_tag();

    if let Some((marks_offsets, selections_offsets)) = doc.get_tag_offsets() {
        dbg_println!("restore marks {:?}", marks_offsets);
        marks.clear();
        for offset in marks_offsets {
            marks.push(Mark { offset });
        }

        select_point.clear();
        for offset in selections_offsets {
            select_point.push(Mark { offset });
        }
    } else {
        dbg_println!("TAG not found");
    }

    tm.mark_index = 0; // ??

    tm.pre_compose_action
        .push(PostInputAction::CenterAroundMainMarkIfOffScreen);

    tm.prev_action = TextModeAction::Undo;
}

/// Redo the previous write operation and sync the screen around the main mark.<br/>
pub fn redo(_editor: &mut Editor, _env: &mut EditorEnv, view: &Rc<RwLock<View>>) {
    let v = &mut view.write();

    let mut doc = v.document.clone();
    let doc = doc.as_mut().unwrap();
    let mut doc = doc.write();

    let tm = v.mode_ctx_mut::<TextModeContext>("text-mode");
    let marks = &mut tm.marks;
    let select_point = &mut tm.select_point;

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

    if let Some((marks_offsets, selections_offsets)) = doc.get_tag_offsets() {
        dbg_println!("doc max size {:?}", doc.size());
        dbg_println!("restore marks {:?}", marks_offsets);
        dbg_println!("restore selections {:?}", selections_offsets);
        marks.clear();
        for offset in marks_offsets {
            marks.push(Mark { offset });
        }

        select_point.clear();
        for offset in selections_offsets {
            select_point.push(Mark { offset });
        }
    } else {
        dbg_println!("REDO: no marks ? doc size {:?}", doc.size());
    }

    tm.pre_compose_action
        .push(PostInputAction::CenterAroundMainMarkIfOffScreen);

    tm.prev_action = TextModeAction::Redo;
    /*
    TODO(ceg): add this function pointer attr
    if TextModeAction::Modification -> save marks before exec, etc ...
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
    tm.pre_compose_action.push(PostInputAction::CheckMarks);
    tm.pre_compose_action.push(PostInputAction::CancelSelection);
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

    tm.pre_compose_action.push(PostInputAction::CheckMarks);
    tm.pre_compose_action.push(PostInputAction::CancelSelection); //TODO register last optype
                                                                  // if doc changes cancel selection ?
}

///////////////////////////////////////////////////////////////////////////////////////////////////

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
    move_mark_to_screen_end(editor, env, view);
}

pub fn scroll_to_next_screen(_editor: &mut Editor, _env: &mut EditorEnv, view: &Rc<RwLock<View>>) {
    let mut v = view.write();
    let n = ::std::cmp::max(v.screen.read().height() - 1, 1);

    let tm = v.mode_ctx_mut::<TextModeContext>("text-mode");
    tm.pre_compose_action
        .push(PostInputAction::ScrollDown { n });
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
        let doc = v.document().unwrap();
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
        &vec![PostInputAction::SaveMarks {
            caller: &"cut_to_end_of_line",
        }],
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
    let remove_eol = false; // !single_mark; // && join_lines // TODO(ceg): use option join-cut-lines

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

    tm.pre_compose_action.push(PostInputAction::CheckMarks);
    tm.pre_compose_action.push(PostInputAction::CancelSelection);
}

pub fn paste(_editor: &mut Editor, _env: &mut EditorEnv, view: &Rc<RwLock<View>>) {
    let v = &mut view.write();

    // doc read only ?
    {
        let doc = v.document().unwrap();
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
    if tm.copy_buffer.is_empty() {
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

    tm.pre_compose_action.push(PostInputAction::CheckMarks);
    tm.pre_compose_action.push(PostInputAction::CancelSelection);

    // // mark off_screen ?
    // let screen = v.screen.read();
    // screen.contains_offset(offset) == false || array.len() > screen.width() * screen.height()
    // };
    //
    // if center {
    // tm.pre_compose_action.push(PostInputAction::CenterAroundMainMark);
    // };
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
            dbg_println!("{:?} set point @ offset {}", vid, m.offset);
            tm.select_point.push(m.clone());
        }

        tm.pre_compose_action.push(PostInputAction::SaveMarks {
            caller: &"set_selection_points_at_marks",
        });
    }

    if sync
    /* always center ? */
    {
        let mut v = view.write();
        let tm = v.mode_ctx_mut::<TextModeContext>("text-mode");
        tm.pre_compose_action
            .push(PostInputAction::CenterAroundMainMark);
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
    editor: &mut Editor<'static>,
    env: &mut EditorEnv<'static>,
    view: &Rc<RwLock<View<'static>>>,
    copy: bool,
    remove: bool,
) -> usize {
    let symmetric = {
        let mut v = view.as_ref().clone().write();
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

    //    if remove {
    //        // save marks before removal insert(s)
    //        run_text_mode_actions_vec(
    //            &mut editor,
    //            &mut env,
    //            &view,
    //            &vec![PostInputAction::DeduplicateAndSaveMarks {
    //                caller: &"copy_maybe_remove_selection",
    //            }],
    //        );
    //    }

    // TODO(ceg): sync view(new_start, adjust_size)
    let (copied, removed) = if symmetric {
        copy_maybe_remove_selection_symmetric(editor, env, view, copy, remove)
    } else {
        copy_maybe_remove_selection_non_symmetric(editor, env, view, copy, remove)
    };

    // save marks: TODO(ceg): save marks before
    // cmp cur marks after and if changed save new marks
    {
        let mut v = view.write();
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

pub fn button_press(
    editor: &mut Editor<'static>,
    env: &mut EditorEnv<'static>,
    view: &Rc<RwLock<View<'static>>>,
) {
    let mut save_marks = false;
    let mut new_offset = None;

    {
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

        dbg_println!("{:?} : CLICK @ x({}) Y({})  W({}) H({})", v.id, x, y, w, h);
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

                new_offset = cpi.offset;
                save_marks = new_offset.unwrap() != tm.marks[tm.mark_index].offset;

                dbg_println!(
                    "{:?} : CLICK @ x({}) Y({}) set main mark at offset : {:?}",
                    v.id,
                    i,
                    y,
                    cpi.offset
                );
                break;
            }
        }
    }

    if save_marks {
        run_text_mode_actions_vec(
            editor,
            env,
            view,
            &vec![PostInputAction::SaveMarks {
                caller: &"button_press",
            }],
        );
    }

    if let Some(new_offset) = new_offset {
        let v = &mut view.write();
        let tm = v.mode_ctx_mut::<TextModeContext>("text-mode");

        // reset main mark
        tm.mark_index = 0;
        tm.marks.clear();
        tm.marks.push(Mark { offset: new_offset });

        tm.prev_action = TextModeAction::Ignore;
    }
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
            dbg_println!("{:?} pointer motion x({}) y({})", vid, x, y);

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
                            if y >= screen.height().saturating_sub(1) {
                                tm.pre_compose_action
                                    .push(PostInputAction::ScrollDown { n: 1 });
                            } else if y <= 1 {
                                tm.pre_compose_action
                                    .push(PostInputAction::ScrollUp { n: 1 });
                            }
                        }
                    }

                    dbg_println!(
                        "{:?} @{:?} : pointer motion x({}) y({}) | select offset({:?})",
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
    env.root_view_id = editor.root_views[env.root_view_index];
    dbg_println!("select {:?}", env.root_view_id);
}

pub fn select_previous_view(editor: &mut Editor, env: &mut EditorEnv, _view: &Rc<RwLock<View>>) {
    env.root_view_index = env.root_view_index.saturating_sub(1);
    env.root_view_id = editor.root_views[env.root_view_index];
    dbg_println!("select {:?}", env.root_view_id);
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

        dbg_println!(
            " get_lines_offsets_direct Loop count {} / count limit {}",
            count,
            count_limit
        );

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
            LayoutPass::ScreenContent,
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
            /*
             // waning large prints
            dbg_print!(
                "lines {:?} + screen.line_offset {:?} = ",
                lines,
                screen.line_offset
            );
            */
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

        // the next screen start is the offset past the last line last offset
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
        scroll_screen_up(editor, env, &view, start_offset, nb_lines)
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

    let (max_offset, offscreen, h) = {
        let v = view.read();

        dbg_println!("SCROLL DOWN VID = {:?}", v.id);

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

        let offscreen = nb_lines >= v.screen.read().height();
        let h = v.screen.read().height();

        (max_offset, offscreen, h)
    };

    if offscreen {
        {
            let _v = view.read();
            dbg_println!("SCROLLDOWN {} > view.H {}:  TRY OFFSCREEN", nb_lines, h);
        }
        // slower : call layout builder to build  nb_lines - screen.height()
        let off = scroll_down_view_off_screen(&view, editor, env, max_offset, nb_lines);
        view.write().start_offset = off;
        dbg_println!("SCROLLDOWN {} > view.H {}: RETURN ", nb_lines, h);
        return;
    }

    {
        dbg_println!("SCROLLDOWN {} <= view.H {}:  TRY ONSCREEN", nb_lines, h);

        let v = view.read();
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

    let (start_offset, end_offset, screen_width, screen_height) = {
        let v = view.read();
        let screen_width = v.screen.read().width();
        let screen_height = v.screen.read().height() + 32;

        let start_offset = v.start_offset;
        let end_offset = ::std::cmp::min(
            v.start_offset + (4 * nb_lines * screen_width) as u64,
            max_offset,
        );
        (start_offset, end_offset, screen_width, screen_height)
    };

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

pub fn print_buffer_log(_editor: &mut Editor, _env: &mut EditorEnv, view: &Rc<RwLock<View>>) {
    view.read().document().unwrap().read().buffer_log.dump();
}
