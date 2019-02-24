use cgmath::prelude::*;
use cgmath::*;
use tinygl::{Program, Texture, Uniform, OffscreenBuffer};

fn glsl() -> String {
    String::from("
    struct ShadowMap {
        highp sampler2D map;
        float depth;
        mat4 mvp;
    };

    #define MAX_SHADOW_MAPS 8
    uniform ShadowMap shadowMapsPrevCurr[2 * MAX_SHADOW_MAPS];
    uniform int shadowMapCount;
    uniform float shadowMapProgress;

    vec3 shadow_getDebugColor(float f) {
        f *= 3.0;
        if (f < 1.0) return mix(vec3(1.0, 0.0, 0.0), vec3(1.0, 1.0, 0.0), f);
        if (f < 2.0) return mix(vec3(1.0, 1.0, 0.0), vec3(0.0, 1.0, 0.0), f - 1.0);
        return mix(vec3(0.0, 1.0, 0.0), vec3(0.0, 0.0, 1.0), f - 2.0);
    }

    bool shadow_getShadowForLevel(int level, vec3 pos, out float lit) {
        vec4 posInSunSpace = shadowMapsPrevCurr[level].mvp * vec4(pos, 1.0);
        posInSunSpace /= posInSunSpace.w;
        posInSunSpace = 0.5 * posInSunSpace + vec4(0.5);

        if (all(greaterThan(posInSunSpace.xy, vec2(0.0))) && all(lessThan(posInSunSpace.xy, vec2(1.0)))) {
            float shadowMapSample = texture(shadowMapsPrevCurr[level].map, posInSunSpace.xy).x;
            lit = smoothstep(-1.0, 0.0, (shadowMapSample - posInSunSpace.z) * shadowMapsPrevCurr[level].depth);
            return true;
        } else {
            return false;
        }
    }

    float shadow_getShadowWithOffset(vec3 pos, int start, out vec3 color) {
        float lit = 0.0;
        int i = shadowMapCount - 1;
        while (i >= 0) {
            if (shadow_getShadowForLevel(i + start, pos, lit))
                break;
            --i;
        }
        color = shadow_getDebugColor(float(i) / float(shadowMapCount - 1));
        return lit;
    }

    float getShadow(vec3 pos, out vec3 debugColor) {
        vec3 shadowMapDebugPrev, shadowMapDebugCurr;

        float litPrev = shadow_getShadowWithOffset(pos, 0, shadowMapDebugPrev);
        float litNext = shadow_getShadowWithOffset(pos, MAX_SHADOW_MAPS, shadowMapDebugCurr);

        // interpolate..
        float lit = mix(litPrev, litNext, shadowMapProgress);
        float shadow = mix(0.7, 1.0, lit);
        vec3 shadowMapDebug = mix(shadowMapDebugPrev, shadowMapDebugCurr, shadowMapProgress);

        return shadow;
    }
    ")
}

struct ShadowCascade {
    // constant:
    level: i32,
    fbo: OffscreenBuffer,
    extent: f32,
    granularity: f32,
    orthogonal_depth: f32,

    // may change every time it's rendered:
    center: Vector3<f32>,
    projection: Matrix4<f32>,
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

        let mut ret = ShadowCascade {
            level,
            fbo,
            extent: 0.0,
            granularity: 0.0,
            orthogonal_depth: 0.0,
            center: Vector3::new(0.0, 0.0, 0.0),
            projection: Matrix4::from_scale(1.0)
        };
        ret.set_radius(max_radius);
        ret
    }

    fn set_radius(&mut self, max_radius: f32) {
        self.extent = max_radius * 0.6f32.powi(self.level as i32);
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
        self.projection = Matrix4::from_nonuniform_scale(sz_scale, sz_scale, -depth_scale) * translate
    }
}

struct SunPositionCascades {
    sun_direction: Vector3<f32>,
    sun_rotation: Matrix4<f32>,
    cascades: Vec<ShadowCascade>,
    filled: usize,
}

impl SunPositionCascades {
    fn new(size: u32, radius: f32) -> Self {
        let mut cascades = Vec::new();
        for i in 0..6 {
            cascades.push(ShadowCascade::new(size, i, radius * 1.1));
        }

        SunPositionCascades {
            cascades,
            sun_direction: Vector3::new(0.0, 0.0, 1.0),
            sun_rotation: Matrix4::from_scale(1.0),
            filled: 0,
        }
    }

    fn reset(&mut self, direction: Vector3<f32>) {
        self.sun_direction = direction;

        // Create Sun rotation matrix
        let sun_lon = direction.x.atan2(direction.z);
        let sun_lat = direction.y.asin();
        self.sun_rotation = Matrix4::from_angle_x(Rad(sun_lat)) * Matrix4::from_angle_y(Rad(-sun_lon));

        self.filled = 0;
    }

    fn is_complete(&self) -> bool {
        self.filled == self.cascades.len()
    }
}

#[derive(Clone, Copy)]
enum CascadeType {
    Prev,
    Curr,
    Next
}

#[derive(Clone, Copy)]
pub struct CascadeInfo {
    index: usize,
    tp: CascadeType,
}

pub struct ShadowMap {
    radius: f32,
    program: Program,

    prev: Option<SunPositionCascades>,
    curr: Option<SunPositionCascades>,
    next: Option<SunPositionCascades>,
    next_sun_direction: Vector3<f32>,
}

impl ShadowMap {
    pub fn glsl() -> String {
        glsl()
    }

    pub fn new(size: u32, radius: f32) -> Self {
        ShadowMap {
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
            ),
            prev: Some(SunPositionCascades::new(size, radius)),
            curr: Some(SunPositionCascades::new(size, radius)),
            next: Some(SunPositionCascades::new(size, radius)),
            next_sun_direction: Vector3::new(0.0, 0.0, 1.0),
        }
    }

    pub fn set_radius(&mut self, radius: f32) {
        if self.radius != radius {
            self.radius = radius;
            for mut cascade in &mut self.prev.as_mut().unwrap().cascades { cascade.set_radius(radius); }
            for mut cascade in &mut self.curr.as_mut().unwrap().cascades { cascade.set_radius(radius); }
            for mut cascade in &mut self.next.as_mut().unwrap().cascades { cascade.set_radius(radius); }
        }
    }

    pub fn push_sun_direction(&mut self, direction: Vector3<f32>) {
        self.next_sun_direction = direction;
    }

    pub fn program(&self) -> &Program {
        &self.program
    }

    fn get_sun_cascades(&mut self, which: CascadeType) -> &mut SunPositionCascades {
        match which {
            CascadeType::Prev => self.prev.as_mut().unwrap(),
            CascadeType::Curr => self.curr.as_mut().unwrap(),
            CascadeType::Next => self.next.as_mut().unwrap(),
            _ => self.next.as_mut().unwrap(),
        }
    }

    fn get_cascade(&mut self, which: CascadeInfo) -> &mut ShadowCascade {
        &mut self.get_sun_cascades(which.tp).cascades[which.index]
    }

    pub fn prepare_render(
        &mut self,
        eye: Vector3<f32>,
        look: Vector3<f32>,
    ) -> (Matrix4<f32>, Vector3<f32>)
    {
        //
        // if any of the cascades isn't filled yet, do that first
        //
        let to_render = if !self.prev.as_mut().unwrap().is_complete() {
            CascadeInfo { index: self.prev.as_mut().unwrap().filled, tp: CascadeType::Prev }
        }
        else if !self.curr.as_mut().unwrap().is_complete() {
            CascadeInfo { index: self.curr.as_mut().unwrap().filled, tp: CascadeType::Curr }
        }
        else if !self.next.as_mut().unwrap().is_complete() {
            CascadeInfo { index: self.next.as_mut().unwrap().filled, tp: CascadeType::Next }
        }
        //
        // if all cascades, inculding the last ones, have been filled, we can flip!
        //
        else {
            let prev = self.prev.take();
            let curr = self.curr.take();
            let next = self.next.take();

            self.prev = curr;
            self.curr = next;

            self.next = prev;
            self.next.as_mut().unwrap().reset(self.next_sun_direction);

            CascadeInfo { index: 0, tp: CascadeType::Next }
        };

        //
        // select central point to render: cheap for now -: center on camera eye
        //
        let rel_idx = to_render.index as f32 / 6.0;
        let eye_height = eye.magnitude() - self.radius;
        let look_center = eye + look * eye_height * rel_idx;
        let look_surface_center = look_center.normalize() * self.radius;

        let sun_rotation = self.get_sun_cascades(to_render.tp).sun_rotation;
        let sunspace_center = sun_rotation.transform_vector(look_surface_center);

        let projection = {
            // setup FBO
            let cascade = self.get_cascade(to_render);
            cascade.set_center(sunspace_center);
            cascade.fbo.bind();
            cascade.projection
        };

        let mvp = projection * sun_rotation;

        // setup shader
        self.program.bind();
        self.program.uniform("mvp", Uniform::Mat4(mvp));

        // setup GL
        unsafe {
            gl::Clear(gl::DEPTH_BUFFER_BIT);
            gl::Enable(gl::POLYGON_OFFSET_FILL);
            gl::DepthFunc(gl::LESS);
            gl::PolygonOffset(1.0, 1.0);
        }

        self.get_sun_cascades(to_render.tp).filled += 1;

        (mvp, look_surface_center)
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

    fn bind_shadow_map(program: &Program, index: usize, texunit: u32, sun_rotation: &Matrix4<f32>, cascade: &ShadowCascade) {
        cascade.fbo.depth_texture().unwrap().bind_at(texunit);
        program.uniform(&format!("shadowMapsPrevCurr[{}].map", index),   Uniform::Signed(texunit as i32));
        program.uniform(&format!("shadowMapsPrevCurr[{}].depth", index), Uniform::Float(cascade.orthogonal_depth));
        program.uniform(&format!("shadowMapsPrevCurr[{}].mvp", index),   Uniform::Mat4(cascade.projection * *sun_rotation));
    }

    pub fn prepare_postprocess(&self, program: &Program, texunit_start: u32) {
        let mut texunit = texunit_start;

        let sun_rotation = self.prev.as_ref().unwrap().sun_rotation;
        for cascade in self.prev.as_ref().unwrap().cascades.iter().enumerate() {
            Self::bind_shadow_map(program, cascade.0, texunit, &sun_rotation, cascade.1);
            texunit += 1;
        }

        let sun_rotation = self.curr.as_ref().unwrap().sun_rotation;
        for cascade in self.curr.as_ref().unwrap().cascades.iter().enumerate() {
            Self::bind_shadow_map(program, cascade.0 + 8, texunit, &sun_rotation, cascade.1);
            texunit += 1;
        }

        let count = self.prev.as_ref().unwrap().cascades.len();
        let filled = self.next.as_ref().unwrap().filled;
        let progress = filled as f32 / count as f32;

        let sun_direction = (1.0 - progress) * self.prev.as_ref().unwrap().sun_direction + progress * self.curr.as_ref().unwrap().sun_direction;
        program.uniform("sunDirection", Uniform::Vec3(sun_direction.normalize()));
        program.uniform("shadowMapCount", Uniform::Signed(count as i32));
        program.uniform("shadowMapProgress", Uniform::Float(progress));
    }
}
