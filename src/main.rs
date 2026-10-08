use std::fs;
use std::io::{BufRead, BufReader, Result, Write};
use std::net::{self, TcpStream};

fn main() -> Result<()> {
    let listener = net::TcpListener::bind("127.0.0.1:8888")?;
    println!("Listening on port 8888");

    for stream in listener.incoming() {
        let stream = stream.unwrap();

        handle_connection_stream(stream);
    }

    Ok(())
}

fn handle_connection_stream(mut stream: TcpStream) {
    let buf_reader = BufReader::new(&stream);
    let request_header = buf_reader.lines().next().unwrap().unwrap();
    println!("Got request: {}", &request_header);

    let status_line;
    let contents;

    match request_header.as_str() {
        "GET / HTTP/1.1" => {
            match fs::read_to_string("index.html") {
                Ok(v) => {
                    status_line = "HTTP/1.1 200 OK";
                    contents = v;
                }
                Err(_) => {
                    status_line = "HTTP/1.1 404 Not Found";
                    contents = "".to_string();
                }
            };
        }
        _ => {
            let filename = request_header.split_whitespace().collect::<Vec<&str>>()[1]
                .to_string()
                .split_at(1)
                .1
                .to_string();
            match fs::read_to_string(filename) {
                Ok(v) => {
                    status_line = "HTTP/1.1 200 OK";
                    contents = v;
                }
                Err(_) => {
                    status_line = "HTTP/1.1 404 Not Found";
                    contents = "".to_string();
                }
            }
        }
    }
    let length = contents.len();

    let response = format!("{status_line}\r\nContent-Length: {length}\r\n\r\n{contents}");

    stream.write_all(response.as_bytes()).unwrap();
}
