use std::any::Any;
use std::cell::RefCell;

use std::rc::Rc;

use super::Mode;

use crate::core::codepointinfo::CodepointInfo;

use crate::core::editor::register_input_stage_action;
use crate::core::editor::InputStageActionMap;
use crate::core::Editor;
use crate::core::EditorEnv;

use crate::core::view::layout::Filter;
use crate::core::view::layout::FilterIo;
use crate::core::view::layout::LayoutEnv;

use crate::core::view::View;

pub struct VsplitMode {
    // add common fields
}
pub struct VsplitModeContext {
    // add per view fields
}

impl<'a> Mode for VsplitMode {
    fn name(&self) -> &'static str {
        &"vsplit-mode"
    }

    fn build_action_map(&self) -> InputStageActionMap<'static> {
        let mut map = InputStageActionMap::new();
        Self::register_input_stage_actions(&mut map);
        map
    }

    fn alloc_ctx(&self) -> Box<dyn Any> {
        dbg_println!("alloc vsplit-mode ctx");
        let ctx = VsplitModeContext {};
        Box::new(ctx)
    }

    fn configure_view(
        &self,
        _editor: &mut Editor<'static>,
        _env: &mut EditorEnv<'static>,
        view: &mut View<'static>,
    ) {
        view.compose_filters
            .borrow_mut()
            .push(Box::new(VsplitModeComposeFilter::new()));
    }
}

impl VsplitMode {
    pub fn new() -> Self {
        dbg_println!("VsplitMode");
        VsplitMode {}
    }

    pub fn register_input_stage_actions<'a>(mut map: &'a mut InputStageActionMap<'a>) {
        register_input_stage_action(&mut map, "template-fn1", template_input_action_fn1);
    }
}

pub fn template_input_action_fn1(
    _editor: &mut Editor,
    _env: &mut EditorEnv,
    view: &Rc<RefCell<View>>,
) {
    let v = view.borrow();
    let doc = v.document().unwrap();
    let _doc = doc.read().unwrap();
}

///////////////////////////////////////////////////////////////////////////////////////////////////

pub struct VsplitModeComposeFilter {}

impl VsplitModeComposeFilter {
    pub fn new() -> Self {
        VsplitModeComposeFilter {}
    }
}

impl Filter<'_> for VsplitModeComposeFilter {
    fn name(&self) -> &'static str {
        &"vsplit-compose-filter"
    }

    fn setup(&mut self, _env: &mut LayoutEnv, _view: &View) {}

    fn run(
        &mut self,
        _view: &View,
        env: &mut LayoutEnv,
        _filter_in: &Vec<FilterIo>,
        _filter_out: &mut Vec<FilterIo>,
    ) {
        // hack
        let mut cpi = CodepointInfo::new();
        cpi.style.is_selected = false;
        // cpi.style.bg_color = (100, 123, 153);
        cpi.cp = '│';
        cpi.displayed_cp = '│';
        cpi.metadata = true;
        cpi.size = 0;

        loop {
            let (b, _) = env.screen.push(cpi.clone());
            if b == false {
                env.quit = true;
                break;
            }
        }
    }

    fn finish(&mut self, _view: &View, _env: &mut LayoutEnv) -> () {}
}
