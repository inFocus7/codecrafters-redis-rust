use std::{
    io::Write,
    net::{TcpListener, TcpStream},
};

fn handle_connection(mut stream: TcpStream) -> std::io::Result<usize> {
    let resp = "+PONG\r\n";
    return stream.write(resp.as_bytes());
}

fn main() {
    let listener = TcpListener::bind("127.0.0.1:6379").unwrap();

    for stream in listener.incoming() {
        match stream {
            Ok(_stream) => match handle_connection(_stream) {
                Ok(bytes) => {
                    println!("Bytes written: {bytes}");
                }
                Err(e) => {
                    println!("error: {e}");
                }
            },
            Err(e) => {
                println!("error: {}", e);
            }
        }
    }
}
