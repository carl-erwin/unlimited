use core::panic;
//
use std::fs;
use std::sync::mpsc::channel;
use std::sync::mpsc::Receiver;
use std::sync::mpsc::Sender;
use std::thread;

use parking_lot::RwLock;

use std::sync::Arc;

use regex::Regex;

use std::sync::atomic::{AtomicUsize, Ordering};

#[macro_use]
pub(crate) mod macros;

pub mod buffer;
pub mod codec;
pub mod codepointinfo;
pub mod config;
pub mod editor;
pub mod error;
pub mod event;
pub mod mapped_file;
pub mod modes;
pub mod screen;
pub mod view;

use crate::core::buffer::Buffer;
use crate::core::buffer::BufferBuilder;
use crate::core::buffer::BufferEvent;
use crate::core::buffer::BufferKind;
use crate::core::buffer::BufferPosition;

use crate::core::config::Config;
use crate::core::editor::Editor;
use crate::core::editor::EditorEnv;
use crate::core::event::Event;
use crate::core::event::EventMessage;

use crate::core::view::View;

//use crate::core::error::Error;
//type UnlResult<T> = Result<T, Error>;

pub const VERSION: &str = env!("CARGO_PKG_VERSION");

static OFFSET_PREFIX_REGEX: &str = r"^\+?@([0-9]+)";
static LINE_COLUMN_PREFIX_REGEX: &str = r"^\+([0-9]+):?([0-9]+)?";
static OFFSET_SUFFIX_REGEX: &str = r"^(.*):@([0-9]+)";
static FILE_LINE_COLUMN_REGEX: &str = r"^([^:]+):([0-9]+):?([0-9]+)?";

//
pub static DBG_PRINTLN_FLAG: AtomicUsize = AtomicUsize::new(0);

pub fn enable_dbg_println() {
    DBG_PRINTLN_FLAG.store(1, Ordering::Relaxed);
}
pub fn disable_dbg_println() {
    DBG_PRINTLN_FLAG.store(0, Ordering::Relaxed);
}
pub fn toggle_dbg_println() {
    match DBG_PRINTLN_FLAG.load(Ordering::Relaxed) {
        0 => enable_dbg_println(),
        _ => disable_dbg_println(),
    }
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

  buffer.predefined_modes() Option<vec["internal:welcome-mode"]>
  buffer.predefined_modes() Option<vec["internal:debug-message"]>

  based on extension we will load predefine modes / keywords list etc ..

    replay-mode
      ctrl+x ctrl+x r :  {left,right} alt+{left,right}
*/

pub static WELCOME_MESSAGE: &str = r#"unlimitED! is an experimental text editor.
           There is ABSOLUTELY NO WARRANTY.

USAGE: unlimited [OPTIONS] [--] [FILES]...    ( -h for help )

It supports:
  - basic UTF-8
  - very large file
  - "unlimited" undo/redo
  - multi-cursors (wip)
  - mouse selection (graphical terminal)

[Quit]                 (NB: quit will wait for pending file(s) sync)
    Quit:           => ctrl+x ctrl+q
    Quit (no save)  => ctrl+x ctrl+x ctrl+q

[Moves]
    Arrows          => move the main mark Left,Right,Up,Down
    PageUp,PageDown => scroll the current view Up/Down
    ctrl+a          => go to beginning of current line
    ctrl+e          => go to end of current line
    ctrl+l          => center view arround main mark

[Edit]
    characters      => inserted as is
    ctrl+u          => Undo
    ctrl+r          => Redo
    ctrl+o          => Open file (TODO)

[Selection/Copy/Cut/Paste]
    with the keyboard:
    ctrl+Space      => start selection at main Mark
    alt+w           => copy current selection
    ctrl+w          => cut  current slection
    ctrl+y          => paste last cut

    with the mouse (X11 terminal, no clipboard support yet)

[Save]
    ctrl+x ctrl+s   => Save file(s) in the background (read only operations allowed).

[Buffer Selection]
     (TODO)
"#;

/// This function is the core of the editor.
/// It should be ran in an other thread than the main one (which is kept for ui)
pub fn run(
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

    editor::main_loop(&mut editor, &mut env, core_rx, ui_tx);

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

                Event::SyncTask { buffer } => {
                    buffer::sync_to_storage(&buffer);

                    let msg = EventMessage::new(0, Event::RefreshView);
                    core_tx.send(msg).unwrap_or(());
                }

                //                Event::OutsourcedTask { task_uid,  editor, editor_env, buffer_id, vid, action, params ? } => {
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
                Event::IndexTask { buffer_map } => {
                    dbg_println!("[receive index task ]");

                    let mut refresh_ui = false;
                    let map = buffer_map.read();
                    let mut t0 = std::time::Instant::now();
                    for (id, buffer) in map.iter() {
                        let is_indexed = buffer::build_index(buffer);
                        if is_indexed == false {
                            continue;
                        }

                        // notify
                        let msg = EventMessage::new(
                            0,
                            Event::Buffer {
                                event: BufferEvent::BufferFullyIndexed { buffer_id: *id },
                            },
                        );
                        core_tx.send(msg).unwrap_or(());

                        // TODO: remove this: let the ui decide if the refresh is needed base on buffer_id

                        // send ui refresh event
                        let msg = EventMessage::new(0, Event::RefreshView);
                        core_tx.send(msg).unwrap_or(());

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

#[derive(Debug, Clone)]
struct ArgInfo {
    path: String, // todo pathbuf
    start_position: BufferPosition,
}

impl ArgInfo {
    pub fn new(path: String) -> Self {
        ArgInfo {
            path,
            start_position: BufferPosition::new(),
        }
    }
}

fn filesystem_entry_exists(path: String) -> bool {
    match fs::metadata(path) {
        Ok(_metadata) => true,
        Err(_e) => {
            // permission etc ..
            false
        }
    }
}

// parse command line files list and infer line,column positions
fn build_buffer_options(editor: &Editor<'static>) -> Vec<ArgInfo> {
    let mut v = vec![];

    // TODO(ceg): move regex to static str, add unit test to compile them

    let re_offset_prefix = Regex::new(OFFSET_PREFIX_REGEX).unwrap();
    let re_line_column_prefix = Regex::new(LINE_COLUMN_PREFIX_REGEX).unwrap();
    let re_offset_suffix = Regex::new(OFFSET_SUFFIX_REGEX).unwrap();
    let re_file_line_column = Regex::new(FILE_LINE_COLUMN_REGEX).unwrap();

    let mut it = editor.config.files_list.iter();
    loop {
        let f = it.next();
        if f.is_none() {
            break;
        }

        let f = f.unwrap();

        // check file exits ?
        match fs::metadata(f) {
            // file exits ? yes -> add to list
            Ok(_metadata) => {
                // let file_type = metadata.file_type();
                v.push(ArgInfo::new(f.clone()));
            }

            // file does not exits -> try regex
            Err(_e) => {
                // prefix
                match re_line_column_prefix.captures(f) {
                    None => {}
                    Some(cap) => {
                        dbg_println!("found re_line_column_prefix match {:?}", cap);
                        // take next arg as file, no checking
                        match it.next() {
                            None => {}
                            Some(path) => {
                                if filesystem_entry_exists(path.clone()) {
                                    let mut arg = ArgInfo::new(path.clone());
                                    arg.start_position.line =
                                        Some(cap[1].trim_end().parse::<u64>().unwrap_or(1));
                                    if let Some(col) = cap.get(2) {
                                        arg.start_position.column = Some(
                                            col.as_str().trim_end().parse::<u64>().unwrap_or(1),
                                        );
                                    }
                                    dbg_println!("new arg {:?}", arg);
                                    v.push(arg);
                                    continue;
                                }
                            }
                        }
                    }
                }

                // prefix
                match re_offset_prefix.captures(f) {
                    None => {}
                    Some(cap) => {
                        dbg_println!("found re_offset match {:?}", cap);
                        // take next arg as file, no checking
                        match it.next() {
                            None => {}
                            Some(path) => {
                                if filesystem_entry_exists(path.clone()) {
                                    let mut arg = ArgInfo::new(path.clone());
                                    arg.start_position.offset =
                                        Some(cap[1].trim_end().parse::<u64>().unwrap_or(0));
                                    dbg_println!("new arg {:?}", arg);
                                    v.push(arg);
                                    continue;
                                }
                            }
                        }
                    }
                }

                // suffix
                match re_offset_suffix.captures(f) {
                    None => {}
                    Some(cap) => {
                        if filesystem_entry_exists(cap[1].to_owned()) {
                            dbg_println!("found re_offset_suffix match {:?}", cap);
                            let mut arg = ArgInfo::new(cap[1].to_owned());
                            arg.start_position.offset =
                                Some(cap[2].trim_end().parse::<u64>().unwrap_or(0));
                            dbg_println!("new arg {:?}", arg);
                            v.push(arg);
                            continue;
                        }
                    }
                }

                // suffix
                match re_file_line_column.captures(f) {
                    None => {}
                    Some(cap) => {
                        dbg_println!("found re_file_line_column match {:?}", cap);

                        if filesystem_entry_exists(cap[1].to_owned()) {
                            let mut arg = ArgInfo::new(cap[1].to_owned());
                            arg.start_position.line =
                                Some(cap[2].trim_end().parse::<u64>().unwrap_or(1));

                            if let Some(col) = cap.get(3) {
                                arg.start_position.column =
                                    Some(col.as_str().trim_end().parse::<u64>().unwrap_or(1));
                            }
                            dbg_println!("new arg {:?}", arg);
                            v.push(arg);
                            continue;
                        }
                    }
                }

                // check permission ...
                dbg_println!("no match for file {:?}, try create", f);
                v.push(ArgInfo::new(f.clone()));
            }
        }
    }

    v
}

/// TODO(ceg): remove duplicates
// do symlink resolution (annotation) before real path
// check reap paths
fn filter_arg_list(arg_info: Vec<ArgInfo>) -> Vec<ArgInfo> {
    let v = arg_info;

    v
}

pub fn path_to_buffer_kind(path: &String) -> BufferKind {
    match fs::metadata(&path) {
        Ok(metadata) => {
            let file_type = metadata.file_type();

            // ignore directories for now
            if file_type.is_dir() {
                BufferKind::Directory
            } else if file_type.is_file() {
                BufferKind::File
            } else {
                // display error
                // links not handled yet
                panic!("not supported yet");
            }
        }

        Err(_e) => {
            // check no such file
            BufferKind::File
        }
    }
}

/// TODO(ceg): replace this by load/unload buffer functions
/// the ui will open the buffers on demand
pub fn load_files(editor: &mut Editor<'static>, env: &mut EditorEnv<'static>) {
    let arg_info = build_buffer_options(editor);

    let arg_info = filter_arg_list(arg_info);

    dbg_println!("processing arg_info {:?}", arg_info);

    for arg in &arg_info {
        dbg_println!("processing arg {:?}", arg);

        // check file type
        let kind = match fs::metadata(&arg.path) {
            Ok(metadata) => {
                let file_type = metadata.file_type();

                // ignore directories for now
                if file_type.is_dir() {
                    BufferKind::Directory
                } else if file_type.is_file() {
                    BufferKind::File
                } else {
                    // display error
                    // links not handled yet
                    continue;
                }
            }

            Err(_e) => {
                // check no such file
                BufferKind::File
            }
        };

        let b = BufferBuilder::new(kind)
            .buffer_name(&arg.path)
            .file_name(&arg.path)
            .internal(false)
            .use_buffer_log(true)
            .start_position(arg.start_position)
            .finalize();

        if let Some(b) = b {
            let buffer_id = b.read().id;
            editor.buffer_map.write().insert(buffer_id, b);
        }
    }

    // default buffer ?
    let map_is_empty = editor.buffer_map.read().is_empty();
    if map_is_empty {
        // edit.get_untitled_count() -> 1

        let b = BufferBuilder::new(BufferKind::File)
            .buffer_name("welcome")
            .internal(false)
            .use_buffer_log(true)
            // .read_only(true) // TODO
            .finalize();
        if let Some(b) = b {
            {
                let mut d = b.write();
                let s = WELCOME_MESSAGE.as_bytes();

                // move 1st tag to ctor/buffer::new() ?
                d.tag(env.current_time, 0, vec![0], vec![]); // TODO(ceg): rm this only if the buffer log is cleared
                d.insert(0, s.len(), s);

                // do not allow to go back to empty buffer
                d.buffer_log_reset();
                d.changed = false;
            }
            let buffer_id = b.read().id;
            editor.buffer_map.write().insert(buffer_id, b);
        }
    }

    // configure buffer

    let file_modes = editor.modes.clone();
    let dir_modes = editor.dir_modes.clone();

    // per mode buffer metadata
    let map = editor.buffer_map.clone();
    let mut map = map.write();

    for (_, buffer) in map.iter_mut() {
        let mut buffer = buffer.write();

        let modes = match buffer.kind {
            BufferKind::File => file_modes.borrow(),
            BufferKind::Directory => dir_modes.borrow(),
        };

        for (mode_name, mode) in modes.iter() {
            dbg_println!("setup mode[{}] buffer metadata", mode_name);
            let mut mode = mode.borrow_mut();
            mode.configure_buffer(editor, env, &mut buffer);
        }
    }
}

pub fn create_views(editor: &mut Editor<'static>, env: &mut EditorEnv<'static>) {
    let buffer_map = editor.buffer_map.clone();
    let buffer_map = buffer_map.read();

    // create default views
    // sort by arg pos first
    let mut buffers_id: Vec<buffer::Id> = buffer_map.iter().map(|(k, _v)| *k).collect();
    buffers_id.sort();
    let mut buffers: Vec<Arc<RwLock<Buffer>>> = vec![];
    for id in buffers_id.iter() {
        if let Some(buffer) = buffer_map.get(id) {
            buffers.push(Arc::clone(buffer));
        }
    }

    // create views
    for buffer in buffers {
        let modes = match buffer.as_ref().read().kind {
            BufferKind::File => match std::env::var("SINGLE_VIEW") {
                Ok(_) => vec!["simple-view".to_owned()],
                _ => vec!["basic-editor".to_owned()],
            },
            BufferKind::Directory => vec!["core-mode".to_owned(), "dir-mode".to_owned()],
        };

        let view = View::new(editor, env, None, (0, 0), (1, 1), Some(buffer), &modes, 0);
        dbg_println!("create {:?}", view.id);

        // top level views
        editor.root_views.push(view.id);
        editor.add_view(view.id, view);
    }

    // index buffers
    // TODO(ceg): send one event per doc
    if true {
        let msg = EventMessage {
            seq: 0,
            event: Event::IndexTask {
                buffer_map: Arc::clone(&editor.buffer_map),
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

use crate::core::modes::OpenDocMode;

use crate::core::modes::DirMode;

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

    editor.register_mode(Box::new(OpenDocMode::new()));

    editor.register_directory_mode(Box::new(DirMode::new()));
}

#[cfg(test)]
mod tests {

    #[test]
    fn test_buffer_position_regex() {
        use super::*;

        Regex::new(OFFSET_PREFIX_REGEX).unwrap();
        Regex::new(LINE_COLUMN_PREFIX_REGEX).unwrap();
        Regex::new(OFFSET_SUFFIX_REGEX).unwrap();
        Regex::new(FILE_LINE_COLUMN_REGEX).unwrap();
    }
}
