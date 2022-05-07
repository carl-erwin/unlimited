use parking_lot::RwLock;
use std::any::Any;

use std::rc::Rc;

use super::Mode;

use crate::core::codepointinfo::CodepointInfo;

use crate::core::editor::register_input_stage_action;
use crate::core::editor::set_focus_on_vid;
use crate::core::editor::InputStageActionMap;
use crate::core::event::input_map::build_input_event_map;

use crate::core::Editor;
use crate::core::EditorEnv;

use crate::core::view;
use crate::core::view::ContentFilter;
use crate::core::view::FilterIo;
use crate::core::view::LayoutEnv;

use crate::core::view::View;

use crate::core::view::ViewEvent;
use crate::core::view::ViewEventDestination;
use crate::core::view::ViewEventSource;

use crate::core::event::*;

static VSCROLLBAR_INPUT_MAP: &str = r#"
[
  {
    "events": [
     { "in": [{ "pointer-motion": "" }], "action": "vscrollbar:input-event" },
     { "default": [],                    "action": "vscrollbar:input-event" }
   ]
  }

]"#;

pub struct VscrollbarMode {
    // add common fields
}
pub struct VscrollbarModeContext {
    pub target_vid: view::Id,

    pub percent: f64,
    pub percent_end: f64,
    pub scroll_start: usize,
    pub scroll_end: usize,
    pub selected: bool,
    pub t: std::time::Instant,
}

impl<'a> Mode for VscrollbarMode {
    fn name(&self) -> &'static str {
        &"vscrollbar-mode"
    }

    fn build_action_map(&self) -> InputStageActionMap<'static> {
        let mut map = InputStageActionMap::new();
        Self::register_input_stage_actions(&mut map);
        map
    }

    fn alloc_ctx(&self) -> Box<dyn Any> {
        dbg_println!("alloc vscrollbar-mode ctx");
        let ctx = VscrollbarModeContext {
            target_vid: view::Id(0),
            percent: 0.0,
            percent_end: 0.0,
            scroll_start: 0,
            scroll_end: 0,
            selected: false,
            t: std::time::Instant::now(),
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
        let input_map = build_input_event_map(VSCROLLBAR_INPUT_MAP).unwrap();
        let mut input_map_stack = view.input_ctx.input_map.as_ref().borrow_mut();
        input_map_stack.push(input_map);

        view.compose_content_filters
            .borrow_mut()
            .push(Box::new(VscrollbarModeComposeFilter::new()));
    }

    fn on_view_event(
        &self,
        editor: &mut Editor<'static>,
        _env: &mut EditorEnv<'static>,
        src: ViewEventSource,
        dst: ViewEventDestination,
        event: &ViewEvent,
        _parent: Option<&mut View<'static>>,
    ) {
        match event {
            ViewEvent::Subscribe => {}

            ViewEvent::PreComposition => {
                let src = editor.view_map.get(&src.id).unwrap().write();

                let dim = src.screen.read().dimension();

                let mut dst = editor.view_map.get(&dst.id).unwrap().write();

                let mut mode_ctx = dst.mode_ctx_mut::<VscrollbarModeContext>("vscrollbar-mode");

                mode_ctx.target_vid = src.id;

                let doc = src.document.as_ref().unwrap();
                let doc = doc.read();
                let doc_size = doc.size();

                let off = src.start_offset as f64 / doc_size as f64;
                let off2 = src.end_offset as f64 / doc_size as f64;

                mode_ctx.percent = off * 100.0;
                mode_ctx.percent_end = off2 * 100.0;

                let height = dim.1 as f64 / 100.0;
                mode_ctx.scroll_start = (height * mode_ctx.percent) as usize;
                mode_ctx.scroll_start =
                    std::cmp::min(dim.1.saturating_sub(1), mode_ctx.scroll_start);
                mode_ctx.scroll_end = (height * mode_ctx.percent_end) as usize;
                mode_ctx.scroll_end = if mode_ctx.scroll_end == mode_ctx.scroll_start {
                    mode_ctx.scroll_start + 1
                } else {
                    mode_ctx.scroll_end
                };
                mode_ctx.scroll_end = std::cmp::min(dim.1, mode_ctx.scroll_end);

                dbg_println!("SCROLLBAR: mode_ctx.percent {}", mode_ctx.percent);
                dbg_println!("SCROLLBAR: mode_ctx.percent_end {}", mode_ctx.percent_end);
            }

            _ => {}
        }
    }
}

impl VscrollbarMode {
    pub fn new() -> Self {
        dbg_println!("VscrollbarMode");
        VscrollbarMode {}
    }

    pub fn register_input_stage_actions<'a>(mut map: &'a mut InputStageActionMap<'a>) {
        register_input_stage_action(&mut map, "vscrollbar:input-event", vscrollbar_input_event);
    }
}

// TODO?: mode:on_button_press(btn, x,y) ...
// TODO?: mode:on_button_release(btn ?) ...
// TODO?: mode:on_pointer_drag(btn, x,y)

pub fn vscrollbar_input_event(
    mut editor: &mut Editor<'static>,
    mut env: &mut EditorEnv<'static>,
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
                dbg_println!("VSCROLLBAR btn press evt {} {}x{}", button, x, y);

                if *button == 0 {
                    let y = *y as usize;
                    let mut mode_ctx = v.mode_ctx_mut::<VscrollbarModeContext>("vscrollbar-mode");
                    if y >= mode_ctx.scroll_start && y < mode_ctx.scroll_end {
                        mode_ctx.selected = true;
                        env.focus_locked_on = Some(v.id);
                    }

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
                dbg_println!("VSCROLLBAR btn release evt {} {}x{}", button, x, y);

                if *button == 0 {
                    let mode_ctx = v.mode_ctx_mut::<VscrollbarModeContext>("vscrollbar-mode");
                    mode_ctx.selected = false;
                    env.focus_locked_on = None;

                    // explicit focus on target view
                    set_focus_on_vid(&mut editor, &mut env, mode_ctx.target_vid);
                }
            }
        },

        Some(InputEvent::PointerMotion(PointerEvent { x, y, mods: _ })) => {
            dbg_println!("VSCROLLBAR CLIPPING x {} y {}", x, y);
            let target_vid = {
                let mode_ctx = v.mode_ctx::<VscrollbarModeContext>("vscrollbar-mode");

                // explicit focus on target view
                // set_focus_on_vid(&mut editor, &mut env, mode_ctx.target_vid);

                if !mode_ctx.selected {
                    return;
                }

                if mode_ctx.target_vid == view::Id(0) {
                    return;
                }

                mode_ctx.target_vid
            };

            let dim = v.screen.read().dimension();
            let mut dst = editor.view_map.get(&target_vid).unwrap().write();

            let doc_size = {
                let doc = dst.document.as_ref().unwrap();
                let doc = doc.read();
                std::cmp::max(1, doc.size()) // avoid div by zero
            };

            let y = std::cmp::max(0, *y) as usize; //  coordinates can be negative

            let y = if y < dim.1.saturating_sub(1) {
                y
            } else {
                dim.1.saturating_sub(1)
            };

            let percent = y as f64 / dim.1 as f64;
            let percent = if y < dim.1.saturating_sub(1) {
                percent
            } else {
                1.0
            };

            dbg_println!("SCROLLBAR H: {}", dim.1);
            dbg_println!("SCROLLBAR Y: {}", y);
            dbg_println!("SCROLLBAR percent: {}", percent);

            // set target's offset
            // the scrollbar dimension are recomputed in on_view_event
            let offset = (doc_size as f64 * percent) as u64;
            dst.start_offset = offset;

            // TODO(ceg): push action
            // center + scroll-down (h / 2)

            // TODO(ceg): update scrollbar.subscription()
            // view.on_event(ViewEvent { ChangeOffset{ offset } });
            // view.on_request(src, dsc, ViewEvent { ChangeOffset{ offset } });
        }

        _ => {
            dbg_println!("VSCROLLBAR unhandled event {:?}", evt);
            return;
        }
    };

    // TODO(ceg): move scrollbar
    {
        let mode_ctx = v.mode_ctx_mut::<VscrollbarModeContext>("vscrollbar-mode");
        if !mode_ctx.selected {
            return;
        }
    }
}

///////////////////////////////////////////////////////////////////////////////////////////////////

pub struct VscrollbarModeComposeFilter {}

impl VscrollbarModeComposeFilter {
    pub fn new() -> Self {
        VscrollbarModeComposeFilter {}
    }
}

impl ContentFilter<'_> for VscrollbarModeComposeFilter {
    fn name(&self) -> &'static str {
        &"vscrollbar-compose-filter"
    }

    fn run(
        &mut self,
        view: &View,
        env: &mut LayoutEnv,
        _filter_in: &Vec<FilterIo>,
        _filter_out: &mut Vec<FilterIo>,
    ) {
        let mode_ctx = view.mode_ctx::<VscrollbarModeContext>("vscrollbar-mode");
        let mut cpi = CodepointInfo::new();
        cpi.style.bg_color = (45, 49, 54);
        cpi.cp = ' ';
        cpi.displayed_cp = ' ';
        loop {
            let (b, _) = env.screen.push(cpi.clone());
            if !b {
                break;
            }
        }

        dbg_println!("SCROLLBAR height {}", env.screen.height());
        dbg_println!("SCROLLBAR start {}", mode_ctx.scroll_start);
        dbg_println!("SCROLLBAR end {}", mode_ctx.scroll_end);

        for i in mode_ctx.scroll_start..mode_ctx.scroll_end {
            if let Some(cpi) = env.screen.get_cpinfo_mut(0, i) {
                cpi.displayed_cp = ' ';
                cpi.style.is_selected = true;
                let add = 25;
                cpi.style.bg_color = (45 + add, 49 + add, 54 + add);
                if mode_ctx.selected {
                    let add = 50;
                    cpi.style.bg_color = (45 + add, 49 + add, 54 + add);
                } else {
                    let add = 25;
                    cpi.style.bg_color = (45 + add, 49 + add, 54 + add);
                }
            }
        }

        env.quit = true;
    }

    fn finish(&mut self, _view: &View, _env: &mut LayoutEnv) {}
}
