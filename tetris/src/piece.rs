
#[derive(Debug, Copy, Clone, PartialEq)]
#[derive(FromPrimitive, ToPrimitive)]
#[derive(Serialize, Deserialize)]
pub enum Type {
    O,
    I,
    T,
    L,
    J,
    S,
    Z,
    None,
}

impl Type {
    pub fn offset(&self) -> (f32, f32) {
        match &self {
            Type::O => (0.0, 0.0),
            Type::I => (0.0, 0.5),
            Type::T => (-0.5, 0.0),
            Type::L => (-0.5, 0.0),
            Type::J => (-0.5, 0.0),
            Type::S => (-0.5, 0.0),
            Type::Z => (-0.5, 0.0),
            _ => (0.0, 0.0),
        }
    }

    pub fn from_int(i: u32) -> Self {
        num::FromPrimitive::from_u32(i).unwrap()
    }
}

pub type Orientation = u8;

#[derive(Debug, Copy, Clone, PartialEq)]
#[derive(Serialize, Deserialize)]
pub struct Piece {
    tp: Type,
    orientation: Orientation
}

impl Piece {
    pub fn get_type(&self) -> Type {
        self.tp
    }

    pub fn new(tp: Type, orientation: Orientation) -> Self {
        Piece { tp, orientation }
    }

    pub fn rotate(&self, clockwise: bool) -> Self {
        let delta = if clockwise { 3 } else { 1 };
        Piece {
            tp: self.tp,
            orientation: (self.orientation + delta) % 4
        }
    }

    pub fn blocks(&self) -> [bool; 16] {
        match &self.tp {
            Type::None => [
                false, false, false, false,
                false, false, false, false,
                false, false, false, false,
                false, false, false, false,
            ],

            Type::O => [
                false, false, false, false,
                false, true,  true,  false,
                false, true,  true,  false,
                false, false, false, false,
            ],

            Type::I =>  match self.orientation % 4 {
                0 | 2 => [
                    false, false, false, false,
                    false, false, false, false,
                    true,  true,  true,  true,
                    false, false, false, false,
                ],
                1 | 3 => [
                    false, false, true , false,
                    false, false, true , false,
                    false, false, true , false,
                    false, false, true , false,
                ],
                _ => unreachable!("Piece Orientation does not compute")
            },

            Type::T => match self.orientation % 4 {
                0 => [
                    false, false, false, false,
                    false, false, false, false,
                    false, true , true , true ,
                    false, false, true , false,
                ],
                1 => [
                    false, false, false, false,
                    false, false, true , false,
                    false, true , true , false,
                    false, false, true , false,
                ],
                2 => [
                    false, false, false, false,
                    false, false, true , false,
                    false, true , true , true ,
                    false, false, false, false,
                ],
                3 => [
                    false, false, false, false,
                    false, false, true , false,
                    false, false, true , true ,
                    false, false, true , false,
                ],
                _ => unreachable!("Piece Orientation does not compute")
            },

            Type::J => match self.orientation % 4 {
                0 => [
                    false, false, false, false,
                    false, false, false, false,
                    false, true , true , true ,
                    false, true , false, false,
                ],
                1 => [
                    false, false, false, false,
                    false, true , true , false,
                    false, false, true , false,
                    false, false, true , false,
                ],
                2 => [
                    false, false, false, false,
                    false, false, false, true,
                    false, true , true , true ,
                    false, false, false, false,
                ],
                3 => [
                    false, false, false, false,
                    false, false, true , false,
                    false, false, true , false,
                    false, false, true , true ,
                ],
                _ => unreachable!("Piece Orientation does not compute")
            }

            Type::L => match self.orientation % 4 {
                0 => [
                    false, false, false, false,
                    false, false, false, false,
                    false, true , true , true ,
                    false, false, false, true ,
                ],
                1 => [
                    false, false, false, false,
                    false, false, true , false,
                    false, false, true , false,
                    false, true , true , false,
                ],
                2 => [
                    false, false, false, false,
                    false, true , false, false,
                    false, true , true , true ,
                    false, false, false, false,
                ],
                3 => [
                    false, false, false, false,
                    false, false, true , true ,
                    false, false, true , false,
                    false, false, true , false,
                ],
                _ => unreachable!("Piece Orientation does not compute")
            }

            Type::S => match self.orientation % 4 {
                0 | 2 => [
                    false, false, false, false,
                    false, true , true , false,
                    false, false, true , true ,
                    false, false, false, false,
                ],
                1 | 3 => [
                    false, false, false, true ,
                    false, false, true , true ,
                    false, false, true , false,
                    false, false, false, false,
                ],
                _ => unreachable!("Piece Orientation does not compute")
            }

            Type::Z => match self.orientation % 4 {
                0 | 2 => [
                    false, false, false, false,
                    false, false, true , true ,
                    false, true , true , false,
                    false, false, false, false,
                ],
                1 | 3 => [
                    false, false, true , false,
                    false, false, true , true ,
                    false, false, false, true ,
                    false, false, false, false,
                ],
                _ => unreachable!("Piece Orientation does not compute")
            }
        }
    }
}
