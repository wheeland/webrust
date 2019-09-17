use cgmath::prelude::*;
use cgmath::*;
use std::f32::consts::PI;

pub struct FlyCamera {
    radius: f32,
    speed_move: f32,
    speed_look: f32,
    acceleration_look: f32,

    position : Vector3<f32>,
    position_normed: Vector3<f32>,
    neutral_view_dir: Vector3<f32>,
    vertical_angle: f32,

    view_matrix: Matrix4<f32>,
    look: Vector3<f32>,
    up: Vector3<f32>,
    right: Vector3<f32>
}

impl FlyCamera {
    pub fn new(radius: f32) -> Self {
        let mut cam = FlyCamera {
            radius,
            speed_move: radius / 2.0,
            speed_look: 25.0,
            acceleration_look: 1.5,
            position: Vector3::new(0.0, 0.0, 2.0 * radius),
            position_normed: Vector3::new(0.0, 0.0, 1.0),
            neutral_view_dir: Vector3::new(0.0, 1.0, 0.0),
            vertical_angle: -0.4 * PI,
            view_matrix: Matrix4::identity(),
            look: Vector3::new(0.0, 1.0, 0.0),
            up: Vector3::new(0.0, 1.0, 0.0),
            right: Vector3::new(1.0, 0.0, 0.0)
        };
        cam.update();
        cam
    }

    pub fn eye(&self) -> Vector3<f32> {
        self.position
    }

    pub fn scale_with_planet(&mut self, new_radius: f32) {
        let height = self.position.magnitude() - self.radius;
        self.radius = new_radius;
        self.position = self.position.normalize() * (new_radius + height);
        self.update();
    }

    pub fn move_up(&mut self, amount: f32) {
        self.position += self.position.normalize() * amount;
        self.update();
    }

    pub fn look(&self) -> Vector3<f32> {
        self.look
    }

    pub fn up(&self) -> Vector3<f32> {
        self.up
    }

    pub fn right(&self) -> Vector3<f32> {
        self.right
    }

    pub fn neutral_view_dir(&self) -> Vector3<f32> {
        self.neutral_view_dir
    }

    pub fn view_matrix(&self) -> Matrix4<f32> {
        self.view_matrix
    }

    pub fn move_speed(&self) -> f32 {
        self.speed_move
    }

    pub fn set_move_speed(&mut self, value: f32) {
        self.speed_move = value;
    }

    fn update(&mut self) {
        self.position_normed = self.eye().normalize();

        // keep neutral view distance orthogonal
        self.neutral_view_dir -= self.neutral_view_dir.dot(self.position_normed) * self.position_normed;
        self.neutral_view_dir = self.neutral_view_dir.normalize();

        self.vertical_angle = self.vertical_angle.min(PI).max(-PI);

        self.right = self.neutral_view_dir.cross(self.position_normed);
        self.look = self.vertical_angle.cos() * self.neutral_view_dir + self.vertical_angle.sin() * self.position_normed;
        self.up = self.right.cross(self.look);
        self.view_matrix = Matrix4::look_at(Point3::from_vec(self.position),
                                            Point3::from_vec(self.position + self.look),
                                            self.up);
    }

    pub fn translate_absolute(&mut self, delta: &Vector3<f32>) {
        self.position += *delta;

        // limit eye to be within maximum distance to planet
        let len = self.position.magnitude();
        if len > 5.0 * self.radius {
            self.position *= 5.0 * self.radius / len;
        }

        self.update();
    }

    pub fn translate(&mut self, rel: &Vector3<f32>) {
        let nonside_delta = self.speed_move * (self.look * rel.z + self.up * rel.y);
        let delta = nonside_delta + self.speed_move * (self.right * rel.x);

        if nonside_delta.magnitude2() > 0.00001 {
            let new_pos_norm = (self.eye() + delta).normalize();
            let new_angle = 0.5 * PI - new_pos_norm.dot(self.look).acos();
            if !new_angle.is_nan() {
                self.vertical_angle = new_angle;
            }
        }

        self.translate_absolute(&delta);
    }

    pub fn pan(&mut self, dx: f32, dy: f32) {
        let dx = dx.signum() * dx.abs().powf(self.acceleration_look) * self.speed_look;
        let dy = dy.signum() * dy.abs().powf(self.acceleration_look) * self.speed_look;

        self.neutral_view_dir = dx.cos() * self.neutral_view_dir + dx.sin() * self.right;
        self.vertical_angle += dy;
        self.vertical_angle = self.vertical_angle.min(std::f32::consts::FRAC_PI_2);
        self.vertical_angle = self.vertical_angle.max(-std::f32::consts::FRAC_PI_2);

        self.update();
    }

    pub fn near(&self) -> f32 {
        0.00001 * self.far()
    }

    pub fn far(&self) -> f32 {
        self.position.magnitude() * 2.0
    }

    pub fn mvp(&self, windowsize: (u32, u32), highrange: bool) -> Matrix4<f32> {
        let proj = Matrix4::from(PerspectiveFov {
            fovy: Rad::from(Deg(45.0)),
            aspect: windowsize.0 as f32 / windowsize.1 as f32,
            near: if highrange { self.near() } else { self.near() * 100.0 },
            far: self.far(),
        });
        proj * self.view_matrix()
    }
}
