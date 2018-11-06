/// A CodepointInfo contains displayed character attributes.<br/>
/// The displayed screen is composed of LineCell(s), that contain CodepointInfo(s).
#[derive(Default, Debug, Clone, Copy, Eq, PartialEq)]
pub struct CodepointInfo {
    pub cp: char,
    pub displayed_cp: char,
    pub offset: u64,
    pub is_selected: bool,
}

impl CodepointInfo {
    pub fn new() -> Self {
        CodepointInfo {
            cp: ' ',
            displayed_cp: ' ',
            offset: 0,
            is_selected: false,
        }
    }
}
