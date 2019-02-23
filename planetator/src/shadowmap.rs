use cgmath::prelude::*;
use cgmath::*;
use tinygl::{Program, Texture, Uniform, OffscreenBuffer};

struct ShadowCascade {
    // constant:
    level: i32,
    fbo: OffscreenBuffer,
    extent: f32,
    granularity: f32,
    orthogonal_depth: f32,

    // may change every time it's rendered:
    center: Vector3<f32>,
    mvp: Matrix4<f32>,
}

impl ShadowCascade {
    fn new(size: u32, level: i32, max_radius: f32) -> Self {
        // Create FBO
        let mut fbo = OffscreenBuffer::new((size as _, size as _));
        fbo.add_depth_texture();
        {
            let tex = fbo.depth_texture_mut().unwrap();
            tex.bind();
            tex.wrap(gl::TEXTURE_WRAP_S, gl::CLAMP_TO_EDGE);
            tex.wrap(gl::TEXTURE_WRAP_T, gl::CLAMP_TO_EDGE);
            tex.filter(gl::TEXTURE_MIN_FILTER, gl::LINEAR as _);
            tex.filter(gl::TEXTURE_MAG_FILTER, gl::LINEAR as _);
        }

        let extent = max_radius * 0.4f32.powi(level as i32);

        ShadowCascade {
            level,
            fbo,
            extent,
            granularity: 2.0 * extent / size as f32,
            orthogonal_depth: extent * 20.0,
            center: Vector3::new(0.0, 0.0, 0.0),
            mvp: Matrix4::from_scale(1.0)
        }
    }

    fn set_radius(&mut self, max_radius: f32) {
        self.extent = max_radius * 0.4f32.powi(self.level as i32);
        self.granularity = 2.0 * self.extent / self.fbo.size().0 as f32;
        self.orthogonal_depth = self.extent * 20.0;
    }

    fn set_center(&mut self, center: Vector3<f32>) {
        let cx = (center.x / self.granularity).round() * self.granularity;
        let cy = (center.y / self.granularity).round() * self.granularity;
        let cz = (center.z / self.granularity).round() * self.granularity;
        self.center = Vector3::new(cx, cy, cz);;

        let translate = Matrix4::from_translation(-self.center);
        let sz_scale = 1.0 / self.extent;
        let depth_scale = 1.0 / self.orthogonal_depth;
        self.mvp = Matrix4::from_nonuniform_scale(sz_scale, sz_scale, -depth_scale) * translate
    }
}

pub struct ShadowMap {
    size: u32,
    radius: f32,
    cascades: Vec<ShadowCascade>,
    program: Program,
}

static mut foo: usize = 0;

impl ShadowMap {
    pub fn new(size: u32, radius: f32) -> Self {
        // Create FBOs
        let mut cascades = Vec::new();
        for i in 0..6 {
            cascades.push(ShadowCascade::new(size, i, radius * 1.1));
        }

        ShadowMap {
            size,
            cascades,
            radius,
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

    pub fn set_radius(&mut self, radius: f32) {
        if self.radius != radius {
            self.radius = radius;
            for mut cascade in &mut self.cascades {
                cascade.set_radius(radius);
            }
        }
    }

    pub fn cascades(&self) -> Vec<(&Texture, Matrix4<f32>, f32)> {
        self.cascades
            .iter()
            .map(|cascade| {
                (cascade.fbo.depth_texture().unwrap(), cascade.mvp, cascade.orthogonal_depth)
            })
            .collect()
    }

    pub fn program(&self) -> &Program {
        &self.program
    }

    pub fn collect_renderable_cascades(
        &mut self,
        sun_direction: Vector3<f32>,
        eye: Vector3<f32>,
        look: Vector3<f32>,
    ) -> Vec<usize>
    {
        // Create Sun rotation matrix
        let sun_lon = sun_direction.x.atan2(sun_direction.z);
        let sun_lat = sun_direction.y.asin();
        let sun_rotation = Matrix4::from_angle_x(Rad(sun_lat)) * Matrix4::from_angle_y(Rad(-sun_lon));

        // cheap for now: center on camera eye
        let look_surface_center = eye.normalize() * self.radius;
        let sunspace_center = sun_rotation.transform_vector(look_surface_center);

        let count = self.cascades.len();
        unsafe { foo += 1; }

        let mut ret = Vec::new();

        // go through all passes
        for (num, mut cascade) in self.cascades.iter_mut().enumerate() {
            let do_render = unsafe { foo % count == num };

            if do_render {
                println!("render {} {}", num, do_render);
                cascade.set_center(sunspace_center);
                ret.push(num);
            }
        }

        // setup GL
        self.program.bind();
        unsafe {
            gl::Clear(gl::DEPTH_BUFFER_BIT);
            gl::Enable(gl::POLYGON_OFFSET_FILL);
            gl::DepthFunc(gl::LESS);
            gl::PolygonOffset(2.0, 2.0);
        }

        ret
    }

    pub fn prepare_render(&self, num: usize) -> Matrix4<f32> {
        // configure program
        self.program.uniform("mvp", Uniform::Mat4(self.cascades[num].mvp));

        // render into depth map
        self.cascades[num].fbo.bind();
        unsafe { gl::Clear(gl::DEPTH_BUFFER_BIT); }

        self.cascades[num].mvp
    }

    pub fn finish_render(&self) {
        // reset GL
        unsafe {
            gl::Disable(gl::POLYGON_OFFSET_FILL);
            gl::DepthFunc(gl::LEQUAL);
            gl::PolygonOffset(0.0, 0.0);
        }
        tinygl::OffscreenBuffer::unbind();
    }
}
