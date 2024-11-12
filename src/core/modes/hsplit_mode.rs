use std::any::Any;

use parking_lot::RwLock;
use std::rc::Rc;

use super::Mode;

use crate::core::codepointinfo::CodepointInfo;
use crate::core::codepointinfo::TextStyle;

use crate::core::editor::get_view_by_id;

use crate::core::editor::register_input_stage_action;
use crate::core::editor::InputStageActionMap;
use crate::core::event::input_map::build_input_event_map;
use crate::core::Editor;
use crate::core::EditorEnv;

use crate::core::view::ContentFilter;
use crate::core::view::FilterIo;
use crate::core::view::LayoutEnv;

use crate::core::view::View;

use crate::core::view::ViewEvent;
use crate::core::view::ViewEventDestination;
use crate::core::view::ViewEventSource;

use crate::core::event::*;

use crate::core::modes::core_mode::decrease_layout_op;
use crate::core::modes::core_mode::increase_layout_op;

static HSPLIT_INPUT_MAP: &str = r#"
[
  {
    "events": [
     { "in": [{ "pointer-motion": "" }], "action": "hsplit:input-event" },
     { "default": [],                    "action": "hsplit:input-event" }
   ]
  }

]"#;

pub struct HsplitMode {
    // add common fields
}
pub struct HsplitModeContext {
    // add per view fields
    pub selected: bool,
    pub hover: bool,
}

impl<'a> Mode for HsplitMode {
    fn name(&self) -> &'static str {
        &"hsplit-mode"
    }

    fn build_action_map(&self) -> InputStageActionMap<'static> {
        let mut map = InputStageActionMap::new();
        Self::register_input_stage_actions(&mut map);
        map
    }

    fn alloc_ctx(&self) -> Box<dyn Any> {
        dbg_println!("alloc hsplit-mode ctx");
        let ctx = HsplitModeContext {
            selected: false,
            hover: false,
        };
        Box::new(ctx)
    }

    fn configure_view(
        &mut self,
        _editor: &mut Editor<'static>,
        _env: &mut EditorEnv<'static>,
        view: &mut View<'static>,
    ) {
        // setup input map for core actions
        let input_map = build_input_event_map(HSPLIT_INPUT_MAP).unwrap();
        let mut input_map_stack = view.input_ctx.input_map.as_ref().borrow_mut();
        input_map_stack.push((self.name(), input_map));

        view.compose_content_filters
            .borrow_mut()
            .push(Box::new(HsplitModeComposeFilter::new()));
    }

    fn on_view_event(
        &self,
        _editor: &mut Editor<'static>,
        _env: &mut EditorEnv<'static>,
        src: ViewEventSource,
        dst: ViewEventDestination,
        event: &ViewEvent,
        src_view: &mut View<'static>,
        _parent: Option<&mut View<'static>>,
    ) {
        dbg_println!(
            "mode '{}' on_view_event src: {:?} dst: {:?}, event {:?} view.id {:?}",
            self.name(),
            src,
            dst,
            event,
            src_view.id
        );

        match event {
            ViewEvent::Enter => {
                let mod_ctx = src_view.mode_ctx_mut::<HsplitModeContext>("hsplit-mode");
                mod_ctx.hover = true;
            }

            ViewEvent::Leave => {
                let mod_ctx = src_view.mode_ctx_mut::<HsplitModeContext>("hsplit-mode");
                mod_ctx.hover = false;
            }

            _ => {}
        }
    }
}

impl HsplitMode {
    pub fn new() -> Self {
        dbg_println!("HsplitMode");
        HsplitMode {}
    }

    pub fn register_input_stage_actions<'a>(mut map: &'a mut InputStageActionMap<'a>) {
        register_input_stage_action(&mut map, "hsplit:input-event", hsplit_input_event);
    }
}

// TODO?: mode:on_button_press(btn, x,y) ...
// TODO?: mode:on_button_release(btn ?) ...
// TODO?: mode:on_pointer_drag(btn, x,y)

pub fn hsplit_input_event(
    editor: &mut Editor<'static>,
    env: &mut EditorEnv,
    view: &Rc<RwLock<View>>,
) {
    let mut v = view.write();

    let evt = v.input_ctx.trigger.last();
    match evt {
        Some(InputEvent::ButtonPress(ref button_event)) => match button_event {
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
            } => {
                dbg_println!("HSPLIT btn press evt {} {}x{}", button, x, y);

                if *button == 0 {
                    let mod_ctx = v.mode_ctx_mut::<HsplitModeContext>("hsplit-mode");
                    mod_ctx.selected = true;
                    env.focus_locked_on_view_id = Some(v.id);
                    return;
                }
            }
        },

        Some(InputEvent::ButtonRelease(ref button_event)) => match button_event {
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
            } => {
                dbg_println!("HSPLIT btn release evt {} {}x{}", button, x, y);

                if *button == 0 {
                    let mod_ctx = v.mode_ctx_mut::<HsplitModeContext>("hsplit-mode");
                    mod_ctx.selected = false;
                    env.focus_locked_on_view_id = None;
                }
            }
        },

        Some(InputEvent::PointerMotion(PointerEvent {
            x: _,
            y: _,
            mods: _,
        })) => {}

        _ => {
            dbg_println!("HSPLIT unhandled event {:?}", evt);
            return;
        }
    };

    {
        let mod_ctx = v.mode_ctx_mut::<HsplitModeContext>("hsplit-mode");
        if !mod_ctx.selected {
            return;
        }
    }

    if let Some(pvid) = v.parent_id {
        let pv = get_view_by_id(editor, pvid);
        let mut pv = pv.write();

        let lidx = v.layout_index.unwrap() - 1; // text-view
        dbg_println!("HSPLIT SCREEN HEIGHT  = {}", env.height);

        let max_size = pv.screen.read().height();

        let sibling_view_id = pv.children[lidx].id;
        let sbv = get_view_by_id(editor, sibling_view_id);
        let sbv = sbv.read();
        let cur_size = sbv.screen.read().height();

        dbg_println!(
            "VSPLIT LIDX to resize = {}, sibling_ {:?}",
            lidx,
            sibling_view_id
        );
        dbg_println!("VSPLIT p.children {:?}", pv.children);
        dbg_println!("VSPLIT env.diff_y = {}", env.diff_y);

        let new_op = if env.diff_y < 0 {
            // TODO(ceg): find a better way to refresh global coords
            let diff = -env.diff_y;
            let gy = env.global_y.unwrap();
            if gy <= diff {
                return;
            }
            let gy = gy.saturating_sub(-env.diff_y);
            env.global_y = Some(gy);
            //
            decrease_layout_op(
                &pv.children[lidx].layout_op,
                max_size,
                cur_size,
                diff as usize,
            )
        } else if env.diff_y > 0 {
            // TODO(ceg): find a better way to refresh global coords
            let gy = env.global_y.unwrap() + env.diff_y;
            env.global_y = Some(gy);

            increase_layout_op(
                &pv.children[lidx].layout_op,
                max_size,
                cur_size,
                env.diff_y as usize,
            )
        } else {
            return;
        };

        pv.children[lidx].layout_op = new_op;
    }
    // TODO(ceg): refresh global coords
}

///////////////////////////////////////////////////////////////////////////////////////////////////

pub struct HsplitModeComposeFilter {}

impl HsplitModeComposeFilter {
    pub fn new() -> Self {
        HsplitModeComposeFilter {}
    }
}

impl ContentFilter<'_> for HsplitModeComposeFilter {
    fn name(&self) -> &'static str {
        &"hsplit-compose-filter"
    }

    fn run(
        &mut self,
        view: &View,
        env: &mut LayoutEnv,
        _filter_in: &[FilterIo],
        _filter_out: &mut Vec<FilterIo>,
    ) {
        let mod_ctx = view.mode_ctx::<HsplitModeContext>("hsplit-mode");

        let mut cpi = CodepointInfo::new();
        cpi.style.is_selected = false;

        cpi.style.color = (45 + 25, 49 + 25, 54 + 25);
        if mod_ctx.selected {
            cpi.style.color = (0, 119, 184);
        } else {
            if mod_ctx.hover {
                cpi.style.color = TextStyle::default_color();
            }
        }

        cpi.cp = '─';
        cpi.displayed_cp = '─';

        loop {
            let (b, _) = env.screen.push(&cpi);
            if !b {
                env.quit = true;
                break;
            }
        }
    }

    fn finish(&mut self, _view: &View, _env: &mut LayoutEnv) {}
}
