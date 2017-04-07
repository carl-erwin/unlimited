
// CodepointInfo contains displayed char attributes
#[derive(Debug)]
pub struct CodepointInfo {
    pub cp: char,
    pub displayed_cp: char,
    pub offset: u64,
}
