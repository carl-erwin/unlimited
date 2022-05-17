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

        // (160, 160, 160)

        // (180, 180, 180)

        (199, 199, 199)
    }

    pub fn default_bg_color() -> (u8, u8, u8) {
        // (35, 35, 35) // black
        // (0, 0, 48)
        // (38, 41, 44)
        // (40, 44, 52) // soft grey
        (11, 16, 39) // deep blue
    }

    pub fn title_color() -> (u8, u8, u8) {
        Self::default_bg_color()
    }

    pub fn title_bg_color() -> (u8, u8, u8) {
        Self::default_color()
    }

    pub fn default_selected_bg_color() -> (u8, u8, u8) {
        let c = Self::default_bg_color();
        let add = 20;
        (c.0 + add, c.1 + add, c.2 + add)
    }

    pub fn default_mark_line_bg_color() -> (u8, u8, u8) {
        Self::default_selected_bg_color()
    }
}

/// A CodepointInfo contains displayed character attributes.<br/>
/// The displayed screen is composed of LineCell(s), that contain CodepointInfo(s).
#[derive(Hash, Default, Debug, Clone, Copy, Eq, PartialEq)]
pub struct CodepointInfo {
    /// Ignore this CodepointInfo if set to false.
    pub used: bool, // rename into ignore ?
    /// The CodepointInfo is some part metadata.<br/>
    /// When set to true offset must be set to None
    pub metadata: bool, // TODO(ceg): prefer enum use enum { Normal, Eof, ... }
    /// The real codepoint found on storage
    pub cp: char,
    /// The codepoint to display
    pub displayed_cp: char,
    /// Storage offset
    pub offset: Option<u64>,
    /// Storage size in bytes
    pub size: usize,
    /// Hints for render
    pub skip_render: bool,
    /// Style to apply when rendering
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
