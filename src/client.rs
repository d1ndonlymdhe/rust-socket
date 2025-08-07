use std::{net::TcpStream, sync::{Arc, Mutex}, thread};

use crate::{parser::Message, shared::{extract_message, write_message}};


pub fn client(){
    let server_stream = TcpStream::connect("127.0.0.1:8000").expect("Could not connect to server");
    server_stream.set_nonblocking(true).unwrap();
    let server_stream = Arc::new(Mutex::new(server_stream));
    let read_stream = Arc::clone(&server_stream);
    let read_thread = thread::spawn({
        move || {
            loop {
                let msg = extract_message(&mut read_stream.lock().unwrap());
                if let Ok(msg) = msg {
                    for message in msg {
                        let peer = message.peer;
                        let content = String::from_utf8_lossy(&message.content);
                        print!("Received from {}: {}\n", peer, content);
                    }
                }
            }
        }
    });
    let write_stream = Arc::clone(&server_stream);
    let write_thread = thread::spawn(move || {
        let mut input = String::new();
        loop {
            input.clear();
            std::io::stdin().read_line(&mut input).expect("Failed to read line");
            let peer = input.trim().to_string().parse::<usize>().expect("Invalid peer number");
            input.clear();
            std::io::stdin().read_line(&mut input).expect("Failed to read line");
            let msg = input.trim().to_string();
            println!("Sending to {}: {}", peer, msg);
            let msg = Message::encode(peer, msg.as_bytes().to_vec().as_ref());
            write_message(&mut write_stream.lock().unwrap(), msg);
        }
    });
    read_thread.join().expect("Read thread panicked");
    write_thread.join().expect("Write thread panicked");
    println!("Client terminated.");
}