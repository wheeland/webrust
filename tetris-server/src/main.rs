extern crate webutil;
extern crate tetris;

extern crate serde;
extern crate base64;
extern crate bincode;
#[macro_use] extern crate serde_derive;

extern crate rusqlite;
extern crate chrono;

use tetris::networking::*;
use std::io::{Read, Write};
use std::error::Error;

use rusqlite::types::ToSql;
use rusqlite::{Connection, NO_PARAMS};

use chrono::TimeZone;

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
                idtag       TEXT NOT NULL,
                timestamp   INTEGER,
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
    let db = open_database("/var/tetris/tetris.sqlite")?;

    let ret = match message {
        ServerMessage::UploadReplay { name, idtag, replay } => {
            // re-play to find out score and final level
            let mut replayer = tetris::replay::Replayer::new(&replay);
            let len = replayer.length();
            replayer.jump(len);
            let state = replayer.snapshot();

            let now = chrono::Utc::now().timestamp();
            let game = bincode::serialize(&replay).unwrap();

            // get new ID
            let id: i32 = db.query_row_and_then(
                "SELECT MAX(id) FROM replay",
                NO_PARAMS,
                |row| row.get_checked(0)
            ).unwrap_or(0) + 1;

            db.execute(
                "INSERT INTO replay (id, name, idtag, timestamp, endLevel, score, game)
                VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)",
                 &[
                     &id, &name as &ToSql, &idtag as &ToSql, &now, &state.level(), &state.score(), &game
                 ]
            ).map_err(|err| String::from("INSERT failed: ") + &err.description())?;

            let close = db.close().map_err(|err| err.1.description().to_string())?;

            let game = tetris::PlayedGame::new(id as usize, chrono::Utc::now(), name, state.score(), replay.config().level, state.level(), len);
            ServerAnswer::UploadResult(Some(game))
        },

        ServerMessage::RequestHighscores { by_score, idtag, from, to } => {
            let query = if let Some(idtag) = idtag.as_ref() {
                let idtag = idtag.replace("'", "''");
                format!(
                    "SELECT id, name, timestamp, endLevel, score, game
                    FROM replay
                    WHERE idtag = '{}'
                    ORDER BY {} DESC", idtag, if by_score { "score" } else { "timestamp" })
            } else {
                format!(
                    "SELECT id, name, timestamp, endLevel, score, game
                    FROM replay
                    ORDER BY {} DESC", if by_score { "score" } else { "timestamp" })
            };

            let mut stmt = db
                .prepare(&query)
                .map_err(|err| String::from("SELECT failed: ") + &err.description())?;

            let iter = stmt
                .query_map(NO_PARAMS, |row| {
                    let id: i32 = row.get(0);
                    let ts = chrono::Utc.timestamp(row.get(2), 0);
                    let replay: Vec<u8> = row.get(5);
                    let replay = bincode::deserialize::<tetris::replay::Replay>(&replay).unwrap();

                    tetris::PlayedGame::new(
                        id as usize,
                        ts,
                        row.get(1),
                        row.get(4),
                        replay.config().level,
                        row.get(3),
                        replay.frames() as f32 / 1000.0
                    )
                })
                .map_err(|err| String::from("query_map failed: ") + &err.description())?;

            let mut ret = Vec::new();
            for game in iter {
                ret.push(game.unwrap());
            }

            ServerAnswer::HighscoreList {
                by_score,
                idtagged: idtag.is_some(),
                from,
                to,
                data: ret
            }
        },

        ServerMessage::RequestReplays { ids } => {
            let id = ids[0] as i32;

            let replay: Vec<u8> = db
                .query_row_and_then("SELECT game FROM replay WHERE id = ?1", &[&id], |row| row.get_checked(0))
                .map_err(|err| String::from("SELECT failed: ") + &err.description())?;

            let replay = bincode::deserialize::<tetris::replay::Replay>(&replay).unwrap();

            ServerAnswer::ReplayList {
                data: vec!((id as usize, replay))
            }
        },
    };

    Ok(ret)
}

fn main() {
    // read all bytes from stdin
    let mut data = String::new();
    std::io::stdin().read_to_string(&mut data).unwrap();

    // log shit
    // let mut file = std::fs::OpenOptions::new()
        // .append(true)
        // .create(true)
        // .open("server.log")
        // .unwrap();

    // file.write_all(format!("buffer: {:?}\n", data).as_bytes());

    // print 'em
    let answer = match decode::<ServerMessage>(&data) {
        None => {
            // file.write_all(format!("error parsing: {:?}\n", data).as_bytes());
            ServerAnswer::InvalidMessage(format!("Couldn't parse {} bytes", data.len()))
        }
        Some(request) => {
            // file.write_all(format!("message: {:?}\n", request).as_bytes());
            match process(request) {
                Err(err) => ServerAnswer::ServerError(String::from("")),
                Ok(ret) => ret,
            }
        }
    };

    // file.write_all(format!("answer: {:?}\n\n", answer).as_bytes());

    println!("{}", encode(&answer));
}
