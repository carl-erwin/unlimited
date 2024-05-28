use std::any::Any;
use std::rc::Rc;

use parking_lot::RwLock;

use super::Mode;

use crate::core::editor::get_view_by_id;
use crate::core::editor::remove_view_by_id;

use crate::core::editor::Editor;
use crate::core::editor::EditorEnv;

use crate::core::editor::InputStageActionMap;

use crate::core::modes::text_mode::RawDataFilter;
use crate::core::modes::text_mode::ScreenFilter;

use crate::core::buffer::BufferBuilder;
use crate::core::buffer::BufferKind;

use crate::core::view;
use crate::core::view::ChildView;
use crate::core::view::View;
use crate::core::view::ViewEvent;
use crate::core::view::ViewEventDestination;
use crate::core::view::ViewEventSource;

use crate::core::editor::check_view_by_id;

//use crate::core::view::FilterData;
use crate::core::view::FilterIo;

use crate::dbg_println;

use crate::core::view::ContentFilter;

use crate::core::view::LayoutDirection;
use crate::core::view::LayoutEnv;
use crate::core::view::LayoutSize;

pub struct TabBarModeContext {
    pub hover: bool,
}

pub struct TabBarMode {}

impl TabBarMode {
    pub fn new() -> Self {
        dbg_println!("TabBarMode");
        TabBarMode {}
    }

    pub fn register_input_stage_actions<'a>(_map: &'a mut InputStageActionMap<'a>) {}
}

impl<'a> Mode for TabBarMode {
    fn name(&self) -> &'static str {
        &"tab-bar-mode"
    }

    fn alloc_ctx(&self) -> Box<dyn Any> {
        let ctx = TabBarModeContext { hover: false };
        Box::new(ctx)
    }

    fn build_action_map(&self) -> InputStageActionMap<'static> {
        let mut map = InputStageActionMap::new();
        Self::register_input_stage_actions(&mut map);
        map
    }

    fn configure_view(
        &mut self,
        _editor: &mut Editor<'static>,
        _env: &mut EditorEnv<'static>,
        view: &mut View<'static>,
    ) {
        //
        let use_utf8_codec = true;
        let use_tabulation_exp = !true; // char map ? with <tab>

        // mandatory data reader
        view.compose_content_filters
            .borrow_mut()
            .push(Box::new(RawDataFilter::new()));
        //
        /*
                if use_utf8_codec {
                    //
                    // DEBUG codec error
                    view.compose_content_filters
                        .borrow_mut()
                        .push(Box::new(Utf8Filter::new()));
                } else {
                    view.compose_content_filters
                        .borrow_mut()
                        .push(Box::new(TextCodecFilter::new()));
                }
                //
                view.compose_content_filters
                    .borrow_mut()
                    .push(Box::new(UnicodeToTextFilter::new()));

                // TODO: char map 0x9 -> "\t"
                if use_tabulation_exp {
                    view.compose_content_filters
                        .borrow_mut()
                        .push(Box::new(TabFilter::new()));
                }
        */
        let mut screen_filter = ScreenFilter::new();
        screen_filter.display_eof = false;

        view.compose_content_filters
            .borrow_mut()
            .push(Box::new(screen_filter));

        // move to overlay
        view.compose_content_filters
            .borrow_mut()
            .push(Box::new(TabBarModeCompose::new()));
    }

    fn watch_editor_event(&self) -> bool {
        true
    }

    //
    fn on_editor_event(
        &self,
        _editor: &mut Editor<'static>,
        _env: &mut EditorEnv<'static>,
        event: &EditorEvent,
        watcher_view: &mut View<'static>,
    ) {
        dbg_println!(
            "mode '{}' on_editor_event: event {:?} watcher_view {:?}",
            self.name(),
            event,
            watcher_view.id
        );
    }

    //
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

        dbg_println!("src_view.tags {:?}", src_view.tags);

        match event {
            ViewEvent::Subscribe => {}

            ViewEvent::PreComposition => {}

            ViewEvent::Enter => {
                let mode_ctx = src_view.mode_ctx_mut::<TabBarModeContext>("tab-bar-mode");
                mode_ctx.hover = true;
            }

            ViewEvent::Leave => {
                let mode_ctx = src_view.mode_ctx_mut::<TabBarModeContext>("tab-bar-mode");
                mode_ctx.hover = false;
            }

            _ => {}
        }
    }
}

use crate::core::editor::EditorEvent;
use crate::core::editor::EditorEventCb;

impl EditorEventCb for TabBarMode {
    fn cb(&mut self, event: &EditorEvent) {
        dbg_println!("TabBarMode EditorEvent ev {:?} CB", event);

        match event {
            _ => {
                dbg_println!("unhandled event {:?}", event);
            }
        }
    }
}

pub fn create_tab_entry(
    mut editor: &mut Editor<'static>,
    mut env: &mut EditorEnv<'static>,
    parent_id: view::Id,
    label: &str,
) {
    let label_buffer = BufferBuilder::new(BufferKind::File)
        .buffer_name("tab:entry")
        .internal(true)
        .use_buffer_log(false)
        .finalize();

    {
        let mut d = label_buffer.as_ref().unwrap().write();
        d.append(label.as_bytes());
    }

    let width = label.len();
    // create view
    let label_view = View::new(
        &mut editor,
        &mut env,
        Some(parent_id),
        (0, 0),
        (width, 1),
        label_buffer,
        &vec![], // tags
        &vec!["empty-line-mode".to_owned()],
        0,
        LayoutDirection::Horizontal,
        LayoutSize::Floating,
    );

    let id = label_view.id;
    {
        let tab_bar = get_view_by_id(editor, parent_id);
        let mut tab_bar = tab_bar.write();

        tab_bar.children.push(ChildView {
            id: id,
            layout_op: LayoutSize::Fixed { size: width },
        });
    }

    editor.add_view(id, label_view);
}

pub struct TabBarModeCompose {}

impl TabBarModeCompose {
    pub fn new() -> Self {
        dbg_println!("TabBarMode");
        TabBarModeCompose {}
    }

    pub fn register_input_stage_actions<'a>(_map: &'a mut InputStageActionMap<'a>) {}
}

impl ContentFilter<'_> for TabBarModeCompose {
    fn name(&self) -> &'static str {
        &"TabBarModeCompose"
    }

    fn setup(
        &mut self,
        mut editor: &mut Editor<'static>,
        mut editor_env: &mut EditorEnv<'static>,
        layout_env: &mut LayoutEnv,
        view: &Rc<RwLock<View>>,
        parent_view: Option<&View<'static>>,
    ) {
        // hack
        // FIXME: move to event base populate
        // destroy/recreate all children (labels + cross)
        // TODO(ceg): destroy_view((&editor, c.id);
        {
            let mut v = view.write();

            let mode_ctx = v.mode_ctx_mut::<TabBarModeContext>("tab-bar-mode");

            // MUST will remove view from parent.children
            for c in &v.children {
                remove_view_by_id(&editor, c.id);
            }
            v.children.clear();
        }

        // scan active group/view and populate children
        {
            let mut labels = vec![];

            // scroll widget
            // labels.push("⮜ ⮞ ".to_owned());

            let parent_id = { view.read().id };
            let list = editor.active_views.clone();
            for (idx, vid) in list.iter().enumerate() {
                if let Some(cv) = check_view_by_id(editor, *vid) {
                    if let Some(b) = cv.read().buffer() {
                        let mut l = String::new();
                        if idx != 0 {
                            l.push_str(&"|");
                        }
                        l.push_str(&b.read().name);
                        l.push_str(&" ⨯");
                        labels.push(l);
                    }
                };
            }
            // (re)create new children
            for n in labels {
                create_tab_entry(&mut editor, &mut editor_env, parent_id, &n);
            }
        }
    }

    fn run(
        &mut self,
        _view: &View,
        env: &mut LayoutEnv,
        _input: &[FilterIo],
        _output: &mut Vec<FilterIo>,
    ) {
    }

    fn finish(&mut self, view: &View, env: &mut LayoutEnv) {}
}
