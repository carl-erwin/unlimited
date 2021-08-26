use std::any::Any;

use super::Mode;

use crate::core::editor::InputStageActionMap;
use crate::core::modes::core_mode::split_with_direction;
use crate::core::Editor;
use crate::core::EditorEnv;

use crate::core::view::LayoutDirection;
use crate::core::view::LayoutOperation;

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
        &self,
        mut editor: &mut Editor<'static>,
        mut env: &mut EditorEnv<'static>,
        mut v: &mut View<'static>,
    ) {
        let doc = v.document();

        // children_layout_and_modes
        let ops_modes = vec![
            /*
                    // line numbers
            (
                            LayoutOperation::Fixed { size: 0 }, // TODO(ceg): adjust size based on screen content
                            doc.clone(),
                            vec!["vscrollbar-mode".to_owned()], // TODO(ceg): "line-number-mode" in screen overlay pass
                        ),
                    // line changed
                        (
                            LayoutOperation::Fixed { size: 0 }, // TODO(ceg): adjust size based on screen content
                            doc.clone(),
                            vec!["vscrollbar-mode".to_owned()], // TODO(ceg): "line-change-mode" in screen overlay pass
                        ),
                    // fold
                        (
                            LayoutOperation::Fixed { size: 0 }, // TODO(ceg): adjust size based on screen content
                            doc.clone(),
                            vec!["vscrollbar-mode".to_owned()], // TODO(ceg): "fold-mode" in screen overlay pass
                        ),
            */
            (
                LayoutOperation::RemainMinus { minus: 1 },
                doc.clone(),
                vec![
                    "core-mode".to_owned(),
                    "text-mode".to_owned(),
                    "find-mode".to_owned(),
                ],
            ),
            (
                LayoutOperation::Fixed { size: 1 },
                doc.clone(),
                vec!["vscrollbar-mode".to_owned()],
            ),
        ];

        v.layout_ops = ops_modes.iter().map(|e| e.0.clone()).collect();
        let docs = ops_modes.iter().map(|e| e.1.clone()).collect();
        let modes = ops_modes.iter().map(|e| e.2.clone()).collect();

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
            &docs,
            &modes,
        );

        v.is_group_leader = true; // allow generic split code

        // TODO(ceg): set focus
        // set focus on text view
        let text_view_idx = 0;
        let scroll_bar_idx = 1;
        v.main_child = Some(text_view_idx); // index in children
        v.focus_to = Some(v.children[text_view_idx]); // TODO(ceg):
        env.focus_changed_to = Some(v.children[text_view_idx]); // TODO(ceg):

        // register siblings view
        // text <--> scrollbar

        let vscrollbar_mode = editor.get_mode("vscrollbar-mode").unwrap().clone();

        let src = ViewEventSource {
            id: v.children[text_view_idx],
        };
        let dst = ViewEventDestination {
            id: v.children[scroll_bar_idx],
        };

        eprintln!("simple-view: children: {:?}", v.children);

        // view events -> scrollbar
        register_view_subscriber(editor, env, vscrollbar_mode.clone(), src, dst);
    }
}

impl SimpleViewMode {
    pub fn new() -> Self {
        dbg_println!("SimpleViewMode");
        SimpleViewMode {}
    }

    pub fn register_input_stage_actions<'a>(_map: &'a mut InputStageActionMap<'a>) {}
}
