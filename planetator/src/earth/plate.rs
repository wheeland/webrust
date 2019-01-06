use cgmath::{Vector2, Vector3,InnerSpace};

pub static STRETCH: f32 = 0.9;
pub static STRETCH_ASIN: f32 = 1.1197695149986342;

fn stretch(v: f32) -> f32 {
    (v * STRETCH).asin() / STRETCH_ASIN
}

fn unstretch(v: f32) -> f32 {
    (v * STRETCH_ASIN).sin() / STRETCH
}

#[derive(Debug)]
#[derive(Copy)]
#[derive(Clone)]
#[derive(PartialEq)]
#[derive(Hash)]
pub enum Direction {
    PosX,
    NegX,
    PosY,
    NegY,
    PosZ,
    NegZ
}

impl Direction {
    fn split(value: f32) -> (bool, f32) {
        if value > 0.0 { (true, value) } else { (false, -value) }
    }

    // returns ([0,1,2], pos/neg, value) for largest absolute x/y/z component
    fn largest_component(position: &Vector3<f32>) -> (i32, bool, f32) {
        let x = Self::split(position.x);
        let y = Self::split(position.y);
        let z = Self::split(position.z);
        if x.1 > y.1 {
            if x.1 > z.1 {
                (0, x.0, x.1)
            } else {
                (2, z.0, z.1)
            }
        } else {
            if y.1 > z.1 {
                (1, y.0, y.1)
            } else {
                (2, z.0, z.1)
            }
        }
    }

    pub fn from(position: &Vector3<f32>) -> Self {
        let largest = Self::largest_component(position);
        match largest.0 {
            0 => { if largest.1 { Direction::PosX } else { Direction::NegX } },
            1 => { if largest.1 { Direction::PosY } else { Direction::NegY } }
            2 => { if largest.1 { Direction::PosZ } else { Direction::NegZ } }
            _ => { unreachable!() }
        }
    }

    pub fn square_to_cubic(&self, sq: &Vector2<f32>) -> Vector3<f32> {
        let lx = stretch(sq.x);
        let ly = stretch(sq.y);

        match self {
            Direction::PosX => Vector3::new(1.0, ly, lx),
            Direction::NegX => Vector3::new(-1.0, lx, ly),
            Direction::PosY => Vector3::new(lx, 1.0, ly),
            Direction::NegY => Vector3::new(ly, -1.0, lx),
            Direction::PosZ => Vector3::new(ly, lx, 1.0),
            Direction::NegZ => Vector3::new(lx, ly, -1.0),
        }
    }

    pub fn spherical_to_dir_and_square(pos: &Vector3<f32>) -> (Self, Vector2<f32>) {
        let e = 1e-10;
        let largest = Self::largest_component(pos);
        let pos = pos / largest.2.max(e);

        let mut result = match largest.0 {
            0 => { if largest.1 { (Direction::PosX, Vector2::new(pos.z, pos.y)) } else { (Direction::NegX, Vector2::new(pos.y, pos.z)) } }
            1 => { if largest.1 { (Direction::PosY, Vector2::new(pos.x, pos.z)) } else { (Direction::NegY, Vector2::new(pos.z, pos.x)) } }
            2 => { if largest.1 { (Direction::PosZ, Vector2::new(pos.y, pos.x)) } else { (Direction::NegZ, Vector2::new(pos.x, pos.y)) } }
            _ => { unreachable!() }
        };
        result.1.x = unstretch(result.1.x);
        result.1.y = unstretch(result.1.y);
        result
    }

    pub fn spherical_to_square(&self, p: &Vector3<f32>) -> Vector2<f32> {
        let e = 1e-10;
        let mut result = match self {
            Direction::PosX => Vector2::new(p.z, p.y) / p.x.max(e),
            Direction::NegX => Vector2::new(p.y, p.z) / p.x.max(e),
            Direction::PosY => Vector2::new(p.x, p.z) / p.y.max(e),
            Direction::NegY => Vector2::new(p.z, p.x) / p.y.max(e),
            Direction::PosZ => Vector2::new(p.y, p.x) / p.z.max(e),
            Direction::NegZ => Vector2::new(p.x, p.y) / p.z.max(e),
        };
        result.x = unstretch(result.x);
        result.y = unstretch(result.y);
        result
    }
}

#[derive(Copy)]
#[derive(Clone)]
#[derive(PartialEq)]
#[derive(Hash)]
pub struct Position {
    direction: Direction,
    depth: i32,
    x: i32,
    y: i32
}

impl Eq for Position {
}

impl Position {
    pub fn root(dir: Direction) -> Self {
        Position {
            direction: dir,
            depth: 0,
            x: 0,
            y: 0
        }
    }

    pub fn child(&self, dx: i32, dy: i32) -> Self {
        debug_assert!(dx == 0 || dx == 1);
        debug_assert!(dy == 0 || dy == 1);
        Position {
            direction: self.direction,
            depth: self.depth + 1,
            x: self.x * 2 + dx,
            y: self.y * 2 + dy
        }
    }

    pub fn direction(&self) -> Direction{ self.direction }
    pub fn depth(&self) -> i32 { self.depth }
    pub fn x(&self) -> i32 { self.x }
    pub fn y(&self) -> i32 { self.y }

    pub fn square_to_cube(&self, local: &Vector2<f32>) -> Vector3<f32> {
        self.direction.square_to_cubic(&self.square_to_root(local))
    }

    pub fn square_to_sphere(&self, local: &Vector2<f32>) -> Vector3<f32> {
        self.square_to_cube(local).normalize()
    }

    pub fn square_to_root(&self, local: &Vector2<f32>) -> Vector2<f32> {
        let gx = -1.0 + (self.x as f32 + local.x) * 0.5f32.powi(self.depth - 1);
        let gy = -1.0 + (self.y as f32 + local.y) * 0.5f32.powi(self.depth - 1);
        Vector2::new(gx, gy)
    }

    pub fn square_from_root(&self, global: &Vector2<f32>) -> Vector2<f32> {
        let lx = (global.x + 1.0) * 2.0f32.powi(self.depth - 1) - self.x as f32;
        let ly = (global.y + 1.0) * 2.0f32.powi(self.depth - 1) - self.y as f32;
        Vector2::new(lx, ly)
    }
}

impl std::fmt::Display for Position {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        write!(f, "{:?}({}: {},{})", self.direction, self.depth, self.x, self.y)
    }
}

impl std::fmt::Debug for Position {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        write!(f, "{:?}({}: {},{})", self.direction, self.depth, self.x, self.y)
    }
}