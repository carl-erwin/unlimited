
// CodepointInfo contains displayed char attributes
#[derive(Debug, Clone, Copy)]
pub struct CodepointInfo {
    pub cp: char,
    pub displayed_cp: char,
    pub offset: u64,
    pub is_selected: bool,
}
