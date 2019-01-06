use super::piece;
use super::stack;
use super::state::*;

pub struct Replayer {
    config: super::Config,
    state: GameHistory,

    pub paused: bool,
    pub speed: f32,

    frames: usize,
    time: f32,
}

impl Replayer {
    pub fn new(replay: &super::Replay) -> Self {
        let state = GameHistory::new(&replay.config, replay.first, replay.second);

        let mut ret = Replayer {
            config: replay.config.clone(),
            state,
            paused: false,
            speed: 1.0,
            frames: replay.frames(),
            time: 0.0,
        };

        for entry in &replay.data {
            match entry {
                super::ReplayEntry::Move{time, rotate, x, y} => {
                    let mut piece = ret.state.snapshot().piece().unwrap();
                    if *rotate != 0 {
                        piece.0 = piece.0.rotate(*rotate > 0);
                    }

                    ret.state.try_move(*time, piece.0, *x as i32, *y as i32);
                },
                super::ReplayEntry::Merge{time, drop, next} => {
                    ret.state.merge(*time, *next, *drop as _);
                },
                super::ReplayEntry::NewPiece{time} => {
                    ret.state.start_new_piece(*time);
                }
            }
        }

        ret
    }

    pub fn frame(&self) -> f32 {
        self.time
    }

    pub fn jump(&mut self, time: f32) {
        self.time = time.max(0.0).min(self.length());
    }

    pub fn advance(&mut self, dt: f32) {
        if !self.paused {
            self.time = (self.time + dt).max(0.0).min(self.length());
        }
    }

    pub fn length(&self) -> f32 {
        self.frames as f32 / 60.0
    }

    pub fn timestamp(&self) -> usize {
        (self.time * 60.0) as usize
    }

    pub fn snapshot(&self) -> &Snapshot {
        self.state.snapshot_at(self.timestamp())
    }
}