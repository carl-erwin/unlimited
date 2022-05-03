use core::panic;
//
use std::fs;
use std::sync::mpsc::channel;
use std::sync::mpsc::Receiver;
use std::sync::mpsc::Sender;
use std::thread;

use parking_lot::RwLock;
use std::rc::Rc;
use std::sync::Arc;

#[macro_use]
pub(crate) mod macros;

pub mod codec;
pub mod codepointinfo;
pub mod config;
pub mod document;
pub mod editor;
pub mod event;
pub mod mapped_file;
pub mod modes;
pub mod screen;
pub mod view;

use crate::core::config::Config;
use crate::core::document::Document;
use crate::core::editor::Editor;
use crate::core::editor::EditorEnv;
use crate::core::event::Event;
use crate::core::event::EventMessage;
use crate::core::view::View;

pub const VERSION: &str = env!("CARGO_PKG_VERSION");

use std::sync::atomic::{AtomicUsize, Ordering};

//
pub static DBG_PRINTLN_FLAG: AtomicUsize = AtomicUsize::new(0);

pub fn enable_dbg_println() {
    DBG_PRINTLN_FLAG.store(1, Ordering::Relaxed);
}
pub fn disable_dbg_println() {
    DBG_PRINTLN_FLAG.store(0, Ordering::Relaxed);
}
pub fn toggle_dbg_println() {
    let v = DBG_PRINTLN_FLAG.load(Ordering::Relaxed);
    DBG_PRINTLN_FLAG.store(!v, Ordering::Relaxed);
}

//
pub static USE_READ_CACHE: AtomicUsize = AtomicUsize::new(1);
pub fn disable_read_cache() {
    USE_READ_CACHE.store(0, Ordering::Relaxed);
}

pub fn use_read_cache() -> bool {
    USE_READ_CACHE.load(Ordering::Relaxed) != 0
}

//
pub static USE_BYTE_INDEX: AtomicUsize = AtomicUsize::new(1);
pub fn disable_byte_index() {
    USE_BYTE_INDEX.store(0, Ordering::Relaxed);
}

pub fn use_byte_index() -> bool {
    USE_BYTE_INDEX.load(Ordering::Relaxed) != 0
}

//
pub static BENCH_TO_EOF: AtomicUsize = AtomicUsize::new(0);
pub fn enable_bench_to_eof() {
    BENCH_TO_EOF.store(1, Ordering::Relaxed);
}
pub fn bench_to_eof() -> bool {
    BENCH_TO_EOF.load(Ordering::Relaxed) != 0
}

//
pub static RAW_FILTER_TO_SCREEN: AtomicUsize = AtomicUsize::new(0);
pub fn enable_raw_data_filter_to_screen() {
    RAW_FILTER_TO_SCREEN.store(1, Ordering::Relaxed);
}
pub fn raw_data_filter_to_screen() -> bool {
    RAW_FILTER_TO_SCREEN.load(Ordering::Relaxed) != 0
}

//
pub static NO_UI_RENDER: AtomicUsize = AtomicUsize::new(0);
pub fn set_no_ui_render(b: bool) {
    NO_UI_RENDER.store(b as usize, Ordering::Relaxed);
}
pub fn no_ui_render() -> bool {
    NO_UI_RENDER.load(Ordering::Relaxed) != 0
}

/*
 TODO(ceg):

    "core-mode" {
        scrollbar-mode
        v-split-mode
        h-split-mode
    }

text-mode =
    "raw-data"
    "utf8-codec"
    "mark-mode"
    "undo-mode"
    ----------
    "wrap-line"
    "wrap-word"

    --------------
    "replay-mode"

    "tab-mode"
    "selection/high-light"
    "fold-mode"
    "search-mode"
    "regex-mode"

    "exec-bin-mode"

    "dir-mode"

    "shell-mode"

    "ffi-mode"

    "hex-mode"

    "follow-mode"





    split layout.rs -> modes

  document.predefined_modes() Option<vec["internal:welcome-mode"]>
  document.predefined_modes() Option<vec["internal:debug-message"]>

  based on extension we will load predefine modes / keywords list etc ..

    replay-mode
      ctrl+x ctrl+x r :  {left,right} alt+{left,right}
*/

pub static WELCOME_MESSAGE: &str = r#"-*- Welcome to unlimitED! -*-

unlimitED! is an experimental text editor (running in the terminal).


SYNOPSIS
unlimited [options] [file ..]


It comes with:

  - basic UTF-8 support
  - very large file support
  - "infinite" undo/redo
  - multi-cursors
  - mouse selection (graphical terminal)

[Quit]
    Quit:           => ctrl+x ctrl+c

    Quit (no save)  => ctrl+x ctrl+q

    NB: quit will wait for large file(s) sync to storage.


[Moves]
    Left            =>
    Right           =>
    Up              =>
    Down            =>


[Edit]
    ctrl+o          => Open file (TODO)

    ctrl+u          => Undo
    ctrl+r          => Redo

[Selection/Copy/Paste]
    with the keyboard:

    with the mouse (X11 terminal):

[Save]
    ctrl+x ctrl+s   => Save
                    synchronization of large file(s) is done in the background and does not block the ui.



[Document Selection]



NB: unlimitED! comes with ABSOLUTELY NO WARRANTY
"#;

/// This function is the core of the editor.
/// It should be ran in an other thread than the main one (which is kept for ui)
pub fn run<'a>(
    config: Config,
    core_rx: &Receiver<EventMessage<'static>>,
    core_tx: &Sender<EventMessage<'static>>,
    ui_tx: &Sender<EventMessage<'static>>,
) {
    let (worker_tx, worker_rx) = channel();
    let (indexer_tx, indexer_rx) = channel();

    let mut editor = Editor::new(
        config,
        core_tx.clone(),
        ui_tx.clone(),
        worker_tx.clone(),
        indexer_tx.clone(),
    );
    let mut env = EditorEnv::new();

    // create worker thread
    let worker_th = {
        let core_tx = core_tx.clone();
        Some(thread::spawn(move || worker(&worker_rx, &core_tx)))
    };
    let indexer_th = {
        let core_tx = core_tx.clone();
        Some(thread::spawn(move || indexer(&indexer_rx, &core_tx)))
    };

    editor.worker_tx = worker_tx.clone();

    load_modes(&mut editor, &mut env);

    load_files(&mut editor, &mut env);

    create_views(&mut editor, &mut env);

    editor::main_loop(&mut editor, &mut env, &core_rx, &ui_tx);

    // wait for worker thread
    if let Some(worker_handle) = worker_th {
        worker_handle.join().unwrap()
    }
    if let Some(indexer_handle) = indexer_th {
        indexer_handle.join().unwrap()
    }
}

/////////////////////// worker.rs

pub fn worker(
    worker_rx: &Receiver<EventMessage<'static>>,
    core_tx: &Sender<EventMessage<'static>>,
) {
    dbg_println!("[starting worker thread]");
    loop {
        if let Ok(evt) = worker_rx.recv() {
            match evt.event {
                Event::ApplicationQuit => {
                    dbg_println!("[stopping worker thread]");
                    break;
                }

                Event::SyncTask { doc } => {
                    document::sync_to_storage(&doc);

                    let msg = EventMessage::new(0, Event::RefreshView);
                    core_tx.send(msg).unwrap_or(());
                }

                //                Event::OutsourcedTask { task_uid,  editor, editor_env, doc_id, vid, action, params ? } => {
                //                    action();
                //                }
                _ => {
                    panic!("worker thread received an unexpected message");
                }
            }
        }
    }
}

pub fn indexer(
    worker_rx: &Receiver<EventMessage<'static>>,
    core_tx: &Sender<EventMessage<'static>>,
) {
    if !use_byte_index() {
        return;
    }

    dbg_println!("[starting worker thread (indexer)]");
    loop {
        if let Ok(evt) = worker_rx.recv() {
            match evt.event {
                Event::ApplicationQuit => {
                    dbg_println!("[stopping worker thread]");
                    break;
                }

                // TODO(ceg): split in sub-threads/async task
                Event::IndexTask { document_map } => {
                    dbg_println!("[receive index task ]");

                    let mut refresh_ui = false;
                    let map = document_map.read();
                    let mut t0 = std::time::Instant::now();
                    for (_id, doc) in map.iter() {
                        document::build_index(doc);
                        refresh_ui = true;
                        let t1 = std::time::Instant::now();
                        if (t1 - t0).as_millis() > 1000 {
                            // send ui refresh event
                            let msg = EventMessage::new(0, Event::RefreshView);
                            core_tx.send(msg).unwrap_or(());

                            refresh_ui = false;
                            t0 = t1;
                        }
                    }

                    // last ui refresh
                    if refresh_ui {
                        let msg = EventMessage::new(0, Event::RefreshView);
                        core_tx.send(msg).unwrap_or(());
                    }
                }

                _ => {
                    panic!("worker thread received an unexpected message");
                }
            }
        }
    }
}

use crate::core::document::DocumentBuilder;

/// TODO(ceg): replace this by load/unload doc functions
/// the ui will open the documents on demand
pub fn load_files(editor: &mut Editor<'static>, env: &mut EditorEnv<'static>) {
    let mut id = editor.document_map.read().len();

    for f in &editor.config.files_list {
        // check file type
        if let Ok(metadata) = fs::metadata(f) {
            let file_type = metadata.file_type();

            // ignore directories for now
            if file_type.is_dir() {
                continue;
            }
        } else {
            // log error
            eprintln!("cannot check {} file type", f);
            continue;
        }

        let b = DocumentBuilder::new()
            .document_name(f)
            .file_name(f)
            .internal(false)
            .finalize();

        if let Some(b) = b {
            let doc_id = document::Id(id);
            b.as_ref().write().id = doc_id; // TODO(ceg): improve doc id generation
            editor.document_map.write().insert(doc_id, b);
            id += 1;
        }
    }

    // default buffer ?
    let map_is_empty = editor.document_map.read().is_empty();
    if map_is_empty {
        // edit.get_untitled_count() -> 1

        let b = DocumentBuilder::new()
            .document_name("untitled-1")
            .internal(false)
            .finalize();
        if let Some(b) = b {
            {
                let mut d = b.write();
                let s = WELCOME_MESSAGE.as_bytes();

                // move 1st tag to ctor/doc::new() ?
                d.tag(env.current_time, 0, vec![0]); // TODO(ceg): rm this only if the buffer log is cleared
                                                     //    create_views(&mut editor, &mut env);

                d.insert(0, s.len(), s);

                // do not allow to go back to empty buffer
                d.buffer_log_reset();
                d.changed = false;
            }
            let doc_id = document::Id(id);
            editor.document_map.write().insert(doc_id, b);
            id += 1;
        }
    }

    // configure document
    let modes = editor.modes.clone();
    for (mode_name, mode) in modes.borrow().iter() {
        // per mode document metadata
        dbg_println!("setup mode[{}] document metadata", mode_name);
        let mut mode = mode.borrow_mut();
        let map = editor.document_map.clone();
        let mut map = map.as_ref().write();
        for (_, doc) in map.iter_mut() {
            let mut doc = doc.write();
            mode.configure_document(editor, env, &mut doc);
        }
    }

    dbg_println!("id {}", id);
}

pub fn create_views(mut editor: &mut Editor<'static>, mut env: &mut EditorEnv<'static>) {
    let document_map = editor.document_map.clone();
    let document_map = document_map.read();

    // create default views
    // sort by arg pos first
    let mut docs_id: Vec<document::Id> = document_map.iter().map(|(k, _v)| *k).collect();
    docs_id.sort();
    let mut docs: Vec<Arc<RwLock<Document>>> = vec![];
    for id in docs_id.iter() {
        if let Some(doc) = document_map.get(id) {
            docs.push(Arc::clone(doc));
        }
    }

    let modes = match std::env::var("SINGLE_VIEW") {
        Ok(_) => vec!["simple-view".to_owned()],
        _ => vec!["basic-editor".to_owned()],
    };

    // create views
    for doc in docs {
        let view = View::new(
            &mut editor,
            &mut env,
            None,
            0,
            0,
            1,
            1,
            Some(doc),
            &modes,
            0,
        );
        dbg_println!("create {:?}", view.id);

        // top level views
        editor.root_views.push(view.id);
        editor.view_map.insert(view.id, Rc::new(RwLock::new(view)));
    }

    // index documents
    // TODO(ceg): send one event per doc
    if true {
        let msg = EventMessage {
            seq: 0,
            event: Event::IndexTask {
                document_map: Arc::clone(&editor.document_map),
            },
        };
        editor.indexer_tx.send(msg).unwrap_or(());
    }
}

use crate::core::modes::BasicEditorMode;
use crate::core::modes::SimpleViewMode;

use crate::core::modes::CoreMode;
use crate::core::modes::FindMode;
use crate::core::modes::TextMode;

use crate::core::modes::StatusMode;

use crate::core::modes::HsplitMode;
use crate::core::modes::VsplitMode;

use crate::core::modes::VscrollbarMode;

use crate::core::modes::GotoLineMode;
use crate::core::modes::LineNumberMode;

pub fn load_modes(editor: &mut Editor, _env: &mut EditorEnv) {
    // set default mode(s)
    editor.register_mode(Box::new(CoreMode::new()));
    editor.register_mode(Box::new(BasicEditorMode::new()));
    editor.register_mode(Box::new(SimpleViewMode::new()));

    editor.register_mode(Box::new(VsplitMode::new()));
    editor.register_mode(Box::new(HsplitMode::new()));

    editor.register_mode(Box::new(VscrollbarMode::new()));

    editor.register_mode(Box::new(TextMode::new()));
    editor.register_mode(Box::new(StatusMode::new()));

    editor.register_mode(Box::new(FindMode::new()));

    editor.register_mode(Box::new(LineNumberMode::new()));
    editor.register_mode(Box::new(GotoLineMode::new()));
}
