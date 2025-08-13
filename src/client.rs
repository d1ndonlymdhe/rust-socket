use crate::{
    parser::Message,
    shared::{extract_message, write_message},
};
use raylib::prelude::*;
use simplelog::{CombinedLogger, Config, LevelFilter, WriteLogger};
use std::{
    collections::HashMap,
    fs::File,
    net::TcpStream,
    sync::{Arc, Mutex},
    thread,
};

struct ClientState {
    peers: Mutex<HashMap<usize, Vec<Message>>>,
}

impl ClientState {
    fn new() -> Self {
        ClientState {
            peers: Mutex::new(HashMap::new()),
        }
    }

    fn set_peers(&self, peers: Vec<usize>) {
        let mut peers_map = self.peers.lock().unwrap();
        for peer in peers {
            peers_map.entry(peer).or_default();
        }
    }

    fn add_message(&self, peer: usize, message: Message) {
        let mut peers = self.peers.lock().unwrap();
        peers.entry(peer).or_default().push(message);
    }

    fn get_messages(&self, peer: usize) -> Vec<Message> {
        let peers = self.peers.lock().unwrap();
        peers.get(&peer).cloned().unwrap_or_default()
    }
}

pub fn client(num: usize) {
    let server_stream = TcpStream::connect("127.0.0.1:8000").expect("Could not connect to server");
    server_stream.set_nonblocking(true).unwrap();

    CombinedLogger::init(vec![WriteLogger::new(
        LevelFilter::Info,
        Config::default(),
        File::create(format!("client_{}.log", num)).unwrap(),
    )])
    .unwrap();

    let server_stream = Arc::new(Mutex::new(server_stream));

    let client_state = Arc::new(Mutex::new(ClientState::new()));

    let read_stream = Arc::clone(&server_stream);
    let read_client_state = client_state.clone();

    let (ui_tx, ui_rx) = std::sync::mpsc::channel::<SystemMessage>();

    let read_thread = thread::spawn({
        move || {
            loop {
                let msg = extract_message(&mut read_stream.lock().unwrap());
                if let Ok(msg) = msg {
                    let peer = msg.peer;

                    if peer == 0 {
                        if let Ok(SystemMessage::Peers(peers)) = read_system_message(msg) {
                            update_peers(peers.clone(), read_client_state.clone());
                            ui_tx
                                .send(SystemMessage::Peers(peers.clone()))
                                .expect("Failed to send peers to UI thread");
                        }
                        continue;
                    }

                    let content = String::from_utf8_lossy(&msg.content);
                    log::info!("Received from {}: {}", peer, content);
                }
            }
        }
    });

    let write_stream = Arc::clone(&server_stream);
    let write_thread = thread::spawn(move || {
        let mut input = String::new();
        loop {
            input.clear();
            std::io::stdin()
                .read_line(&mut input)
                .expect("Failed to read line");
            let peer = input
                .trim()
                .to_string()
                .parse::<usize>()
                .expect("Invalid peer number");
            input.clear();
            std::io::stdin()
                .read_line(&mut input)
                .expect("Failed to read line");
            let msg = input.trim().to_string();
            log::info!("Sending to {}: {}", peer, msg);
            let msg = Message::encode(peer, msg.as_bytes().to_vec().as_ref());
            write_message(&mut write_stream.lock().unwrap(), msg);
        }
    });

    let (mut rl, thread) = raylib::init().size(640, 480).title("Hello, World").build();
    let mut peers = vec![];
    while !rl.window_should_close() {
        let mut d = rl.begin_drawing(&thread);

        if let Ok(msg) = ui_rx.try_recv() {
            match msg {
                SystemMessage::Peers(peers_list) => {
                    log::info!("Updated peers: {:?}", peers);
                    peers = peers_list
                        .iter()
                        .map(|&p| p.to_string())
                        .collect::<Vec<String>>()
                }
            }
        }
        d.clear_background(Color::WHITE);

        for (i, peer) in peers.iter().enumerate() {
            d.draw_text(
                &format!("Peer {}: {}", i, peer),
                12,
                12 + i as i32 * 20,
                20,
                Color::BLACK,
            );
        }

    }

    read_thread.join().expect("Read thread panicked");
    write_thread.join().expect("Write thread panicked");
    // ui_thread.join().expect("UI thread panicked");
    log::info!("Client terminated.");
}

#[derive(Debug)]
enum SystemMessageError {
    NotASysMsg,
}
enum SystemMessage {
    Peers(Vec<usize>),
}

fn read_system_message(message: Message) -> Result<SystemMessage, SystemMessageError> {
    let peer = message.peer;
    if peer != 0 {
        return Err(SystemMessageError::NotASysMsg);
    }
    let content = String::from_utf8_lossy(&message.content);
    if content.starts_with("PEERS:") {
        let peers: Vec<usize> = content[6..]
            .split(',')
            .filter_map(|s| s.trim().parse().ok())
            .collect();
        Ok(SystemMessage::Peers(peers))
    } else {
        log::info!("System message from {}: {}", peer, content);
        Err(SystemMessageError::NotASysMsg)
    }
}

fn update_peers(peers: Vec<usize>, client_state: Arc<Mutex<ClientState>>) {
    client_state.lock().unwrap().set_peers(peers);
}
