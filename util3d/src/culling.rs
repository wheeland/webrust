use cgmath::prelude::*;
use cgmath::*;

pub struct Sphere {
    center: Vector3<f32>,
    radius: f32
}

impl Sphere {
    pub fn from(center: Vector3<f32>, radius: f32) -> Self {
        Sphere {
            center,
            radius
        }
    }
}

pub struct Culler {
    planes: [Vector4<f32>; 4]
}

impl Culler {
    fn get_plane_equation(v: Vector4<f32>) -> Vector4<f32> {
        v / Vector3::new(v.x, v.y, v.z).magnitude()
    }

    pub fn new(mvp: &Matrix4<f32>) -> Self {
        Culler {
            planes: [
                Self::get_plane_equation(mvp.row(3) + mvp.row(0)),  // left
                Self::get_plane_equation(mvp.row(3) - mvp.row(0)),  // right
                Self::get_plane_equation(mvp.row(3) + mvp.row(1)),  // top
                Self::get_plane_equation(mvp.row(3) - mvp.row(1)),  // bottom
//                Self::get_plane_equation(mvp.row(3) + mvp.row(2)),  // front
//                Self::get_plane_equation(mvp.row(3) - mvp.row(2)),  // back
            ]
        }
    }

    pub fn visible(&self, sphere: &Sphere) -> bool {
        let center = Vector4::new(sphere.center.x, sphere.center.y, sphere.center.z, 1.0);
        self.planes.iter().all(|plane| {
            plane.dot(center) > -sphere.radius
        })
    }
}
