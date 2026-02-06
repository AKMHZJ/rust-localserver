use serde::Deserialize;
use std::collections::HashMap;
use std::fs;
use std::path::Path;

#[derive(Debug, Deserialize, Clone)]
pub struct Config {
    pub servers: Vec<ServerConfig>,
}

#[derive(Debug, Deserialize, Clone)]
pub struct ServerConfig {
    pub host: String,
    pub ports: Vec<u16>,
    pub server_names: Option<Vec<String>>,
    pub error_pages: Option<HashMap<u16, String>>,
    pub client_max_body_size: Option<usize>,
    pub routes: Vec<RouteConfig>,
}

#[derive(Debug, Deserialize, Clone)]
pub struct RouteConfig {
    pub path: String,
    pub root: Option<String>,
    pub index: Option<String>,
    pub methods: Option<Vec<String>>,
    pub autoindex: Option<bool>,
    pub redirect: Option<String>,
    pub allow_uploads: Option<bool>,
    pub cgi_extensions: Option<HashMap<String, String>>,
}

impl Config {
    pub fn from_file<P: AsRef<Path>>(path: P) -> Result<Self, Box<dyn std::error::Error>> {
        let content = fs::read_to_string(path)?;
        let config: Config = serde_yaml::from_str(&content)?;
        Ok(config)
    }
}
