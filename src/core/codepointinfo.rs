
/// contains displayed character attributes
/// The displayed screen is composed of CodepointInfo
#[derive(Default, Debug, Clone, Copy)]
pub struct CodepointInfo {
    pub cp: char,
    pub displayed_cp: char,
    pub offset: u64,
    pub is_selected: bool,
}


impl CodepointInfo {
    pub fn new() -> CodepointInfo {
        CodepointInfo {
            cp: ' ',
            displayed_cp: ' ',
            offset: 0,
            is_selected: false,
        }
    }
}
