use core::panic;
//
use std::fs;
use std::sync::mpsc::channel;
use std::sync::mpsc::Receiver;
use std::sync::mpsc::Sender;
use std::thread;

use parking_lot::RwLock;

use regex::Regex;
use std::rc::Rc;
use std::sync::Arc;

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
use crate::core::editor::EditorEvent;

use crate::core::event::Event;
use crate::core::event::Message;

use crate::core::view::ChildView;
use crate::core::view::LayoutDirection;
use crate::core::view::LayoutSize;
use crate::core::view::View;
use crate::core::view::ViewEventDestination;
use crate::core::view::ViewEventSource;

use crate::core::editor::get_checked_view_by_id;
use crate::core::editor::get_view_by_id;
use crate::core::editor::get_view_ids_by_tags;

use crate::core::view::register_view_subscriber;

use crate::core::editor::process_editor_events;
use crate::core::editor::push_editor_event;

//use crate::core::error::Error;
//type UnlResult<T> = Result<T, Error>;

use once_cell::sync::Lazy;
pub static BOOT_TIME: Lazy<std::time::SystemTime> = Lazy::new(|| std::time::SystemTime::now());

pub const VERSION: &str = env!("CARGO_PKG_VERSION");

static OFFSET_PREFIX_REGEX: &str = r"^\+?@([0-9]+)";
static LINE_COLUMN_PREFIX_REGEX: &str = r"^\+([0-9]+):?([0-9]+)?";
static OFFSET_SUFFIX_REGEX: &str = r"^(.*):@([0-9]+)";
static FILE_LINE_COLUMN_REGEX: &str = r"^([^:]+):([0-9]+):?([0-9]+)?";

use std::sync::{Mutex, OnceLock};

pub static LOG_FILE: OnceLock<Mutex<std::io::BufWriter<std::fs::File>>> = OnceLock::new();

pub static LOG_FILENAME: OnceLock<String> = OnceLock::new();

//
pub fn get_dbg_println_flag() -> usize {
    DBG_PRINTLN_FLAG.load(Ordering::Relaxed)
}

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

pub fn get_log_file() -> &'static Mutex<std::io::BufWriter<std::fs::File>> {
    // TODO: handle Windows Path drive, etc..

    #[cfg(unix)]
    let log_file_path = "/tmp/u.log";

    #[cfg(windows)]
    let log_file_path = {
        use std::env;

        let key = "TEMP";
        let base = match env::var(key) {
            Ok(val) => val,
            Err(e) => panic!("couldn't find {key}: {e}"),
        };

        let sep = std::path::MAIN_SEPARATOR;
        format!("{base}{sep}u.log")
    };

    crate::core::LOG_FILE.get_or_init(|| {
        let logfile = std::fs::File::options()
            .create(true)
            .append(true)
            .open(crate::core::LOG_FILENAME.get_or_init(|| log_file_path.into()))
            .expect("cannot open log file");
        Mutex::new(std::io::BufWriter::new(logfile))
    })
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

pub static WELCOME_MESSAGE: &str = std::include_str!("../../res/welcome_screen.txt");

pub static HELP_MESSAGE: &str = std::include_str!("../../res/help_screen.txt");

/// This function is the core of the editor.
/// It should be ran in an other thread than the main one (which is kept for ui)
pub fn run(
    config: Config,
    core_rx: &Receiver<Message<'static>>,
    core_tx: &Sender<Message<'static>>,
    ui_tx: &Sender<Message<'static>>,
) {
    let (worker_tx, worker_rx) = channel();
    let (indexer_tx, indexer_rx) = channel();

    let (executor_tx, executor_rx) = channel::<(i32, Box<dyn FnOnce() + Send>)>();

    let mut editor = Editor::new(
        config,
        core_tx.clone(),
        ui_tx.clone(),
        worker_tx.clone(),
        indexer_tx.clone(),
        executor_tx.clone(),
    );
    let mut env = EditorEnv::new();

    // create worker thread
    let worker_th = {
        let core_tx = core_tx.clone();
        Some(thread::spawn(move || worker(&worker_rx, &core_tx)))
    };

    // create executor thread
    let executor_th = {
        let core_tx = core_tx.clone();
        Some(thread::spawn(move || executor_fn(&executor_rx, &core_tx)))
    };

    let indexer_th = {
        let core_tx = core_tx.clone();
        Some(thread::spawn(move || indexer(&indexer_rx, &core_tx)))
    };

    load_buffers(&mut editor, &mut env);

    load_modes(&mut editor, &mut env);

    configure_modes(&mut editor, &mut env);

    create_layout(&mut editor, &mut env);

    //
    process_editor_events(&mut editor, &mut env);

    // TODO(ceg): send one event per
    // index buffers,
    {
        let ts = crate::core::BOOT_TIME.elapsed().unwrap().as_millis();

        let msg = Message {
            seq: 0,
            input_ts: 0,
            ts,
            event: Event::IndexTask {
                buffer_map: Arc::clone(&editor.buffer_map),
            },
        };
        editor.indexer_tx.send(msg).unwrap_or(());
    }

    editor::main_loop(&mut editor, &mut env, core_rx, ui_tx);

    // force executor quit
    let _ = editor.executor_tx.send((1, Box::new(|| {})));
    if let Some(executor_handle) = executor_th {
        executor_handle.join().unwrap()
    }
    // wait for worker thread
    if let Some(worker_handle) = worker_th {
        worker_handle.join().unwrap()
    }
    if let Some(indexer_handle) = indexer_th {
        indexer_handle.join().unwrap()
    }
}

/////////////////////// worker.rs

pub fn worker(worker_rx: &Receiver<Message<'static>>, core_tx: &Sender<Message<'static>>) {
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
                    let ts = crate::core::BOOT_TIME.elapsed().unwrap().as_millis();

                    let msg = Message::new(0, 0, ts, Event::RefreshView);
                    crate::core::event::pending_input_event_inc(1);
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

pub fn executor_fn(
    worker_rx: &Receiver<(i32, Box<dyn FnOnce() + Send>)>,
    _core_tx: &Sender<Message<'static>>,
) {
    dbg_println!("[starting executor thread]");

    while let Ok((_idx, task)) = worker_rx.recv() {
        dbg_println!("receive new task");
        task();
        if _idx == 1 {
            break;
        }
    }
}

pub fn indexer(worker_rx: &Receiver<Message<'static>>, core_tx: &Sender<Message<'static>>) {
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
                // better: Event::IndexTask { buffer_map , Vec<id> }
                // get buffers(Vec<id>) -> Vec<Option<Arc<Rw<Buffer>>>>
                Event::IndexTask { buffer_map } => {
                    dbg_println!("[receive index task ]");

                    // NB: lock contention on buffer_map.read()

                    // put buffer+id in a special list
                    let mut buffers = vec![];
                    {
                        let map = buffer_map.read();
                        for (id, buffer) in map.iter() {
                            {
                                let buffer = buffer.read();
                                if buffer.indexed {
                                    continue;
                                }
                            }
                            buffers.push((buffer.clone(), *id));
                        }
                    }

                    for (buffer, id) in buffers {
                        let is_indexed = buffer::build_index(&buffer);
                        if !is_indexed {
                            continue;
                        }

                        let ts = crate::core::BOOT_TIME.elapsed().unwrap().as_millis();

                        // notify
                        let msg = Message::new(
                            0,
                            0,
                            ts,
                            Event::Buffer {
                                event: BufferEvent::BufferFullyIndexed { buffer_id: id },
                            },
                        );
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

#[cfg(test)]
mod test_regex {

    #[test]
    fn test_buffer_position_regex() {
        use super::*;

        [
            OFFSET_PREFIX_REGEX,
            LINE_COLUMN_PREFIX_REGEX,
            OFFSET_SUFFIX_REGEX,
            FILE_LINE_COLUMN_REGEX,
        ]
        .map(|s| Regex::new(s).unwrap());
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
    arg_info
}

pub fn path_to_buffer_kind(path: &String) -> BufferKind {
    match fs::metadata(path) {
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
pub fn load_buffers(editor: &mut Editor<'static>, env: &mut EditorEnv<'static>) {
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

                let s = HELP_MESSAGE.as_bytes();
                let dsz = d.size() as u64;
                d.insert(dsz, s.len(), s);

                // do not allow to go back to empty buffer
                d.buffer_log_reset();
                d.changed = false;
            }
            let buffer_id = b.read().id;
            editor.buffer_map.write().insert(buffer_id, b);
        }
    }
}

pub fn configure_modes(editor: &mut Editor<'static>, env: &mut EditorEnv<'static>) {
    // configure buffer
    // TODO(ceg): use this for per mode config ? runtime configuration ?

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

use crate::core::modes::CoreMode;
use crate::core::modes::FindMode;
use crate::core::modes::TextMode;

use crate::core::modes::EmptyLineMode;
use crate::core::modes::SideBarMode;

use crate::core::modes::TabBarMode;

use crate::core::modes::StatusLineMode;

use crate::core::modes::TitleBarMode;

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

    editor.register_mode(Box::new(EmptyLineMode::new()));

    editor.register_mode(Box::new(SideBarMode::new()));

    editor.register_mode(Box::new(TabBarMode::new()));

    editor.register_mode(Box::new(VsplitMode::new()));
    editor.register_mode(Box::new(HsplitMode::new()));

    editor.register_mode(Box::new(VscrollbarMode::new()));

    editor.register_mode(Box::new(TitleBarMode::new()));

    editor.register_mode(Box::new(StatusLineMode::new()));

    editor.register_mode(Box::new(TextMode::new()));

    editor.register_mode(Box::new(FindMode::new()));

    editor.register_mode(Box::new(LineNumberMode::new()));
    editor.register_mode(Box::new(GotoLineMode::new()));

    editor.register_mode(Box::new(OpenDocMode::new()));

    editor.register_directory_mode(Box::new(DirMode::new()));
}

pub static DEFAULT_LAYOUT_JSON: &str = std::include_str!("../../res/default_layout.json");

use serde_json::Value;

pub fn parse_layout_str(json: &str) -> Result<serde_json::Value, serde_json::error::Error> {
    // Parse the string of data into serde_json::Value.
    let json: Value = serde_json::from_str(json)?;
    // dbg_println!("layout json {:?}", json);
    //dbg_println!("parsing {:?}", json);

    Ok(json)
}

pub fn build_view_layout_from_json_str(
    mut editor: &mut Editor<'static>,
    mut env: &mut EditorEnv<'static>,
    all_layouts: &serde_json::Value,
    buffer: Option<Arc<RwLock<Buffer<'static>>>>,
    attr: &str,
    _depth: usize,
) -> Option<view::Id> {
    let json = serde_json::from_str(attr);
    if json.is_err() {
        dbg_print!("json parse error {:?}", json);
        return None;
    }

    let attr = json.unwrap();

    build_view_layout_from_attr(&mut editor, &mut env, all_layouts, buffer.clone(), &attr, 0)
}

fn build_view_layout_from_attr(
    mut editor: &mut Editor<'static>,
    mut env: &mut EditorEnv<'static>,
    json: &Value,
    buffer: Option<Arc<RwLock<Buffer<'static>>>>,
    attr: &Value,
    depth: usize,
) -> Option<view::Id> {
    dbg_println!(
        "build_view_layout_from_attr [{depth}] parsing attrs {:?}",
        attr
    );

    let mut dir = view::LayoutDirection::Horizontal;
    let mut leader = false;
    let mut tags: Vec<String> = vec![];

    let mut modes: Vec<String> = vec![];

    let mut sub_views_id = Vec::<Option<view::Id>>::new();

    let mut view_size = LayoutSize::Percent { p: 100.0 };

    let mut focus_idx = None;
    let mut status_idx = None;

    let mut internal_buffer_name: Option<String> = None;

    let mut allow_split = false;
    let mut allow_destroy = false;

    if let Value::Object(obj) = attr {
        if let Some(val) = obj.get("sub-layout") {
            if let Value::String(val) = val {
                return build_view_layout_typed(&mut editor, &mut env, buffer.clone(), &json, val);
            }
        }

        if let Some(val) = obj.get("internal-buffer") {
            if let Value::String(val) = val {
                internal_buffer_name = Some(val.clone());
            } else {
                // invalid type
            }
        }

        if let Some(sz_obj) = obj.get("size") {
            if let Some(n) = sz_obj.get("percent") {
                if n.is_f64() {
                    let p = n.as_f64().unwrap() as f32;
                    view_size = LayoutSize::Percent { p: f32::from(p) }
                } else if n.is_u64() {
                    let p = n.as_u64().unwrap() as f32;
                    view_size = LayoutSize::Percent { p: f32::from(p) }
                } else {
                    // syntax error
                    panic!("percent: invalid syntax")
                }
            }

            if let Some(n) = sz_obj.get("fixed") {
                if n.is_u64() {
                    let size = n.as_u64().unwrap() as usize;
                    view_size = LayoutSize::Fixed { size };
                } else {
                    // syntax error
                }
            }

            if let Some(n) = sz_obj.get("remain") {
                if n.is_u64() {
                    let p = n.as_f64().unwrap() as f32;
                    view_size = LayoutSize::Percent { p }
                } else {
                    // syntax error
                    panic!("remain: invalid syntax")
                }
            }

            if let Some(n) = sz_obj.get("remain_percent") {
                if n.is_f64() {
                    let p = n.as_f64().unwrap() as f32;
                    view_size = LayoutSize::RemainPercent { p }
                } else if n.is_u64() {
                    let p = n.as_u64().unwrap() as f32;
                    view_size = LayoutSize::RemainPercent { p }
                } else {
                    // syntax error
                    panic!("remain_percent: invalid syntax")
                }
            }

            if let Some(n) = sz_obj.get("remain_minus") {
                if n.is_u64() {
                    let minus = n.as_u64().unwrap() as usize;
                    view_size = LayoutSize::RemainMinus { minus }
                } else {
                    // syntax error
                    panic!("remain_minus: invalid syntax")
                }
            }
        }

        if let Some(val) = obj.get("leader") {
            if let Value::Bool(val) = val {
                leader = *val;
            } else {
                // invalid type
            }
        }

        if let Some(val) = obj.get("allow-split") {
            if let Value::Bool(val) = val {
                allow_split = *val;
            } else {
                // invalid type
            }
        }

        if let Some(val) = obj.get("allow-destroy") {
            if let Value::Bool(val) = val {
                allow_destroy = *val;
            } else {
                // invalid type
            }
        }

        if let Some(tags_array) = obj.get("tags") {
            if tags_array.is_array() {
                if let Value::Array(ref vec) = tags_array {
                    for m in vec {
                        if let Value::String(s) = m {
                            dbg_println!(" --- found tag  = {}", m);
                            tags.push(s.clone());
                        }
                    }
                }
            }
        }

        if let Some(modes_array) = obj.get("modes") {
            if modes_array.is_array() {
                if let Value::Array(ref vec) = modes_array {
                    for m in vec {
                        if let Value::String(s) = m {
                            dbg_println!(" --- found mode  = {}", m);
                            modes.push(s.clone());
                        }
                    }
                }
            }
        }

        if let Some(val) = obj.get("children_layout") {
            dir = if let Value::String(val) = val {
                if *val == "vertical" {
                    LayoutDirection::Vertical
                } else {
                    LayoutDirection::Horizontal
                }
            } else {
                // invalid type
                LayoutDirection::Horizontal
            };
        }

        if let Some(Value::Number(ref n)) = obj.get("focus_idx") {
            if n.is_u64() {
                focus_idx = Some(n.as_u64().unwrap() as usize);
            } else {
                // syntax error
            }
        }

        if let Some(Value::Number(ref n)) = obj.get("status_idx") {
            if n.is_u64() {
                status_idx = Some(n.as_u64().unwrap() as usize);
            } else {
                // syntax error
            }
        }
    }

    dbg_println!(
        "build_view_layout_from_attr [{depth}] create view: buffer: {:?}, view_size {:?}",
        buffer,
        view_size
    );

    dbg_println!(
        "build_view_layout_from_attr [{depth}] create view: leader: {:?}",
        leader
    );

    let buffer = if let Some(internal_buffer_name) = internal_buffer_name {
        BufferBuilder::new(BufferKind::File)
            .buffer_name(&internal_buffer_name)
            .internal(true)
            //           .use_buffer_log(false)
            .finalize()
    } else {
        buffer
    };

    let mut view = View::new(
        editor,
        env,
        None,
        (0, 0),
        (1, 1),
        buffer.clone(),
        &tags,
        &modes,
        0,
        dir,
        view_size,
    );

    view.json_attr = Some(attr.to_string());
    dbg_println!("view.json_attr {:?}", view.json_attr);

    // FIXME(ceg): split core is not up to date
    view.is_splittable = allow_split; // Nb: do not remove , allow recursive splitting

    view.destroyable = allow_destroy;

    view.is_leader = leader;

    // select first active view
    if env.active_view.is_none() {
        if view.tags.get("target-view").is_some() {
            // TODO(ceg): find better naming for target view
            // env.active_view = Some(view.id);
        }
    }

    // select first status view
    if env.status_view_id.is_none() {
        if view.tags.get("status-line").is_some() {
            // TODO(ceg): find better naming
            env.status_view_id = Some(view.id);
        }
    }

    dbg_println!(
        "build_view_layout_from_attr [{depth}] setup view modes: {:?}",
        modes
    );

    // parse children
    if let Value::Object(obj) = attr {
        // look for children first
        if let Some(children) = obj.get("children") {
            if let Value::Array(ref vec) = children {
                for child_layout in vec {
                    dbg_println!(" >>>> recursive call");
                    let child_view = build_view_layout_from_attr(
                        &mut editor,
                        &mut env,
                        json,
                        buffer.clone(),
                        &child_layout,
                        depth + 1,
                    );
                    sub_views_id.push(child_view);
                }
            }
        }
    }

    // add children
    for (idx, vid) in sub_views_id.iter().enumerate() {
        if let Some(vid) = vid {
            // set parent link
            {
                let child = get_view_by_id(editor, *vid);
                let mut child = child.write();
                child.parent_id = Some(view.id);
                child.layout_index = Some(idx);

                let id = *vid;
                let op = child.layout_size.clone();
                view.children.push(ChildView {
                    layout_op: op.clone(),
                    id,
                });
            }
        }
    }

    #[derive(Debug)]
    struct RegisterParam {
        pub mode: Option<String>,
        pub src_idx: Option<usize>,
        pub dst_idx: Option<usize>,
    }

    impl RegisterParam {
        pub fn new() -> Self {
            RegisterParam {
                mode: None,
                src_idx: None,
                dst_idx: None,
            }
        }

        fn is_valid(&self) -> bool {
            self.mode.is_some() && self.src_idx.is_some() && self.dst_idx.is_some()
        }
    }

    // parse children
    let mut links = vec![];

    if let Value::Object(obj) = attr {
        // look for children first
        if let Some(subscribe) = obj.get("children-subscribe") {
            if let Value::Array(ref vec) = subscribe {
                for sub in vec {
                    let mut p = RegisterParam::new();
                    if let Some(Value::String(ref s)) = sub.get("mode") {
                        p.mode = Some(s.clone());
                    }

                    if let Some(Value::Number(ref n)) = sub.get("src") {
                        if n.is_u64() {
                            p.src_idx = Some(n.as_u64().unwrap() as usize);
                        } else {
                            // syntax error
                        }
                    }

                    if let Some(Value::Number(ref n)) = sub.get("dst") {
                        if n.is_u64() {
                            p.dst_idx = Some(n.as_u64().unwrap() as usize);
                        } else {
                            // syntax error
                        }
                    }

                    if p.is_valid() {
                        links.push(p);
                    }
                }
            }
        }
    }

    for l in links {
        let mode_name = l.mode.unwrap();
        if let Some(mode) = editor.get_mode(&mode_name) {
            register_view_subscriber(
                editor,
                env,
                Rc::clone(&mode),
                // publisher
                ViewEventSource {
                    id: view.children[l.src_idx.unwrap()].id,
                },
                // subscriber
                ViewEventDestination {
                    id: view.children[l.dst_idx.unwrap()].id,
                },
            );
        }
    }

    if let Some(focus_idx) = focus_idx {
        view.transfer_focus_to = Some(view.children[focus_idx].id); // TODO(ceg):
    }

    if let Some(status_idx) = status_idx {
        view.status_view_id = Some(view.children[status_idx].id);
        env.status_view_id = Some(view.children[status_idx].id);
    }

    // insert in global map
    let id = view.id;
    let view = Rc::new(RwLock::new(view));
    editor.view_map.write().insert(id, Rc::clone(&view));

    dbg_println!(" <<<< return");

    Some(id)
}

pub fn build_view_layout_typed(
    mut editor: &mut Editor<'static>,
    mut env: &mut EditorEnv<'static>,
    buffer: Option<Arc<RwLock<Buffer<'static>>>>,
    all_layouts: &Value, //
    view_type: &str,
) -> Option<view::Id> {
    // 1st level is object["view-type"]
    let depth = 0;
    if let Value::Object(ref root) = *all_layouts {
        if let Some((_view_type, view_layout)) = root.get_key_value(view_type) {
            //dbg_println!("view_type = {:?}, v = {:?}", view_type, view_layout);
            return build_view_layout_from_attr(
                &mut editor,
                &mut env,
                all_layouts,
                buffer.clone(),
                &view_layout,
                depth,
            );
        }
    }

    return None;
}

pub fn get_view_parents(editor: &mut Editor<'static>, id: view::Id) -> Option<Vec<view::Id>> {
    dbg_println!("get_view_parents {:?}", id);

    let mut ids = vec![];

    let mut id = id;
    loop {
        if let Some(v) = get_checked_view_by_id(editor, id) {
            if let Some(pid) = v.read().parent_id {
                id = pid;
                ids.push(id);
            } else {
                break;
            }
        } else {
            break;
        }
    }

    if ids.is_empty() {
        None
    } else {
        Some(ids)
    }
}

pub fn create_layout(mut editor: &mut Editor<'static>, mut env: &mut EditorEnv<'static>) {
    let json = parse_layout_str(DEFAULT_LAYOUT_JSON);

    if json.is_err() {
        dbg_print!("json parse error {:?}", json);
        return;
    }
    let json = json.unwrap();

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

    // TODO(ceg): handle ctor with no buffer
    let root_buf = BufferBuilder::new(BufferKind::File)
        .buffer_name("root")
        .internal(true)
        .use_buffer_log(false)
        //.read_only(true) // TODO
        .finalize();

    let root_id = build_view_layout_typed(&mut editor, &mut env, root_buf, &json, "main-view");
    dbg_println!("root_id {:?}", root_id);

    // TODO: implement side bar click to create groups
    // create a default group and attach it to "workspace-view":
    if let Some(workspace_id) = view::get_view_by_tag(editor, env, "workspace") {
        let group_buf = BufferBuilder::new(BufferKind::File)
            .buffer_name("group")
            .internal(true)
            .use_buffer_log(false)
            .finalize();

        // add (default) group
        if let Some(group_id) =
            build_view_layout_typed(&mut editor, &mut env, group_buf, &json, "group-view")
        {
            get_view_by_id(editor, workspace_id)
                .write()
                .children
                .push(ChildView {
                    id: group_id,
                    layout_op: LayoutSize::Percent { p: 100.0 },
                });
        }

        // create views
        for buffer in buffers {
            dbg_println!("-------------");

            dbg_println!("loading buffer '{}'", buffer.as_ref().read().name);
            let kind = buffer.as_ref().read().kind;
            let _vid = match kind {
                BufferKind::File => build_view_layout_typed(
                    &mut editor,
                    &mut env,
                    Some(buffer),
                    &json,
                    "single-file-view",
                ),
                BufferKind::Directory => {
                    build_view_layout_typed(&mut editor, &mut env, Some(buffer), &json, "dir-view")
                }
            };
        }

        // populate active views (file)
        // default behavior / no session restore yet
        {
            let map = editor.view_map.clone();
            for (id, v) in map.read().iter() {
                let v = v.read();
                if !v.tags.contains("file-view") {
                    continue;
                }

                if let Some(b) = v.buffer() {
                    if b.read().kind == BufferKind::File {
                        editor.active_views.push(*id);
                        push_editor_event(&mut editor, EditorEvent::ViewAdded { id: *id });
                    }
                }
            }

            // show 1st view
            if !editor.active_views.is_empty() {
                dbg_println!("active views {:?}", editor.active_views);

                let id = editor.active_views[0];

                env.active_view = Some(id);

                dbg_println!("active view {:?}", id);

                // find inner child
                if let Some(target_ids) = get_view_ids_by_tags(&editor, "target-view") {
                    dbg_println!("target_ids {:?}", target_ids);

                    for target_id in &target_ids {
                        // check it active view contains target-view
                        if let Some(parents) = get_view_parents(editor, *target_id) {
                            dbg_println!("parents {:?}", parents);

                            for pid in &parents {
                                dbg_println!("pid {:?} == id {:?}", pid, id);
                                if *pid == id {
                                    env.active_view = Some(*target_id);
                                }
                            }
                        }
                    }
                }

                // find in id the target-view
                // TODO: exchange with file slot ?
                if let Some(ids) = get_view_ids_by_tags(&editor, "file-slot") {
                    let parent_id = ids[0];
                    get_view_by_id(editor, parent_id)
                        .write()
                        .children
                        .push(ChildView {
                            id: id,
                            layout_op: LayoutSize::Percent { p: 100.0 },
                        });
                }
            }
        }
    }
}
