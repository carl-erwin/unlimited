use std::any::Any;

use parking_lot::RwLock;
use std::rc::Rc;

use super::Mode;

use crate::core::codepointinfo::CodepointInfo;

use crate::core::editor::register_input_stage_action;
use crate::core::editor::InputStageActionMap;
use crate::core::event::input_map::build_input_event_map;
use crate::core::Editor;
use crate::core::EditorEnv;

use crate::core::view::layout::ContentFilter;
use crate::core::view::layout::FilterIo;
use crate::core::view::layout::LayoutEnv;

use crate::core::view::View;

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
        let ctx = HsplitModeContext { selected: false };
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
        input_map_stack.push(input_map);

        view.compose_content_filters
            .borrow_mut()
            .push(Box::new(HsplitModeComposeFilter::new()));
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

pub fn hsplit_input_event(editor: &mut Editor, env: &mut EditorEnv, view: &Rc<RwLock<View>>) {
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
                    env.focus_locked_on = Some(v.id);
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
                    env.focus_locked_on = None;
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
        if mod_ctx.selected == false {
            return;
        }
    }

    if let Some(pvid) = v.parent_id {
        let pv = editor.view_map.get(&pvid).unwrap();
        let mut pv = pv.write();

        let lidx = v.layout_index.unwrap() - 1; // text-view
        dbg_println!("HSPLIT SCREEN HEIGHT  = {}", env.height);

        let max_size = pv.screen.read().height();

        let sibling_vid = pv.children[lidx];
        let sbv = editor.view_map.get(&sibling_vid).unwrap();
        let sbv = sbv.read();
        let cur_size = sbv.screen.read().height();

        dbg_println!(
            "VSPLIT LIDX to resize = {}, sibling_vid {}",
            lidx,
            sibling_vid
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
            decrease_layout_op(pv.layout_ops[lidx], max_size, cur_size, diff as usize)
        } else if env.diff_y > 0 {
            // TODO(ceg): find a better way to refresh global coords
            let gy = env.global_y.unwrap() + env.diff_y;
            env.global_y = Some(gy);

            increase_layout_op(pv.layout_ops[lidx], max_size, cur_size, env.diff_y as usize)
        } else {
            return;
        };

        pv.layout_ops[lidx] = new_op;
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
        _filter_in: &Vec<FilterIo>,
        _filter_out: &mut Vec<FilterIo>,
    ) {
        let mod_ctx = view.mode_ctx::<HsplitModeContext>("hsplit-mode");

        let mut cpi = CodepointInfo::new();
        cpi.style.is_selected = false;
        if env.focus_vid == view.id && mod_ctx.selected {
            cpi.style.bg_color = (113, 114, 123);
        }
        cpi.style.color = (45 + 25, 49 + 25, 54 + 25);

        cpi.cp = '─';
        cpi.displayed_cp = '─';

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
