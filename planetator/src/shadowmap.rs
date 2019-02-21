use cgmath::prelude::*;
use cgmath::*;
use tinygl::{Program, Texture, Uniform, OffscreenBuffer};

pub struct ShadowMap {
    fbo: Option<OffscreenBuffer>,
    program: Program,

    mvp: Matrix4<f32>,
    eye: Vector3<f32>,
}

impl ShadowMap {
    pub fn new() -> Self {
        ShadowMap {
            fbo: None,
            mvp:  Matrix4::from_scale(1.0),
            eye: Vector3::new(0.0, 0.0, 0.0),
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
                100
            )
        }
    }

    fn create_fbo(size: (u32, u32)) -> OffscreenBuffer {
        let mut buf = OffscreenBuffer::new((size.0 as _, size.1 as _));
        buf.add_depth_texture();
        buf
    }

    pub fn texture(&self) -> Option<&Texture> {
        self.fbo.as_ref().and_then(|fbo| fbo.depth_texture())
    }

    pub fn program(&self) -> &Program {
        &self.program
    }

    pub fn mvp(&self) -> Matrix4<f32> {
        self.mvp
    }

    pub fn prepare(&mut self, size: (u32, u32), sun_direction: Vector3<f32>, center: Vector3<f32>, distance: f32) {
        // (Re-) create FBO
        self.fbo = Some(match self.fbo.take() {
            None => Self::create_fbo(size),
            Some(fbo) => {
                if fbo.size() == size {
                    fbo
                } else {
                    Self::create_fbo(size)
                }
            }
        });
        self.fbo.as_ref().unwrap().bind();

        unsafe {
            gl::Clear(gl::DEPTH_BUFFER_BIT);
            gl::Enable(gl::POLYGON_OFFSET_FILL);
            gl::DepthFunc(gl::LESS);
            gl::PolygonOffset(2.0, 2.0);
        }

        // Create MVP matrix
        let s = 1.0 / distance;
        self.mvp = Matrix4::from_nonuniform_scale(s, s, -s);

        // configure program
        self.program.bind();
        self.program.uniform("mvp", Uniform::Mat4(self.mvp));
    }

    pub fn finish() {
        tinygl::OffscreenBuffer::unbind();
        unsafe {
            gl::Disable(gl::POLYGON_OFFSET_FILL);
            gl::DepthFunc(gl::LEQUAL);
            gl::PolygonOffset(0.0, 0.0);
        }
    }
}
