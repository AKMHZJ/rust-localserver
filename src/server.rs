use crate::config::Config;
use mio::net::{TcpListener, TcpStream};
use mio::{Events, Interest, Poll, Token};
use std::collections::HashMap;
use std::io::{self, Read, Write};
use std::net::SocketAddr;

use crate::http::{Parser, ParseState, Response};
use crate::router::Router;
use std::time::{Duration, Instant};

const TIMEOUT: Duration = Duration::from_secs(30);

pub struct Server {
    poll: Poll,
    listeners: Vec<(TcpListener, Token)>,
    connections: HashMap<Token, Connection>,
    next_token: usize,
    router: Router,
}

struct Connection {
    socket: TcpStream,
    parser: Parser,
    response_buf: Vec<u8>,
    is_closing: bool,
    last_activity: Instant,
}

impl Server {
    pub fn new(config: Config) -> io::Result<Self> {
        let poll = Poll::new()?;
        let mut listeners = Vec::new();
        let next_token = 100; // Start high to avoid conflicts with listeners
        
        for server_cfg in &config.servers {
            for port in &server_cfg.ports {
                let addr: SocketAddr = format!("{}:{}", server_cfg.host, port).parse()
                    .map_err(|e| io::Error::new(io::ErrorKind::InvalidInput, e))?;
                
                let mut listener = TcpListener::bind(addr)?;
                let token = Token(listeners.len());
                
                poll.registry().register(&mut listener, token, Interest::READABLE)?;
                listeners.push((listener, token));
                println!("Listening on {}", addr);
            }
        }

        Ok(Server {
            poll,
            listeners,
            connections: HashMap::new(),
            next_token,
            router: Router::new(config),
        })
    }

    pub fn run(&mut self) -> io::Result<()> {
        let mut events = Events::with_capacity(1024);
        let mut buffer = [0; 4096];

        loop {
            self.poll.poll(&mut events, None)?;

            for event in events.iter() {
                let token = event.token();

                if token.0 < self.listeners.len() {
                    // New connection
                    loop {
                        match self.listeners[token.0].0.accept() {
                            Ok((mut socket, _)) => {
                                let conn_token = Token(self.next_token);
                                self.next_token += 1;

                                self.poll.registry().register(
                                    &mut socket,
                                    conn_token,
                                    Interest::READABLE | Interest::WRITABLE,
                                )?;

                                self.connections.insert(conn_token, Connection { 
                                    socket, 
                                    parser: Parser::new(),
                                    response_buf: Vec::new(),
                                    is_closing: false,
                                    last_activity: Instant::now(),
                                });
                            }
                            Err(ref e) if e.kind() == io::ErrorKind::WouldBlock => break,
                            Err(e) => return Err(e),
                        }
                    }
                } else if let Some(connection) = self.connections.get_mut(&token) {
                    connection.last_activity = Instant::now();
                    if event.is_readable() {
                        loop {
                            match connection.socket.read(&mut buffer) {
                                Ok(0) => {
                                    connection.is_closing = true;
                                    break;
                                }
                                Ok(n) => {
                                    connection.parser.parse(&buffer[..n]);
                                    if connection.parser.state == ParseState::Done {
                                        let response = self.router.handle(&connection.parser.request);
                                        connection.response_buf.extend_from_slice(&response.to_bytes());
                                        // Reset parser for next request (keep-alive support could be here)
                                        connection.parser = Parser::new();
                                    } else if connection.parser.state == ParseState::Error {
                                        let response = Response::new(400);
                                        connection.response_buf.extend_from_slice(&response.to_bytes());
                                        connection.is_closing = true;
                                        break;
                                    }
                                }
                                Err(ref e) if e.kind() == io::ErrorKind::WouldBlock => break,
                                Err(_) => {
                                    connection.is_closing = true;
                                    break;
                                }
                            }
                        }
                    }

                    if event.is_writable() && !connection.response_buf.is_empty() {
                        match connection.socket.write(&connection.response_buf) {
                            Ok(n) => {
                                connection.response_buf.drain(..n);
                            }
                            Err(ref e) if e.kind() == io::ErrorKind::WouldBlock => {}
                            Err(_) => {
                                connection.is_closing = true;
                            }
                        }
                    }
                }
            }

            // Cleanup closed or timed-out connections
            let now = Instant::now();
            self.connections.retain(|_, conn| {
                if (conn.is_closing && conn.response_buf.is_empty()) || now.duration_since(conn.last_activity) > TIMEOUT {
                    false
                } else {
                    true
                }
            });
        }
    }
}
