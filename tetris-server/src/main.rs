extern crate webutil;
extern crate tetris;

extern crate serde;
extern crate base64;
extern crate bincode;
#[macro_use] extern crate serde_derive;

extern crate rusqlite;
extern crate chrono;

mod server;

use tetris::networking::*;
use std::io::Read;
use std::error::Error;

use rusqlite::types::ToSql;
use rusqlite::{Connection, NO_PARAMS};

fn open_database(path: &str) -> Result<Connection, String> {
    let exists = std::path::Path::new(path).exists();

    // open SQLite connection
    let db = Connection::open(path).map_err(|err| err.description().to_string())?;

    // create tables?
    if !exists {
        db.execute(
            "CREATE TABLE replay (
                id          INTEGER PRIMARY KEY,
                name        TEXT NOT NULL,
                timestamp   INTEGER,
                startLevel  INTEGER,
                score       INTEGER,
                endLevel    INTEGER,
                game        BLOB
            )", 
            NO_PARAMS
        ).map_err(|err| String::from("Creation failed: ") + err.description())?;
    }

    Ok(db)
}

fn process(message: ServerMessage) -> Result<ServerAnswer, String> {
    // open SQLite connection
    let db = open_database("tetris.sqlite")?;

    let ret = match message {
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
        ServerMessage::UploadReplay { name, replay } => {
            // re-play to find out score and final level
            let mut replayer = tetris::replay::Replayer::new(&replay);
            let len = replayer.length();
            replayer.jump(len);
            let state = replayer.snapshot();

            let now = chrono::Utc::now().timestamp();
            let game = bincode::serialize(&replay).unwrap();

            db.execute(
                "INSERT INTO replay (name, timestamp, startLevel, endLevel, score, game) 
                VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
                 &[
                     &name as &ToSql, &now, &replay.config().level, &state.level(), &state.score(), &game
                 ]
            ).map_err(|err| String::from("INSERT failed: ") + &err.description())?;

            let close = db.close().map_err(|err| err.1.description().to_string())?;

            ServerAnswer::UploadResult(true)
        }
    };

    Ok(ret)
}

fn main() {
    // read all bytes from stdin
    let mut data = String::new();
    std::io::stdin().read_to_string(&mut data).unwrap();

    // print 'em
    let answer = match decode::<ServerMessage>(&data) {
        None => ServerAnswer::InvalidMessage(format!("Couldn't parse {} bytes", data.len())),
        Some(request) => {
            match process(request) {
                Err(err) => ServerAnswer::ServerError(String::new()),
                Ok(ret) => ret,
            }
        }
    };

    // std::fs::write("server.answer", format!("{}\n\n{:?}", data, answer));

    println!("{}", encode(&answer));
}
