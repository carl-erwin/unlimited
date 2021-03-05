use std::any::Any;
use std::cell::RefCell;

use std::rc::Rc;
use std::sync::Arc;
use std::sync::RwLock;

use super::Mode;

use crate::core::codepointinfo::CodepointInfo;

use crate::core::document::Document;
use crate::core::editor::register_input_stage_action;
use crate::core::editor::InputStageActionMap;
use crate::core::modes::core_mode::split_with_direction;
use crate::core::Editor;
use crate::core::EditorEnv;

use crate::core::event::*;

use crate::core::view;
use crate::core::view::layout::Filter;
use crate::core::view::layout::FilterIoData;
use crate::core::view::layout::LayoutEnv;
use crate::core::view::LayoutDirection;
use crate::core::view::LayoutOperation;

use crate::core::view::View;

pub struct BasicEditorMode {
    // add common fields
}
pub struct BasicEditorModeContext {
    // add per view fields
}

impl<'a> Mode for BasicEditorMode {
    fn name(&self) -> &'static str {
        &"basic-editor"
    }

    fn build_action_map(&self) -> InputStageActionMap<'static> {
        let mut map = InputStageActionMap::new();
        Self::register_input_stage_actions(&mut map);
        map
    }

    fn alloc_ctx(&self) -> Box<dyn Any> {
        dbg_println!("alloc BasicEditorMode-mode ctx");
        let ctx = BasicEditorModeContext {};
        Box::new(ctx)
    }

    fn configure_view(
        &self,
        mut editor: &mut Editor<'static>,
        mut env: &mut EditorEnv<'static>,
        mut view: &mut View<'static>,
    ) {
        let doc = view.document.clone();

        // children_layout_and_modes
        let ops_modes = vec![
            (
                LayoutOperation::Fixed { size: 1 },
                None,
                vec!["hsplit-mode".to_owned()],
            ),
            (
                LayoutOperation::RemainMinus { minus: 3 },
                doc.clone(),
                vec!["core-mode".to_owned(), "text-mode".to_owned()],
            ),
            (
                LayoutOperation::Fixed { size: 3 },
                None,
                vec!["vsplit-mode".to_owned()],
            ),
        ];

        view.layout_direction = LayoutDirection::Horizontal;
        view.layout_ops = ops_modes.iter().map(|e| e.0.clone()).collect();
        let docs = ops_modes.iter().map(|e| e.1.clone()).collect();
        let modes = ops_modes.iter().map(|e| e.2.clone()).collect();

        let (width, height) = view.dimension();
        dbg_println!("width {}  height {}", width, height);

        split_with_direction(
            &mut editor,
            &mut env,
            &mut view,
            width,
            height,
            LayoutDirection::Horizontal,
            &docs,
            &modes,
        );

        env.focus_changed_to = Some(view.children[1]); // post input
    }
}

impl BasicEditorMode {
    pub fn new() -> Self {
        dbg_println!("BasicEditorMode");
        BasicEditorMode {}
    }

    pub fn register_input_stage_actions<'a>(mut map: &'a mut InputStageActionMap<'a>) {}
}

///////////////////////////////////////////////////////////////////////////////////////////////////
