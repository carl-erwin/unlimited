/// TextStyle holds the displayed attributes flags.
#[derive(Debug, Default, Hash, Clone, Copy, Eq, PartialEq)]
pub struct TextStyle {
    /// The character should blink.
    pub is_blinking: bool,
    /// The character is a part of a selection.
    pub is_selected: bool,
    /// The fg/bg attributes are reversed (used to simulate cursors/marks).
    pub is_inverse: bool,
    /// The character should be displayed as bold.
    pub is_bold: bool,
    /// The character should be displayed in italic.
    pub is_italic: bool,
    /// rbg tuple for foreground color
    pub color: (u8, u8, u8), // RGB
    /// rbg tuple for background color
    pub bg_color: (u8, u8, u8), // RGB
}

impl TextStyle {
    pub fn new() -> Self {
        TextStyle {
            is_blinking: false,
            is_selected: false,
            is_inverse: false,
            is_bold: false,
            is_italic: false,
            color: Self::default_color(),
            bg_color: Self::default_bg_color(),
        }
    }

    pub fn default_color() -> (u8, u8, u8) {
        // (192, 192, 192) // White
        // (128, 128, 128)    // Gray
        //(177, 177, 177)
        (160, 160, 160)
    }

    pub fn default_bg_color() -> (u8, u8, u8) {
        // (30, 34, 39)
        //(45, 55, 67)
        //(40, 44, 49)
        (39, 40, 54)
    }

    pub fn default_selected_bg_color() -> (u8, u8, u8) {
        let sbg = Self::default_bg_color();
        let add = 25;
        (sbg.0 + add, sbg.1 + add, sbg.2 + add)
    }

    pub fn default_mark_line_bg_color() -> (u8, u8, u8) {
        let sbg = Self::default_bg_color();
        let add = 5;
        (sbg.0 + add, sbg.1 + add, sbg.2 + add)
    }
}

/// A CodepointInfo contains displayed character attributes.<br/>
/// The displayed screen is composed of LineCell(s), that contain CodepointInfo(s).
#[derive(Hash, Default, Debug, Clone, Copy, Eq, PartialEq)]
pub struct CodepointInfo {
    pub used: bool,

    pub metadata: bool, // offset cannot be used, TODO(ceg): use enum to tag Eof, Normal

    // pub is_eof ?
    pub cp: char,            // the real codepoint
    pub displayed_cp: char,  // the displayed codepoint
    pub offset: Option<u64>, // TODO(ceg): Option<(u64, usize)>, back end size (codec)
    pub size: usize,         // TODO(ceg): Option<(u64, usize)>, back end size (codec)

    pub skip_render: bool,

    // TODO(ceg): add n/m fragments ie tabs ?
    // TODO(ceg): add real_size ? in bytes
    pub style: TextStyle,
}

impl CodepointInfo {
    pub fn new() -> Self {
        CodepointInfo {
            used: false,
            metadata: true,
            cp: ' ',
            displayed_cp: ' ',
            offset: None,
            size: 0,
            skip_render: false,
            //
            style: TextStyle::new(),
        }
    }
}

/*
Logical Color	Terminal Color	RGB Value Used by SGD
Black	0 0 0
Light_red	255 0 0
Light_green	0 255 0
Yellow	255 255 0
Light_blue	0 0 255
Light_magenta	255 0 255
Light_cyan	    0 255 255
High_white	    255 255 255
Gray	    128 128 128
Red	    128 0 0
Green	0 128 0
Brown	128 128 0
Blue	0 0 128
Magenta	128 0 128
Cyan	0 128 128
White	192 192 192
*/
