use serde::{Deserialize, Serialize};
use serde_json::Result;
use std::{
    fs,
    io::{prelude::*, BufReader},
    net::{TcpListener, TcpStream},
    collections::HashMap,
};
use rust_http_server::ThreadPool;

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

fn parse_request(buf_reader: &mut BufReader<&mut TcpStream>) -> std::result::Result<(String, String, HashMap<String, String>, String), RequestError> {
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
            headers.insert(header_parts[0].to_string(), header_parts[1].trim().to_string());
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

    let (status_line, response_body) = match method.as_str() {
        "GET" => handle_get(&uri),
        "POST" => handle_post(&uri, &body),
        "PUT" => handle_put(&uri, &body),
        "DELETE" => handle_delete(&uri),
        "UPDATE" => handle_update(&uri, &body),
        _ => ("HTTP/1.1 405 METHOD NOT ALLOWED", "405 - Method Not Allowed".to_string()),
    };

    let length = response_body.len();
    let response = format!("{status_line}\r\nContent-Length: {length}\r\n\r\n{response_body}");
    stream.write_all(response.as_bytes()).unwrap();
}
fn handle_get(uri: &str) -> (&str, String) {
    match uri {
        "/" => ("HTTP/1.1 200 OK", "Welcome to the homepage!".to_string()),
        "/hello" => ("HTTP/1.1 200 OK", "Hello, world!".to_string()),
        "/data" => ("HTTP/1.1 200 OK", "Here is your data.".to_string()),
        _ => ("HTTP/1.1 404 NOT FOUND", "404 - Not Found".to_string()),
    }
}

fn handle_post<'a>(uri: &'a str, body: &'a str) -> (&'a str, String) {
    if uri == "/submit" {
        ("HTTP/1.1 201 CREATED", "Data submitted successfully.".to_string())
    } else {
        ("HTTP/1.1 404 NOT FOUND", "404 - Not Found".to_string())
    }
}

fn handle_put<'a>(uri: &'a str, body: &'a str) -> (&'a str, String) {
    ("HTTP/1.1 200 OK", format!("Resource at {} updated with data: {}", uri, body))
}

fn handle_update<'a>(uri: &'a str, body: &'a str) -> (&'a str, String) {
    ("HTTP/1.1 200 OK", format!("Resource at {} partially updated with data: {}", uri, body))
}

fn handle_delete(uri: &str) -> (&str, String) {
    ("HTTP/1.1 200 OK", format!("Resource at {} deleted", uri))
}