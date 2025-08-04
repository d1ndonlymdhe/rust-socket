use crate::parser::{Message, ParseError};
use std::{
    collections::HashMap,
    io::{Read, Write},
    net::{TcpListener, TcpStream},
    sync::{
        mpsc::{self, SendError, Sender}, Arc, Mutex
    },
};
mod parser;


fn session_maker_chan() -> (
    impl FnMut(Sender<Message>) -> usize,
    impl FnMut(usize),
    impl FnMut(usize) -> Option<Sender<Message>>,
) {
    let users: Arc<Mutex<HashMap<usize, Sender<Message>>>> = Arc::new(Mutex::new(HashMap::new()));
    let mut next_index = 0;
    let users_clone_for_add = Arc::clone(&users);
    let add_user = move |sender: Sender<Message>| {
        users_clone_for_add
            .lock()
            .unwrap()
            .insert(next_index, sender);
        let current_index = next_index;
        next_index += 1;
        return current_index;
    };
    let users_clone_for_remove = Arc::clone(&users);
    let remove_user = move |index: usize| {
        users_clone_for_remove.lock().unwrap().remove(&index);
    };
    let users_clone_for_get = Arc::clone(&users);
    let get_user = move |index: usize| users_clone_for_get.lock().unwrap().get(&index).cloned();
    return (add_user, remove_user, get_user);
}

fn main() {
    let listener = TcpListener::bind("0.0.0.0:8000").expect("Could not bind to 8000");
    loop {
        let (mut client_stream, client_addr) = listener.accept().expect("Failed to accept connection");
        println!("Client connected from: {}", client_addr);
        handler_chan(&mut client_stream);
    }
}

fn handler_chan(client_stream: &mut TcpStream) {
    let (mut add_user, _remove_user, _get_user) = session_maker_chan();
    let (tx, rx) = mpsc::channel::<Message>();
    let current_index = add_user(tx);
    loop {
        let messages = extract_message(client_stream);
        if messages.is_err() {
            let err = messages.unwrap_err();
            match err {
                ExtractError::IOError(e) => {
                    eprintln!("IO Error: {}", e);
                    break;
                }
                ExtractError::InvalidMessage(parse_error) => {
                    eprintln!("Parse Error: {:?}", parse_error);
                    let msg = Message::encode(0, "Invalid message".as_bytes().to_vec().as_ref());
                    write_message(client_stream, msg);
                    continue;
                },
                ExtractError::NotReady => {
                    continue; // Not ready, continue to read more data
                },
            }
        }
        let messages = messages.unwrap();
        for msg in messages {
            let res = handle_message(current_index, msg);
            if res.is_err() {
                let err = res.unwrap_err();
                match err {
                    HandleMessageError::PeerNotFound(peer) => {
                        eprintln!("Peer not found: {}", peer);
                        let msg = Message::encode(0, "Peer not found".as_bytes().to_vec().as_ref());
                        write_message(client_stream, msg);
                    }
                    HandleMessageError::SendChanError(send_error) => {
                        eprintln!("Failed to send message: {:?}", send_error);
                        let msg = Message::encode(0, "Failed to send message".as_bytes().to_vec().as_ref());
                        write_message(client_stream, msg);
                    }
                }
            }
        }
        let res = rx.try_recv();
        if res.is_ok() {
            let msg = res.unwrap();
            let content = msg.content;
            let msg = Message::encode(current_index, &content);
            write_message(client_stream, msg);
        }
    }
}

fn write_message(client_stream: &mut TcpStream, msg: Vec<Vec<u8>>) {
    for part in msg {
        if let Err(e) = client_stream.write_all(&part) {
            eprintln!("Failed to write message to client: {}", e);
            break;
        }
    }
}


enum HandleMessageError{
    PeerNotFound(usize),
    SendChanError(SendError<Message>)
}

fn handle_message(src: usize, msg: Message) -> Result<(), HandleMessageError> {
    let ( _add_user, _remove_user, mut get_user) = session_maker_chan();
    let peer = msg.clone().peer;
    let peer_chan = get_user(peer);
    if peer_chan.is_none() {
        return Err(HandleMessageError::PeerNotFound(peer));
    }
    let peer_chan = peer_chan.unwrap();
    let msg = Message {
        peer: src,
        content: msg.content,
        has_more: msg.has_more,
    };
    let res = peer_chan.send(msg);
    if res.is_err() {
        return Err(HandleMessageError::SendChanError(res.unwrap_err()));
    }
    Ok(())
}

#[derive(Debug)]
enum ExtractError {
    InvalidMessage(ParseError),
    NotReady,
    IOError(std::io::Error),
}

fn extract_message(client_stream: &mut TcpStream) -> Result<Vec<Message>, ExtractError> {
    let mut buff = [0; 1029];
    let mut res = vec![];
    loop {
        let bytes_read = client_stream.read(&mut buff);
        match bytes_read {
            Ok(bytes_read) => {
                let msg = Message::parse(buff[..bytes_read].to_vec().as_ref());
                match msg {
                    Ok(msg) => {
                        res.push(msg.clone());
                        if !msg.has_more {
                            return Ok(res);
                        }
                        buff = [0; 1029]; // Reset buffer for next read
                    }
                    Err(err) => {
                        return Err(ExtractError::InvalidMessage(err));
                    }
                }
            }
            Err(ref e) if e.kind() == std::io::ErrorKind::WouldBlock => {
                return Err(ExtractError::NotReady);
            }
            Err(e) => {
                return Err(ExtractError::IOError(e));
            }
        }
    }
}

