use crate::core::event::Message;
use std::sync::mpsc::Sender;

use crate::core::view::ContentFilter;
use crate::core::view::FilterData;
use crate::core::view::FilterIo;
use crate::core::view::LayoutEnv;
use crate::core::view::View;
use crate::core::Editor;
use crate::core::EditorEnv;

use parking_lot::RwLock;
use std::rc::Rc;

use crate::core::codepointinfo::CodepointInfo;
use crate::core::codepointinfo::TextStyle;

use crate::core::codec::text::u32_to_char;

use crate::core::bench_to_eof;

///////////////////////////////////////////////////////////////////////////////////////////////////

// TRANSFORM into filter pass char_map_filter before word wrap

///////////////////////////////////////////////////////////////////////////////////////////////////

pub struct ScreenFilter {
    // data
    first_offset: Option<u64>,
    last_offset: Option<u64>,
    screen_is_full: bool,
    pub display_eof: bool,
    pub ui_tx: Option<Sender<Message<'static>>>,
}

impl<'a> ScreenFilter {
    pub fn new() -> Self {
        ScreenFilter {
            // data
            first_offset: None,
            last_offset: None,

            screen_is_full: false,
            display_eof: true,
            ui_tx: None,
        }
    }

    #[inline(always)]
    fn add_text_to_screen(
        &mut self,
        env: &mut LayoutEnv,
        cpi: CodepointInfo,
        offset: Option<u64>,
    ) -> bool {
        let ret = env.screen.push(&cpi);
        if !ret.0 {
            if bench_to_eof() {
                let ts = crate::core::BOOT_TIME.elapsed().unwrap().as_millis();
                let new_screen = env.screen.clone();
                let msg = Message::new(
                    0, // get_next_seq(&mut seq), TODO
                    0,
                    ts,
                    crate::core::event::Event::Draw {
                        screen: std::sync::Arc::new(RwLock::new(Box::new(new_screen))),
                    },
                );

                {
                    crate::core::event::pending_render_event_inc(1);
                    self.ui_tx.clone().unwrap().send(msg).unwrap_or(());
                }

                // TO EOF
                env.screen.clear();
                env.screen.push(&cpi);
                env.screen.first_offset = offset; // restart
                self.first_offset = offset;
                self.last_offset = offset;
            } else {
                // screen full ? stop here
                dbg_println!("env.screen.push -> false, cpi {:?}", cpi);
                env.quit = true;
                return false;
            }
        }

        true
    }
}

//

impl ContentFilter<'_> for ScreenFilter {
    fn name(&self) -> &'static str {
        &"ScreenFilter"
    }

    fn setup(
        &mut self,
        editor: &mut Editor<'static>,
        _editor_env: &mut EditorEnv<'static>,

        _env: &mut LayoutEnv,
        _view: &Rc<RwLock<View>>,
        _parent_view: Option<&View<'static>>,
    ) {
        self.first_offset = None;
        self.last_offset = None;

        self.screen_is_full = false;

        self.ui_tx = Some(editor.ui_tx.clone());

        dbg_println!("SCREEN : SETUP");
    }

    fn run(
        &mut self,
        _view: &View,
        env: &mut LayoutEnv,
        filter_in: &[FilterIo],
        _filter_out: &mut Vec<FilterIo>,
    ) {
        /*
                dbg_println!(
                    "screen.push_available({}) + screen.push_count({}) == screen.push_capacity({})",
                    env.screen.push_available(),
                    env.screen.push_count(),
                    env.screen.push_capacity()
                );

                dbg_println!(
                    "ScreenFilter :  env.screen.push_available(); {}",
                    env.screen.push_available()
                );
        */

        env.screen.check_invariants();

        for io in filter_in.iter() {
            match &io {
                &FilterIo {
                    data: FilterData::CustomLimitReached,
                    ..
                } => {
                    dbg_println!("screen filler FilterData::CustomLimitReached");

                    env.quit = true;
                    break;
                }

                &FilterIo {
                    data: FilterData::EndOfStream,
                    offset,
                    ..
                } => {
                    let mut style = TextStyle::new();
                    style.color = (255, 255, 0);

                    let eof_char = if self.display_eof { '$' } else { ' ' };

                    let eof_cpi = CodepointInfo {
                        used: true,
                        metadata: true,
                        cp: u32_to_char(eof_char as u32),
                        displayed_cp: u32_to_char(eof_char as u32),
                        offset: offset.clone(),
                        size: 0,
                        skip_render: false,
                        style,
                    };
                    dbg_println!("add EOF to stream {:?}", io.offset);
                    let ret = env.screen.push(&eof_cpi);
                    env.screen.check_invariants();
                    if !ret.0 {
                        env.quit = true;
                        break;
                    }
                    env.screen.set_has_eof();
                }

                //////////
                &FilterIo {
                    data:
                        FilterData::TextInfo {
                            real_cp,
                            displayed_cp,
                            ..
                        },
                    ..
                } => {
                    let cpi = CodepointInfo {
                        used: true,
                        metadata: io.metadata,
                        cp: u32_to_char(*real_cp),
                        displayed_cp: u32_to_char(*displayed_cp),
                        offset: io.offset.clone(),
                        size: io.size,
                        skip_render: false,
                        style: io.style,
                    };

                    let ret = self.add_text_to_screen(env, cpi, io.offset);
                    if !ret {
                        // return enum ScreenFull, etc ...
                        dbg_println!("self.add_text_to_screen -> false, cpi {:?}", cpi);
                        break;
                    }

                    match (self.last_offset, io.offset) {
                        (Some(prev), Some(off)) => assert!(prev <= off),
                        _ => {}
                    }

                    self.last_offset = io.offset;
                }

                //////////
                &FilterIo {
                    data: FilterData::UnicodeArray { vec },
                    offset,
                    ..
                } => {
                    let default_style = TextStyle::new();

                    let mut cur_offset = offset.unwrap();

                    // dbg_println!("FilterData::ByteArray {{ vec {:?} }} ", vec);

                    for u in vec.iter() {
                        // share with TextInfo

                        let cp = char::from_u32(u.cp).unwrap();
                        let cpi = CodepointInfo {
                            used: true,
                            metadata: io.metadata,
                            cp,
                            displayed_cp: cp,
                            offset: Some(cur_offset),
                            size: 1,
                            skip_render: false,
                            style: default_style.clone(),
                        };

                        let ret = self.add_text_to_screen(env, cpi, Some(cur_offset));
                        if !ret {
                            // return enum ScreenFull, etc ...
                            dbg_println!("self.add_text_to_screen -> false, cpi {:?}", cpi);
                            break;
                        }

                        cur_offset += 1;
                    }

                    // save last offset for next pass ?
                    self.last_offset = Some(cur_offset);
                }

                //////////
                &FilterIo {
                    data: FilterData::ByteArray { vec },
                    offset,
                    ..
                } => {
                    let default_style = TextStyle::new();

                    let mut cur_offset = offset.unwrap();

                    // dbg_println!("FilterData::ByteArray {{ vec {:?} }} ", vec);

                    for b in vec.iter() {
                        // share with TextInfo

                        let cpi = CodepointInfo {
                            used: true,
                            metadata: io.metadata,
                            cp: u32_to_char(*b as u32),
                            displayed_cp: u32_to_char(*b as u32),
                            offset: Some(cur_offset),
                            size: 1,
                            skip_render: false,
                            style: default_style.clone(),
                        };

                        let ret = self.add_text_to_screen(env, cpi, Some(cur_offset));
                        if !ret {
                            // return enum ScreenFull, etc ...
                            dbg_println!("self.add_text_to_screen -> false, cpi {:?}", cpi);
                            break;
                        }

                        cur_offset += 1;
                    }

                    // save last offset for next pass ?
                    self.last_offset = Some(cur_offset);
                }
            }
        }
    }

    fn finish(&mut self, _view: &View, env: &mut LayoutEnv) {
        //env.screen.finalize();

        env.screen.check_invariants();
        env.screen.buffer_max_offset = env.max_offset;
        //        assert_eq!(env.base_offset, env.screen.first_offset.unwrap()); // ?

        dbg_println!(
            "ScreenFilter finish :  env.screen.push_count(); {}, first_offset {:?}, last_offset {:?}",
            env.screen.push_count(),
            env.screen.first_offset, env.screen.last_offset
        );
    }
}
