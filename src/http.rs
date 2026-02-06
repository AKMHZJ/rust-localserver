use std::collections::HashMap;

#[derive(Debug)]
pub enum Method {
    GET,
    POST,
    DELETE,
    OTHER(String),
}

impl From<&str> for Method {
    fn from(s: &str) -> Self {
        match s {
            "GET" => Method::GET,
            "POST" => Method::POST,
            "DELETE" => Method::DELETE,
            _ => Method::OTHER(s.to_string()),
        }
    }
}

#[derive(Debug)]
pub struct Request {
    pub method: Method,
    pub path: String,
    pub version: String,
    pub headers: HashMap<String, String>,
    pub body: Vec<u8>,
}

impl Request {
    pub fn new() -> Self {
        Request {
            method: Method::GET,
            path: String::new(),
            version: String::new(),
            headers: HashMap::new(),
            body: Vec::new(),
        }
    }
}

#[derive(Debug, PartialEq)]
pub enum ParseState {
    RequestLine,
    Headers,
    Body,
    ChunkSize,
    ChunkData,
    ChunkTrailer,
    Done,
    Error,
}

pub struct Parser {
    pub state: ParseState,
    pub request: Request,
    buffer: Vec<u8>,
    chunk_size: usize,
}

impl Parser {
    pub fn new() -> Self {
        Parser {
            state: ParseState::RequestLine,
            request: Request::new(),
            buffer: Vec::new(),
            chunk_size: 0,
        }
    }

    pub fn parse(&mut self, data: &[u8]) {
        self.buffer.extend_from_slice(data);

        loop {
            match self.state {
                ParseState::RequestLine => {
                    if let Some(pos) = self.buffer.windows(2).position(|w| w == b"\r\n") {
                        let line = String::from_utf8_lossy(&self.buffer[..pos]);
                        let parts: Vec<&str> = line.split_whitespace().collect();
                        if parts.len() == 3 {
                            self.request.method = Method::from(parts[0]);
                            self.request.path = parts[1].to_string();
                            self.request.version = parts[2].to_string();
                            self.state = ParseState::Headers;
                            self.buffer.drain(..pos + 2);
                        } else {
                            self.state = ParseState::Error;
                            return;
                        }
                    } else {
                        break;
                    }
                }
                ParseState::Headers => {
                    if let Some(pos) = self.buffer.windows(2).position(|w| w == b"\r\n") {
                        if pos == 0 {
                            self.buffer.drain(..2);
                            if let Some(te) = self.request.headers.get("Transfer-Encoding") {
                                if te.to_lowercase() == "chunked" {
                                    self.state = ParseState::ChunkSize;
                                } else {
                                    self.state = ParseState::Error;
                                    return;
                                }
                            } else if let Some(len_str) = self.request.headers.get("Content-Length") {
                                if let Ok(len) = len_str.parse::<usize>() {
                                    if len == 0 {
                                        self.state = ParseState::Done;
                                    } else {
                                        self.state = ParseState::Body;
                                    }
                                } else {
                                    self.state = ParseState::Error;
                                    return;
                                }
                            } else {
                                self.state = ParseState::Done;
                            }
                        } else {
                            let line = String::from_utf8_lossy(&self.buffer[..pos]);
                            if let Some(colon) = line.find(':') {
                                let key = line[..colon].trim().to_string();
                                let value = line[colon + 1..].trim().to_string();
                                self.request.headers.insert(key, value);
                            }
                            self.buffer.drain(..pos + 2);
                        }
                    } else {
                        break;
                    }
                }
                ParseState::Body => {
                    let content_length = self.request.headers.get("Content-Length")
                        .and_then(|l| l.parse::<usize>().ok())
                        .unwrap_or(0);
                    
                    if self.buffer.len() >= content_length {
                        self.request.body = self.buffer.drain(..content_length).collect();
                        self.state = ParseState::Done;
                    } else {
                        break;
                    }
                }
                ParseState::ChunkSize => {
                    if let Some(pos) = self.buffer.windows(2).position(|w| w == b"\r\n") {
                        let line = String::from_utf8_lossy(&self.buffer[..pos]);
                        if let Ok(size) = usize::from_str_radix(line.trim(), 16) {
                            self.chunk_size = size;
                            self.buffer.drain(..pos + 2);
                            if size == 0 {
                                self.state = ParseState::ChunkTrailer;
                            } else {
                                self.state = ParseState::ChunkData;
                            }
                        } else {
                            self.state = ParseState::Error;
                            return;
                        }
                    } else {
                        break;
                    }
                }
                ParseState::ChunkData => {
                    if self.buffer.len() >= self.chunk_size + 2 {
                        self.request.body.extend_from_slice(&self.buffer[..self.chunk_size]);
                        self.buffer.drain(..self.chunk_size + 2);
                        self.state = ParseState::ChunkSize;
                    } else {
                        break;
                    }
                }
                ParseState::ChunkTrailer => {
                    if let Some(pos) = self.buffer.windows(2).position(|w| w == b"\r\n") {
                        if pos == 0 {
                            self.buffer.drain(..2);
                            self.state = ParseState::Done;
                        } else {
                            self.buffer.drain(..pos + 2);
                        }
                    } else {
                        break;
                    }
                }
                ParseState::Done | ParseState::Error => break,
            }
        }
    }
}

pub struct Response {
    pub status_code: u16,
    pub headers: HashMap<String, String>,
    pub body: Vec<u8>,
}

impl Response {
    pub fn new(status_code: u16) -> Self {
        let mut headers = HashMap::new();
        headers.insert("Server".to_string(), "RustLocalServer/0.1.0".to_string());
        Response {
            status_code,
            headers,
            body: Vec::new(),
        }
    }

    pub fn to_bytes(&self) -> Vec<u8> {
        let status_text = match self.status_code {
            200 => "OK",
            201 => "Created",
            204 => "No Content",
            301 => "Moved Permanently",
            400 => "Bad Request",
            403 => "Forbidden",
            404 => "Not Found",
            405 => "Method Not Allowed",
            413 => "Payload Too Large",
            500 => "Internal Server Error",
            _ => "Unknown",
        };

        let mut resp = format!("HTTP/1.1 {} {}\r\n", self.status_code, status_text).into_bytes();
        for (key, value) in &self.headers {
            resp.extend_from_slice(format!("{}: {}\r\n", key, value).as_bytes());
        }
        resp.extend_from_slice(b"\r\n");
        resp.extend_from_slice(&self.body);
        resp
    }
}