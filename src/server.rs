use std::{collections::HashMap, net::{TcpListener, TcpStream}, sync::{mpsc::{self, SendError, Sender}, Mutex, OnceLock}};

use crate::{parser::Message, shared::{extract_message, write_message, ExtractError}};
struct GlobalState {
    users: Mutex<HashMap<usize, Sender<Message>>>,
}
impl GlobalState {
    fn new() -> Self {
        GlobalState {
            users: Mutex::new(HashMap::new()),
        }
    }
    fn add_user(&self, sender: Sender<Message>) -> usize {
        let mut users = self.users.lock().unwrap();
        let next_index = users.len();
        users.insert(next_index, sender);
        next_index
    }
    fn remove_user(&self, index: usize) {
        let mut users = self.users.lock().unwrap();
        users.remove(&index);
    }
    fn get_user(&self, index: usize) -> Option<Sender<Message>> {
        let users = self.users.lock().unwrap();
        users.get(&index).cloned()
    }
}

static SESSION: OnceLock<GlobalState> = OnceLock::new();

pub fn server(){
    SESSION.get_or_init(|| GlobalState::new());
    let listener = TcpListener::bind("0.0.0.0:8000").expect("Could not bind to 8000");
    loop {
        let (mut client_stream, client_addr) =
            listener.accept().expect("Failed to accept connection");
        client_stream
            .set_nonblocking(true)
            .expect("Failed to set non-blocking mode");
        println!("Client connected from: {}", client_addr);
        handler_chan(&mut client_stream);
    }
}

fn handler_chan(client_stream: &mut TcpStream) {
    // let (mut add_user, _remove_user, _get_user) = session_maker_chan();
    let session = SESSION.get().expect("Global state not initialized");
    let (tx, rx) = mpsc::channel::<Message>();
    // let current_index = add_user(tx);
    let current_index = session.add_user(tx);
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
                }
                ExtractError::NotReady => {
                    continue; // Not ready, continue to read more data
                }
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
                        let msg = Message::encode(
                            0,
                            "Failed to send message".as_bytes().to_vec().as_ref(),
                        );
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

pub enum HandleMessageError {
    PeerNotFound(usize),
    SendChanError(SendError<Message>),
}

pub fn handle_message(src: usize, msg: Message) -> Result<(), HandleMessageError> {
    // let (_add_user, _remove_user, mut get_user) = session_maker_chan();
    let session = SESSION.get().expect("Global state not initialized");
    let peer = msg.clone().peer;
    let peer_chan = session.get_user(peer);
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