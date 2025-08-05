mod parser;
mod server;
mod shared;
mod client;

use std::env;

fn main() {
    let args: Vec<String> = env::args().collect();
    let mode = if args.len() > 1 {
        &args[1]
    } else {
        "server"
    };
    if mode == "server" {
        server::server();
    } else {
        client::client();
    }
}
