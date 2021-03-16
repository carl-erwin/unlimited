// Copyright (c) Carl-Erwin Griffith

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

use crate::core::view;
use crate::core::view::View;

// TODO remove this impl details

//
pub struct LayoutEnv<'a> {
    pub graphic_display: bool,
    pub quit: bool,
    pub base_offset: u64,
    pub max_offset: u64,
    pub screen: &'a mut Screen,
}

// TODO: add ?
//        doc,
//        view

pub trait Filter<'a> {
    fn name(&self) -> &'static str;

    fn setup(&mut self, env: &LayoutEnv, _view: &View);

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
        // default
        *output = input.clone();
    }

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

    pub is_selected: bool,
    pub color: (u8, u8, u8),
    pub bg_color: (u8, u8, u8),

    pub offset: Option<u64>,
    pub size: usize,

    pub data: FilterData,
    // TODO: add style infos ?
}

impl FilterIo {
    pub fn replace_codepoint(io: &FilterIo, new_cp: char) -> FilterIo {
        if let &FilterIo {
            // general info
            metadata,
            is_selected,
            color,
            bg_color,
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
                is_selected,
                offset: from_offset,
                color,
                bg_color,
                size: cp_size,
                data: FilterData::Unicode {
                    real_cp: new_cp as u32,
                    displayed_cp: real_cp,
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
) {
    {
        let view = view.borrow();
        run_compositing_stage_direct(editor, env, &view, base_offset, max_offset, screen)
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
) -> bool {
    if view.children.len() == 0 {
        return false;
    }

    dbg_println!("COMPOSE CHILDREN OF VID {}", view.id);

    // vertically
    let split_is_vertical = view.layout_direction == view::LayoutDirection::Vertical;

    let (width, height) = (screen.width(), screen.height());
    if width == 0 || height == 0 {
        return false;
    }

    // cache size ?
    let sizes = if split_is_vertical {
        view::compute_layout_sizes(width, &view.layout_ops)
    } else {
        view::compute_layout_sizes(height, &view.layout_ops)
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
        let (w, h) = if split_is_vertical {
            (sizes[idx], height)
        } else {
            (width, sizes[idx])
        };

        compose_idx.push((idx, (x, y), (w, h))); // to sort later

        // TODO: resize instead of replace
        let child_screen = Screen::new(w, h);
        child_v.screen = Arc::new(RwLock::new(Box::new(child_screen)));

        if split_is_vertical {
            x += w;
        } else {
            y += h;
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
            let (w, h) = if split_is_vertical {
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
    );
    if draw {
        return;
    }

    dbg_println!("COMPOSE VID {}", view.id);

    // Draw Leaf View
    let mut layout_env = LayoutEnv {
        graphic_display: editor_env.graphic_display,
        quit: false,
        base_offset,
        max_offset,
        screen,
    };

    // screen must be cleared by caller
    assert_eq!(0, layout_env.screen.push_count());

    // setup
    let mut compose_filters = view.compose_filters.borrow_mut();

    let mut filter_in = Vec::with_capacity(layout_env.screen.width() * layout_env.screen.height());
    let mut filter_out = Vec::with_capacity(layout_env.screen.width() * layout_env.screen.height());

    // TODO
    for f in compose_filters.iter_mut() {
        f.setup(&mut layout_env, &view);
    }

    if compose_filters.len() == 0 {
        layout_env.quit = true;
    }

    layout_env.screen.check_invariants();

    // is interactive rendering possible ?
    while layout_env.quit == false {
        for f in compose_filters.iter_mut() {
            filter_out.clear();

            if false {
                dbg_println!(
                    "running {:32} : in({}) out({})",
                    f.name(),
                    filter_in.len(),
                    filter_out.len()
                );
            }
            f.run(&view, &mut layout_env, &filter_in, &mut filter_out);
            for i in &filter_out {
                i.check_invariants();
            }

            std::mem::swap(&mut filter_in, &mut filter_out);
        }
    }

    for f in compose_filters.iter_mut() {
        f.finish(&view, &mut layout_env);
    }
}
