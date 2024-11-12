/* DO NOT SPLIT THIS FILE YET: the filter apis are not stable enough */

use parking_lot::RwLock;
use std::char;

use std::rc::Rc;
use std::sync::Arc;

//

use crate::dbg_println;

use crate::core::screen::Screen;

use crate::core::editor::get_view_by_id;
use crate::core::editor::Editor;
use crate::core::editor::EditorEnv;

use crate::core::codepointinfo::TextStyle;
use crate::core::view;
use crate::core::view::View;
use crate::core::view::ViewEvent;

// TODO remove this impl details

#[derive(Debug, Clone, Copy, Eq, PartialEq)]
pub enum LayoutPass {
    ScreenContent = 1,
    ScreenOverlay = 2,
    ScreenContentAndOverlay = 3,
}

//
pub struct LayoutEnv<'a> {
    pub graphic_display: bool,
    pub quit: bool,
    pub base_offset: u64,
    pub max_offset: u64,
    pub screen: &'a mut Screen,
    pub active_view_id: view::Id,
    pub current_view_id: view::Id,
}

// TODO(ceg): add ?
//        buffer,
//        view
//
//  input_mime_type() -> &str "" | ""
//  output_mime_type() -> &str "application/octet-stream"

pub trait ContentFilter<'a> {
    fn name(&self) -> &'static str;

    fn setup(
        &mut self,
        mut _editor: &mut Editor<'static>,
        mut _editor_env: &mut EditorEnv<'static>,
        _env: &mut LayoutEnv,
        _view: &Rc<RwLock<View>>,
        _parent_view: Option<&View<'static>>,
    ) {
        /* default implementation is empty*/
    }

    fn run_managed(
        &mut self,
        view: &Rc<RwLock<View>>,
        env: &mut LayoutEnv,
        input: &[FilterIo],
        output: &mut Vec<FilterIo>,
    ) {
        let view = view.read();
        self.run(&view, env, input, output);
    }

    fn run(
        &mut self,
        _view: &View,
        env: &mut LayoutEnv,
        _input: &[FilterIo],
        _output: &mut Vec<FilterIo>,
    ) {
        // default: stop pipeline
        if _input.is_empty() {
            env.quit = true;
        } else {
            *_output = _input.to_vec();
        }
    }

    fn finish(&mut self, _view: &View, _env: &mut LayoutEnv) {
        // default
    }
}

pub trait ScreenOverlayFilter<'a> {
    fn name(&self) -> &'static str;

    fn setup(
        &mut self,
        _editor: &Editor<'static>,
        _env: &mut LayoutEnv,
        _view: &Rc<RwLock<View>>,
        _parent_view: Option<&View<'static>>,
    ) {
        /* default implementation is empty*/
    }

    fn run_managed(&mut self, view: &Rc<RwLock<View>>, env: &mut LayoutEnv) {
        let view = view.read();
        self.run(&view, env);
    }

    fn run(&mut self, _view: &View, _env: &mut LayoutEnv) {}

    fn finish(&mut self, _view: &View, _env: &mut LayoutEnv) {
        // default
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Unicode {
    pub size: u32,
    pub cp: u32,
}

// content_type == unicode
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum FilterData {
    EndOfStream,
    CustomLimitReached,

    ByteArray { vec: Vec<u8> },

    UnicodeArray { vec: Vec<Unicode> },

    // text array ?
    TextInfo { real_cp: u32, displayed_cp: u32 },
}

// TODO(ceg): move to core/view/filterio.rs
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FilterIo {
    // general info
    pub metadata: bool,
    pub style: TextStyle,
    //
    pub offset: Option<u64>,
    pub size: usize, // count(data) ?
    //
    pub data: FilterData,
    // TODO(ceg): add style infos ?
}

impl FilterIo {
    pub fn replace_displayed_codepoint(io: &FilterIo, disp_cp: char) -> FilterIo {
        if let &FilterIo {
            // general info
            metadata,
            style,
            offset: from_offset,
            size: cp_size,
            data: FilterData::TextInfo { real_cp, .. },
        } = io
        {
            return FilterIo {
                // general info
                metadata,
                style,
                offset: from_offset,
                size: cp_size,
                data: FilterData::TextInfo {
                    real_cp,
                    displayed_cp: disp_cp as u32,
                },
            };
        }

        io.clone()
    }

    pub fn check_invariants(&self) {
        if let FilterIo {
            metadata,
            size,
            offset: _,
            data: FilterData::TextInfo { .. },
            ..
        } = self
        {
            if *size > 0 && *metadata {
                dbg_println!("INVALID IO [METADATA] {:?}", self);
                panic!("");
            }
            if *size == 0 && !metadata {
                dbg_println!("INVALID IO [NON META] {:?}", self);
                panic!("");
            }
        }
    }
}

pub fn run_compositing_stage(
    editor: &mut Editor<'static>,
    env: &mut EditorEnv<'static>,
    view: &Rc<RwLock<View<'static>>>,
    base_offset: u64, // default view.start_offset start -> Option<u64>
    max_offset: u64,  // default view.buffer.size()   end  -> Option<u64>
    screen: &mut Screen,
    pass_mask: LayoutPass,
) {
    run_compositing_stage_direct(
        editor,
        env,
        view,
        base_offset,
        max_offset,
        screen,
        pass_mask,
    );

    let mut view = view.write();
    if let Some(offset) = screen.last_offset {
        view.end_offset = offset;
    }
}

fn notify_children(
    editor: &mut Editor<'static>,
    editor_env: &mut EditorEnv<'static>,
    parent_view: &mut View<'static>,
    all_children: &[view::ChildView],
    event: ViewEvent,
) {
    for child in all_children.iter() {
        dbg_println!(" notify {:?}", child);

        let child_v = get_view_by_id(editor, child.id);
        let mut child_v = child_v.write();
        let subscribers = child_v.subscribers.clone();

        // NB: notify subscribers just before composition
        // use View::compose_priority to order notifications
        // NOTE(ceg): currently we do not have event filters

        for cb in subscribers.iter() {
            let mode = cb.0.as_ref();

            if cb.1.id == cb.2.id {
                // ignore self registration
                continue;
            }

            dbg_println!(
                "call mode {} on_view_event : {:?}",
                mode.borrow().name(),
                event
            );

            mode.borrow().on_view_event(
                editor,
                editor_env,
                cb.1,
                cb.2,
                &event,
                &mut child_v,
                Some(parent_view),
            );
        }
    }
}

#[inline(always)]
fn compose_children(
    editor: &mut Editor<'static>,
    editor_env: &mut EditorEnv<'static>,
    view: &Rc<RwLock<View<'static>>>,
    _base_offset: u64, // default view.start_offset start -> Option<u64>
    max_offset: u64,   // default view.buffer.size()   end  -> Option<u64>
    screen: &mut Screen,
    pass_mask: LayoutPass,
) -> bool {
    let mut view = view.write();
    if view.children.is_empty() && view.floating_children.is_empty() {
        return false;
    }

    dbg_println!("COMPOSE CHILDREN OF  {:?} tags {:?}", view.id, view.tags);

    // split direction
    let layout_dir_is_vertical = view.layout_direction == view::LayoutDirection::Vertical;

    let (width, height) = screen.dimension();
    if width == 0 || height == 0 {
        return false;
    }

    // TODO(ceg): add pass here to let the children resize themselves
    // ex: line view width depends on target view number of line
    // add: flag to allow resize ?
    // View::self_resize_allowed: bool
    let mut all_children = view.children.clone();

    dbg_println!("all_children {:?}", all_children);

    let mut floating_children = view.floating_children.clone();
    all_children.append(&mut floating_children);

    // - notify modes via ViewEvent::PreLayoutSizing
    //- modes can resize the view, change layout
    notify_children(
        editor,
        editor_env,
        &mut view,
        &all_children,
        ViewEvent::PreLayoutSizing,
    );

    dbg_println!("COMPOSE checking {:?} non floating children", view.id);

    // non floating children
    // cache size ?
    let layout_ops = view.children.iter().map(|e| e.layout_op.clone()).collect();
    let total_size = if layout_dir_is_vertical {
        height
    } else {
        width
    };
    let sizes = view::compute_layout_sizes(total_size, &layout_ops);
    assert_eq!(view.children.len(), sizes.len());

    #[derive(Debug)]
    struct ComposeInfo {
        pub view_id: view::Id,
        pub x: usize,
        pub y: usize,
        pub w: usize,
        pub h: usize,
    }

    let mut compose_info = vec![];

    // - compute child local position and size
    {
        let mut x = 0;
        let mut y = 0;
        let children = view.children.clone();
        for (idx, child) in children.iter().enumerate() {
            let child_v = get_view_by_id(editor, child.id);
            let mut child_v = child_v.write();

            let (w, h) = if layout_dir_is_vertical {
                (width, sizes[idx])
            } else {
                (sizes[idx], height)
            };

            // update global position
            if let (Some(g_x), Some(g_y)) = (view.global_x, view.global_y) {
                child_v.global_x = Some(g_x + x);
                child_v.global_y = Some(g_y + y);
            };

            child_v.x = x;
            child_v.y = y;
            child_v.width = w;
            child_v.height = h;

            compose_info.push(ComposeInfo {
                view_id: child.id,
                x,
                y,
                w,
                h,
            }); // not sorted yet

            if layout_dir_is_vertical {
                y += h;
            } else {
                x += w;
            }
        }
    }

    // - we will compose based on sibling dependencies/priority ( non floating children)
    // - sort views based on depth/priority (children)
    compose_info.sort_by(|idxa, idxb| {
        let vida = idxa.view_id;
        let vidb = idxb.view_id;

        let va = get_view_by_id(editor, vida);
        let vb = get_view_by_id(editor, vidb);

        let pa = va.read().compose_priority;
        let pb = vb.read().compose_priority;

        pb.cmp(&pa)
    });

    // - add floating children (not yet sorted)
    let floating_children = view.floating_children.clone();
    for (_idx, child) in floating_children.iter().enumerate() {
        let child_v = get_view_by_id(editor, child.id);
        let child_v = child_v.read();

        let x = child_v.x;
        let y = child_v.y;
        let w = child_v.width;
        let h = child_v.height;

        compose_info.push(ComposeInfo {
            view_id: child.id,
            x,
            y,
            w,
            h,
        }); // just append no sort
    }

    // - notify modes via ViewEvent::PreComposition event
    notify_children(
        editor,
        editor_env,
        &mut view,
        &all_children,
        ViewEvent::PreComposition,
    );

    // - call run_compositing_stage_direct for each child
    for info in &compose_info {
        let (x, y) = (info.x, info.y);
        let (w, h) = (info.w, info.h);

        if w == 0 || h == 0 {
            continue;
        }

        let vid = info.view_id;

        dbg_println!("compose child view {:?}", vid);

        let child_view_rc = get_view_by_id(editor, vid);

        let start_offset = {
            let child_v = child_view_rc.write();
            child_v.start_offset
        };

        {
            let (w, h) = (info.w, info.h);

            // alloc new screen or clear ?
            let mut do_clear = true;
            let child_screen = {
                let mut child_v = child_view_rc.write();
                let dim = {
                    let screen = child_v.screen.read();
                    screen.dimension()
                };
                if dim.0 != w || dim.1 != h {
                    child_v.screen = Arc::new(RwLock::new(Box::new(Screen::new(w, h))));
                    child_v.width = w;
                    child_v.height = h;
                    do_clear = false;
                }

                child_v.screen.clone()
            };

            let mut child_screen = child_screen.write();

            if do_clear {
                child_screen.clear();
            }

            run_compositing_stage_direct(
                editor,
                editor_env,
                &child_view_rc,
                start_offset,
                max_offset,
                &mut child_screen,
                pass_mask,
            );
        }

        {
            // copy child to (parent's) output screen
            let subscribers = {
                let mut child_v = child_view_rc.write();
                let last_offset = {
                    let child_screen = child_v.screen.as_ref().read();

                    dbg_println!("copy child {:?} screen at x({}) y({})", vid, x, y);
                    dbg_println!(
                        " child screen w({}) h({})",
                        child_screen.width(),
                        child_screen.height()
                    );

                    screen.copy_screen_at_xy(&child_screen, x, y);
                    child_screen.last_offset
                };

                child_v.end_offset = last_offset.unwrap_or(0);
                child_v.subscribers.clone()
            };

            //
            // NB: notify subscribers just after composition
            // use View::compose_priority to order notifications
            //
            // NOTE(ceg): currently we do not have event filters
            if !screen.is_off_screen {
                let mut child_v = child_view_rc.write();

                for cb in subscribers.iter() {
                    let mode = cb.0.as_ref();

                    if cb.1.id == cb.2.id {
                        // ignore self registration
                        continue;
                    }

                    dbg_println!("call mode {} on_view_event ", mode.borrow().name());

                    mode.borrow().on_view_event(
                        editor,
                        editor_env,
                        cb.1,
                        cb.2,
                        &ViewEvent::PostComposition,
                        &mut child_v,
                        None,
                    );
                }
            }
        }
    }

    true
}

// This function can be considered as the core of the editor.<br/>
// It will run the configured filters until the screen is filled or eof is reached.<br/>
// the screen MUST be cleared first (for LayoutPass::ScreenContent,  LayoutPass::ScreenContentAndOverlay)
//
// TODO(ceg): we ca precompute the rendering order
// by sorting the Views.
// ie: if View(id=A) depends on the content of View(id=B)
// we render B first etc..
//
//
pub fn run_compositing_stage_direct(
    editor: &mut Editor<'static>,
    editor_env: &mut EditorEnv<'static>,
    view: &Rc<RwLock<View<'static>>>,
    base_offset: u64, // default view.start_offset start -> Option<u64>
    max_offset: u64,  // default view.buffer.size()   end  -> Option<u64>
    screen: &mut Screen,
    pass_mask: LayoutPass,
) {
    // check screen size
    if screen.width() == 0 || screen.height() == 0 {
        return;
    }

    let current_view_id = {
        dbg_println!(
            "[START] COMPOSE VID {:?} tags {:?} ",
            view.read().id,
            view.read().tags
        );

        view.read().id
    };

    // Render parent before children
    // if no filter configured, will do nothing
    let active_view_id = editor_env.active_view.unwrap_or(view::Id(0));
    //    let target_view_id = editor_env.target_view.unwrap_or(active_view_id);

    let mut layout_env = LayoutEnv {
        graphic_display: editor_env.graphic_display,
        quit: false,
        base_offset,
        max_offset,
        screen,
        active_view_id,
        current_view_id,
    };

    // screen must be cleared by caller
    if pass_mask == LayoutPass::ScreenContent || pass_mask == LayoutPass::ScreenContentAndOverlay {
        assert_eq!(0, layout_env.screen.push_count());
    }

    layout_env.screen.check_invariants();

    let mut time_spent: Vec<u128> = vec![];

    if pass_mask == LayoutPass::ScreenContent || pass_mask == LayoutPass::ScreenContentAndOverlay {
        run_content_filters(
            editor,
            editor_env,
            &mut layout_env,
            &mut time_spent,
            view,
            None,
        );
    }

    if pass_mask == LayoutPass::ScreenOverlay || pass_mask == LayoutPass::ScreenContentAndOverlay {
        run_screen_overlay_filters(
            editor,
            editor_env,
            &mut layout_env,
            &mut time_spent,
            view,
            None,
        );
    }

    {
        dbg_println!("[START] RENDER CHILDREN of VID {:?}, ", view.read().id);
    }

    // Render children/View
    // (recursive) children compositing
    compose_children(
        editor,
        editor_env,
        view,
        base_offset,
        max_offset,
        screen,
        pass_mask,
    );

    {
        dbg_println!("[END] COMPOSE VID {:?}, ", view.read().id);
    }
}

fn run_content_filters(
    editor: &mut Editor<'static>,
    editor_env: &mut EditorEnv<'static>,

    layout_env: &mut LayoutEnv,
    time_spent: &mut Vec<u128>,
    view: &Rc<RwLock<View>>,
    parent_view: Option<&View<'static>>,
) {
    // setup
    let (filters, filter_in, filter_out) = {
        let v = view.read();
        let filter_in = v.filter_in.clone();
        let filter_out = v.filter_out.clone();
        let filters = v.compose_content_filters.clone();
        (filters, filter_in, filter_out)
    };

    time_spent.resize(filters.borrow().len(), 0);

    for (idx, f) in filters.borrow_mut().iter_mut().enumerate() {
        //dbg_println!("setup {}", f.name());
        let t0 = std::time::Instant::now();
        f.setup(editor, editor_env, layout_env, view, parent_view);
        let t1 = std::time::Instant::now();
        let diff = (t1 - t0).as_micros();
        time_spent[idx] += diff;
    }

    let mut filters = filters.borrow_mut();
    if filters.is_empty() {
        layout_env.quit = true;
    }

    let mut filter_in = filter_in.borrow_mut();
    let mut filter_out = filter_out.borrow_mut();
    filter_in.clear();

    let mut loop_count = 0;

    // is interactive rendering possible ?

    let view = view.read();
    layout_env.quit = filters.is_empty();
    while !layout_env.quit {
        loop_count += 1;

        for (idx, f) in filters.iter_mut().enumerate() {
            // always clear filter output
            filter_out.clear();

            if false {
                dbg_println!(
                    "run {:32} : filter_in.len() {})\r",
                    f.name(),
                    filter_in.len()
                );
            }
            let t0 = std::time::Instant::now();

            f.run(&view, layout_env, &filter_in, &mut filter_out);

            let t1 = std::time::Instant::now();

            if false {
                dbg_println!(
                    "run {:32} : filter_out.len() {})\r",
                    f.name(),
                    filter_out.len()
                );
            }

            let diff = (t1 - t0).as_micros();
            time_spent[idx] += diff;

            // pre loop stats
            if false {
                dbg_println!(
                    "time spent in {:32} : {:4} µs (inner loop {})\r",
                    f.name(),
                    diff,
                    loop_count
                );
            }

            // TODO(ceg): only in debug mode
            if false {
                for i in filter_out.iter() {
                    i.check_invariants();
                }
            }

            // swap input/output for next filter
            // current output is next filter input
            std::mem::swap(&mut filter_in, &mut filter_out);
        }
    }

    for (idx, f) in filters.iter_mut().enumerate() {
        let t0 = std::time::Instant::now();
        f.finish(&view, layout_env);
        let t1 = std::time::Instant::now();
        let diff = (t1 - t0).as_micros();
        time_spent[idx] += diff;
    }

    let mut total_time = 0;
    for (idx, f) in filters.iter_mut().enumerate() {
        dbg_println!("time spent in {:32} : {:4} µs\r", f.name(), time_spent[idx]);
        total_time += time_spent[idx];
    }

    dbg_println!(
        "total time spent in content filter pipeline: {} µs, loop_count {}\r",
        total_time,
        loop_count
    );
}

fn run_screen_overlay_filters(
    editor: &Editor<'static>,
    _editor_env: &EditorEnv<'static>,
    layout_env: &mut LayoutEnv,
    time_spent: &mut Vec<u128>,
    view: &Rc<RwLock<View>>,
    parent_view: Option<&View<'static>>,
) {
    // setup
    let filters = {
        let v = view.read();
        v.compose_screen_overlay_filters.clone()
    };

    time_spent.resize(filters.borrow().len(), 0);

    for (idx, f) in filters.borrow_mut().iter_mut().enumerate() {
        //dbg_println!("setup {}", f.name());
        let t0 = std::time::Instant::now();
        f.setup(editor, layout_env, view, parent_view);
        let t1 = std::time::Instant::now();
        let diff = (t1 - t0).as_micros();
        time_spent[idx] += diff;
    }

    let mut filters = filters.borrow_mut();
    if filters.is_empty() {
        layout_env.quit = true;
    }

    // is interactive rendering possible ?

    let view = view.read();

    for (idx, f) in filters.iter_mut().enumerate() {
        let t0 = std::time::Instant::now();
        f.run(&view, layout_env);
        let t1 = std::time::Instant::now();
        let diff = (t1 - t0).as_micros();
        time_spent[idx] += diff;
    }

    for (idx, f) in filters.iter_mut().enumerate() {
        let t0 = std::time::Instant::now();
        f.finish(&view, layout_env);
        let t1 = std::time::Instant::now();
        let diff = (t1 - t0).as_micros();
        time_spent[idx] += diff;
    }

    let mut total_time = 0;
    for (idx, f) in filters.iter_mut().enumerate() {
        dbg_println!("time spent in {:32} : {:4} µs\r", f.name(), time_spent[idx]);
        total_time += time_spent[idx];
    }

    dbg_println!(
        "total time spent in screen overlay filter pipeline: {} µs\r",
        total_time
    );
}
