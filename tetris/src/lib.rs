extern crate rand;
extern crate chrono;

extern crate num;
#[macro_use] extern crate num_derive;

extern crate base64;
extern crate serde;
extern crate serde_json;
extern crate bincode;
#[macro_use] extern crate serde_derive;

extern crate array2d;

pub mod piece;
pub mod stack;
pub mod game;
pub mod state;
pub mod replay;
pub mod networking;

use chrono::{DateTime, Utc, Local, Timelike, Datelike};

#[derive(PartialEq,Clone)]
#[derive(Serialize, Deserialize)]
pub struct Config {
    pub width: i32,
    pub height: i32,

    pub level: i32,

    pub gravity: Vec<f32>,

    pub das_initial: f32,
    pub das_step: f32,
    pub das_down: f32,

    pub are_base: i32,
    pub are_max: i32,
    pub line_clear: i32,
}

impl Config {
    pub fn new() -> Self {
        Config {
            width: 10,
            height: 20,
            level: 0,
            gravity: vec!(48.0 / 60.0, 43.0 / 60.0, 38.0 / 60.0, 33.0 / 60.0,
                          28.0 / 60.0, 23.0 / 60.0, 18.0 / 60.0, 13.0 / 60.0,
                          8.0 / 60.0,
                          6.0 / 60.0,
                          5.0 / 60.0, 5.0 / 60.0, 5.0 / 60.0,
                          4.0 / 60.0, 4.0 / 60.0, 4.0 / 60.0,
                          3.0 / 60.0, 3.0 / 60.0, 3.0 / 60.0,
                          2.0 / 60.0, 2.0 / 60.0, 2.0 / 60.0, 2.0 / 60.0, 2.0 / 60.0,
                          2.0 / 60.0, 2.0 / 60.0, 2.0 / 60.0, 2.0 / 60.0, 2.0 / 60.0,
                          1.0 / 60.0
            ),
            das_initial: 16.0 / 60.0,
            das_step: 6.0 / 60.0,
            das_down: 2.0 / 60.0,
            are_base: 10,
            are_max: 20,
            line_clear: 18,
        }
    }

    pub fn transition(&self) -> i32 {
        (self.level * 10 + 10).min(100.max(self.level * 10 - 50))
    }
}

// TODO: reduce Replay size:
// - replace timestamp with "u8 delta timestamp to last entry"
// - enumerate all actions: move { -x, +x, -y, +y, rotl, rotr, merge{drop 0..20}, next, nextpiece }
// -> 12 byte per entry

#[derive(PartialEq, Clone, Debug)]
#[derive(Serialize, Deserialize)]
enum ReplayEntry {
    Move {
        time: usize,
        rotate: i8,
        x: i8,
        y: i8,
    },
    Merge {
        time: usize,
        drop: i8,
        next: piece::Piece
    },
    NewPiece {
        time: usize,
    }
}

impl ReplayEntry {
    fn time(&self) -> usize {
        match self {
            ReplayEntry::Move{time, ..} => *time,
            ReplayEntry::Merge{time, ..} => *time,
            ReplayEntry::NewPiece{time, ..} => *time,
        }
    }
}

#[derive(PartialEq,Clone)]
#[derive(Serialize, Deserialize)]
pub struct Replay {
    config: Config,
    first: piece::Piece,
    second: piece::Piece,

    data: Vec<ReplayEntry>,
}

impl Replay {
    fn add_move(&mut self, time: usize, rotate: Option<bool>, x: i32, y: i32) {
        self.data.push(ReplayEntry::Move {
            time,
            rotate: rotate.map(|b| if b { 1 } else { -1 }).unwrap_or(0),
            x: x as i8,
            y: y as i8,
        });
    }

    fn add_merge(&mut self, time: usize, drop: i32, next: piece::Piece) {
        self.data.push(ReplayEntry::Merge {
            time,
            drop: drop as i8,
            next,
        })
    }

    fn add_new_piece(&mut self, time: usize) {
        self.data.push(ReplayEntry::NewPiece {
            time,
        })
    }

    pub fn config(&self) -> &Config { &self.config }

    fn frames(&self) -> usize {
        self.data.last().map(|entry| entry.time()).unwrap_or(0)
    }
}

#[derive(Serialize, Deserialize)]
pub struct PlayedGame {
    // TODO: optimize data buffering, don't use replay, but custom Vec<u32> format
    replay: Replay,
    name: String,
    utc: DateTime<Utc>,
    score: i32,
    level: i32,
}

impl PlayedGame {
    pub fn replay(&self) -> &Replay { &self.replay }
    pub fn name(&self) -> String { self.name.clone() }
    pub fn score(&self) -> i32 { self.score }
    pub fn level(&self) -> i32 { self.level }
    pub fn utc(&self) -> DateTime<Utc> { self.utc }
    pub fn time_str(&self) -> String {
        let now = Local::now();
        let t = self.utc.with_timezone(&now.timezone());
        if t.date() == now.date() {
            format!("{:02}:{:02}", t.time().hour(), t.time().minute())
        } else {
            format!("{}-{:02}-{:02}", t.date().year(), t.date().month(), t.date().day())
        }
    }
}

#[derive(Serialize, Deserialize)]
pub struct Savegame {
    games: Vec<PlayedGame>,
}

impl Savegame {
    pub fn load(path: &str) -> Self {
        if let Ok(data) = std::fs::read(path) {
            if let Ok(game) = bincode::deserialize(&data) {
                return game;
            }
        }
        Savegame {
            games: Vec::new()
        }
    }

    pub fn save(&self, path: &str) {
        if let Ok(data) = bincode::serialize(self) {
            std::fs::write(path, data);
        }
    }

    pub fn add(&mut self, replay: Replay, name: String, score: i32, level: i32) {
        self.games.push(PlayedGame {
            replay,
            name,
            utc: Utc::now(),
            score,
            level
        });
    }

    pub fn by_score(&self) -> Vec<&PlayedGame> {
        let mut ret: Vec<&PlayedGame> = self.games.iter().collect();
        ret.sort_by(|a, b| b.score.cmp(&a.score));
        ret
    }

    pub fn by_date(&self) -> Vec<&PlayedGame> {
        let mut ret: Vec<&PlayedGame> = self.games.iter().collect();
        ret.sort_by(|a, b| b.utc.cmp(&a.utc));
        ret
    }
}
