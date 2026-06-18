use std::{
    io::{Read, Write},
    net::{TcpListener, TcpStream},
    thread::spawn,
};

fn handle_connection(mut stream: TcpStream) {
    let mut req = [0; 1024];
    while let Ok(bytes) = stream.read(&mut req) {
        println!("Bytes read: {bytes}");
        if bytes == 0 {
            break;
        }

        stream.write(b"+PONG\r\n").unwrap();
    }
}

fn main() {
    let listener = TcpListener::bind("127.0.0.1:6379").unwrap();

    for stream in listener.incoming() {
        match stream {
            Ok(_stream) => {
                spawn(move || handle_connection(_stream));
            }
            Err(e) => {
                println!("error: {}", e);
            }
        }
    }
}
