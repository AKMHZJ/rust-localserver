# Rust Local Server - Project Explanation

## Overview
This is a custom-built HTTP/1.1 server written in Rust, designed to handle high concurrency using non-blocking I/O. It avoids high-level async runtimes (like Tokio) in favor of low-level socket management with `mio`, giving you a deep understanding of how servers work "under the hood."

## Architecture

### 1. Event Loop (`src/server.rs`)
The heart of the server is the `Server` struct.
-   **Poll:** Uses `mio::Poll` to monitor multiple sockets (listeners and client connections) simultaneously.
-   **Token:** Each connection is assigned a unique `Token` to identify it in the event loop.
-   **Single-Threaded:** All processing happens in one thread. This eliminates race conditions but requires all operations (like parsing and routing) to be fast and non-blocking.

### 2. HTTP Parsing (`src/http.rs`)
Incoming data is fed into a `Parser` state machine.
-   It processes bytes as they arrive, transitioning between states: `RequestLine` -> `Headers` -> `Body` (or `ChunkSize` -> `ChunkData` for chunked encoding).
-   This allows the server to handle fragmented packets correctly without blocking.

### 3. Routing & Configuration (`src/router.rs`, `src/config.rs`)
-   **Config:** On startup, `config.yaml` is parsed to define servers, ports, and routes.
-   **Routing:** The `Router` matches the request path against defined routes (longest prefix match).
-   It handles:
    -   **Static Files:** Reading and serving files from the disk.
    -   **Directory Listing:** Generating HTML for directory contents if enabled.
    -   **Redirects:** Returning 301 responses.
    -   **Methods:** Enforcing allowed methods (e.g., rejecting POST on a GET-only route).

### 4. CGI (`src/cgi.rs`)
For dynamic content (like `.py` scripts), the server spawns a child process.
-   It sets environment variables (RFC 3875) like `REQUEST_METHOD`, `PATH_INFO`, and `CONTENT_LENGTH`.
-   The request body is piped to the script's Standard Input (stdin).
-   The script's Standard Output (stdout) is captured and sent back as the HTTP response.

## How to Run
1.  **Build:**
    ```bash
    cargo build --release
    ```
2.  **Run:**
    ```bash
    ./target/release/rust-localserver config.yaml
    ```

## Testing
-   **Static Files:** Open `http://localhost:8080/index.html` (assuming port 8080).
-   **CGI:** Access a python script mapped in your config, e.g., `http://localhost:8080/cgi-bin/hello.py`.
-   **Uploads:** Use `curl` or a form to POST data to an upload route.
    ```bash
    curl -X POST --data-binary @test.txt http://localhost:8080/upload
    ```
-   **Stress Test:**
    ```bash
    siege -b 127.0.0.1:8080
    ```

## Project Structure
-   `src/main.rs`: Entry point.
-   `src/server.rs`: Network event loop.
-   `src/http.rs`: Request/Response parsing and formatting.
-   `src/router.rs`: Business logic for handling requests.
-   `src/cgi.rs`: Interface for running external scripts.
-   `src/config.rs`: YAML configuration loader.
