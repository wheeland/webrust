#[derive(Clone)]
pub struct Array2D<T: Copy> {
    width: usize,
    height: usize,
    data: Vec<T>
}

impl<T: Copy> Array2D<T> {
    pub fn new(w: usize, h: usize, default: T) -> Self {
        let mut data: Vec<T> = Vec::new();
        data.resize((w * h) as usize, default);
        Array2D {
            width: w,
            height: h,
            data
        }
    }

    #[inline]
    pub fn width(&self) -> usize {
        self.width
    }

    #[inline]
    pub fn height(&self) -> usize {
        self.height
    }

    pub fn for_each<F: FnMut(i32, i32, &T)>(&self, functor: &mut F) {
        for j in 0..self.height {
            for i in 0..self.width {
                let idx = i + self.width * j;
                functor(i as i32, j as i32, &self.data[idx]);
            }
        }
    }

    #[inline]
    pub fn at(&self, x: usize, y: usize) -> &T {
        debug_assert!(x < self.width);
        debug_assert!(y < self.height);
        let idx = x + self.width * y;
        &self.data[idx as usize]
    }

    #[inline]
    pub fn at_mut(&mut self, x: usize, y: usize) -> &mut T {
        debug_assert!(x < self.width);
        debug_assert!(y < self.height);
        let idx = x + self.width * y;
        &mut self.data[idx as usize]
    }

    #[inline]
    pub fn set(&mut self, x: usize, y: usize, value: T) {
        debug_assert!(x < self.width);
        debug_assert!(y < self.height);
        let idx = x + self.width * y;
        self.data[idx as usize] = value;
    }
}

impl<T: Copy+std::fmt::Debug> std::fmt::Debug for Array2D<T> {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        write!(f, "Array2D ({}x{}) [\n", self.width, self.height)?;
        for j in 0..self.height {
            write!(f, "  ")?;
            for i in 0..self.width {
                write!(f, "{:1?}, ", self.at(i, j))?;
            }
            write!(f, "\n")?;
        }
        write!(f, "]\n")
    }
}
