// Copyright (c) Carl-Erwin Griffith
//
// Permission is hereby granted, free of charge, to any
// person obtaining a copy of this software and associated
// documentation files (the "Software"), to deal in the
// Software without restriction, including without
// limitation the rights to use, copy, modify, merge,
// publish, distribute, sublicense, and/or sell copies of
// the Software, and to permit persons to whom the Software
// is furnished to do so, subject to the following
// conditions:
//
// The above copyright notice and this permission notice
// shall be included in all copies or substantial portions
// of the Software.
//
// THE SOFTWARE IS PROVIDED "AS IS", WITHOUT WARRANTY OF
// ANY KIND, EXPRESS OR IMPLIED, INCLUDING BUT NOT LIMITED
// TO THE WARRANTIES OF MERCHANTABILITY, FITNESS FOR A
// PARTICULAR PURPOSE AND NONINFRINGEMENT. IN NO EVENT
// SHALL THE AUTHORS OR COPYRIGHT HOLDERS BE LIABLE FOR ANY
// CLAIM, DAMAGES OR OTHER LIABILITY, WHETHER IN AN ACTION
// OF CONTRACT, TORT OR OTHERWISE, ARISING FROM, OUT OF OR
// IN CONNECTION WITH THE SOFTWARE OR THE USE OR OTHER

/// A CodepointInfo contains displayed character attributes.<br/>
/// The displayed screen is composed of LineCell(s), that contain CodepointInfo(s).

#[derive(Hash, Default, Debug, Clone, Copy, Eq, PartialEq)]
pub struct CodepointInfo {
    pub metadata: bool, // offset cannot be used
    pub cp: char,
    pub displayed_cp: char,
    pub offset: u64,
    pub is_selected: bool,
    pub color: (u8, u8, u8), // (R,G,B)
}

impl CodepointInfo {
    pub fn default_color() -> (u8, u8, u8) {
        (192, 192, 192)
    }

    pub fn new() -> Self {
        CodepointInfo {
            metadata: false,
            cp: ' ',
            displayed_cp: ' ',
            offset: 0,
            is_selected: false,
            color: CodepointInfo::default_color(),
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
