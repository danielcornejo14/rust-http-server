mod endpoints;

use rust_http_server::ThreadPool;
use serde::{Deserialize, Serialize};
use std::{
    collections::HashMap,
    fs,
    io::{prelude::*, BufReader},
    net::{TcpListener, TcpStream},
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

fn load_data() -> Vec<Data> {
    let file_content = fs::read_to_string("data.json").unwrap_or("[]".to_string());
    serde_json::from_str(&file_content).unwrap_or_else(|_| vec![])
}

fn main() {
    let listener = TcpListener::bind("127.0.0.1:7878").unwrap();
    let data = load_data();
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

fn set_cookie(response_headers: &mut HashMap<String, String>, name: &str, value: &str) {
    let cookie = format!("{}={}; Path=/; HttpOnly", name, value);
    response_headers.insert("Set-Cookie".to_string(), cookie);
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

    // Prepare response headers
    let mut response_headers = HashMap::new();

    // Set a cookie in the response
    set_cookie(&mut response_headers, "session_id", "123456");

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
    let response = format!(
        "{status_line}\r\nContent-Length: {length}\r\nSet-Cookie: {}\r\n\r\n{response_body}",
        response_headers.get("Set-Cookie").unwrap()
    );
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
    use std::{sync::mpsc, time::Duration};

    use endpoints::{delete_entry, get_entries, patch_entry_name, post_entry, put_entry};

    use super::*;

    // HTTP Operations Unit Tests

    #[test]
    fn test_get_entries() {
        let response = get_entries(5); // Get 5 entries
        print!("{}", response);

        let expected_json = r#"{"id":1,"rank":"29,290","trend":"11","season":1,"episode":2,"name":"test","start":1999,"total_votes":"473","average_rating":7.8}"#;

        assert!(response.contains(expected_json));
    }

    #[test]
    fn test_post_entry() {
        let new_character = r#"{
            "id": 0,
            "rank": "Captain", 
            "trend": "up", 
            "season": 3, 
            "episode": 20, 
            "name": "Zoro", 
            "start": 2, 
            "total_votes": "200", 
            "average_rating": 8.9
        }"#;

        let response = post_entry(new_character);
        print!("{}", response);
        assert_eq!(response, "Success!"); // Ensure the response indicates success
    }

    #[test]
    fn test_put_entry() {
        let updated_character = r#"{
            "id": 1,
            "rank": "Pirate King", 
            "trend": "up", 
            "season": 10, 
            "episode": 100, 
            "name": "Monkey D. Luffy", 
            "start": 1, 
            "total_votes": "100000", 
            "average_rating": 9.9
        }"#;

        let response = put_entry(updated_character);
        assert_eq!(response, "Success!"); // Ensure the response indicates success
    }

    #[test]
    fn test_delete_entry() {
        let delete_request = r#"{"id": 3}"#;

        let response = delete_entry(delete_request);
        assert_eq!(response, "Success!"); // Ensure the entry is deleted successfully
    }

    #[test]
    fn test_patch_entry_name() {
        let patch_request = r#"{
            "id": 1,
            "name": "Pirate King Luffy"
        }"#;

        let response = patch_entry_name(patch_request);
        assert_eq!(response, "Success"); // Ensure the patch operation succeeds
    }

    #[test]
    fn test_concurrent_post_requests() {
        let pool = ThreadPool::new(5);

        let num_requests = 4;
        let post_data = r#"{
            "id": 1,
            "rank": "Captain",
            "trend": "up",
            "season": 3,
            "episode": 20,
            "name": "Zoro",
            "start": 2,
            "total_votes": "200",
            "average_rating": 8.9
        }"#;

        let (tx, rx) = mpsc::channel(); // Create a channel to track task completion

        for _ in 0..num_requests {
            let tx = tx.clone();
            let post_data = post_data.to_string();

            pool.execute(move || {
                let (status_line, response) = handle_post("/submit", &post_data);
                assert_eq!(status_line, "HTTP/1.1 200 OK");
                assert!(response.contains("Success"));
                tx.send(()).unwrap(); // Signal that the task is complete
            });
        }

        // Wait for all requests to finish
        for _ in 0..num_requests {
            rx.recv_timeout(Duration::from_secs(2))
                .expect("Test timed out");
        }
    }

    #[test]
    fn test_concurrent_get_requests() {
        let pool = ThreadPool::new(5);

        let num_requests = 5;

        let expected_jsons = vec![
            r#"{"id":3,"rank":"28,818","trend":"8","season":1,"episode":4,"name":"Luffy's Past! The Red-haired Shanks Appears!","start":1999,"total_votes":"449","average_rating":8.1}"#,
            r#"{"id":4,"rank":"37,113","trend":"4","season":1,"episode":5,"name":"Fear, Mysterious Power! Pirate Clown Captain Buggy!","start":1999,"total_votes":"370","average_rating":7.5}"#,
            r#"{"id":5,"rank":"36,209","trend":"4","season":1,"episode":6,"name":"Desperate Situation! Beast Tamer Mohji vs. Luffy!","start":1999,"total_votes":"364","average_rating":7.7}"#,
            r#"{"id":6,"rank":"37,648","trend":"4","season":1,"episode":7,"name":"Sozetsu Ketto! Kengo Zoro VS Kyokugei no Kabaji!","start":1999,"total_votes":"344","average_rating":7.7}"#,
            r#"{"id":7,"rank":"38,371","trend":"6","season":1,"episode":8,"name":"Shousha wa docchi? Akuma no mi no nouryoku taiketsu!","start":1999,"total_votes":"335","average_rating":7.7}"#,
        ];

        let (tx, rx) = mpsc::channel(); // Create a channel to track task completion

        for i in 0..num_requests {
            let tx = tx.clone();
            let expected_json = expected_jsons[i].to_string();

            pool.execute(move || {
                let (status_line, response) = handle_get("/entries");
                assert_eq!(status_line, "HTTP/1.1 200 OK");
                assert!(response.contains(&expected_json));
                tx.send(()).unwrap();
            });
        }

        // Wait for all requests to finish
        for _ in 0..num_requests {
            rx.recv_timeout(Duration::from_secs(2))
                .expect("Test timed out");
        }
    }
}
