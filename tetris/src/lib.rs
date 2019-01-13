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

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PlayedGame {
    name: String,
    utc: chrono::DateTime<chrono::Utc>,
    score: i32,
    start_level: i32,
    end_level: i32,
    duration: f32,
    replay_id: usize,
}

impl PlayedGame {
    pub fn new(replay_id: usize, utc: DateTime<Utc>, name: String, score: i32, start_level: i32, end_level: i32, duration: f32) -> Self {
        PlayedGame {
            name,
            utc,
            score,
            start_level,
            end_level,
            replay_id,
            duration
        }
    }

    pub fn replay(&self) -> &usize { &self.replay_id }
    pub fn name(&self) -> String { self.name.clone() }
    pub fn score(&self) -> i32 { self.score }
    pub fn start_level(&self) -> i32 { self.start_level }
    pub fn end_level(&self) -> i32 { self.end_level }
    pub fn duration(&self) -> f32 { self.duration }
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
