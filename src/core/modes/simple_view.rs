use std::any::Any;
use std::rc::Rc;

use super::Mode;

use crate::core::editor::InputStageActionMap;
use crate::core::modes::core_mode::split_with_direction;
use crate::core::Editor;
use crate::core::EditorEnv;

use crate::core::view::LayoutDirection;
use crate::core::view::LayoutSize;

use crate::core::view::register_view_subscriber;

use crate::core::view::View;
use crate::core::view::ViewEventDestination;
use crate::core::view::ViewEventSource;

pub struct SimpleViewMode {
    // add common fields
}
pub struct SimpleViewModeContext {
    // add per view fields
}

impl<'a> Mode for SimpleViewMode {
    fn name(&self) -> &'static str {
        &"simple-view"
    }

    fn build_action_map(&self) -> InputStageActionMap<'static> {
        let mut map = InputStageActionMap::new();
        Self::register_input_stage_actions(&mut map);
        map
    }

    fn alloc_ctx(&self) -> Box<dyn Any> {
        dbg_println!("alloc SimpleViewMode-mode ctx");
        let ctx = SimpleViewModeContext {};
        Box::new(ctx)
    }

    fn configure_view(
        &mut self,
        mut editor: &mut Editor<'static>,
        mut env: &mut EditorEnv<'static>,
        mut v: &mut View<'static>,
    ) {
        let buffer = v.buffer();
        //        let buffer_sz = buffer.as_ref().unwrap().read().size();
        let line_number_view_width = match std::env::var("SINGLE_VIEW") {
            Ok(_) => 0,
            _ => 12,
        };

        // children_layout_and_modes
        let ops_modes = vec![
            /*
                        // fs tree changed
                        (
                            LayoutSize::Fixed { size: 0 }, // TODO(ceg): adjust size based on screen content
                            buffer.clone(),
                            vec!["fstree-mode".to_owned()],
                        ),
            */
            // line numbers
            (
                LayoutSize::Fixed {
                    size: line_number_view_width,
                }, // TODO(ceg): adjust size based on screen content
                buffer.clone(),
                vec!["line-number-mode".to_owned()], // TODO(ceg): "line-number-mode" in screen overlay pass
            ),
            // empty column
            (
                LayoutSize::Fixed { size: 1 },
                buffer.clone(),
                vec!["".to_owned()],
            ),
            /*
                        // line changed
                        (
                            LayoutSize::Fixed { size: 0 }, // TODO(ceg): adjust size based on screen content
                            buffer.clone(),
                            vec!["vscrollbar-mode".to_owned()], // TODO(ceg): "line-change-mode" in screen overlay pass
                        ),
                        // fold
                        (
                            LayoutSize::Fixed { size: 0 }, // TODO(ceg): adjust size based on screen content
                            buffer.clone(),
                            vec!["vscrollbar-mode".to_owned()], // TODO(ceg): "fold-mode" in screen overlay pass
                        ),
            */
            // text
            (
                LayoutSize::RemainMinus { minus: 1 },
                buffer.clone(),
                vec![
                    "core-mode".to_owned(),
                    "text-mode".to_owned(),
                    "find-mode".to_owned(),
                    "goto-line-mode".to_owned(),
                    "open-doc-mode".to_owned(),
                ],
            ),
            // scrollbar
            (
                LayoutSize::Fixed { size: 1 },
                buffer.clone(),
                vec!["vscrollbar-mode".to_owned()],
            ),
        ];

        let mut layout_ops = vec![];
        let mut buffers = vec![];
        let mut modes = vec![];

        for e in &ops_modes {
            layout_ops.push(e.0.clone());
            buffers.push(e.1.clone());
            modes.push(e.2.clone());
        }

        let width = v.width;
        let height = v.height;

        v.layout_direction = LayoutDirection::Horizontal;

        split_with_direction(
            &mut editor,
            &mut env,
            &mut v,
            width,
            height,
            LayoutDirection::Horizontal,
            &layout_ops,
            &buffers,
            &modes,
        );

        v.is_splittable = true; // Nb: do not remove , allow recursive splitting

        // TODO(ceg): set focus
        // set focus on text view
        let line_numbers_view_idx = 0;
        let _empty_bar = 1;

        let text_view_idx = 2;
        let scroll_bar_idx = 3;

        v.focus_to = Some(v.children[text_view_idx].id); // TODO(ceg):

        dbg_println!("simple-view: children: {:?}", v.children);
        // register siblings view
        // text <--> scrollbar

        let vscrollbar_mode = editor.get_mode("vscrollbar-mode").unwrap();

        let text_view_src = ViewEventSource {
            id: v.children[text_view_idx].id,
        };

        let scrollbar_dst = ViewEventDestination {
            id: v.children[scroll_bar_idx].id,
        };

        // view events -> scrollbar
        register_view_subscriber(
            editor,
            env,
            Rc::clone(&vscrollbar_mode),
            text_view_src, // publisher
            scrollbar_dst, // subscriber
        );

        // view events -> line_number
        let line_number_dst = ViewEventDestination {
            id: v.children[line_numbers_view_idx].id,
        };

        let line_number_mode = editor.get_mode("line-number-mode").unwrap();
        register_view_subscriber(
            editor,
            env,
            Rc::clone(&line_number_mode),
            text_view_src,   // publisher
            line_number_dst, // subscriber
        );
    }
}

impl SimpleViewMode {
    pub fn new() -> Self {
        dbg_println!("SimpleViewMode");
        SimpleViewMode {}
    }

    pub fn register_input_stage_actions<'a>(_map: &'a mut InputStageActionMap<'a>) {}
}
