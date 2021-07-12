/* DO NOT SPLIT THIS FILE YET: the filter apis are not stable enough */

use std::cell::RefCell;
use std::char;

use std::rc::Rc;
use std::sync::Arc;
use std::sync::RwLock;

//

use crate::dbg_println;

use crate::core::screen::Screen;

use crate::core::editor::Editor;
use crate::core::editor::EditorEnv;

use crate::core::codepointinfo::TextStyle;
use crate::core::view;
use crate::core::view::View;

// TODO remove this impl details

#[derive(Debug, Clone, Copy, Eq, PartialEq)]
pub enum LayoutPass {
    Content = 1,
    ScreenOverlay = 2,
    ContentAndScreenOverlay = 3,
}

//
pub struct LayoutEnv<'a> {
    pub graphic_display: bool,
    pub quit: bool,
    pub base_offset: u64,
    pub max_offset: u64,
    pub screen: &'a mut Screen,
    pub focus_vid: view::Id,
}

// TODO: add ?
//        doc,
//        view

pub trait ContentFilter<'a> {
    fn name(&self) -> &'static str;

    fn setup(&mut self, env: &mut LayoutEnv, _view: &View);

    fn run_managed(
        &mut self,
        view: &Rc<RefCell<View>>,
        mut env: &mut LayoutEnv,
        input: &Vec<FilterIo>,
        output: &mut Vec<FilterIo>,
    ) -> () {
        let mut view = view.borrow();
        self.run(&mut view, &mut env, input, output);
    }

    fn run(
        &mut self,
        _view: &View,
        _env: &mut LayoutEnv,
        input: &Vec<FilterIo>,
        output: &mut Vec<FilterIo>,
    ) -> () {
        //*output = input.clone();
    }

    fn finish(&mut self, _view: &View, _env: &mut LayoutEnv) -> () {
        // default
    }
}

pub trait ScreenOverlayFilter<'a> {
    fn name(&self) -> &'static str;

    fn setup(&mut self, env: &mut LayoutEnv, _view: &View);

    fn run_managed(&mut self, view: &Rc<RefCell<View>>, mut env: &mut LayoutEnv) -> () {
        let mut view = view.borrow();
        self.run(&mut view, &mut env);
    }

    fn run(&mut self, _view: &View, _env: &mut LayoutEnv) -> () {}

    fn finish(&mut self, _view: &View, _env: &mut LayoutEnv) -> () {
        // default
    }
}

// content_type == unicode
#[derive(Debug, Clone)]
pub enum FilterData {
    EndOfStream,

    ByteArray {
        vec: Vec<u8>,
    },

    Byte {
        val: u8,
    },

    Unicode {
        real_cp: u32,
        displayed_cp: u32,
        fragment_flag: bool,
        fragment_count: u32,
    },

    // codec_change
    CodecInfo {
        codec_id: u32,
        codec_context_id: u64, //
    },
}

#[derive(Debug, Clone)]
pub struct FilterIo {
    // general info
    pub metadata: bool,
    pub style: TextStyle,
    //
    pub offset: Option<u64>,
    pub size: usize,
    //
    pub data: FilterData,
    // TODO: add style infos ?
}

impl FilterIo {
    pub fn replace_displayed_codepoint(io: &FilterIo, disp_cp: char) -> FilterIo {
        if let &FilterIo {
            // general info
            metadata,
            style,
            offset: from_offset,
            size: cp_size,
            data:
                FilterData::Unicode {
                    real_cp,
                    fragment_flag,
                    fragment_count,
                    ..
                },
        } = io
        {
            return FilterIo {
                // general info
                metadata,
                style,
                offset: from_offset,
                size: cp_size,
                data: FilterData::Unicode {
                    real_cp,
                    displayed_cp: disp_cp as u32,
                    fragment_flag,
                    fragment_count,
                },
            };
        }

        io.clone()
    }

    pub fn check_invariants(&self) {
        if self.size > 0 && self.metadata == true {
            dbg_println!("INVALID IO [METADATA] {:?}", self);
            panic!("");
        }
        if self.size == 0 && self.metadata == false {
            dbg_println!("INVALID IO [NON META] {:?}", self);
            panic!("");
        }
    }
}

pub fn run_compositing_stage(
    editor: &Editor,
    env: &EditorEnv,
    view: &Rc<RefCell<View>>,
    base_offset: u64, // default view.start_offset start -> Option<u64>
    max_offset: u64,  // default view.doc.size()   end  -> Option<u64>
    screen: &mut Screen,
    pass_mask: LayoutPass,
) {
    {
        let view = view.borrow();
        run_compositing_stage_direct(
            editor,
            env,
            &view,
            base_offset,
            max_offset,
            screen,
            pass_mask,
        )
    }
    {
        let mut view = view.borrow_mut();
        if let Some(offset) = screen.last_offset {
            view.end_offset = offset;
        }
    }
}

// This function can be considered as the core of the editor.<br/>
// It will run the configured filters until the screen is filled or eof is reached.<br/>
// the screen should be cleared first
// TODO: pass list of filter function to be applied
// 0 - allocate context for each configured plugin
// 1 - utf8 || hexa
// 2 - highlight (some) keywords
// 3 - highlight selection
//  4 - tabulation
//  5 - word wrap
fn compose_children(
    editor: &Editor,
    editor_env: &EditorEnv,
    view: &View,
    _base_offset: u64, // default view.start_offset start -> Option<u64>
    max_offset: u64,   // default view.doc.size()   end  -> Option<u64>
    screen: &mut Screen,
    pass_mask: LayoutPass,
) -> bool {
    if view.children.len() == 0 {
        return false;
    }

    dbg_println!("COMPOSE CHILDREN OF VID {}", view.id);

    // split direction
    let layout_dir_is_vertical = view.layout_direction == view::LayoutDirection::Vertical;

    let (width, height) = (screen.width(), screen.height());
    if width == 0 || height == 0 {
        return false;
    }

    // cache size ?
    let sizes = if layout_dir_is_vertical {
        view::compute_layout_sizes(height, &view.layout_ops)
    } else {
        view::compute_layout_sizes(width, &view.layout_ops)
    };

    dbg_println!(
        "ITER over VID {}, CHILDREN {:?}, size {:?}",
        view.id,
        view.children,
        sizes
    );

    assert_eq!(view.children.len(), sizes.len());

    let mut compose_idx = vec![];
    // 1 - compute position and size
    // 2 - compose based on sibling dependencies/priority
    let mut x = 0;
    let mut y = 0;
    for (idx, vid) in view.children.iter().enumerate() {
        let mut child_v = editor.view_map.get(vid).unwrap().borrow_mut();
        child_v.x = x;
        child_v.y = y;
        let (w, h) = if layout_dir_is_vertical {
            (width, sizes[idx])
        } else {
            (sizes[idx], height)
        };

        compose_idx.push((idx, (x, y), (w, h))); // to sort later

        // TODO: resize instead of replace
        let child_screen = Screen::new(w, h);
        child_v.screen = Arc::new(RwLock::new(Box::new(child_screen)));

        if layout_dir_is_vertical {
            y += h;
        } else {
            x += w;
        }
    }

    // TODO: sort based on deps/prio
    compose_idx.sort_by(|idxa, idxb| {
        let vida = view.children[idxa.0];
        let vidb = view.children[idxb.0];

        let va = Rc::clone(editor.view_map.get(&vida).unwrap());
        let vb = Rc::clone(editor.view_map.get(&vidb).unwrap());

        let pa = vb.borrow().compose_priority;
        let pb = va.borrow().compose_priority;
        pb.cmp(&pa)
    });
    //

    dbg_println!("COMPOSE sub VIDs {:?}, ", compose_idx);

    for info in &compose_idx {
        let idx = info.0;
        let (x, y) = info.1;
        let (_w, _h) = info.2;
        if sizes[idx] == 0 {
            continue;
        }

        let vid = view.children[idx];

        let mut child_v = editor.view_map.get(&vid).unwrap().borrow_mut();
        {
            child_v.x = x;
            child_v.y = y;
            let (w, h) = if layout_dir_is_vertical {
                (sizes[idx], height)
            } else {
                (width, sizes[idx])
            };

            assert!(w > 0);
            assert!(h > 0);

            let mut child_screen = child_v.screen.write().unwrap();
            run_compositing_stage_direct(
                editor,
                editor_env,
                &child_v,
                child_v.start_offset,
                max_offset, // TODO take child doc size
                &mut child_screen,
                pass_mask,
            );
        }

        let child_screen = child_v.screen.as_ref().read().unwrap();

        if idx == 0 {
            screen.first_offset = child_screen.first_offset.clone();
        }

        // composition copy child to (parent's) output screen
        screen.copy_to(x, y, &child_screen);
    }

    true
}

// core
pub fn run_compositing_stage_direct(
    editor: &Editor,
    editor_env: &EditorEnv,
    view: &View,
    base_offset: u64, // default view.start_offset start -> Option<u64>
    max_offset: u64,  // default view.doc.size()   end  -> Option<u64>
    mut screen: &mut Screen,
    pass_mask: LayoutPass,
) {
    // check screen size
    if screen.width() == 0 || screen.height() == 0 {
        return;
    }

    // (recursive) children compositing
    let draw = compose_children(
        &editor,
        &editor_env,
        &view,
        base_offset,
        max_offset,
        &mut screen,
        pass_mask,
    );
    if draw {
        return;
    }

    //dbg_println!("COMPOSE VID {}", view.id);

    // Draw Leaf View
    let mut layout_env = LayoutEnv {
        graphic_display: editor_env.graphic_display,
        quit: false,
        base_offset,
        max_offset,
        screen,
        focus_vid: editor_env.focus_on,
    };

    // screen must be cleared by caller
    if pass_mask == LayoutPass::Content || pass_mask == LayoutPass::ContentAndScreenOverlay {
        assert_eq!(0, layout_env.screen.push_count());
    }

    // setup
    let mut compose_content_filters = view.compose_content_filters.borrow_mut();
    let mut compose_screen_overlay_filters = view.compose_screen_overlay_filters.borrow_mut();

    if compose_content_filters.len() == 0 && compose_screen_overlay_filters.len() == 0 {
        layout_env.quit = true;
    }

    layout_env.screen.check_invariants();

    let mut time_spent: Vec<u128> = vec![];
    time_spent.resize(compose_content_filters.len(), 0);

    if pass_mask == LayoutPass::Content || pass_mask == LayoutPass::ContentAndScreenOverlay {
        run_content_filters(
            &mut time_spent,
            &mut compose_content_filters,
            &view,
            &mut layout_env,
        );
    }

    if pass_mask == LayoutPass::ScreenOverlay || pass_mask == LayoutPass::ContentAndScreenOverlay {
        run_screen_overlay_filters(
            &mut time_spent,
            &mut compose_screen_overlay_filters,
            &view,
            &mut layout_env,
        );
    }
}

fn run_content_filters(
    time_spent: &mut Vec<u128>,
    filters: &mut Vec<Box<dyn ContentFilter>>,
    view: &View,
    mut layout_env: &mut LayoutEnv,
) {
    let mut filter_in = view.filter_in.borrow_mut();
    let mut filter_out = view.filter_out.borrow_mut();

    filter_in.clear();

    for f in filters.iter_mut() {
        //dbg_println!("setup {}", f.name());
        f.setup(&mut layout_env, &view);
    }

    let mut loop_count = 0;

    // is interactive rendering possible ?

    layout_env.quit = false;
    while layout_env.quit == false {
        loop_count += 1;

        for (idx, f) in filters.iter_mut().enumerate() {
            filter_out.clear();

            let t0 = std::time::Instant::now();

            f.run(&view, &mut layout_env, &filter_in, &mut filter_out);

            let t1 = std::time::Instant::now();

            let diff = (t1 - t0).as_micros();
            time_spent[idx] += diff;

            if false {
                for i in filter_out.iter() {
                    i.check_invariants();
                }
            }

            std::mem::swap(&mut filter_in, &mut filter_out);
        }
    }

    for (idx, f) in filters.iter_mut().enumerate() {
        let t0 = std::time::Instant::now();
        f.finish(&view, &mut layout_env);
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
        total_time, loop_count
    );
}

fn run_screen_overlay_filters(
    time_spent: &mut Vec<u128>,
    filters: &mut Vec<Box<dyn ScreenOverlayFilter>>,
    view: &View,
    mut layout_env: &mut LayoutEnv,
) {
    for f in filters.iter_mut() {
        //dbg_println!("setup {}", f.name());
        f.setup(&mut layout_env, &view);
    }

    // is interactive rendering possible ?

    // single pass
    for (idx, f) in filters.iter_mut().enumerate() {
        let t0 = std::time::Instant::now();

        f.run(&view, &mut layout_env);

        let t1 = std::time::Instant::now();

        let diff = (t1 - t0).as_micros();
        time_spent[idx] += diff;
    }

    for (idx, f) in filters.iter_mut().enumerate() {
        let t0 = std::time::Instant::now();
        f.finish(&view, &mut layout_env);
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
        "total time spent in screen overlay pipeline: µs {}\r",
        total_time
    );
}
