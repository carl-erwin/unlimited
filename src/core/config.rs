use std::collections::HashMap;

pub type ConfigVariables = HashMap<String, String>;

#[derive(Debug, Clone)]
pub struct Config {
    pub files_list: Vec<String>,
    pub ui_frontend: String,
    pub vars: ConfigVariables,
}
