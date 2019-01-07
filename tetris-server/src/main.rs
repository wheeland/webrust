extern crate webutil;
extern crate tetris;

extern crate serde;
extern crate base64;
extern crate bincode;
#[macro_use] extern crate serde_derive;

mod server;

use tetris::networking::*;
use std::io::Read;

fn main() {
    // read all bytes from stdin
    let mut data = String::new();
    std::io::stdin().read_to_string(&mut data).unwrap();

    // print 'em
    let answer = match decode::<ServerMessage>(&data) {
        None => {
            ServerAnswer::InvalidMessage(format!("Couldn't parse {} bytes", data.len()))
        },
        Some(request) => {
            match request {
                ServerMessage::CreateAccount { namepass } => {
                    ServerAnswer::CreateAccountResult {
                        error: None,
                    }
                },
                ServerMessage::SaltRequest { name } => {
                    ServerAnswer::Salt {
                        salt: Vec::new(),
                    }
                },
                ServerMessage::Login { namesalthashpass } => {
                    ServerAnswer::LoginResult {
                        error: None,
                    }
                },
            }
        }
    };

    println!("{}", encode(&answer));
}
