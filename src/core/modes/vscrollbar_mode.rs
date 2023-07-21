use parking_lot::RwLock;
use std::any::Any;

use std::rc::Rc;

use super::Mode;

use crate::core::codepointinfo::CodepointInfo;

use crate::core::editor::get_view_by_id;
use crate::core::editor::register_input_stage_action;
use crate::core::editor::set_focus_on_view_id;

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
    pub target_view_id: view::Id,

    pub percent: f64,
    pub percent_end: f64,
    pub scroll_start: usize,
    pub scroll_end: usize,
    pub selected: bool,
    pub pointer_over: bool,
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
            target_view_id: view::Id(0),
            percent: 0.0,
            percent_end: 0.0,
            scroll_start: 0,
            scroll_end: 0,
            selected: false,
            pointer_over: false,
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
        input_map_stack.push((self.name(), input_map));

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
            ViewEvent::Subscribe => {
                // we subscribe to src events
                if src.id == dst.id {
                    return;
                }

                let dst = get_view_by_id(editor, dst.id);
                let mut dst = dst.write();

                let mode_ctx = dst.mode_ctx_mut::<VscrollbarModeContext>("vscrollbar-mode");
                mode_ctx.target_view_id = src.id;
            }

            ViewEvent::PreComposition => {
                let src = src_view;
                let dim = src.screen.read().dimension();

                let dst = get_view_by_id(editor, dst.id);
                let mut dst = dst.write();

                let mode_ctx = dst.mode_ctx_mut::<VscrollbarModeContext>("vscrollbar-mode");

                let buffer = src.buffer.as_ref().unwrap();
                let buffer = buffer.read();
                let buffer_size = buffer.size();

                let off = src.start_offset as f64 / buffer_size as f64;
                let off2 = src.end_offset as f64 / buffer_size as f64;

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

            ViewEvent::Enter => {
                let mode_ctx = src_view.mode_ctx_mut::<VscrollbarModeContext>("vscrollbar-mode");
                mode_ctx.pointer_over = true;
            }

            ViewEvent::Leave => {
                let mode_ctx = src_view.mode_ctx_mut::<VscrollbarModeContext>("vscrollbar-mode");
                mode_ctx.pointer_over = false;
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

    pub fn register_input_stage_actions<'a>(map: &'a mut InputStageActionMap<'a>) {
        register_input_stage_action(map, "vscrollbar:input-event", vscrollbar_input_event);
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
                    let mode_ctx = v.mode_ctx_mut::<VscrollbarModeContext>("vscrollbar-mode");
                    if y >= mode_ctx.scroll_start && y < mode_ctx.scroll_end {
                        mode_ctx.selected = true;
                        env.focus_locked_on_view_id = Some(v.id);
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
                    env.focus_locked_on_view_id = None;

                    // explicit focus on target view
                    set_focus_on_view_id(&mut editor, &mut env, mode_ctx.target_view_id);
                }
            }
        },

        Some(InputEvent::PointerMotion(PointerEvent { x, y, mods: _ })) => {
            dbg_println!("VSCROLLBAR CLIPPING x {} y {}", x, y);
            let target_view_id = {
                let mode_ctx = v.mode_ctx::<VscrollbarModeContext>("vscrollbar-mode");

                // explicit focus on target view
                // set_focus_on_view_id(&mut editor, &mut env, mode_ctx.target_view_id);

                if !mode_ctx.selected {
                    return;
                }

                if mode_ctx.target_view_id == view::Id(0) {
                    return;
                }

                mode_ctx.target_view_id
            };

            let dim = v.screen.read().dimension();
            let dst = get_view_by_id(editor, target_view_id);
            let mut dst = dst.write();

            let buffer_size = {
                let buffer = dst.buffer.as_ref().unwrap();
                let buffer = buffer.read();
                std::cmp::max(1, buffer.size()) // avoid div by zero
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
            let offset = (buffer_size as f64 * percent) as u64;
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
        _filter_in: &[FilterIo],
        _filter_out: &mut Vec<FilterIo>,
    ) {
        let mode_ctx = view.mode_ctx::<VscrollbarModeContext>("vscrollbar-mode");
        let mut cpi = CodepointInfo::new();

        //        cpi.style.bg_color = (45, 49, 54);
        //        cpi.style.color = (192,0,0);

        //        cpi.style.bg_color = TextStyle::default_bg_color();

        // TextStyle::scrollbar_non_selected_color();
        // TextStyle::scrollbar_selected_color();
        // TextStyle::scrollbar_pointer_over_color();

        cpi.cp = ' ';
        cpi.displayed_cp = ' ';
        loop {
            let (b, _) = env.screen.push(cpi.clone());
            if !b {
                break;
            }
        }

        // by default the scrollbar is fill with invisible ' ' and default fg/bg color
        // fill whole scrollbar bg
        // let h = env.screen.height();
        // for i in 0..h {
        //     if let Some(cpi) = env.screen.get_cpinfo_mut(0, i) {
        //         cpi.displayed_cp = ' ';
        //     }
        // }

        dbg_println!("SCROLLBAR height {}", env.screen.height());
        dbg_println!("SCROLLBAR start {}", mode_ctx.scroll_start);
        dbg_println!("SCROLLBAR end {}", mode_ctx.scroll_end);

        for i in mode_ctx.scroll_start..mode_ctx.scroll_end {
            if let Some(cpi) = env.screen.get_cpinfo_mut(0, i) {
                cpi.displayed_cp = ' ';

                if mode_ctx.selected {
                    cpi.style.bg_color = (34, 167, 242);
                } else {
                    if mode_ctx.pointer_over {
                        cpi.style.bg_color = (0, 119, 184);
                    } else {
                        cpi.style.bg_color = (31, 36, 59);
                    }
                }
            }
        }

        env.quit = true;
    }

    fn finish(&mut self, _view: &View, _env: &mut LayoutEnv) {}
}
