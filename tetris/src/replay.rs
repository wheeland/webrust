use super::Config;
use super::state::*;
use super::piece;

#[derive(Debug, FromPrimitive, ToPrimitive)]
enum EntryType {
    Nop,
    MoveX,
    MoveDown,
    Rot,
    Merge,
    Spawn,
    NextPiece,
}

struct Entry (
    u16
);

impl Entry {
    fn from(dt: usize, tp: EntryType, detail: u8) -> Self {
        let dt = dt as u16;
        let tp = tp as u16;
        Entry (
            (dt << 8) + (tp << 5) + (detail as u16)
        )
    }

    fn entry_type(&self) -> EntryType {
        num::FromPrimitive::from_u32(((self.0 >> 5) & 0x7) as u32).unwrap()
    }

    fn dt(&self) -> usize {
        (self.0 >> 8) as usize
    }

    fn detail(&self) -> u8 {
        (self.0 & 0x1f) as u8
    }
}

#[derive(Clone, Serialize, Deserialize)]
pub struct Replay {
    config: Config,
    first: piece::Type,
    second: piece::Type,
    time: i32,
    data: Vec<u16>,
}

impl Replay {
    pub fn new(config: &Config, first: piece::Type, second: piece::Type, time: i32) -> Self {
        Replay {
            config: config.clone(),
            first,
            second,
            time,
            data: Vec::new(),
        }
    }

    fn add(&mut self, time: i32, tp: EntryType, detail: u8) {
        let mut dt = time - self.time;
        while dt > 255 {
            self.data.push(Entry::from(255, EntryType::Nop, 0).0);
            dt -= 255;
        }
        self.data.push(Entry::from(dt.max(0) as usize, tp, detail).0);
        self.time = time;
    }

    pub fn add_move(&mut self, time: i32, rotate: Option<bool>, x: i32, y: i32) {
        if let Some(clockwise) = rotate {
            self.add(time, EntryType::Rot, if clockwise { 1 } else { 0 });
        }
        if x != 0 {
            self.add(time, EntryType::MoveX, (x + 16) as u8);
        }
        if y != 0 {
            self.add(time, EntryType::MoveDown, (y + 16) as u8);
        }
    }

    pub fn add_merge(&mut self, time: i32, drop: i32, next: piece::Piece) {
        self.add(time, EntryType::Merge, drop as u8);
        self.add(time, EntryType::NextPiece, next.get_type() as u8);
    }

    pub fn add_new_piece(&mut self, time: i32) {
        self.add(time, EntryType::Spawn, 0);
    }

    pub fn config(&self) -> &Config {
        &self.config
    }

    pub fn frames(&self) -> i32 {
        self.time
    }
}

impl std::fmt::Debug for Replay {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        write!(f, "Replay{{entries={}, time={}s}}",
                self.data.len(),
                self.time)
    }
}

pub struct Replayer {
    config: super::Config,
    state: GameHistory,

    pub paused: bool,
    pub speed: f32,

    frames: usize,
    time: f32,
}

impl Replayer {
    pub fn new(replay: &Replay) -> Self {
        let state = GameHistory::new(
            &replay.config,
            piece::Piece::new(replay.first, 2),
            piece::Piece::new(replay.second, 2)
        );

        let mut ret = Replayer {
            config: replay.config.clone(),
            state,
            paused: false,
            speed: 1.0,
            frames: replay.frames().max(0) as usize,
            time: 0.0,
        };

        // keep track of current time, piece, soft-drop
        let mut time = 0;
        let mut drop = 0;
        let mut posx = ret.state.snapshot().piece().unwrap().1;
        let mut posy = ret.state.snapshot().piece().unwrap().2;

        for entry in &replay.data {
            let entry = Entry(*entry);
            let detail = entry.detail();
            time += entry.dt() as i32;

            match entry.entry_type() {
                EntryType::Nop => {
                }
                EntryType::MoveX => {
                    let piece = ret.state.snapshot().piece().unwrap().0;
                    posx += detail as i32 - 16;
                    ret.state.try_move(time, piece, posx, posy);
                }
                EntryType::MoveDown => {
                    let piece = ret.state.snapshot().piece().unwrap().0;
                    posy += detail as i32 - 16;
                    ret.state.try_move(time, piece, posx, posy);
                }
                EntryType::Rot => {
                    let piece = ret.state.snapshot().piece().unwrap().0.rotate(detail != 0);
                    ret.state.try_move(time, piece, posx, posy);
                }
                EntryType::Merge => {
                    drop = detail;
                }
                EntryType::Spawn => {
                    ret.state.start_new_piece(time);
                    posx = ret.state.snapshot().piece().unwrap().1;
                    posy = ret.state.snapshot().piece().unwrap().2;
                }
                EntryType::NextPiece => {
                    let tp = piece::Type::from_int(detail as u32);
                    ret.state.merge(time, piece::Piece::new(tp, 2), drop as i32);
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

    pub fn timestamp(&self) -> i32 {
        (self.time * 60.0) as i32
    }

    pub fn snapshot(&self) -> &Snapshot {
        self.state.snapshot_at(self.timestamp() as i32)
    }
}
