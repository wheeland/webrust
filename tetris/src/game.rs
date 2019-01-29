use super::piece;
use super::stack;
use super::state::*;

#[derive(PartialEq,Clone,Copy,Debug)]
enum Move {
    None,
    Left,
    Right,
}

pub enum Outcome {
    None,
    Merge,
    Clear(Vec<i32>, stack::Stack),
    Death,
}

impl std::fmt::Debug for Outcome {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        write!(f, "{}", match &self {
            Outcome::None => "None",
            Outcome::Merge => "Merge",
            Outcome::Clear(..) => "Clear",
            Outcome::Death => "Death",
        })
    }
}

pub struct Game {
    config: super::Config,

    state: GameHistory,

    timestamp: i32,
    lost: Option<(i32, i32)>,

    drop_timer: i32,
    left: bool,
    right: bool,
    movekey: Move,
    das: i32,

    down_pressed: bool,
    down: i32,
    down_das: i32,

    replay: super::replay::Replay,
}

impl Game {
    fn gen_piece(last: piece::Type) -> piece::Piece {
        let first = rand::random::<u32>() % 8;

        let tp = if first < 7 && first != last as u32 {
            piece::Type::from_int(first)
        } else {
            piece::Type::from_int(rand::random::<u32>() % 7)
        };

        piece::Piece::new(tp, 2)
    }

    pub fn new(config: &super::Config) -> Self {
        let first = Self::gen_piece(piece::Type::None);
        let second = Self::gen_piece(first.get_type());
        let timestamp = 0;

        let state = GameHistory::new(config, first, second);
        let replay = super::replay::Replay::new(config, first.get_type(), second.get_type(), timestamp);

        Game {
            config: config.clone(),

            state,
            timestamp,
            lost: None,

            drop_timer: -90,
            movekey: Move::None,
            left: false,
            right: false,
            das: 0,

            down: 0,
            down_pressed: false,
            down_das: 0,

            replay,
        }
    }

    fn try_move(&mut self, rotate: Option<bool>, dx: i32, dy: i32) -> bool {
        if self.lost.is_some() {
            return false;
        }

        let curr_frame = self.state.snapshot().clone();

        if let Some(mut piece) = curr_frame.piece() {
            if let Some(rot) = rotate {
                piece.0 = piece.0.rotate(rot);
            }

            let ret = self.state.try_move(self.timestamp, piece.0, piece.1 + dx, piece.2 + dy);
            if ret {
                self.replay.add_move(self.timestamp, rotate, dx, dy);
            }
            ret
        } else {
            false
        }
    }

    fn update_move(&mut self) {
        let movekey = if self.left && !self.right {
            Move::Left
        } else if !self.left && self.right {
            Move::Right
        } else {
            Move::None
        };

        if movekey != self.movekey {
            self.movekey = movekey;

            // try move piece
            if movekey != Move::None {
                let direction = if movekey == Move::Left { -1 } else { 1 };
                let moved = self.try_move(None, direction, 0);

                // init DAS
                self.das = if moved { self.config.das_initial } else { 0 };
            }
        }
    }

    pub fn left(&mut self, pressed: bool) {
        self.left = pressed;
        self.update_move();
    }

    pub fn right(&mut self, pressed: bool) {
        self.right = pressed;
        self.update_move();
    }

    pub fn down(&mut self, pressed: bool) {
        if !self.down_pressed && pressed {
            if self.try_move(None, 0, -1) {
                self.down = 1;
                self.down_das = self.config.das_down;
            }
        }
        else if !pressed {
            self.down = 0;
        }
        self.down_pressed = pressed;
    }

    pub fn rotate(&mut self, clockwise: bool) {
        self.try_move(Some(clockwise), 0, 0);
    }

    fn move_down(&mut self) -> Outcome {
        if self.lost.is_some() {
            return Outcome::Death;
        }

        // If we don't have a current piece (because we ARE in ARE), just ignore
        let curr_piece = self.state.snapshot().piece();
        if curr_piece.is_none() {
            return Outcome::None;
        }

        // try to drop piece one tile further and merge it if it doesn't work
        if !self.try_move(None, 0, -1) {
            // generate next piece
            let next_piece = Self::gen_piece(self.state.snapshot().next_piece().get_type());

            // merge piece
            self.replay.add_merge(self.timestamp, self.down, next_piece);
            self.state.merge(self.timestamp, next_piece, self.down);

            // adjust timers
            self.drop_timer = -self.state.snapshot().are_duration().unwrap();
            self.down = 0;

            let animation = self.state.snapshot().animation();
            return match animation {
                None => Outcome::Merge,
                Some(anim) => Outcome::Clear(anim.0.clone(), anim.1.clone())
            }
        }

        Outcome::None
    }

    pub fn frame(&mut self) -> Outcome {
        self.timestamp += 1;

        let mut ret = Outcome::None;

        // update DAS movement
        if self.movekey != Move::None && self.das <= 0 {
            let direction = if self.movekey == Move::Left { -1 } else { 1 };
            if self.try_move(None, direction, 0) {
                self.das = self.config.das_step;
            }
        }
        self.das -= 1;

        // If we ARE in ARE, count down and possibly start next tile
        // only move tiles down if ARE is finished
        let are_start = self.state.snapshot().timestamp();
        let mut are_we_are = false;
        if let Some(duration) = self.state.snapshot().are_duration() {
            if are_start + duration < self.timestamp {
                self.replay.add_new_piece(self.timestamp);
                if !self.state.start_new_piece(self.timestamp) {
                    let last_breath = self.state.snapshot();
                    self.lost = Some((last_breath.score(), last_breath.level()));
                    ret = Outcome::Death;
                }
                self.drop_timer = 0;   // re-set gravity timer
            } else {
                are_we_are = true;
            }
        }

        // Compute gravity for current level
        let gravity = self.state.snapshot().level();
        let gravity = gravity.min(self.config.gravity.len() as i32 - 1);
        let gravity = *self.config.gravity.get(gravity as usize).unwrap();

        // update soft drop
        let mut move_down = false;
        if self.down > 0 && self.down_das <= 0 {
            self.drop_timer = gravity;
            self.down_das = self.config.das_down;
            self.down += 1;
            move_down = true;
        }
        self.down_das -= 1;

        if !are_we_are {
            self.drop_timer += 1;

            // If down is not pressed, we might want to move down becaue of gravity
            if !self.down_pressed && self.drop_timer >= gravity {
                move_down = true;
            }

            if move_down {
                ret = self.move_down();
                self.drop_timer = 0;
            }
        }

        ret
    }

    pub fn snapshot(&self) -> &Snapshot {
        self.state.snapshot()
    }

    pub fn timestamp(&self) -> i32 {
        self.timestamp
    }

    pub fn replay(&self) -> &super::replay::Replay {
        &self.replay
    }
}
