use std::collections::HashMap;

#[derive(Debug)]
pub struct Config {
    pub files_list: Vec<String>,
    pub ui_frontend: String,
    pub vars: HashMap<String, String>,
}
