# Rust Local Server - Audit Answers

This document provides answers and justifications for the project audit, referencing the source code implementation.

## Functional

### How does an HTTP server work?
An HTTP server listens for TCP connections on a specific port. When a client connects, the server reads the raw bytes, parses them according to the HTTP/1.1 protocol (Request Line, Headers, Body), processes the request (routing, CGI, static files), and sends back a formatted HTTP response.

### Which function was used for I/O Multiplexing and how does it work?
**Function:** `mio::Poll::poll` (in `src/server.rs`).
**Explanation:** `mio` provides a cross-platform wrapper around OS-level selectors like `epoll` (Linux), `kqueue` (macOS), or IOCP (Windows). The `poll` function blocks the thread until one or more registered events (like a socket becoming readable or writable) occur. This allows a single thread to monitor thousands of connections efficiently.

### Is the server using only one select/epoll to read/write?
**Yes.**
**Code Reference:** `src/server.rs`. A single `Poll` instance is created in `Server::new` and reused in the `run` loop. All client sockets and listeners are registered to this single registry.

### Why is it important to use only one select/epoll?
Using a single event loop avoids the overhead of context switching and thread synchronization (locks/mutexes) associated with multi-threaded models. It maximizes CPU efficiency for I/O-bound tasks.

### Read/Write per client per select/epoll?
**Yes.**
**Code Reference:** Inside the `run` loop in `src/server.rs`:
-   **Reading:** When `event.is_readable()` is true, the code loops `connection.socket.read` until it returns `WouldBlock` or 0 (EOF). This ensures all available data is drained from the kernel buffer.
-   **Writing:** When `event.is_writable()` is true, it attempts to write the response buffer until `WouldBlock` or the buffer is empty.

### Are return values for I/O functions checked properly?
**Yes.**
**Code Reference:**
-   `Ok(n)`: Data processed.
-   `Ok(0)`: Connection closed (EOF).
-   `Err(ref e) if e.kind() == io::ErrorKind::WouldBlock`: Valid case for non-blocking I/O; loop breaks and waits for next event.
-   `Err(_)`: Other errors cause the connection to be marked for closing.

### If an error is returned, is the client removed?
**Yes.**
**Code Reference:** In `src/server.rs`, if a read/write error occurs (other than `WouldBlock`), `connection.is_closing` is set to `true`. The cleanup logic at the end of the loop removes connections where `is_closing` is true and the response buffer is empty.

### Is writing and reading ALWAYS done through a select/epoll?
**Yes.**
All I/O operations happen inside the loop iterating over `events` returned by `poll`.

## Configuration File

### Setup single/multiple servers and ports?
**Yes.**
**Code Reference:** `src/config.rs` parses a list of `ServerConfig` objects. `src/server.rs` iterates over this list and binds a `TcpListener` for every port defined in the configuration.

### Setup multiple hostnames?
**Yes.**
**Code Reference:** `src/router.rs` checks the `Host` header of the incoming request against the `server_names` vector in the config to select the correct server block.

### Setup custom error pages?
**Yes.**
**Code Reference:** `src/error.rs` checks `server_cfg.error_pages` for a path corresponding to the status code. If found, it loads that file; otherwise, it uses a default string.

### Limit client body size?
**Status:** Configuration exists (`client_max_body_size` in `src/config.rs`), but the enforcement logic inside `src/http.rs` or `src/router.rs` appears to be simplified or missing in the current snippet.
*Note for defense: The `Content-Length` header is parsed, and a robust implementation would check this against the config before reading the body.*

### Setup routes, default files, and accepted methods?
**Yes.**
**Code Reference:** `src/router.rs` implements `find_route` (longest prefix match), checks `route.methods`, and serves `route.index` (default file) or generates a directory listing if `autoindex` is true.

## Methods and Cookies

### GET / POST / DELETE?
**Yes.**
**Code Reference:** `src/router.rs` handles these explicitly:
-   **GET:** Serves static files or directory listings.
-   **POST:** Handled for CGI and uploads (`handle_upload`).
-   **DELETE:** Implemented in `handle_delete` using `fs::remove_file`.

### Wrong request handling?
**Yes.**
-   **Bad Request:** `Parser` sets state to `ParseState::Error`, returning 400.
-   **Method Not Allowed:** `Router` returns 405.
-   **Not Found:** `Router` returns 404.

### Uploads?
**Yes.**
**Code Reference:** `handle_upload` in `src/router.rs` saves the request body to a file.

### Session and Cookies?
**Status:** **Not Implemented.**
The files `src/utils/cookie.rs` and `src/utils/session.rs` are present but empty. The server parses headers but does not currently generate or validate session tokens.

## Interaction with Browser

### Browser connection?
**Yes.** Standard HTTP/1.1 response headers are sent (`Server`, `Content-Length`, `Content-Type`), allowing browsers to render pages correctly.

### CGI?
**Yes.**
**Code Reference:** `src/cgi.rs` uses `std::process::Command` to execute scripts. Environment variables (RFC 3875) like `REQUEST_METHOD` and `PATH_INFO` are set, and the body is piped to stdin.

## Port Issues

### Multiple ports/servers?
**Yes.**
The architecture supports binding multiple listeners. If a port is already in use (e.g., same port configured twice), `TcpListener::bind` will return an error, which `Server::new` propagates, causing the program to exit with an error message (safe failure).

## Siege & Stress Test

### Availability?
The server uses non-blocking I/O (`mio`), which is designed for high concurrency. It should handle `siege` tests without crashing, assuming system file descriptor limits are not hit.

### Memory Leaks?
Rust's ownership model guarantees memory safety. The server cleans up `connections` in the hash map explicitly when they close or timeout, preventing memory growth over time.
