use crate::config::ServerConfig;
use crate::http::Response;
use std::fs;

pub fn generate_error_response(status_code: u16, server_cfg: &ServerConfig) -> Response {
    let mut res = Response::new(status_code);
    
    if let Some(error_pages) = &server_cfg.error_pages {
        if let Some(path) = error_pages.get(&status_code) {
            if let Ok(content) = fs::read(path) {
                res.body = content;
                res.headers.insert("Content-Type".to_string(), "text/html".to_string());
                res.headers.insert("Content-Length".to_string(), res.body.len().to_string());
                return res;
            }
        }
    }

    // Default error body
    res.body = format!("<h1>{} Error</h1>", status_code).into_bytes();
    res.headers.insert("Content-Type".to_string(), "text/html".to_string());
    res.headers.insert("Content-Length".to_string(), res.body.len().to_string());
    res
}
