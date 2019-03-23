use cgmath::prelude::*;
use cgmath::*;
use tinygl::{Program, Texture, Uniform, OffscreenBuffer};
use super::guiutil;

static MAX_SHADOW_MAPS: usize = 6;
static MAX_REL_EXTENT: f32 = 1.2;

fn glsl() -> String {
    String::from("
    struct ShadowMap {
        highp sampler2D map;
        float depth;
        mat4 mvp;
    };

    #define MAX_SHADOW_MAPS ") + &MAX_SHADOW_MAPS.to_string() + "
    uniform ShadowMap shadowMapsPrevCurr[2 * MAX_SHADOW_MAPS];
    uniform int shadowMapCount;
    uniform float shadowMapSize;
    uniform float shadowMapProgress;
    uniform float shadowBlurRadius;

    vec3 shadow_getDebugColor(float f) {
        f *= 3.0;
        if (f < 1.0) return mix(vec3(1.0, 0.0, 0.0), vec3(1.0, 1.0, 0.0), f);
        if (f < 2.0) return mix(vec3(1.0, 1.0, 0.0), vec3(0.0, 1.0, 0.0), f - 1.0);
        return mix(vec3(0.0, 1.0, 0.0), vec3(0.0, 0.0, 1.0), f - 2.0);
    }

    float shadow_compare(sampler2D depths, vec2 uv, vec2 compare) {
        float depth = texture(depths, uv).r;
        return smoothstep(compare.x, compare.y, depth);
    }

    /*
    TODO:
    we look at our filter region (may be small initially) and gather all the distances (sample / shadowmap.depth)
    if the distances are large, that means that we want to draw a smooth shadow = large radius
    if the distances are small, we want a sharp shadow = small radius
    after gathering all the distances, we can adjust the size of the kernel depending on that
    if the distances are very large, they will surely still be 4 texels away, so we can gradually increase
    the step-size

    the problem is that we definitely need a variable-sized filter kernel, as otherwise the areas between levels will look
    distorted, because the one filter size is double the other.

    and if we do want to do that properly, and the camera is very close to the ground, and there are a lot of shadow-texels
    covering the ground, then we need a filter kernel that iterates over a large swath of texel-space, so we need some
    mechanism that does this _consistently_, meaning that it approximates a stable kernel filter function, e.g. by only
    sampling every texel mod (4,4). this would give it stability w.r.t. movement
    */
    float shadow_blur(sampler2D depths, vec2 uv, vec2 compare, float radius) {
        vec2 texelSize = vec2(1.0 / shadowMapSize);
        vec2 f = fract(uv * shadowMapSize + 0.5);
        vec2 centroidUV = floor(uv * shadowMapSize + 0.5) / shadowMapSize;

        float total = 0.0;
        float sum = 0.0;
        int bound = int(ceil(radius)) - 1;

        for (int i = -bound; i < 2 + bound; ++i) {
            float dx = max(1.0 - abs(float(i) - f.x) / radius, 0.0);

            for (int j = -bound; j < 2 + bound; ++j) {
                float shadowSample = shadow_compare(depths, centroidUV + texelSize * vec2(float(i), float(j)), compare);
                float dy = max(1.0 - abs(float(j) - f.y) / radius, 0.0);
                total += dx * dy;
                sum += shadowSample * dx * dy;
            }
        }

        return sum / total;
    }

    float shadow_lerp(sampler2D depths, vec2 uv, vec2 compare) {
        vec2 texelSize = vec2(1.0 / shadowMapSize);
        vec2 f = fract(uv * shadowMapSize + 0.5);
        vec2 centroidUV = floor(uv * shadowMapSize + 0.5) / shadowMapSize;

        float lb = shadow_compare(depths, centroidUV + texelSize * vec2(0.0, 0.0), compare);
        float lt = shadow_compare(depths, centroidUV + texelSize * vec2(0.0, 1.0), compare);
        float rb = shadow_compare(depths, centroidUV + texelSize * vec2(1.0, 0.0), compare);
        float rt = shadow_compare(depths, centroidUV + texelSize * vec2(1.0, 1.0), compare);
        float a = mix(lb, lt, f.y);
        float b = mix(rb, rt, f.y);
        float c = mix(a, b, f.x);
        return c;
    }

    bool shadow_getShadowForLevel(int level, vec3 pos, float dist, float dotSunNormal, out float lit) {
        vec4 posInSunSpace = shadowMapsPrevCurr[level].mvp * vec4(pos, 1.0);
        posInSunSpace /= posInSunSpace.w;
        posInSunSpace = 0.5 * posInSunSpace + vec4(0.5);

        vec2 compare = vec2(posInSunSpace.z - 0.1 * dist / shadowMapsPrevCurr[level].depth, posInSunSpace.z);

        if (all(greaterThan(posInSunSpace.xy, vec2(0.05))) && all(lessThan(posInSunSpace.xy, vec2(0.95)))) {
            if (shadowBlurRadius <= 1.0)
                lit = shadow_lerp(shadowMapsPrevCurr[level].map, posInSunSpace.xy, compare);
            else
                lit = shadow_blur(shadowMapsPrevCurr[level].map, posInSunSpace.xy, compare, shadowBlurRadius);

            return true;
        } else {
            return false;
        }
    }

    float shadow_getShadowWithOffset(vec3 pos, float dist, float dotSunNormal, int start, out vec3 color) {
        float lit = 0.0;
        int i = shadowMapCount - 1;
        while (i >= 0) {
            if (shadow_getShadowForLevel(i + start, pos, dist, dotSunNormal, lit))
                break;
            --i;
        }
        color = shadow_getDebugColor(float(i) / float(shadowMapCount - 1));
        return lit;
    }

    float getShadow(vec3 pos, float dotSunNormal, float dist, out vec3 debugColor) {
        // if the surface is not facing the sun, it's shadow anyway.
        if (dotSunNormal < 0.0)
            return 0.0;

        vec3 shadowMapDebugPrev, shadowMapDebugCurr;

        float litPrev = shadow_getShadowWithOffset(pos, dist, dotSunNormal, 0, shadowMapDebugPrev);
        float litNext = shadow_getShadowWithOffset(pos, dist, dotSunNormal, MAX_SHADOW_MAPS, shadowMapDebugCurr);

        debugColor = mix(shadowMapDebugPrev, shadowMapDebugCurr, shadowMapProgress);
        float shadow = mix(litPrev, litNext, shadowMapProgress);

        float sunCutOff = 0.2;
        if (dotSunNormal < sunCutOff)
            shadow *= max(dotSunNormal / sunCutOff, 0.0);

        return shadow;
    }
    "
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
    fn new(size: u32, level: i32, extent: f32) -> Self {
        // Create FBO
        let mut fbo = OffscreenBuffer::new((size as _, size as _));
        fbo.add_depth_texture();
        // fbo.add("depth", gl::R32F, gl::RED, gl::FLOAT);
        {
            // let tex = fbo.texture_mut("depth").unwrap();
            let tex = fbo.depth_texture_mut().unwrap();
            tex.wrap(gl::TEXTURE_WRAP_S, gl::CLAMP_TO_EDGE);
            tex.wrap(gl::TEXTURE_WRAP_T, gl::CLAMP_TO_EDGE);
            // tex.filter(gl::TEXTURE_MIN_FILTER, gl::LINEAR as _);
            // tex.filter(gl::TEXTURE_MAG_FILTER, gl::LINEAR as _);
            unsafe { gl::TexParameteri(gl::TEXTURE_2D, gl::TEXTURE_COMPARE_MODE, gl::NONE as _) }
            tex.filter(gl::TEXTURE_MIN_FILTER, gl::NEAREST as _);
            tex.filter(gl::TEXTURE_MAG_FILTER, gl::NEAREST as _);
        }

        let mut ret = ShadowCascade {
            level,
            fbo,
            extent,
            granularity: 0.0,
            orthogonal_depth: 0.0,
            center: Vector3::new(0.0, 0.0, 0.0),
            projection: Matrix4::from_scale(1.0)
        };
        ret.set_extent(extent);
        ret
    }

    fn depth_texture(&self) -> &Texture {
        // self.fbo.texture("depth").unwrap()
        self.fbo.depth_texture().unwrap()
    }

    fn set_extent(&mut self, extent: f32) {
        self.extent = extent;
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
    fn new(size: u32, radius: f32, levels: i32) -> Self {
        let mut cascades = Vec::new();
        for i in 0..levels {
            cascades.push(ShadowCascade::new(size, i, radius * MAX_REL_EXTENT));
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
    size_step: u32,
    levels: i32,
    level_scale: f32,
    radius: f32,
    blur_radius: f32,
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

    pub fn new(radius: f32) -> Self {
        let levels = 6;
        let level_scale = 0.4;

        let size = 2u32.pow(8);

        let mut ret = ShadowMap {
            size_step: 8,
            radius,
            blur_radius: 1.0,
            levels,
            level_scale,
            program: Program::new_versioned("
                in vec4 posHeight;
                uniform float radius;
                uniform mat4 mvp;
                void main()
                {
                    vec3 pos = posHeight.xyz * (posHeight.w + radius);
                    gl_Position = mvp * vec4(pos, 1.0);
                }",
                "//out float depth;
                void main()
                {
                    //depth = gl_FragCoord.z;
                }",
                300
            ),
            prev: Some(SunPositionCascades::new(size, radius, levels)),
            curr: Some(SunPositionCascades::new(size, radius, levels)),
            next: Some(SunPositionCascades::new(size, radius, levels)),
            next_sun_direction: Vector3::new(0.0, 0.0, 1.0),
        };

        ret.scale_cascades();

        ret
    }

    fn scale_cascades(&mut self) {
        for lvl in 0..self.levels {
            let extent = self.radius * MAX_REL_EXTENT * self.level_scale.powi(lvl);
            self.prev.as_mut().unwrap().cascades[lvl as usize].set_extent(extent);
            self.curr.as_mut().unwrap().cascades[lvl as usize].set_extent(extent);
            self.next.as_mut().unwrap().cascades[lvl as usize].set_extent(extent);
        }
    }

    fn create_cascades(&mut self) {
        let size = 2u32.pow(self.size_step as _);
        self.prev = Some(SunPositionCascades::new(size, self.radius, self.levels));
        self.curr = Some(SunPositionCascades::new(size, self.radius, self.levels));
        self.next = Some(SunPositionCascades::new(size, self.radius, self.levels));
    }

    pub fn size_step(&self) -> u32 {
        self.size_step
    }

    pub fn set_size_step(&mut self, size: u32) {
        if self.size_step != size {
            self.size_step = size;
            let size = 2u32.pow(self.size_step as _);
            self.create_cascades();
            self.scale_cascades();
        }
    }

    pub fn set_radius(&mut self, radius: f32) {
        if self.radius != radius {
            self.radius = radius;
            self.scale_cascades();
        }
    }

    pub fn levels(&self) -> i32 {
        self.levels
    }

    pub fn set_levels(&mut self, levels: i32) {
        if self.levels != levels {
            self.levels = levels;
            self.create_cascades();
            self.scale_cascades();
        }
    }

    pub fn level_scale(&self) -> f32 {
        self.level_scale
    }

    pub fn set_level_scale(&mut self, level_scale: f32) {
        if self.level_scale != level_scale {
            self.level_scale = level_scale;
            self.scale_cascades();
        }
    }

    pub fn blur_radius(&self) -> f32 {
        self.blur_radius
    }

    pub fn set_blur_radius(&mut self, blur_radius: f32) {
        self.blur_radius = blur_radius;
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

    // TODO: make this dependent on the time passed. i.e. make sure to render 1 new texture every N ms, so
    // that the movement of the sun is stable
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
        let look_surface_center = if to_render.index == 0 {
            // for the first global cascade, always put the light vertically head on to the planet
            self.get_sun_cascades(to_render.tp).sun_direction * self.radius
        } else {
            // all further detailed cascades are oriented towards the view frustum
            let rel_idx = to_render.index as f32 / 6.0;
            let eye_height = eye.magnitude() - self.radius;
            let look_center = eye + look * eye_height * rel_idx;
            look_center.normalize() * self.radius
        };

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
            gl::Clear(gl::DEPTH_BUFFER_BIT | gl::COLOR_BUFFER_BIT);
            gl::Enable(gl::POLYGON_OFFSET_FILL);
            gl::Disable(gl::BLEND);
            gl::DepthFunc(gl::LESS);
            gl::PolygonOffset(1.0, 1.0);
        }

        self.get_sun_cascades(to_render.tp).filled += 1;

        (mvp, look_surface_center)
    }

    pub fn finish_render(&self) {
        self.program().disable_all_vertex_attribs();
        // reset GL
        unsafe {
            gl::Disable(gl::POLYGON_OFFSET_FILL);
            gl::DepthFunc(gl::LEQUAL);
            gl::PolygonOffset(0.0, 0.0);
        }
        tinygl::OffscreenBuffer::unbind();
    }

    fn bind_shadow_map(program: &Program, index: usize, texunit: u32, sun_rotation: &Matrix4<f32>, cascade: &ShadowCascade) {
        cascade.depth_texture().bind_at(texunit);
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
            Self::bind_shadow_map(program, cascade.0 + MAX_SHADOW_MAPS, texunit, &sun_rotation, cascade.1);
            texunit += 1;
        }

        let filled = self.next.as_ref().unwrap().filled;
        let progress = filled as f32 / self.levels as f32;

        let sun_direction = (1.0 - progress) * self.prev.as_ref().unwrap().sun_direction + progress * self.curr.as_ref().unwrap().sun_direction;
        program.uniform("sunDirection", Uniform::Vec3(sun_direction.normalize()));
        program.uniform("shadowMapCount", Uniform::Signed(self.levels));
        program.uniform("shadowMapSize", Uniform::Float(2u32.pow(self.size_step as _) as f32));
        program.uniform("shadowMapProgress", Uniform::Float(progress));
        program.uniform("shadowBlurRadius", Uniform::Float(self.blur_radius));
    }

    pub fn num_textures(&self) -> usize {
        self.prev.as_ref().map(|cascades| cascades.cascades.len()).unwrap_or(0) +
        self.curr.as_ref().map(|cascades| cascades.cascades.len()).unwrap_or(0)
    }

    pub fn options(&mut self, ui: &imgui::Ui) {
        let mapsz = guiutil::slider_exp2int(ui, "Shadow Map Size:", self.size_step as _, (8, 12));
        self.set_size_step(mapsz as _);

        ui.text("Shadow Map Levels:");
        let mut smlcount = self.levels();
        let mut smlscale = self.level_scale();
        ui.slider_int(imgui::im_str!("Count##shadowmaplevels"), &mut smlcount, 2, 6).build();
        ui.slider_float(imgui::im_str!("Scale##shadowmaplevels"), &mut smlscale, 0.2, 0.8).build();
        self.set_levels(smlcount);
        self.set_level_scale(smlscale);

        let smblur = guiutil::slider_float(ui, "Shadow Blur:", self.blur_radius(), (0.0, 5.0), 1.0);
        self.set_blur_radius(smblur);
    }
}
