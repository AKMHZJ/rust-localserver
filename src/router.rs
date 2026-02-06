use crate::config::{Config, RouteConfig, ServerConfig};
use crate::http::{Request, Response, Method};
use crate::cgi::CgiHandler;
use crate::error::generate_error_response;
use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};

pub struct Router {
    config: Config,
}

impl Router {
    pub fn new(config: Config) -> Self {
        Router { config }
    }

    pub fn handle(&self, request: &Request) -> Response {
        let host = request.headers.get("Host").cloned().unwrap_or_default();
        let server_cfg = self.config.servers.iter().find(|s| {
            if let Some(names) = &s.server_names {
                names.iter().any(|n| host.contains(n))
            } else {
                true
            }
        }).unwrap_or(&self.config.servers[0]);

        let route = match self.find_route(server_cfg, &request.path) {
            Some(r) => r,
            None => return generate_error_response(404, server_cfg),
        };

        if let Some(methods) = &route.methods {
            let method_str = match request.method {
                Method::GET => "GET",
                Method::POST => "POST",
                Method::DELETE => "DELETE",
                Method::OTHER(ref s) => s,
            };
            if !methods.contains(&method_str.to_string()) {
                return generate_error_response(405, server_cfg);
            }
        }

        if let Some(redirect) = &route.redirect {
            let mut res = Response::new(301);
            res.headers.insert("Location".to_string(), redirect.clone());
            return res;
        }

        // Handle CGI
        if let Some(cgi_exts) = &route.cgi_extensions {
            let path = Path::new(&request.path);
            if let Some(ext) = path.extension().and_then(|e| e.to_str()) {
                let ext_dot = format!(".{}", ext);
                if let Some(interpreter) = cgi_exts.get(&ext_dot) {
                    let mut script_path = PathBuf::from(route.root.as_deref().unwrap_or("."));
                    let relative_path = request.path.strip_prefix(&route.path).unwrap_or(&request.path);
                    script_path.push(relative_path.trim_start_matches('/'));

                    return self.handle_cgi(request, script_path.to_str().unwrap(), interpreter);
                }
            }
        }

        // Handle Uploads (simplified)
        if matches!(request.method, Method::POST) && route.allow_uploads.unwrap_or(false) {
            return self.handle_upload(request, route);
        }

        // Handle DELETE
        if matches!(request.method, Method::DELETE) {
            return self.handle_delete(request, route, server_cfg);
        }

        // Static file serving
        if let Some(root) = &route.root {
            let mut path = PathBuf::from(root);
            let relative_path = request.path.strip_prefix(&route.path).unwrap_or(&request.path);
            path.push(relative_path.trim_start_matches('/'));

            if path.is_dir() {
                if let Some(index) = &route.index {
                    path.push(index);
                } else if route.autoindex.unwrap_or(false) {
                    return self.list_directory(&path);
                }
            }

            match fs::read(&path) {
                Ok(content) => {
                    let mut res = Response::new(200);
                    res.body = content;
                    if let Some(ext) = path.extension().and_then(|e| e.to_str()) {
                        let mime = match ext {
                            "html" => "text/html",
                            "css" => "text/css",
                            "js" => "application/javascript",
                            "png" => "image/png",
                            _ => "application/octet-stream",
                        };
                        res.headers.insert("Content-Type".to_string(), mime.to_string());
                    }
                    res.headers.insert("Content-Length".to_string(), res.body.len().to_string());
                    return res;
                }
                Err(_) => return generate_error_response(404, server_cfg),
            }
        }

        generate_error_response(404, server_cfg)
    }

    fn handle_cgi(&self, request: &Request, script_path: &str, interpreter: &str) -> Response {
        let handler = CgiHandler::new(script_path.to_string(), interpreter.to_string());
        let mut env_vars = HashMap::new();
        env_vars.insert("REQUEST_METHOD".to_string(), format!("{:?}", request.method));
        env_vars.insert("PATH_INFO".to_string(), request.path.clone());
        if let Some(len) = request.headers.get("Content-Length") {
            env_vars.insert("CONTENT_LENGTH".to_string(), len.clone());
        }

        match handler.execute(env_vars, &request.body) {
            Ok(output) => {
                let mut res = Response::new(200);
                res.body = output;
                res.headers.insert("Content-Length".to_string(), res.body.len().to_string());
                res
            }
            Err(e) => {
                let mut res = Response::new(500);
                res.body = format!("CGI Error: {}", e).into_bytes();
                res
            }
        }
    }

    fn handle_upload(&self, request: &Request, route: &RouteConfig) -> Response {
        // In a real server, we'd parse multipart/form-data. 
        // For simplicity, we'll save the whole body as a file if a filename header is present or use a default.
        let filename = request.headers.get("X-Filename").cloned().unwrap_or_else(|| "uploaded_file".to_string());
        let mut path = PathBuf::from(route.root.as_deref().unwrap_or("static/uploads"));
        path.push(filename);

        if let Some(parent) = path.parent() {
            let _ = fs::create_dir_all(parent);
        }

        match fs::write(&path, &request.body) {
            Ok(_) => {
                let mut res = Response::new(201);
                res.body = b"File uploaded successfully".to_vec();
                res
            }
            Err(e) => {
                let mut res = Response::new(500);
                res.body = format!("Upload Error: {}", e).into_bytes();
                res
            }
        }
    }

    fn handle_delete(&self, request: &Request, route: &RouteConfig, server_cfg: &ServerConfig) -> Response {
        let mut path = PathBuf::from(route.root.as_deref().unwrap_or("."));
        let relative_path = request.path.strip_prefix(&route.path).unwrap_or(&request.path);
        path.push(relative_path.trim_start_matches('/'));

        if path.exists() && path.is_file() {
            match fs::remove_file(path) {
                Ok(_) => Response::new(204),
                Err(_) => generate_error_response(500, server_cfg),
            }
        } else {
            generate_error_response(404, server_cfg)
        }
    }

    fn find_route<'a>(&self, server: &'a crate::config::ServerConfig, path: &str) -> Option<&'a RouteConfig> {
        // Longest prefix match
        server.routes.iter()
            .filter(|r| path.starts_with(&r.path))
            .max_by_key(|r| r.path.len())
    }

    fn list_directory(&self, path: &PathBuf) -> Response {
        let mut html = String::from("<html><body><ul>");
        if let Ok(entries) = fs::read_dir(path) {
            for entry in entries.flatten() {
                if let Ok(name) = entry.file_name().into_string() {
                    html.push_str(&format!("<li><a href=\"{}\">{}</a></li>", name, name));
                }
            }
        }
        html.push_str("</ul></body></html>");
        
        let mut res = Response::new(200);
        res.body = html.into_bytes();
        res.headers.insert("Content-Type".to_string(), "text/html".to_string());
        res.headers.insert("Content-Length".to_string(), res.body.len().to_string());
        res
    }
}
