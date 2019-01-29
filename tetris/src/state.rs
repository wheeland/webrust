use std::rc::Rc;

use super::piece;
use super::stack;

#[derive(Clone, Copy)]
pub struct PieceStats {
    count: [i32; 7],
    drought: [i32; 7],
}

impl PieceStats {
    fn new() -> Self {
        PieceStats {
            count: [0, 0, 0, 0, 0, 0, 0],
            drought: [0, 0, 0, 0, 0, 0, 0],
        }
    }

    fn checkin(&self, tp: piece::Type) -> Self {
        let mut ret = self.clone();
        for i in 0..7 {
            ret.drought[i] += 1;
        }
        let piece_idx = tp as usize;
        ret.count[piece_idx] += 1;
        ret.drought[piece_idx] = 0;
        ret
    }

    pub fn get(&self, tp: piece::Type) -> (i32, i32) {
        let piece_idx = tp as usize;
        (self.count[piece_idx], self.drought[piece_idx])
    }
}

#[derive(Clone)]
struct Turn {
    score: i32,
    level: i32,
    left_to_clear: i32,
    cleared: i32,
    tetrises: i32,
    stack: stack::Stack,
    next_piece: piece::Piece,
    stats: PieceStats,
}

#[derive(Clone)]
enum State {
    ARE {
        waiting: piece::Piece,
        duration: i32,
        animation: Option<(Vec<i32>, stack::Stack)>,
    },
    Piece {
        piece: piece::Piece,
        x: i32,
        y: i32,
    },
}

#[derive(Clone)]
pub struct Snapshot {
    timestamp: i32,
    turn: Rc<Turn>,
    state: State,
}

pub struct GameHistory {
    config: super::Config,
    frames: Vec<Snapshot>
}

impl Snapshot {
    pub fn stack(&self) -> &super::stack::Stack {
        if let State::ARE{animation, ..} = &self.state {
            if let Some(animation) = animation.as_ref() {
                return &animation.1;
            }
        }
        &self.turn.stack
    }

    pub fn timestamp(&self) -> i32 {
        self.timestamp
    }

    pub fn score(&self) -> i32 {
        self.turn.score
    }

    pub fn level(&self) -> i32 {
        self.turn.level
    }

    pub fn lines(&self) -> i32 {
        self.turn.cleared
    }

    pub fn stats(&self) -> &PieceStats {
        &self.turn.stats
    }

    pub fn tetris_rate(&self) -> f32 {
        4.0 * self.turn.tetrises as f32 / self.turn.cleared.max(1) as f32
    }

    pub fn next_piece(&self) -> piece::Piece {
        match self.state {
            State::Piece{..} => self.turn.next_piece,
            State::ARE{waiting, ..} => waiting,
        }
    }

    pub fn piece(&self) -> Option<(piece::Piece, i32, i32)> {
        match self.state {
            State::Piece{piece, x, y} => Some((piece, x, y)),
            _ => None,
        }
    }

    pub fn ghost_piece(&self) -> Option<(piece::Piece, i32, i32)> {
        match self.state {
            State::Piece{piece, x, mut y} => {
                while self.turn.stack.fits(piece, x, y-1) {
                    y -= 1;
                }
                Some((piece, x, y))
            },
            _ => None,
        }
    }

    pub fn are_duration(&self) -> Option<i32> {
        match self.state {
            State::Piece{..} => None,
            State::ARE{duration, ..} => Some(duration)
        }
    }

    pub fn animation(&self) -> Option<(&Vec<i32>, &stack::Stack)> {
        match &self.state {
            State::Piece{..} => None,
            State::ARE{animation, ..} => animation.as_ref().map(|anim| (&anim.0, &anim.1))
        }
    }
}

impl GameHistory {
    pub fn try_move(&mut self, timestamp: i32, piece: piece::Piece, x: i32, y: i32) -> bool {
        // make sure we are progressing ever forward
        let last_frame = self.frames.last().unwrap().clone();
        if last_frame.timestamp > timestamp {
            panic!("Back to the past is not allowed in this reality");
        }

        let fits = last_frame.stack().fits(piece, x, y);
        if fits {
            self.frames.push(Snapshot {
                timestamp,
                turn: last_frame.turn.clone(),
                state: State::Piece { piece, x, y }
            });
        }

        fits
    }

    pub fn start_new_piece(&mut self, timestamp: i32) -> bool {
        let last_frame = self.frames.last().unwrap().clone();

        let new_frame = match last_frame.state {
            State::Piece{..} => panic!("GameHistory::new_piece() without ARE/Animation frame"),
            _ => Snapshot {
                timestamp,
                turn: last_frame.turn.clone(),
                state: State::Piece {
                    piece: last_frame.next_piece(),
                    x: self.config.width / 2 - 2,
                    y: self.config.height - 3,
                }
            }
        };

        self.frames.push(new_frame);

        let piece = self.frames.last().unwrap().piece().unwrap();
        let ret = self.frames.last().unwrap().turn.stack.fits(piece.0, piece.1, piece.2);

        ret
    }

    pub fn merge(&mut self, timestamp: i32, next_piece: piece::Piece, soft_drop: i32) -> bool {
        // make sure we are progressing ever forward
        let last_frame = self.frames.last().unwrap().clone();
        if last_frame.timestamp > timestamp {
            panic!("Back to the past is not allowed in this reality");
        }

        let last_piece = last_frame.piece().unwrap();
        let merged = last_frame.stack().merge(last_piece.0, last_piece.1, last_piece.2);
        let eliminated = merged.eliminate();

        // if we eliminated rows, update score
        let score = last_frame.score() + (last_frame.level() + 1) * (match eliminated.1.len() {
            0 => 0,
            1 => 40,
            2 => 100,
            3 => 300,
            4 => 1200,
            _ => panic!("This is bad")
        } + soft_drop);

        // update level / left-to-clear
        let mut level = last_frame.level();
        let mut left_to_clear = last_frame.turn.left_to_clear - eliminated.1.len() as i32;
        if left_to_clear <= 0 {
            level += 1;
            left_to_clear += 10;
        }
        let tetrises = last_frame.turn.tetrises + (if eliminated.1.len() == 4 { 1 } else { 0 });

        let new_turn = Rc::new(Turn {
            score,
            level,
            left_to_clear,
            cleared: last_frame.turn.cleared + eliminated.1.len() as i32,
            tetrises,
            stack: eliminated.0,
            next_piece,
            stats: last_frame.turn.stats.checkin(last_frame.next_piece().get_type()),
        });

        let line_cleared = !eliminated.1.is_empty();

        let mut are = self.config.are_base + last_piece.2.max(0) * (self.config.are_max - self.config.are_base) / self.config.height;
        if line_cleared {
            are += self.config.line_clear;
        }

        let new_state = State::ARE {
            waiting: last_frame.next_piece(),
            duration: are,
            animation: match line_cleared {
                false => None,
                true => Some((eliminated.1, merged))
            }
        };
        let new_frame = Snapshot {
            timestamp,
            turn: new_turn,
            state: new_state,
        };
        self.frames.push(new_frame);

        line_cleared
    }

    pub fn new(config: &super::Config, first: piece::Piece, second: piece::Piece) -> Self {
        let turn0 = Rc::new(Turn {
            score: 0,
            level: config.level,
            left_to_clear: config.transition(),
            cleared: 0,
            tetrises: 0,
            stack: stack::Stack::new(config.width as usize, config.height as usize),
            next_piece: second,
            stats: PieceStats::new().checkin(first.get_type())
        });

        GameHistory {
            config: config.clone(),
            frames: vec!(Snapshot {
                timestamp: 0,
                turn: turn0.clone(),
                state: State::Piece {
                    piece: first,
                    x: config.width as i32 / 2 - 2,
                    y: config.height as i32 - 3,
                }
            })
        }
    }

    pub fn snapshot(&self) -> &Snapshot {
        self.frames.last().unwrap()
    }

    pub fn snapshot_at(&self, timestamp: i32) -> &Snapshot {
        let mut ret = self.frames.first().unwrap();
        for frame in &self.frames {
            if frame.timestamp > timestamp {
                break;
            }
            ret = frame;
        }
        ret
    }
}