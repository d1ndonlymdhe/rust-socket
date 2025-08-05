use std::{io::{Read, Write}, net::TcpStream};

use crate::parser::{Message, ParseError};

pub fn write_message(stream: &mut TcpStream, msg: Vec<Vec<u8>>) {
    for part in msg {
        if let Err(e) = stream.write_all(&part) {
            eprintln!("Failed to write message to client: {}", e);
            break;
        }
    }
}



#[derive(Debug)]
pub enum ExtractError {
    InvalidMessage(ParseError),
    NotReady,
    IOError(std::io::Error),
}

pub fn extract_message(stream: &mut TcpStream) -> Result<Vec<Message>, ExtractError> {
    let mut buff = [0; 1029];
    let mut res = vec![];
    loop {
        let bytes_read = stream.read(&mut buff);
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
