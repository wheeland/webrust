use super::piece;
use super::super::util::Array2D;

#[derive(Clone)]
pub struct Stack {
    generation: i32,

    width: i32,
    height: i32,

    blocks: Array2D<piece::Type>,
}

impl Stack {
    pub fn new(width: usize, height: usize) -> Self {
        Stack {
            generation: 0,
            width: width as i32,
            height: height as i32,
            blocks: Array2D::new(width, height, piece::Type::None)
        }
    }

    pub fn blocks(&self) -> &Array2D<piece::Type> {
        &self.blocks
    }

    pub fn fits(&self, piece: piece::Piece, x: i32, y: i32) -> bool {
        let blocks = piece.blocks();

        for i in 0..4 {
            let x = x + i as i32;
            for j in 0..4 {
                let y = y + j as i32;
                if blocks[4 * j + i] {
                    // can't go left or right
                    if x < 0 || x >= self.width || y < 0 {
                        return false;
                    }

                    // ignore if the tile would go over the top of the field
                    if y < self.height && *self.blocks.at(x as usize, y as usize) != piece::Type::None {
                        return false;
                    }
                }
            }
        }

        true
    }

    pub fn merge(&self, piece: piece::Piece, x: i32, y: i32) -> Self {
        let piece_blocks = piece.blocks();
        let mut blocks = self.blocks.clone();

        // merge tile into existing blocks
        for i in 0..4 {
            let x = (x + i as i32);
            for j in 0..4 {
                let y = (y + j as i32);
                if piece_blocks[4 * j + i] {
                    if x >= 0 && x < self.width && y >= 0 && y < self.height {
                        blocks.set(x as usize, y as usize, piece.get_type());
                    }
                }
            }
        }

        Stack {
            generation: self.generation + 1,
            width: self.width,
            height: self.height,
            blocks
        }
    }

    pub fn eliminate(&self) -> (Self, Vec<i32>) {
        let mut blocks = self.blocks.clone();

        // check for eliminated rows
        let mut rows = Vec::new();
        for j in (0..self.height).rev() {
            let eliminate = (0..self.width).all(|x| {
                *blocks.at(x as usize, j as usize) != piece::Type::None
            });

            if eliminate {
                for x in 0..self.width {
                    for y in j..(self.height-1) {
                        let block = *blocks.at(x as usize, (y + 1) as usize);
                        blocks.set(x as usize, y as usize, block);
                    }
                    blocks.set(x as usize, (self.height - 1) as usize, piece::Type::None);
                }
                rows.push(j);
            }
        }

        (Stack {
            generation: self.generation + 1,
            width: self.width,
            height: self.height,
            blocks
        }, rows)
    }
}