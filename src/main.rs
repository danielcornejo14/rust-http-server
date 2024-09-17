use serde::{Deserialize, Serialize};
use serde_json::Result;
use std::{
    fs,
    io::{prelude::*, BufReader},
    net::{TcpListener, TcpStream},
};

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
    for stream in listener.incoming() {
        let stream = stream.unwrap();

        handle_connection(stream, &data);
    }
}

fn handle_connection(mut stream: TcpStream, data: &Vec<Data>) {
    let buf_reader = BufReader::new(&mut stream);
    let request_line = buf_reader.lines().next().unwrap().unwrap();
    println!("Request: {}", request_line);
    let parts: Vec<&str> = request_line.split_whitespace().collect();

    // Check that the request line has: Method, Request-URI and HTTP-Version
    if parts.len() >= 3 {
        let method = parts[0];
        let uri = parts[1];

        println!("Method: {}, URI: {}", method, uri);

        let (status_line, response_body) = match (method, uri) {
            // Aqui agregue algunas endpoints de ejemplo para poder agregar todos los que se ocupe
            ("GET", "/") => ("HTTP/1.1 200 OK", "Welcome to the homepage!"),
            ("GET", "/hello") => ("HTTP/1.1 200 OK", "Hello, world!"),
            ("GET", "/data") => ("HTTP/1.1 200 OK", "Here is your data."),
            ("POST", "/submit") => ("HTTP/1.1 201 CREATED", "Data submitted successfully."),
            // Handle other routes as 404 Not Found
            _ => ("HTTP/1.1 404 NOT FOUND", "404 - Not Found"),
        };

        // Create the HTTP response
        let length = response_body.len();
        let response = format!("{status_line}\r\nContent-Length: {length}\r\n\r\n{response_body}");
        stream.write_all(response.as_bytes()).unwrap();
    } else {
        let status_line = "HTTP/1.1 400 BAD REQUEST";
        let contents = "Bad Request";
        let length = contents.len();

        let response = format!("{status_line}\r\nContent-Length: {length}\r\n\r\n{contents}");
        stream.write_all(response.as_bytes()).unwrap();
    }
}
