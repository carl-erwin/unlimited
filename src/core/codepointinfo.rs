// Copyright (c) Carl-Erwin Griffith

/// A CodepointInfo contains displayed character attributes.<br/>
/// The displayed screen is composed of LineCell(s), that contain CodepointInfo(s).

#[derive(Hash, Default, Debug, Clone, Copy, Eq, PartialEq)]
pub struct CodepointInfo {
    pub metadata: bool,      // offset cannot be used
    pub cp: char,            // the real codepoint
    pub displayed_cp: char,  // the displayed codepoint
    pub offset: Option<u64>, //
    // TODO: add n/m fragments ie tabs ?
    // TODO: add real_size ? in bytes

    // regroup in DisplayStyle ?
    pub is_selected: bool,
    // TODO: add underline
    // TODO: add bold ?
    // TODO: add italic ?
    pub color: (u8, u8, u8),    // (R,G,B)
    pub bg_color: (u8, u8, u8), // (R,G,B)
}

impl CodepointInfo {
    pub fn default_color() -> (u8, u8, u8) {
        // (192, 192, 192) // White
        // (128, 128, 128)    // Gray
        (177, 177, 177)
    }

    pub fn default_bg_color() -> (u8, u8, u8) {
        (0, 0, 0)
    }

    pub fn default_selected_bg_color() -> (u8, u8, u8) {
        (56, 56, 83)
    }

    pub fn new() -> Self {
        CodepointInfo {
            metadata: false,
            cp: ' ',
            displayed_cp: ' ',
            offset: None,
            is_selected: false,
            color: CodepointInfo::default_color(),
            bg_color: CodepointInfo::default_bg_color(),
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
