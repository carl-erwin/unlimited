use crate::core::view::ScreenOverlayFilter;

use crate::core::view::LayoutEnv;


use crate::core::screen::Screen;
use crate::core::view::View;

pub struct TextRuler {}

impl TextRuler {
    pub fn new() -> Self {
        Self {}
    }
}

impl ScreenOverlayFilter<'_> for TextRuler {
    fn name(&self) -> &'static str {
        &"TextRuler"
    }

    fn finish(&mut self, _view: &View, env: &mut LayoutEnv) {
        if env.screen.is_off_screen {
            return;
        }
        draw_ruler(&mut env.screen);
    }
}

fn draw_ruler(screen: &mut Screen) {
    for l in 0..screen.height() {
        if let Some(line) = screen.get_line_mut(l) {
            if let Some(cell) = line.get_mut(80) {
                cell.cpi.style.bg_color = (40, 44, 52);
            }
        }
    }
}
