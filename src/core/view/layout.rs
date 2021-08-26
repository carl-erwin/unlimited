/* DO NOT SPLIT THIS FILE YET: the filter apis are not stable enough */

use std::char;
use std::sync::RwLock;

use std::rc::Rc;
use std::sync::Arc;

//

use crate::dbg_println;

use crate::core::screen::Screen;

use crate::core::editor::Editor;
use crate::core::editor::EditorEnv;

use crate::core::codepointinfo::TextStyle;
use crate::core::view;
use crate::core::view::View;
use crate::core::view::ViewEvent;

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

// TODO(ceg): add ?
//        doc,
//        view
//
//  input_mime_type() -> &str "" | ""
//  output_mime_type() -> &str "application/octet-stream"

pub trait ContentFilter<'a> {
    fn name(&self) -> &'static str;

    fn setup(&mut self, _editor: &Editor<'static>, _env: &mut LayoutEnv, _view: &Rc<RwLock<View>>) {
        /* default implementation is empty*/
    }

    fn run_managed(
        &mut self,
        view: &Rc<RwLock<View>>,
        mut env: &mut LayoutEnv,
        input: &Vec<FilterIo>,
        output: &mut Vec<FilterIo>,
    ) -> () {
        let mut view = view.read().unwrap();
        self.run(&mut view, &mut env, input, output);
    }

    fn run(
        &mut self,
        _view: &View,
        _env: &mut LayoutEnv,
        _input: &Vec<FilterIo>,
        _output: &mut Vec<FilterIo>,
    ) -> () {
        //*output = input.clone();
    }

    fn finish(&mut self, _view: &View, _env: &mut LayoutEnv) -> () {
        // default
    }
}

pub trait ScreenOverlayFilter<'a> {
    fn name(&self) -> &'static str;

    fn setup(&mut self, _editor: &Editor, _env: &mut LayoutEnv, _view: &Rc<RwLock<View>>) {
        /* default implementation is empty*/
    }

    fn run_managed(&mut self, view: &Rc<RwLock<View>>, mut env: &mut LayoutEnv) -> () {
        let mut view = view.read().unwrap();
        self.run(&mut view, &mut env);
    }

    fn run(&mut self, _view: &View, _env: &mut LayoutEnv) -> () {}

    fn finish(&mut self, _view: &View, _env: &mut LayoutEnv) -> () {
        // default
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Unicode {
    pub size: u32,
    pub cp: u32,
}

// content_type == unicode
#[derive(Debug, Clone)]
pub enum FilterData {
    EndOfStream,
    StreamLimitReached, // user custom limit

    ByteArray { vec: Vec<u8> },

    UnicodeArray { vec: Vec<Unicode> },

    // text array ?
    TextInfo { real_cp: u32, displayed_cp: u32 },
}

#[derive(Debug, Clone)]
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
    editor: &mut Editor<'static>,
    env: &mut EditorEnv<'static>,
    view: &Rc<RwLock<View>>,
    base_offset: u64, // default view.start_offset start -> Option<u64>
    max_offset: u64,  // default view.doc.size()   end  -> Option<u64>
    screen: &mut Screen,
    pass_mask: LayoutPass,
) {
    run_compositing_stage_direct(
        editor,
        env,
        &view,
        base_offset,
        max_offset,
        screen,
        pass_mask,
    );

    let mut view = view.write().unwrap();
    if let Some(offset) = screen.last_offset {
        view.end_offset = offset;
    }
}

fn compose_children(
    mut editor: &mut Editor<'static>,
    mut editor_env: &mut EditorEnv<'static>,
    view: &Rc<RwLock<View>>,
    _base_offset: u64, // default view.start_offset start -> Option<u64>
    max_offset: u64,   // default view.doc.size()   end  -> Option<u64>
    screen: &mut Screen,
    pass_mask: LayoutPass,
) -> bool {
    let view = view.read().unwrap();
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
        let mut child_v = editor.view_map.get(vid).unwrap().write().unwrap();
        let (w, h) = if layout_dir_is_vertical {
            (width, sizes[idx])
        } else {
            (sizes[idx], height)
        };

        child_v.x = x;
        child_v.y = y;
        child_v.width = w;
        child_v.height = h;

        compose_idx.push((idx, (x, y), (w, h))); // to sort later

        if layout_dir_is_vertical {
            y += h;
        } else {
            x += w;
        }
    }

    // TODO(ceg): sort based on depth/priority
    compose_idx.sort_by(|idxa, idxb| {
        let vida = view.children[idxa.0];
        let vidb = view.children[idxb.0];

        let va = Rc::clone(editor.view_map.get(&vida).unwrap());
        let vb = Rc::clone(editor.view_map.get(&vidb).unwrap());

        let pa = va.read().unwrap().compose_priority;
        let pb = vb.read().unwrap().compose_priority;
        eprintln!("pa vid {} priority: {:?}", vida, pa);
        eprintln!("pb vid {} priority: {:?}", vidb, pb);
        eprintln!("pa.cmp(&pb) {:?}", pb.cmp(&pa));

        pb.cmp(&pa)
    });
    //

    eprintln!("COMPOSE sub VIDs indexes {:?}, ", compose_idx);

    for info in &compose_idx {
        let idx = info.0;
        let (x, y) = info.1;
        let (_w, _h) = info.2;
        if sizes[idx] == 0 {
            continue;
        }

        eprintln!("COMPOSE VID index {:?}, ", idx);

        let vid = view.children[idx];

        let child_rc = editor.view_map.get(&vid).clone();
        let child_rc = child_rc.unwrap().clone();

        let start_offset = {
            let child_v = child_rc.write().unwrap();
            child_v.start_offset
        };
        {
            let (w, h) = if layout_dir_is_vertical {
                (width, sizes[idx])
            } else {
                (sizes[idx], height)
            };

            assert!(w > 0);
            assert!(h > 0);

            // TODO(ceg): resize instead of replace
            let mut child_screen = Screen::new(w, h);
            run_compositing_stage_direct(
                editor,
                editor_env,
                &child_rc,
                start_offset,
                max_offset,
                &mut child_screen,
                pass_mask,
            );

            {
                let mut child_v = child_rc.write().unwrap();
                child_v.screen = Arc::new(RwLock::new(Box::new(child_screen)));
            }
        }

        {
            let cbs = {
                let mut child_v = child_rc.write().unwrap();
                let last_offset = {
                    let child_screen = child_v.screen.as_ref().read().unwrap();
                    // composition: copy child to (parent's) output screen
                    screen.copy_screen_at_xy(&child_screen, x, y);
                    child_screen.last_offset
                };

                child_v.end_offset = last_offset.unwrap_or(0);
                child_v.subscribers.clone()
            };

            //
            // NB: notify subscriberss just after composition
            // use View::compose_priority to order notifications
            //
            // NOTE(ceg): currently we do not have event filters
            {
                for cb in cbs.iter() {
                    let mode = cb.0.as_ref();
                    mode.on_view_event(
                        &mut editor,
                        &mut editor_env,
                        cb.1,
                        cb.2,
                        &ViewEvent::PostComposition,
                    );
                }
            }
        }
    }

    true
}

// This function can be considered as the core of the editor.<br/>
// It will run the configured filters until the screen is filled or eof is reached.<br/>
// the screen should be cleared first
pub fn run_compositing_stage_direct(
    mut editor: &mut Editor<'static>,
    mut editor_env: &mut EditorEnv<'static>,
    view: &Rc<RwLock<View>>,
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
        &mut editor,
        &mut editor_env,
        &view,
        base_offset,
        max_offset,
        &mut screen,
        pass_mask,
    );
    if draw {
        return;
    }

    {
        eprintln!("COMPOSE VID {:?}, ", view.read().unwrap().id);
    }

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

    layout_env.screen.check_invariants();

    let mut time_spent: Vec<u128> = vec![];

    if pass_mask == LayoutPass::Content || pass_mask == LayoutPass::ContentAndScreenOverlay {
        run_content_filters(&editor, &mut time_spent, &view, &mut layout_env);
    }

    if pass_mask == LayoutPass::ScreenOverlay || pass_mask == LayoutPass::ContentAndScreenOverlay {
        run_screen_overlay_filters(&editor, &mut time_spent, &view, &mut layout_env);
    }
}

fn run_content_filters(
    editor: &Editor<'static>,
    time_spent: &mut Vec<u128>,
    view: &Rc<RwLock<View>>,
    mut layout_env: &mut LayoutEnv,
) {
    // setup
    let (filters, filter_in, filter_out) = {
        let v = view.read().unwrap();
        let filter_in = v.filter_in.clone();
        let filter_out = v.filter_out.clone();
        let filters = v.compose_content_filters.clone();
        (filters, filter_in, filter_out)
    };
    for f in filters.borrow_mut().iter_mut() {
        //dbg_println!("setup {}", f.name());
        f.setup(&editor, &mut layout_env, &view);
    }

    let mut filters = filters.borrow_mut();
    if filters.len() == 0 && filters.len() == 0 {
        layout_env.quit = true;
    }

    time_spent.resize(filters.len(), 0);

    let mut filter_in = filter_in.borrow_mut();
    let mut filter_out = filter_out.borrow_mut();
    filter_in.clear();

    let mut loop_count = 0;

    // is interactive rendering possible ?

    let view = view.read().unwrap();
    layout_env.quit = false;
    while layout_env.quit == false {
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

            f.run(&view, &mut layout_env, &filter_in, &mut filter_out);

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
        f.finish(&view, &mut layout_env);
        let t1 = std::time::Instant::now();
        let diff = (t1 - t0).as_micros();
        time_spent[idx] += diff;
    }

    //    return;

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
    time_spent: &mut Vec<u128>,
    view: &Rc<RwLock<View>>,
    mut layout_env: &mut LayoutEnv,
) {
    // setup
    let filters = {
        let v = view.read().unwrap();
        v.compose_screen_overlay_filters.clone()
    };
    for f in filters.borrow_mut().iter_mut() {
        //dbg_println!("setup {}", f.name());
        f.setup(&editor, &mut layout_env, &view);
    }

    let mut filters = filters.borrow_mut();
    if filters.len() == 0 && filters.len() == 0 {
        layout_env.quit = true;
    }

    time_spent.resize(filters.len(), 0);

    // is interactive rendering possible ?

    let view = view.read().unwrap();

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

    //    return;

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
