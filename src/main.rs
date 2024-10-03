mod endpoints;

use rust_http_server::ThreadPool;
use serde::{Deserialize, Serialize};
use std::{
    collections::HashMap,
    fs,
    io::{prelude::*, BufReader, Cursor},
    net::{TcpListener, TcpStream},
    time::{SystemTime, Duration},
    thread,
};
use chrono::{DateTime, Utc, TimeZone};
use thiserror::Error;

#[derive(Error, Debug)]
enum RequestError {
    #[error("Failed to read request line")]
    ReadRequestLineError,
    #[error("Invalid request line format")]
    InvalidRequestLineFormat,
    #[error("Failed to read header line")]
    ReadHeaderLineError,
    #[error("Invalid header line: {0}")]
    InvalidHeaderLine(String),
    #[error("Content-Length exceeds available data")]
    ContentLengthExceedsData,
    #[error("Body length does not match Content-Length header")]
    BodyLengthMismatch,
    #[error("Failed to read body")]
    ReadBodyError,
    #[error("Invalid Content-Length value")]
    InvalidContentLength,
}

#[derive(Serialize, Deserialize, Debug)]
struct Data {
    id: u32,
    name: String,
    password: String,
}

fn main() {
    let listener = TcpListener::bind("127.0.0.1:7878").unwrap();
    let pool = ThreadPool::new(5);
    for stream in listener.incoming() {
        let stream = stream.unwrap();

        pool.execute(|| {
            handle_connection(stream);
        });
    }
}

fn parse_cookies(headers: &HashMap<String, String>) -> HashMap<String, String> {
    let mut cookies = HashMap::new();
    if let Some(cookie_header) = headers.get("Cookie") {
        for cookie in cookie_header.split(';') {
            let parts: Vec<&str> = cookie.splitn(2, '=').collect();
            if parts.len() == 2 {
                cookies.insert(parts[0].trim().to_string(), parts[1].trim().to_string());
            }
        }
    }
    cookies
}

fn set_cookie(cookies: &mut Vec<String>, name: &str, value: &str, expires: Option<&str>) {
    let mut cookie = format!("{}={}; Path=/; HttpOnly", name, value);
    if let Some(expiration_date) = expires {
        cookie = format!("{}; Expires={}", cookie, expiration_date);
    }
    cookies.push(cookie);
}

fn is_cookie_expired(expiration_date: &str) -> bool {
    if let Ok(expiration) = DateTime::parse_from_rfc2822(expiration_date) {
        return expiration < Utc::now();
    }
    false
}

fn parse_request(
    buf_reader: &mut BufReader<&mut TcpStream>,
) -> std::result::Result<(String, String, HashMap<String, String>, String), RequestError> {
    let mut request_line = String::new();
    if buf_reader.read_line(&mut request_line).is_err() {
        return Err(RequestError::ReadRequestLineError);
    }

    let parts: Vec<&str> = request_line.split_whitespace().collect();
    if parts.len() < 2 {
        return Err(RequestError::InvalidRequestLineFormat);
    }

    let method = parts[0].to_string();
    let uri = parts[1].to_string();

    // Read headers
    let mut headers = HashMap::new();
    loop {
        let mut header_line = String::new();
        if buf_reader.read_line(&mut header_line).is_err() {
            return Err(RequestError::ReadHeaderLineError);
        }

        if header_line == "\r\n" || header_line.is_empty() {
            break;
        }

        let header_parts: Vec<&str> = header_line.splitn(2, ": ").collect();
        if header_parts.len() == 2 {
            headers.insert(
                header_parts[0].to_string(),
                header_parts[1].trim().to_string(),
            );
        } else {
            return Err(RequestError::InvalidHeaderLine(header_line));
        }
    }

    // Read body based on Content-Length header
    let mut body = String::new();
    if let Some(content_length) = headers.get("Content-Length") {
        if let Ok(length) = content_length.parse::<usize>() {
            let available_data = buf_reader.buffer().len();
            if length > available_data {
                return Err(RequestError::ContentLengthExceedsData);
            }

            let mut buffer = vec![0; length];
            if buf_reader.read_exact(&mut buffer).is_ok() {
                body = String::from_utf8_lossy(&buffer).to_string();
                if body.len() != length {
                    return Err(RequestError::BodyLengthMismatch);
                }
            } else {
                return Err(RequestError::ReadBodyError);
            }
        } else {
            return Err(RequestError::InvalidContentLength);
        }
    }

    // Check Content-Type and parse body accordingly
    if let Some(content_type) = headers.get("Content-Type") {
        match content_type.as_str() {
            "application/json" => {
                // Handle JSON body
                if let Err(e) = serde_json::from_str::<serde_json::Value>(&body) {
                    return Err(RequestError::InvalidRequestLineFormat);
                }
            }
            "text/plain" => {
                // Handle plain text body
                // No additional parsing needed for plain text
            }
            _ => {
                return Err(RequestError::InvalidRequestLineFormat);
            }
        }
    }

    Ok((method, uri, headers, body))
}

fn handle_connection(mut stream: TcpStream) {
    println!("New Connection");
    let mut buf_reader = BufReader::new(&mut stream);
    let (method, uri, headers, body) = match parse_request(&mut buf_reader) {
        Ok(result) => result,
        Err(e) => {
            eprintln!("Failed to parse request: {}", e);
            return;
        }
    };
    println!("Method: {}, URI: {}", method, uri);
    println!("Headers: {:?}", headers);
    println!("Body: {}", body);

    // Parse cookies from the request
    let cookies = parse_cookies(&headers);
    println!("Cookies: {:?}", cookies);

    // Parse cookies from the request
    let cookies = parse_cookies(&headers);
    let mut valid_cookies = HashMap::new();
    for (name, value) in cookies {
        if !is_cookie_expired(&value) {
            valid_cookies.insert(name, value);
        }
    }
    println!("Valid Cookies: {:?}", valid_cookies);

    // Prepare response headers
    let mut response_headers: HashMap<String, String> = HashMap::new();
    let mut set_cookie_headers = Vec::new();

    // Set a cookie expiration time
    let expiration_time = SystemTime::now() + Duration::from_secs(30);
    let datetime: DateTime<Utc> = DateTime::<Utc>::from(expiration_time);
    let expiration = datetime.format("%a, %d %b %Y %H:%M:%S GMT").to_string();

    // Set a cookie in the response
    set_cookie(&mut set_cookie_headers, "session", "123456", Some(expiration.as_str()));

    let (status_line, response_body) = match method.as_str() {
        "GET" => handle_get(&uri),
        "POST" => handle_post(&uri, &body),
        "PUT" => handle_put(&uri, &body),
        "DELETE" => handle_delete(&uri, &body),
        "PATCH" => handle_patch(&uri, &body),
        _ => (
            "HTTP/1.1 405 METHOD NOT ALLOWED",
            "405 - Method Not Allowed".to_string(),
        ),
    };

    let length = response_body.len();
    let mut response = format!("{status_line}\r\nContent-Length: {length}\r\n");

    for cookie in set_cookie_headers {
        response.push_str(&format!("Set-Cookie: {}\r\n", cookie));
    }

    response.push_str(&format!("\r\n{response_body}"));
    stream.write_all(response.as_bytes()).unwrap();
}

const SERVER_RESPONSE_OK: &str = "HTTP/1.1 200 OK";
const SERVER_RESPONSE_ERROR: &str = "HTTP/1.1 404 NOT FOUND";

fn handle_get(uri: &str) -> (&str, String) {
    match uri {
        "/" => (SERVER_RESPONSE_OK, "Welcome to the homepage!".to_string()),
        "/hello" => (SERVER_RESPONSE_OK, "Hello, world!".to_string()),
        "/data" => (SERVER_RESPONSE_OK, "Here is your data.".to_string()),
        "/entries" => (SERVER_RESPONSE_OK, endpoints::get_entries(0).to_string()),
        _ => ("HTTP/1.1 404 NOT FOUND", "404 - Not Found".to_string()),
    }
}

fn handle_post<'a>(uri: &'a str, body: &'a str) -> (&'a str, String) {
    match uri {
        "/submit" => (SERVER_RESPONSE_OK, endpoints::post_entry(body).to_string()),
        _ => (SERVER_RESPONSE_ERROR, "404 - Not Found".to_string()),
    }
}

fn handle_put<'a>(uri: &'a str, body: &'a str) -> (&'a str, String) {
    match uri {
        "/put_entry" => (SERVER_RESPONSE_OK, endpoints::put_entry(body).to_string()),
        _ => (SERVER_RESPONSE_ERROR, "404 - Not Found".to_string()),
    }
}

fn handle_patch<'a>(uri: &'a str, body: &'a str) -> (&'a str, String) {
    match uri {
        "/patch_entry_name" => (
            SERVER_RESPONSE_OK,
            endpoints::patch_entry_name(body).to_string(),
        ),
        _ => (SERVER_RESPONSE_ERROR, "404 - Not Found".to_string()),
    }
}

fn handle_delete<'a>(uri: &'a str, body: &'a str) -> (&'a str, String) {
    match uri {
        "/delete_entry" => (
            SERVER_RESPONSE_OK,
            endpoints::delete_entry(body).to_string(),
        ),
        _ => (SERVER_RESPONSE_ERROR, "404 - Not Found".to_string()),
    }
}

fn send_request(request: &str) -> String {
    // Establish a connection to the server
    let mut stream = TcpStream::connect("127.0.0.1:7878").expect("Could not connect to server");

    // Send the request
    stream.write_all(request.as_bytes()).unwrap();

    // Read the response
    let mut response = String::new();
    stream.read_to_string(&mut response).unwrap();
    response
}

fn start_server() -> std::sync::Arc<std::sync::Mutex<bool>> {
    let running = std::sync::Arc::new(std::sync::Mutex::new(true));
    let running_clone = running.clone();

    thread::spawn(move || {
        let listener = TcpListener::bind("127.0.0.1:7878").unwrap();
        let pool = ThreadPool::new(5);

        while *running_clone.lock().unwrap() {
            if let Ok((stream, _)) = listener.accept() {
                pool.execute(move || {
                    handle_connection(stream);
                });
            }
        }
    });

    running
}

#[cfg(test)]
mod tests {
    use std::{sync::mpsc, time::Duration};

    use super::*;

    // HTTP Operations Unit Tests
    #[test]
    fn test_get_entries() {
        let running = start_server();
        thread::sleep(Duration::from_secs(1)); // Allow some time for the server to start

        let request = "GET /entries HTTP/1.1\r\nHost: 127.0.0.1\r\n\r\n";
        let response = send_request(request);

        let expected_json = r#"{"id":3,"rank":"28,818","trend":"8","season":1,"episode":4,"name":"Luffy's Past! The Red-haired Shanks Appears!","start":1999,"total_votes":"449","average_rating":8.1}"#;

        assert!(response.contains(expected_json));

        *running.lock().unwrap() = false;
    }

    #[test]
    fn test_post_entry() {
        let running = start_server();
        thread::sleep(Duration::from_secs(1)); // Allow some time for the server to start

        let new_character = r#"{
            "id": 0,
            "rank": "32,043", 
            "trend": "7", 
            "season": 1, 
            "episode": 3, 
            "name": "Morgan vs. Luffy! Who's This Beautiful Young Girl?", 
            "start": 1999, 
            "total_votes": "428", 
            "average_rating": 7.7
        }"#;

        let request = format!(
            "POST /submit HTTP/1.1\r\nContent-Length: {}\r\n\r\n{}",
            new_character.len(),
            new_character
        );

        let response = send_request(&request);
        println!("Response:({})", response);
        assert!(response.contains("Success!"));

        // Stop the server after the test
        *running.lock().unwrap() = false;
    }

    #[test]
    fn test_put_entry() {
        let running = start_server();
        thread::sleep(Duration::from_secs(1)); // Allow some time for the server to start

        let updated_character = r#"{
            "id": 1,
            "rank": "32,043", 
            "trend": "7", 
            "season": 1, 
            "episode": 3, 
            "name": "Morgan vs. Luffy! Who's This Beautiful Young Girl?", 
            "start": 1999, 
            "total_votes": "428", 
            "average_rating": 7.7
        }"#;

        let request = format!(
            "PUT /put_entry HTTP/1.1\r\nContent-Length: {}\r\n\r\n{}",
            updated_character.len(),
            updated_character
        );

        let response = send_request(&request);
        println!("Response:({})", response);
        assert!(response.contains("Success!"));

        *running.lock().unwrap() = false; // Stop the server after the test
    }

    #[test]
    fn test_delete_entry() {
        let running = start_server();
        thread::sleep(Duration::from_secs(1)); // Allow some time for the server to start

        let delete_request = r#"{"id": 5}"#;

        let request = format!(
            "DELETE /delete_entry HTTP/1.1\r\nContent-Length: {}\r\n\r\n{}",
            delete_request.len(),
            delete_request
        );

        let response = send_request(&request);
        println!("Response:({})", response);
        assert!(response.contains("Success!"));

        *running.lock().unwrap() = false; // Stop the server after the test
    }

    #[test]
    fn test_patch_entry_name() {
        let running = start_server();
        thread::sleep(Duration::from_secs(1)); // Allow some time for the server to start

        let patch_request = r#"{
            "id": 1,
            "name": "Pirate King Luffy"
        }"#;

        let request = format!(
            "PATCH /patch_entry_name HTTP/1.1\r\nContent-Length: {}\r\n\r\n{}",
            patch_request.len(),
            patch_request
        );

        let response = send_request(&request);
        assert!(response.contains("Success!"));

        *running.lock().unwrap() = false; // Stop the server after the test
    }

    #[test]
    fn test_concurrent_post_requests() {
        // Start the server and get the running status Arc
        let running = start_server();
        thread::sleep(Duration::from_secs(1)); // Allow server to start

        let pool = ThreadPool::new(5);
        let num_requests = 4; // Number of concurrent requests
        let post_data = r#"{
        "id": 1,
        "rank": "Captain",
        "trend": "up",
        "season": 3,
        "episode": 20,
        "name": "Zoro",
        "start": 2,
        "total_votes": "200",
        "average_rating": 8.9,
        "password": "secret"
    }"#;

        let (tx, rx) = mpsc::channel(); // Create a channel for signaling completion

        for _ in 0..num_requests {
            let tx = tx.clone(); // Clone the transmitter for each thread
            let post_data = post_data.to_string();

            pool.execute(move || {
                println!("Thread {:?} started processing", thread::current().id());

                let request = format!(
                    "POST /submit HTTP/1.1\r\nContent-Length: {}\r\n\r\n{}",
                    post_data.len(),
                    post_data
                );

                // Send the request and receive the response
                let response = send_request(&request);
                println!(
                    "Response from thread {:?}: ({})",
                    thread::current().id(),
                    response
                );

                // Check the response
                assert!(response.contains("Success!"), "Response was: {}", response);

                // Signal task completion
                tx.send(()).unwrap();
                println!("Thread {:?} finished processing", thread::current().id());
            });
        }

        // Wait for all requests to complete
        for _ in 0..num_requests {
            rx.recv_timeout(Duration::from_secs(5))
                .expect("Test timed out");
        }

        // Stop the server by setting running to false
        *running.lock().unwrap() = false;

        // Give some time for the server to shut down properly
        thread::sleep(Duration::from_secs(1));
    }

    #[test]
    fn test_concurrent_get_requests() {
        // Start the server
        let running = start_server();
        thread::sleep(Duration::from_secs(1)); // Allow server to start

        let pool = ThreadPool::new(5);
        let num_requests = 5;

        let expected_jsons = r#"{"id":7,"rank":"38,371","trend":"6","season":1,"episode":8,"name":"Shousha wa docchi? Akuma no mi no nouryoku taiketsu!","start":1999,"total_votes":"335","average_rating":7.7}"#;

        let (tx, rx) = mpsc::channel(); // Create a channel to track task completion

        for i in 0..num_requests {
            let tx = tx.clone();

            pool.execute(move || {
                // Construct a GET request
                let request = format!(
                    "GET /entries HTTP/1.1\r\nHost: 127.0.0.1\r\nConnection: close\r\n\r\n"
                );

                // Send the request and receive the response
                let response = send_request(&request);

                // Check the status line and if the expected JSON is in the response body
                assert!(response.contains(expected_jsons));

                // Signal task completion
                tx.send(()).unwrap();
            });
        }

        // Wait for all requests to finish
        for _ in 0..num_requests {
            rx.recv_timeout(Duration::from_secs(5))
                .expect("Test timed out");
        }

        // Stop the server
        *running.lock().unwrap() = false;
        thread::sleep(Duration::from_secs(1)); // Allow server to shut down
    }
}
