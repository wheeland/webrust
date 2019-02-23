use cgmath::prelude::*;
use cgmath::*;
use tinygl::{Program, Texture, Uniform, OffscreenBuffer};

struct BoundingBox {
    has_data: bool,
    min: Vector3<f32>,
    max: Vector3<f32>,
}

impl BoundingBox {
    pub fn new() -> Self {
        BoundingBox {
            has_data: false,
            min: Vector3::new(0.0, 0.0, 0.0),
            max: Vector3::new(0.0, 0.0, 0.0),
        }
    }
    pub fn add(&mut self, pt: Vector3<f32>) {
        if self.has_data {
            self.min.x = self.min.x.min(pt.x);
            self.min.y = self.min.y.min(pt.y);
            self.min.z = self.min.z.min(pt.z);
            self.max.x = self.max.x.max(pt.x);
            self.max.y = self.max.y.max(pt.y);
            self.max.z = self.max.z.max(pt.z);
        } else {
            self.min = pt;
            self.max = pt;
            self.has_data = true;
        }
    }

    pub fn size(&self) -> f32 {
        let extent = self.max - self.min;
        extent.x.max(extent.y).max(extent.z)
    }

    pub fn depth(&self) -> f32 {
        self.size() * 20.0
    }

    pub fn matrix(&self) -> Matrix4<f32> {
        let translate = Matrix4::from_translation(-0.5 * (self.min + self.max));
        let sz_scale = 1.0 / self.size();
        let depth_scale = 1.0 / self.depth();
        Matrix4::from_nonuniform_scale(sz_scale, sz_scale, -depth_scale) * translate
    }
}

struct Entry {
    bounds: BoundingBox,
    fbo: OffscreenBuffer,
    mvp: Matrix4<f32>,
    orthogonal_depth: f32,
}

pub struct ShadowMap {
    size: (u32, u32),
    entries: Vec<Entry>,
    program: Program,
}

impl ShadowMap {
    pub fn new(size: (u32, u32)) -> Self {
        // Create FBOs
        let mut entries = Vec::new();
        while entries.len() < 4 {
            let mut buf = OffscreenBuffer::new((size.0 as _, size.1 as _));

            buf.add_depth_texture();
            {
                let tex = buf.depth_texture_mut().unwrap();
                tex.bind();
                tex.wrap(gl::TEXTURE_WRAP_S, gl::CLAMP_TO_EDGE);
                tex.wrap(gl::TEXTURE_WRAP_T, gl::CLAMP_TO_EDGE);
                tex.filter(gl::TEXTURE_MIN_FILTER, gl::LINEAR as _);
                tex.filter(gl::TEXTURE_MAG_FILTER, gl::LINEAR as _);
            }

            entries.push(Entry {
                fbo: buf,
                bounds: BoundingBox::new(),
                mvp: Matrix4::from_scale(1.0),
                orthogonal_depth: 0.0,
            });
        }

        ShadowMap {
            size,
            entries,
            program: Program::new_versioned("
                in vec4 posHeight;
                uniform float radius;
                uniform mat4 mvp;
                void main()
                {
                    vec3 pos = posHeight.xyz * (posHeight.w + radius);
                    gl_Position = mvp * vec4(pos, 1.0);
                }",
                "void main() {}",
                300
            )
        }
    }

    pub fn program(&self) -> &Program {
        &self.program
    }

    pub fn render<T: Fn(&Program, Vector3<f32>, Matrix4<f32>)>(
        &mut self,
        radius: f32,
        sun_direction: Vector3<f32>,
        eye: Vector3<f32>,
        look: Vector3<f32>,
        render: T,
    ) -> Vec<(&Texture, Matrix4<f32>, f32)> {
        // Create Sun rotation matrix
        let sun_lon = sun_direction.x.atan2(sun_direction.z);
        let sun_lat = sun_direction.y.asin();
        let sun_rotation = Matrix4::from_angle_x(Rad(sun_lat)) * Matrix4::from_angle_y(Rad(-sun_lon));

        // setup GL
        unsafe {
            gl::Clear(gl::DEPTH_BUFFER_BIT);
            gl::Enable(gl::POLYGON_OFFSET_FILL);
            gl::DepthFunc(gl::LESS);
            gl::PolygonOffset(2.0, 2.0);
        }

        let mut ret = Vec::new();

        // go through all passes
        for (num, mut entry) in self.entries.iter_mut().enumerate() {
            // bogo shadow bounds!
            let cubesize = 1.1 * radius * 0.4f32.powi(num as i32);
            let look_surface_center = eye.normalize() * radius;
            let sunspace_center = sun_rotation.transform_vector(look_surface_center);

            // calculate bounding box for this shadow map
            // this stuff actually works! we just need to figure out where the center of the maze should be
            let mut cube = BoundingBox::new();
            cube.add(sunspace_center);
            cube.add(sunspace_center + Vector3::new(-1.0, 0.0, 0.0) * cubesize);
            cube.add(sunspace_center + Vector3::new(1.0, 0.0, 0.0)  * cubesize);
            cube.add(sunspace_center + Vector3::new(0.0, -1.0, 0.0) * cubesize);
            cube.add(sunspace_center + Vector3::new(0.0, 1.0, 0.0)  * cubesize);

            entry.orthogonal_depth = cube.depth();
            entry.bounds = cube;
            entry.mvp = entry.bounds.matrix() * sun_rotation;

            // configure program
            self.program.bind();
            self.program.uniform("mvp", Uniform::Mat4(entry.mvp));

            // render into depth map
            entry.fbo.bind();
            unsafe { gl::Clear(gl::DEPTH_BUFFER_BIT); }
            render(&self.program, sun_direction * radius * 2.0, entry.mvp);

            ret.push((entry.fbo.depth_texture().unwrap(), entry.mvp, entry.orthogonal_depth));
        }

        // reset GL
        unsafe {
            gl::Disable(gl::POLYGON_OFFSET_FILL);
            gl::DepthFunc(gl::LEQUAL);
            gl::PolygonOffset(0.0, 0.0);
        }
        tinygl::OffscreenBuffer::unbind();

        ret
    }
}
