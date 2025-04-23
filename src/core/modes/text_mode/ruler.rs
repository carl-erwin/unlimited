use crate::core::view::ScreenOverlayFilter;

use crate::core::view::LayoutEnv;

use crate::core::screen::Screen;
use crate::core::view::View;

use crate::core::editor::config_var_get;

use crate::core::editor::Editor;

use parking_lot::RwLock;
use std::rc::Rc;

pub struct TextRuler {
    column: usize,
}

impl TextRuler {
    pub fn new() -> Self {
        Self { column: 0 }
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
            let col = ruler_column
                .trim_ascii_start()
                .trim_end()
                .parse::<usize>()
                .unwrap_or(0);
            if col > 0 {
                self.column = col;
            }
        }
    }

    fn finish(&mut self, _view: &View, env: &mut LayoutEnv) {
        if env.screen.is_off_screen || self.column == 0 {
            return;
        }

        draw_ruler(&mut env.screen, self.column);
    }
}

fn draw_ruler(screen: &mut Screen, column: usize) {
    for l in 0..screen.height() {
        if let Some(line) = screen.get_line_mut(l) {
            if let Some(cell) = line.get_mut(column) {
                cell.cpi.style.bg_color = (40, 44, 52);
            }
        }
    }
}
