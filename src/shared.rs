use std::{
    io::{ErrorKind, Read, Write},
    net::TcpStream,
};

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
    Closed,
}

pub fn extract_message(stream: &mut TcpStream) -> Result<Message, ExtractError> {
    let mut buff = [0; 1029];
    let mut final_msg = Message::new(0, vec![], false);
    loop {
        let bytes_read = stream.read(&mut buff);
        match bytes_read {
            Ok(bytes_read) => {
                if bytes_read == 0 {
                    return Err(ExtractError::Closed);
                }
                let msg = Message::parse(buff[..bytes_read].to_vec().as_ref());
                match msg {
                    Ok(msg) => {
                        final_msg.peer = msg.peer;
                        final_msg.content.extend_from_slice(&msg.content);
                        if !msg.has_more {
                            // for r in res.iter() {
                            //     final_msg.content.extend_from_slice(&r.content);
                            // }
                            return Ok(final_msg);
                        }
                        buff = [0; 1029]; // Reset buffer for next read
                    }
                    Err(err) => {
                        return Err(ExtractError::InvalidMessage(err));
                    }
                }
            }
            Err(ref e) if e.kind() == ErrorKind::WouldBlock => {
                return Err(ExtractError::NotReady);
            }
            Err(ref e)
                if matches!(
                    e.kind(),
                    ErrorKind::ConnectionReset | ErrorKind::BrokenPipe | ErrorKind::UnexpectedEof
                ) =>
            {
                return Err(ExtractError::Closed);
            }
            Err(e) => {
                return Err(ExtractError::IOError(e));
            }
        }
    }
}
