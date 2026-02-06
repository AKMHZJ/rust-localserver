mod config;
mod server;
mod router;
mod http;
mod cgi;
mod error;
mod utils {
    pub mod cookie;
    pub mod session;
}

use config::Config;
use std::env;
use std::process;

fn main() {
    env_logger::init();
    
    let args: Vec<String> = env::args().collect();
    let config_path = if args.len() > 1 {
        &args[1]
    } else {
        "config.yaml"
    };

    let config = match Config::from_file(config_path) {
        Ok(cfg) => cfg,
        Err(e) => {
            eprintln!("Error loading configuration: {}", e);
            process::exit(1);
        }
    };

    println!("Starting server with config from {}", config_path);
    
    let mut server = match server::Server::new(config) {
        Ok(s) => s,
        Err(e) => {
            eprintln!("Error initializing server: {}", e);
            process::exit(1);
        }
    };

    if let Err(e) = server.run() {
        eprintln!("Server error: {}", e);
        process::exit(1);
    }
}