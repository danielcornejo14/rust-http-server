mod endpoints;

use chrono::{DateTime, TimeZone, Utc};
use rust_http_server::ThreadPool;
use serde::{Deserialize, Serialize};
use std::{
    collections::HashMap,
    fs,
    io::{prelude::*, BufReader, Cursor},
    net::{TcpListener, TcpStream},
    thread,
    time::{Duration, Instant, SystemTime},
};
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
        println!("1 {:?}", stream);
        let stream = stream.unwrap();
        println!("2 {:?}", stream);

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

fn get_cookie_expiration(duration_secs: u64) -> String {
    let expiration_time = SystemTime::now() + Duration::from_secs(duration_secs);
    let datetime: DateTime<Utc> = DateTime::<Utc>::from(expiration_time);
    datetime.format("%a, %d %b %Y %H:%M:%S GMT").to_string()
}

fn parse_request(
    buf_reader: &mut BufReader<&mut TcpStream>,
) -> std::result::Result<(String, String, HashMap<String, String>, String), RequestError> {
    let mut request_line = String::new();
    println!("Request Line: {:?}", buf_reader);
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
    let expiration_old = get_cookie_expiration(0);
    let expiration_new = get_cookie_expiration(30);

    // Set a cookie in the response
    set_cookie(
        &mut set_cookie_headers,
        "old_cookie",
        "won't_be_set",
        Some(expiration_old.as_str()),
    );
    set_cookie(
        &mut set_cookie_headers,
        "new_cookie",
        "will_be_set_but_won't_last_long",
        Some(expiration_new.as_str()),
    );

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

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashMap;
    use std::io::{BufReader, Cursor};
    use std::sync::mpsc;

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

    fn start_server() {
        // Check if the server is already running
        if TcpStream::connect("127.0.0.1:7878").is_ok() {
            return;
        }
        // Start the server in a separate thread
        thread::spawn(|| {
            let listener = TcpListener::bind("127.0.0.1:7878").unwrap();
            let pool = ThreadPool::new(5);
            for stream in listener.incoming() {
                let stream = stream.unwrap();

                pool.execute(|| {
                    handle_connection(stream);
                });
            }
        });
    }

    // HTTP Operations Unit Tests
    #[test]
    fn test_get_entries() {
        // Start the server
        start_server();
        thread::sleep(Duration::from_secs(1));

        // Send a GET request
        let request = "GET /entries HTTP/1.1\r\nHost: 127.0.0.1\r\n\r\n";
        let response = send_request(request);
        let expected_json = r#"{"id":3,"rank":"28,818","trend":"8","season":1,"episode":4,"name":"Luffy's Past! The Red-haired Shanks Appears!","start":1999,"total_votes":"449","average_rating":8.1}"#;

        // Check the response
        assert!(response.contains(expected_json));
    }

    #[test]
    fn test_post() {
        // Start the server
        start_server();
        thread::sleep(Duration::from_secs(1));

        // Add a new character
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

        // Create a POST request
        let request = format!(
            "POST /submit HTTP/1.1\r\nContent-Length: {}\r\n\r\n{}",
            new_character.len(),
            new_character
        );

        // Send the request
        let response = send_request(&request);
        println!("Response:({})", response);
        assert!(response.contains("Success!"));
    }

    #[test]
    fn test_put() {
        // Start the server
        start_server();
        thread::sleep(Duration::from_secs(1));

        // Update an existing character
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

        // Create a PUT request
        let request = format!(
            "PUT /put_entry HTTP/1.1\r\nContent-Length: {}\r\n\r\n{}",
            updated_character.len(),
            updated_character
        );

        // Send the request
        let response = send_request(&request);
        println!("Response:({})", response);
        assert!(response.contains("Success!"));
    }

    #[test]
    fn test_delete() {
        // Start the server
        start_server();
        thread::sleep(Duration::from_secs(1));

        // Create a DELETE request
        let delete_request = r#"{"id": 5}"#;
        let request = format!(
            "DELETE /delete_entry HTTP/1.1\r\nContent-Length: {}\r\n\r\n{}",
            delete_request.len(),
            delete_request
        );

        // Send the request
        let response = send_request(&request);
        println!("Response:({})", response);
        assert!(response.contains("Success!"));
    }

    #[test]
    fn test_patch() {
        // Start the server
        start_server();
        thread::sleep(Duration::from_secs(1));

        // Create a PATCH request
        let patch_request = r#"{
            "id": 1,
            "name": "Pirate King Luffy"
        }"#;

        // Create a PATCH request
        let request = format!(
            "PATCH /patch_entry_name HTTP/1.1\r\nContent-Length: {}\r\n\r\n{}",
            patch_request.len(),
            patch_request
        );

        // Send the request
        let response = send_request(&request);
        println!("Response:({})", response);
        assert!(response.contains("Success"));
    }

    // Cookie Management Unit Tests
    #[test]
    fn test_cookie_management() {
        // Start the server
        start_server();
        std::thread::sleep(Duration::from_secs(1));

        // Send a request with cookies
        let cookie_value = "session_id=123456";
        let request = format!(
            "GET /data HTTP/1.1\r\nHost: 127.0.0.1\r\nCookie: {}\r\n\r\n",
            cookie_value
        );

        // Send the request
        let response = send_request(&request);

        // Check the response
        assert!(response
            .contains("Set-Cookie: new_cookie=will_be_set_but_won't_last_long; Path=/; HttpOnly"));
        assert!(response.contains("Set-Cookie: old_cookie=won't_be_set; Path=/; HttpOnly"));

        thread::sleep(Duration::from_secs(1));
    }

    // Concurrent Requests Unit Test
    #[test]
    fn test_concurrent_requests() {
        // Start the server
        start_server();
        thread::sleep(Duration::from_secs(1));

        let pool = ThreadPool::new(5);
        let num_requests = 5;

        let expected_jsons = r#"Hello, world!"#;

        let (tx, rx) = mpsc::channel();

        // Send multiple requests concurrently
        for i in 0..num_requests {
            let tx = tx.clone();

            // Send a request in a separate thread
            pool.execute(move || {
                let request =
                    format!("GET /hello HTTP/1.1\r\nHost: 127.0.0.1\r\nConnection: close\r\n\r\n");

                // Measure the time taken to receive the response
                let start = Instant::now();

                let response = send_request(&request);

                let duration = start.elapsed();

                println!(
                    "Received response for request {} took {:?}",
                    (i + 1),
                    duration
                );

                assert!(response.contains(expected_jsons));

                tx.send(()).unwrap();
            });
        }

        // Wait for all requests to complete
        for _ in 0..num_requests {
            rx.recv_timeout(Duration::from_secs(5))
                .expect("Test timed out");
        }
    }

    #[test]
    fn test_parse_request_valid() {
        start_server();
        // Wait for the server to start
        let mut stream = TcpStream::connect("127.0.0.1:7878").unwrap();
        println!("Stream: {:?}", stream);

        // Send a valid GET request
        let request = "GET /entries HTTP/1.1\r\nHost: localhost\r\n\r\n";
        stream.write_all(request.as_bytes()).unwrap();
        stream.flush().unwrap();

        let mut buf_reader = BufReader::new(&mut stream);

        // Parse the request
        let (method, uri, headers, body) = match parse_request(&mut buf_reader) {
            Ok(result) => result,
            Err(e) => {
                eprintln!("Failed to parse request: {}", e);
                return;
            }
        };

        // Check the parsed values
        assert_eq!(method, "GET");
        assert_eq!(uri, "/entries");
        assert_eq!(headers.get("Host").unwrap(), "localhost");
        assert_eq!(body, "");
    }

    #[test]
    fn test_parse_cookies() {
        let mut headers = HashMap::new();
        headers.insert(
            "Cookie".to_string(),
            "sessionId=abc123; userId=789; lang=en".to_string(),
        );

        // Parse cookies
        let cookies = parse_cookies(&headers);

        // Check the parsed cookies
        assert_eq!(cookies.get("sessionId").unwrap(), "abc123");
        assert_eq!(cookies.get("userId").unwrap(), "789");
        assert_eq!(cookies.get("lang").unwrap(), "en");
    }

    #[test]
    fn test_parse_cookies_empty() {
        // No cookies in the headers
        let headers = HashMap::new();
        let cookies = parse_cookies(&headers);
        assert!(cookies.is_empty());
    }

    #[test]
    fn test_set_cookie() {
        // Set a cookie with an expiry date
        let mut cookies = Vec::new();
        set_cookie(
            &mut cookies,
            "sessionId",
            "abc123",
            Some("Tue, 19 Jan 2038 03:14:07 GMT"),
        );

        // Check the generated cookie
        assert_eq!(cookies.len(), 1);
        assert!(cookies[0].contains("sessionId=abc123"));
        assert!(cookies[0].contains("Expires=Tue, 19 Jan 2038 03:14:07 GMT"));
        assert!(cookies[0].contains("Path=/; HttpOnly"));
    }

    #[test]
    fn test_set_cookie_no_expiry() {
        // Set a cookie without an expiry date
        let mut cookies = Vec::new();
        set_cookie(&mut cookies, "sessionId", "abc123", None);

        // Check the generated cookie
        assert_eq!(cookies.len(), 1);
        assert_eq!(cookies[0], "sessionId=abc123; Path=/; HttpOnly");
    }

    #[test]
    fn test_is_cookie_expired() {
        // Check if a past date is expired and a future date is not
        let past_date = "Wed, 01 Jan 2020 00:00:00 GMT";
        let future_date = "Wed, 01 Jan 2030 00:00:00 GMT";

        // Check the expiration status
        assert!(is_cookie_expired(past_date));
        assert!(!is_cookie_expired(future_date));
    }

    #[test]
    fn test_is_cookie_expired_invalid_format() {
        // Check an invalid date format
        let invalid_date = "Invalid Date";
        assert!(!is_cookie_expired(invalid_date));
    }

    #[test]
    fn test_get_cookie_expiration() {
        // Get the expiration date for a cookie
        let duration_secs = 60 * 60 * 24; // 1 day
        let expiration = get_cookie_expiration(duration_secs);

        // Parse the expiration date
        let parsed_expiration = DateTime::parse_from_rfc2822(&expiration);
        assert!(parsed_expiration.is_ok());

        // Check the expiration time
        let expiration_datetime = parsed_expiration.unwrap();
        let now = Utc::now();
        let difference = expiration_datetime.signed_duration_since(now).num_seconds();
        assert!((difference - (duration_secs as i64)).abs() < 10); // Allowing a small margin of error
    }
}
