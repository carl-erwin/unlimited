use crate::core::view::ScreenOverlayFilter;

use crate::core::view::LayoutEnv;

use crate::core::screen::Screen;
use crate::core::view::View;

use crate::core::editor::config_var_get;

use crate::core::editor::Editor;

use parking_lot::RwLock;
use std::rc::Rc;

pub struct TextRuler {
    columns: Vec<usize>,
}

impl TextRuler {
    pub fn new() -> Self {
        Self { columns: vec![] }
    }
}

impl ScreenOverlayFilter<'_> for TextRuler {
    fn name(&self) -> &'static str {
        &"TextRuler"
    }

    fn setup(
        &mut self,
        editor: &Editor<'static>,
        _env: &mut LayoutEnv,
        _view: &Rc<RwLock<View>>,
        _parent_view: Option<&View<'static>>,
    ) {
        if let Some(ruler_column) = config_var_get(&editor, "text-mode:ruler") {
            self.columns = ruler_column
                .trim_ascii_start()
                .trim_end()
                .split(',')
                .map(|s| s.parse::<usize>().unwrap_or(0))
                .collect();
        }
    }

    fn finish(&mut self, _view: &View, env: &mut LayoutEnv) {
        if env.screen.is_off_screen || self.columns.is_empty() {
            return;
        }

        draw_ruler(&mut env.screen, &self.columns);
    }
}

fn draw_ruler(screen: &mut Screen, columns: &Vec<usize>) {
    for l in 0..screen.height() {
        if let Some(line) = screen.get_line_mut(l) {
            for col in columns {
                if *col == 0 {
                    continue;
                }

                if let Some(cell) = line.get_mut(*col - 1) {
                    cell.cpi.style.bg_color = (40, 44, 52);
                }
            }
        }
    }
}
