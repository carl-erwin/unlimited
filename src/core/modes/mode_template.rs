use std::any::Any;

use parking_lot::RwLock;

use std::rc::Rc;

use super::Mode;

use crate::core::editor::register_input_stage_action;
use crate::core::editor::InputStageActionMap;
use crate::core::Editor;
use crate::core::EditorEnv;

use crate::core::view::ContentFilter;
use crate::core::view::ScreenOverlayFilter;

use crate::core::view::FilterIo;
use crate::core::view::LayoutEnv;

use crate::core::view::View;
use crate::core::view::ViewEvent;
use crate::core::view::ViewEventDestination;
use crate::core::view::ViewEventSource;

pub struct TemplateMode {
    // add common fields
}
pub struct TemplateModeContext {
    // add per view fields
}

impl<'a> Mode for TemplateMode {
    fn name(&self) -> &'static str {
        &"template-mode"
    }

    fn build_action_map(&self) -> InputStageActionMap<'static> {
        let mut map = InputStageActionMap::new();
        Self::register_input_stage_actions(&mut map);
        map
    }

    fn alloc_ctx(&self) -> Box<dyn Any> {
        dbg_println!("alloc template-mode ctx");
        let ctx = TemplateModeContext {};
        Box::new(ctx)
    }

    fn configure_view(
        &mut self,
        _editor: &mut Editor<'static>,
        _env: &mut EditorEnv<'static>,
        view: &mut View<'static>,
    ) {
        view.compose_content_filters
            .borrow_mut()
            .push(Box::new(TemplateComposeFilter::new()));
    }

    fn on_view_event(
        &self,
        _editor: &mut Editor<'static>,
        _env: &mut EditorEnv<'static>,
        _src: ViewEventSource,
        _dst: ViewEventDestination,
        _event: &ViewEvent,
        _src_view: &mut View<'static>,
        _parent: Option<&mut View<'static>>,
    ) {
    }
}

impl TemplateMode {
    pub fn new() -> Self {
        dbg_println!("TemplateMode");
        TemplateMode {}
    }

    pub fn register_input_stage_actions<'a>(mut map: &'a mut InputStageActionMap<'a>) {
        register_input_stage_action(&mut map, "template-fn1", template_input_action_fn1);
    }
}

pub fn template_input_action_fn1(
    _editor: &mut Editor,
    _env: &mut EditorEnv,
    view: &Rc<RwLock<View>>,
) {
    let v = view.read();
    let doc = v.document().unwrap();
    let _doc = doc.read();
}

///////////////////////////////////////////////////////////////////////////////////////////////////

pub struct TemplateComposeFilter {}

impl TemplateComposeFilter {
    pub fn new() -> Self {
        TemplateComposeFilter {}
    }
}

impl ContentFilter<'_> for TemplateComposeFilter {
    fn name(&self) -> &'static str {
        &"template-compose-filter"
    }

    fn run(
        &mut self,
        _view: &View,
        _env: &mut LayoutEnv,
        filter_in: &[FilterIo],
        filter_out: &mut Vec<FilterIo>,
    ) {
        *filter_out = filter_in.to_vec();
    }

    fn finish(&mut self, _: &View, _: &mut LayoutEnv) {}
}

impl ScreenOverlayFilter<'_> for TemplateMode {
    fn name(&self) -> &'static str {
        &"template-screen-overlay-filter"
    }

    fn finish(&mut self, _: &View, _: &mut LayoutEnv) {}
}
