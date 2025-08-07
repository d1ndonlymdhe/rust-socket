use std::{
    collections::HashMap,
    net::{TcpListener, TcpStream},
    sync::{
        mpsc::{self, SendError, Sender, TryRecvError}, Arc, Mutex, OnceLock
    },
    thread,
};


use crate::{
    parser::Message,
    shared::{ExtractError, extract_message, write_message},
};

type RawMessage = Vec<Vec<u8>>;

struct GlobalState {
    users: Mutex<HashMap<usize, Sender<RawMessage>>>,
    next_index: usize,
}
impl GlobalState {
    fn new() -> Self {
        GlobalState {
            users: Mutex::new(HashMap::new()),
            next_index: 1,
        }
    }
    fn add_user(session: &mut Self, sender: Sender<RawMessage>) -> usize {
        let mut users = session.users.lock().unwrap();
        let next_index = session.next_index;
        users.insert(next_index, sender);
        session.next_index += 1;
        let x = users.keys().cloned().collect::<Vec<usize>>();
        let peers = GlobalState::peers_to_string(x.clone());
        let senders = users.values().cloned().collect::<Vec<Sender<RawMessage>>>();
        thread::spawn(move || {
            let msg = Message::encode(0, format!("PEERS:{}", peers).as_bytes().to_vec().as_ref());
            broadcast_message(senders, msg);
        });
        next_index
    }
    fn remove_user(session: &mut Self, index: usize) {
        let mut users = session.users.lock().unwrap();
        users.remove(&index);
        let x = users.keys().cloned().collect::<Vec<usize>>();
        let peers = GlobalState::peers_to_string(x.clone());
        let senders = users.values().cloned().collect::<Vec<Sender<RawMessage>>>();
        thread::spawn(move || {
            let msg = Message::encode(0, format!("PEERS:{}", peers).as_bytes().to_vec().as_ref());
            broadcast_message(senders, msg);
        });
    }
    fn get_user(&self, index: usize) -> Option<Sender<RawMessage>> {
        let users = self.users.lock().unwrap();
        users.get(&index).cloned()
    }
    fn peers_to_string(users: Vec<usize>) -> String {
        users
            .iter()
            .map(|k| k.to_string())
            .collect::<Vec<String>>()
            .join(",")
    }
}

static SESSION: OnceLock<Mutex<GlobalState>> = OnceLock::new();

pub fn server() {
    SESSION.get_or_init(|| Mutex::new(GlobalState::new()));
    let listener = TcpListener::bind("0.0.0.0:8000").expect("Could not bind to 8000");
    loop {
        let (client_stream, _client_addr) =
            listener.accept().expect("Failed to accept connection");
        client_stream
            .set_nonblocking(true)
            .expect("Failed to set non-blocking mode");
        let client_stream = Arc::new(Mutex::new(client_stream));
        thread::spawn(move || {
            handler_chan(client_stream);
        });
        // handler_chan(&mut client_stream);
    }
}

fn handler_chan(client_stream: Arc<Mutex<TcpStream>>) {
    let (tx, rx) = mpsc::channel::<RawMessage>();
    let (internal_tx, internal_rx) = mpsc::channel::<()>();
    let current_index = {
        let mut session = SESSION.get().expect("Not Initialized").lock().unwrap();
        GlobalState::add_user(&mut session, tx)
    };
    let read_stream = Arc::clone(&client_stream);
    let h1 = thread::spawn(move || {
        loop {
            let messages = extract_message(&mut read_stream.lock().unwrap());
            if messages.is_err() {
                let err = messages.unwrap_err();
                match err {
                    ExtractError::IOError(e) => {
                        eprintln!("IO Error: {}", e);
                        internal_tx.send(()).unwrap();
                        break;
                    }
                    ExtractError::InvalidMessage(parse_error) => {
                        eprintln!("Parse Error: {:?}", parse_error);
                        let msg = Message::encode(0, "Invalid message".as_bytes().to_vec().as_ref());
                        write_message(&mut read_stream.lock().unwrap(), msg);
                        continue;
                    }
                    ExtractError::NotReady => {
                        continue; // Not ready, continue to read more data
                    }
                    ExtractError::Closed => {
                        eprintln!("Connection closed by client");
                        internal_tx.send(()).unwrap();
                        break;
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
                            write_message(&mut read_stream.lock().unwrap(), msg);
                        }
                        HandleMessageError::SendChanError(send_error) => {
                            eprintln!("Failed to send message: {:?}", send_error);
                            let msg = Message::encode(
                                0,
                                "Failed to send message".as_bytes().to_vec().as_ref(),
                            );
                            write_message(&mut read_stream.lock().unwrap(), msg);
                        }
                    }
                }
            }
        }
    });
    let write_stream = Arc::clone(&client_stream);
    let h2 = thread::spawn(move || {
        loop {
            let internal_res = internal_rx.try_recv();
            match internal_res {
                Ok(_) => {
                    println!("Internal shutdown signal received, stopping write thread for client: {}", current_index);
                    break;
                },
                Err(TryRecvError::Empty) => {},
                Err(TryRecvError::Disconnected) => {
                    println!("Internal channel disconnected, stopping write thread for client: {}", current_index);
                    break; // Channel disconnected, stop processing
                }
            };
            let res = rx.try_recv();
            match res {
                Ok(msg) => {
                    write_message(&mut write_stream.lock().unwrap(), msg);
                },
                Err(ref e) if *e == TryRecvError::Empty => {
                    // continue;
                },
                Err(e) =>{
                    eprintln!("Error receiving message: {:?}", e);
                    // continue;
                }
            }
            
        }
    });
    h1.join().expect("Failed to join read thread");
    h2.join().expect("Failed to join write thread");
    {
        let mut session = SESSION.get().expect("Not Initialized").lock().unwrap();
        GlobalState::remove_user(&mut session, current_index);
    }
    return;
}

pub enum HandleMessageError {
    PeerNotFound(usize),
    SendChanError(SendError<RawMessage>),
}

pub fn handle_message(src: usize, msg: Message) -> Result<(), HandleMessageError> {
    // let (_add_user, _remove_user, mut get_user) = session_maker_chan();
    let session = SESSION
        .get()
        .expect("Global state not initialized")
        .lock()
        .unwrap();
    let peer = msg.clone().peer;
    let peer_chan = session.get_user(peer);
    if peer_chan.is_none() {
        return Err(HandleMessageError::PeerNotFound(peer));
    }
    let peer_chan = peer_chan.unwrap();

    let msg = Message::encode(src, msg.content.as_ref());

    let res = peer_chan.send(msg);
    if res.is_err() {
        return Err(HandleMessageError::SendChanError(res.unwrap_err()));
    }
    Ok(())
}

fn broadcast_message(users: Vec<Sender<RawMessage>>, content: Vec<Vec<u8>>) {
    for sender in users.iter() {
        sender.send(content.clone()).unwrap_or_else(|e| {
            eprintln!("Failed to send message to peer: {:?}", e);
        });
    }
}
