extern crate cgmath;

pub mod noise;
pub mod flycamera;
pub mod culling;

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

