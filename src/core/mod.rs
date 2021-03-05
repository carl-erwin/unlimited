// Copyright (c) Carl-Erwin Griffith

use core::panic;
//
use std::sync::mpsc::channel;
use std::sync::mpsc::Receiver;
use std::sync::mpsc::Sender;
use std::thread;

use std::cell::RefCell;
use std::rc::Rc;

#[macro_use]
pub(crate) mod macros;

pub mod buffer;
pub mod bufferlog;
pub mod codec;
pub mod codepointinfo;
pub mod config;
pub mod document;
pub mod editor;
pub mod event;
pub mod mapped_file;
pub mod mark;
pub mod modes;
pub mod screen;
pub mod view;

use crate::core::config::Config;
use crate::core::editor::Editor;
use crate::core::editor::EditorEnv;
use crate::core::event::Event;
use crate::core::event::EventMessage;
use crate::core::view::View;

pub const VERSION: &str = env!("CARGO_PKG_VERSION");

/// This thread is the "❤" of unlimited.
pub fn run<'a>(
    config: Config,
    core_rx: &Receiver<EventMessage<'static>>,
    core_tx: &Sender<EventMessage<'static>>,
    ui_tx: &Sender<EventMessage<'static>>,
) {
    let (worker_tx, worker_rx) = channel();

    let mut editor = Editor::new(config, core_tx.clone(), ui_tx.clone(), worker_tx.clone());
    let mut env = EditorEnv::new();

    // create worker thread
    let worker_th = {
        let core_tx = core_tx.clone();
        Some(thread::spawn(move || worker(&worker_rx, &core_tx)))
    };

    editor.worker_tx = worker_tx.clone();
    load_files(&mut editor); // document
    load_modes(&mut editor, &mut env);
    editor::main_loop(&mut editor, &mut env, &core_rx, &ui_tx);

    // wait for worker thread
    if let Some(worker_handle) = worker_th {
        worker_handle.join().unwrap()
    }
}

/////////////////////// worker.rs

pub fn worker(
    worker_rx: &Receiver<EventMessage<'static>>,
    _core_tx: &Sender<EventMessage<'static>>,
) {
    dbg_println!("[starting worker thread]");
    loop {
        if let Ok(evt) = worker_rx.recv() {
            match evt.event {
                Event::ApplicationQuitEvent => {
                    dbg_println!("[stopping worker thread]");
                    break;
                }

                Event::SyncTask { doc } => {
                    document::sync_to_storage(&doc);
                }

                _ => {
                    panic!("worker thread received an unexpected message");
                }
            }
        }
    }
}

use crate::core::document::DocumentBuilder;

/// TODO: replace this by load/unload doc functions
/// the ui will open the documents on demand
pub fn load_files(editor: &mut Editor) {
    let mut id = editor.document_map.len() as u64;

    for f in &editor.config.files_list {
        let b = DocumentBuilder::new()
            .document_name(f)
            .file_name(f)
            .internal(false)
            .finalize();

        if let Some(b) = b {
            editor.document_map.insert(id, b);
            id += 1;
        }
    }

    // default buffer ?
    if editor.document_map.is_empty() {
        // edit.get_untitled_count() -> 1

        let b = DocumentBuilder::new()
            .document_name("untitled-1")
            .file_name("/dev/null")
            .internal(false)
            .finalize();
        if let Some(b) = b {
            editor.document_map.insert(id, b);
            id += 1;
        }
    }

    dbg_println!("id {}", id);

    // create default views
    for doc_id in 0..editor.document_map.len() {
        let id = doc_id as u64;
        let doc = editor.document_map.get(&id);
        if let Some(doc) = doc {
            let view = View::new(None, 0 as u64, 1, 1, Some(doc.clone()));
            dbg_println!("create view id {}", view.id);
            editor.view_map.insert(view.id, Rc::new(RefCell::new(view)));
        }
    }
}

use crate::core::modes::TextMode;

pub fn load_modes(editor: &mut Editor, env: &mut EditorEnv) {
    // set default mode(s)
    editor.register_mode(Box::new(TextMode::new()));

    for (_name, mode) in editor.modes.iter() {
        //  TOOD: pre/post input stage
        // register_mode_input_stage(mode);
        let action_map = mode.build_action_map();
        for (k, v) in action_map {
            env.action_map.insert(k.clone(), v.clone());
        }

        // TODO: pre/post "render" stage
        // register render "stage" function
        // let action_map = mode.build_render_stage_map();
        // for (k, v) in action_map {
        //     env.render_stage_map.insert(k.clone(), v.clone());
        // }

        // create view's mode context
        // allocate per view ModeCtx shared between the stages
        for (_k, v) in editor.view_map.iter() {
            let mut v = v.borrow_mut();

            dbg_println!("v.id = {}", v.id);

            let ctx = mode.alloc_ctx();
            v.set_mode_ctx(mode.name(), ctx);

            mode.configure_view(&mut v);
        }
    }
}
