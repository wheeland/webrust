pub fn hsv(hue: f32, saturation: f32, value: f32) -> cgmath::Vector3<f32> {
    let hue = hue / 60.0;

    let hi = hue as i32;
    let hf = hue - hi as f32;
    let hi = (hi % 6 + 6) % 6;

    let p = value * (1.0 - saturation);
    let q = value * (1.0 - (saturation * hf));
    let t = value * (1.0 - (saturation * (1.0 - hf)));

    match hi {
        0 => cgmath::Vector3::new(value, t, p),
        1 => cgmath::Vector3::new(q, value, p),
        2 => cgmath::Vector3::new(p, value, t),
        3 => cgmath::Vector3::new(p, q, value),
        4 => cgmath::Vector3::new(t, p, value),
        _|5 => cgmath::Vector3::new(value, p, q),
    }
}

pub struct BufferBuilder {
    data: Vec<f32>
}

impl BufferBuilder {
    pub fn new() -> Self {
        BufferBuilder {
            data: Vec::new()
        }
    }

    pub fn f(mut self, v: f32) -> Self {
        self.data.push(v);
        self
    }

    pub fn v2(mut self, v: cgmath::Vector2<f32>) -> Self {
        self.data.push(v.x);
        self.data.push(v.y);
        self
    }

    pub fn v3(mut self, v: cgmath::Vector3<f32>) -> Self {
        self.data.push(v.x);
        self.data.push(v.y);
        self.data.push(v.z);
        self
    }

    pub fn v4(mut self, v: cgmath::Vector4<f32>) -> Self {
        self.data.push(v.x);
        self.data.push(v.y);
        self.data.push(v.z);
        self.data.push(v.w);
        self
    }

    pub fn get(self) -> Vec<f32> {
        self.data
    }
}

pub struct ValueHistory {
    count: usize,
    values: Vec<f32>
}

impl ValueHistory {
    pub fn new(count: usize) -> ValueHistory {
        ValueHistory {
            count,
            values: Vec::new()
        }
    }

    pub fn push(&mut self, value: f32) {
        self.values.push(value);
        while self.values.len() > self.count {
            self.values.remove(0);
        }
    }

    pub fn average(&self, last: usize) -> f32 {
        let mut sum = 0.0;
        let mut cnt = 0;
        for value in self.values.iter() {
            sum += value;
            cnt += 1;
            if cnt >= last {
                break;
            }
        }
        sum / cnt as f32
    }

    pub fn values(&self, last: usize) -> &[f32] {
        let count = std::cmp::min(self.count, self.values.len());
        let lower = std::cmp::max(0, count as i32 - last as i32) as usize;
        &self.values[lower..count]
    }
}

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
        write!(f, "Array2D ({}x{}) [\n", self.width, self.height);
        for j in 0..self.height {
            write!(f, "  ");
            for i in 0..self.width {
                write!(f, "{:1?}, ", self.at(i, j));
            }
            write!(f, "\n");
        }
        write!(f, "]\n")
    }
}

pub fn profile<Y, T: FnOnce()->Y>(s: &str, fun: T) -> Y{
    let start = std::time::Instant::now();
    let ret = fun();
    let delta = start.elapsed();
    let delta = delta.as_secs() as f32 + delta.subsec_nanos() as f32 / 1_000_000_000.0;
    println!("{}: {}", s, delta * 1000.0);
    ret
}