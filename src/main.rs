use std::{
    cell::RefCell,
    collections::HashMap,
    io::{Read, Write},
    net::{TcpListener, TcpStream},
    rc::Rc,
    sync::{
        Arc, Mutex,
        mpsc::{self, Sender},
    },
    thread,
};
use crate::parser::Message;
mod parser;
fn session_maker() -> (
    impl FnMut(TcpStream) -> usize,
    impl FnMut(usize),
    impl FnMut(usize) -> Option<Arc<Mutex<TcpStream>>>,
) {
    let users: Arc<Mutex<HashMap<usize, Arc<Mutex<TcpStream>>>>> =
        Arc::new(Mutex::new(HashMap::new()));
    let mut next_index = 0;
    let add_users = Arc::clone(&users);
    let add_user = move |user: TcpStream| {
        add_users
            .lock()
            .unwrap()
            .insert(next_index, Arc::new(Mutex::new(user)));
        let current_index = next_index;
        next_index += 1;
        return current_index;
    };
    let remove_users = Arc::clone(&users);
    let remove_user = move |index: usize| {
        remove_users.lock().unwrap().remove(&index);
    };
    let get_users = Arc::clone(&users);
    let get_user = move |index: usize| get_users.lock().unwrap().get(&index).cloned();

    (add_user, remove_user, get_user)
}

fn session_maker_chan() -> (
    impl FnMut(Sender<String>) -> usize,
    impl FnMut(usize),
    impl FnMut(usize) -> Option<Sender<String>>,
) {
    let users: Arc<Mutex<HashMap<usize, Sender<String>>>> = Arc::new(Mutex::new(HashMap::new()));
    let mut next_index = 0;
    let users_clone_for_add = Arc::clone(&users);
    let add_user = move |sender: Sender<String>| {
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
    let mut add_user = session_maker();
    let (tx, rx) = mpsc::channel::<String>();
    loop {
        let (client_stream, client_addr) = listener.accept().expect("Failed to accept connection");
        println!("Client connected from: {}", client_addr);
        // add_user(
        //     client_stream
        //         .try_clone()
        //         .expect("Failed to clone client stream"),
        // );
    }
}

fn handler_chan(client_stream: &mut TcpStream) {
    let (mut add_user, mut remove_user, mut get_user) = session_maker_chan();
    let (tx, rx) = mpsc::channel::<String>();
    let current_index = add_user(tx);
    loop {
        let mut buff = [0; 1024];
        let bytes_read = client_stream
            .read(&mut buff);
        match bytes_read{
            Ok(bytes_read)=>{

            },
            Err(ref e) if e.kind() == std::io::ErrorKind::WouldBlock => {
                continue;
            }
            Err(e) => {
                eprintln!("Error reading from client: {}", e);
                break;  
            }
        }
    }
}

fn handler(index: usize) {
    let (mut _add_user, mut remove_user, mut get_user) = session_maker();
    let mut buff = [0; 1024];
    let client_stream = get_user(index).expect("Failed to get user stream");
    loop {
        let mut client_stream = client_stream.lock().expect("Failed to lock client stream");
        let bytes_read = client_stream
            .read(&mut buff)
            .expect("Failed to read from client");

        let received_string = String::from_utf8(buff[..bytes_read].to_vec())
            .expect("Failed to convert bytes to string")
            .trim()
            .to_string();
        println!("Received {} bytes: {:?}", bytes_read, received_string);
        if received_string == "exit" {
            println!("Client requested to exit.");
            break;
        }
        let msg = format!("ECHO: {}\n\r", received_string);
        client_stream
            .write_all(msg.as_bytes())
            .expect("Failed to write to client");
    }
    remove_user(index);
}
