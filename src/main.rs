use std::fs;
use std::io::{BufRead, BufReader, Result, Write};
use std::net::{self, TcpStream};
use webserver_rs::ThreadPool;

fn main() -> Result<()> {
    let listener = net::TcpListener::bind("127.0.0.1:8888")?;
    let pool = ThreadPool::new(4);
    println!("Listening on port 8888");

    for stream in listener.incoming() {
        let stream = stream.unwrap();

        pool.execute(|| {
            handle_connection_stream(stream);
        });
    }

    Ok(())
}

fn handle_connection_stream(mut stream: TcpStream) {
    let buf_reader = BufReader::new(&stream);
    let request_line = buf_reader.lines().next().unwrap().unwrap();
    println!("Got request: {}", &request_line);

    let path = request_line.split_whitespace().nth(1).unwrap_or("/");
    let response = match path {
        "/" => match fs::read_to_string("index.html") {
            Ok(contents) => {
                let mut response = Vec::new();
                response.extend_from_slice(b"HTTP/1.1 200 OK\r\n");
                response.extend_from_slice(
                    format!("Content-Length: {}\r\n\r\n", contents.len()).as_bytes(),
                );
                response.extend_from_slice(contents.as_bytes());
                response
            }
            Err(_) => b"HTTP/1.1 404 Not Found\r\nContent-Length: 0\r\n\r\n".to_vec(),
        },
        "/favicon.ico" => match fs::read("favicon.ico") {
            Ok(contents) => {
                let mut response = Vec::new();
                response.extend_from_slice(b"HTTP/1.1 200 OK\r\n");
                response.extend_from_slice(b"Content-Type: image/x-icon\r\n");
                response.extend_from_slice(
                    format!("Content-Length: {}\r\n", contents.len()).as_bytes(),
                );
                response.extend_from_slice(b"Connection: close\r\n\r\n");
                response.extend_from_slice(&contents);
                response
            }
            Err(_) => b"HTTP/1.1 404 Not Found\r\nContent-Length: 0\r\n\r\n".to_vec(),
        },
        _ => {
            let filename = path.trim_start_matches('/');
            match fs::read_to_string(filename) {
                Ok(contents) => {
                    let mut response = Vec::new();
                    response.extend_from_slice(b"HTTP/1.1 200 OK\r\n");
                    response.extend_from_slice(
                        format!("Content-Length: {}\r\n\r\n", contents.len()).as_bytes(),
                    );
                    response.extend_from_slice(contents.as_bytes());
                    response
                }
                Err(_) => b"HTTP/1.1 404 Not Found\r\nContent-Length: 0\r\n\r\n".to_vec(),
            }
        }
    };

    stream.write_all(&response).unwrap();
}
