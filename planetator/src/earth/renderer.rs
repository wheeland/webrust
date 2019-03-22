use std::collections::HashMap;
use cgmath::prelude::*;
use cgmath::*;
use super::super::culling;

use super::Channels;
use super::flycamera::FlyCamera;
use super::tree;
use super::noise;

pub struct Renderer {
    camera: FlyCamera,
    program: Option<tinygl::Program>,
    default_program: tinygl::Program,

    planet: Option<tree::Planet>,
    planet_depth: i32,
    planet_radius: f32,

    pub wireframe: bool,
    pub reduce_poly_count: bool,
    pub no_update_plates: bool,
    pub hide_backside: bool,
    vertex_detail: f32,

    rendered_triangles: usize,
    rendered_plates: usize,

    // those two have to compile, otherwise there can be no world
    generator: String,
    channels: Channels,

    // the colorator may fail to compile (this may happen after removing a channel or texture), but we'll just fall-back
    colorator: String,
    textures: Vec<(String, tinygl::Texture)>,

    fbo: Option<tinygl::OffscreenBuffer>,

    // errors to be picked up by y'all
    errors_generator: Option<String>,
    errors_channels: Option<String>,
    errors_colorator: Option<String>,
}

fn create_render_program(colorator: &str, channels: &Channels, textures: &Vec<(String, tinygl::Texture)>) -> tinygl::Program {
    let vert_source = "
            uniform mat4 mvp;
            uniform float radius;
            uniform float wf;
            uniform float Far;
            in vec4 posHeight;
            in vec2 plateCoords;
            out vec2 tc;
            out vec3 pos;

            void main()
            {
                float C = 1.0;

                tc = plateCoords;
                pos = posHeight.xyz * (posHeight.w + radius + 0.0001 * wf);
                vec4 wpos = mvp * vec4(pos, 1.0);
                // wpos.z = (2.0 * log(C * wpos.w + 1.0) / log(C * Far + 1.0) - 1.0) * wpos.w;
                gl_Position = wpos;
            }";

    let chan_declarations = channels.channels().iter()
        .fold(String::new(), |acc, chan| {
            let glsltype = Channels::to_glsl_type(*chan.1);
            acc + &glsltype + " " + chan.0 + ";\nuniform sampler2D texture_" + chan.0 + ";\n"
        });

    let chan_assignments = channels.channels().iter()
        .fold(String::new(), |acc, x| {
            let swizzler = String::from(match x.1 {
                1 => ".r",
                2 => ".rg",
                3 => ".rgb",
                4 => ".rgba",
                _ => panic!("Does not compute")
            });
            acc + x.0 + " = texture(texture_" + x.0 + ", tc)" + &swizzler + ";\n"
        });

    let tex_declarations = textures
        .iter()
        .fold(String::new(), |acc, tex| {
            acc + "uniform sampler2D " + &tex.0 + ";\n"
        });

    let frag_source = String::from("
            uniform float wf;
            uniform float radius;
            uniform sampler2D normals;
            uniform sampler2D heights;
            uniform vec3 debugColor;
            in vec2 tc;
            in vec3 pos;
            layout(location = 0) out vec4 outColor;
            layout(location = 1) out vec4 outNormal;
            layout(location = 2) out vec4 outPosition;

            ") + &super::noise::ShaderNoise::declarations() + "\n"
            + &chan_declarations
            + &tex_declarations + "
            \n#line 1\n" + colorator + "

            " + &super::noise::ShaderNoise::definitions() + "

            void main()
            {
                vec3 norm = texture(normals, tc).xyz;
                float height = texture(heights, tc).r;
            " + &chan_assignments + "
                vec3 col = color(norm, pos);

                float wfVal = 1.0 - step(0.8, (0.2126*col.r + 0.7152*col.g + 0.0722*col.b));
                outColor = vec4(mix(col, vec3(wfVal), wf), 1.0 - 0.7* wf);
                // outColor = mix(outColor, vec4(debugColor, 1.0), 0.5);
                outNormal = vec4(vec3(0.5) + 0.5 * norm, 1.0);
                outPosition = vec4(normalize(pos) * (radius + height), 1.0);
            }";

    tinygl::Program::new(vert_source, &frag_source)
}

pub fn default_generator() -> String {
    String::from("void generate(vec3 position, int depth)
{
    float mountain = smoothstep(-0.5, 1.0, noise(position * 0.2));
    float base = noise(position * 0.1, 4, 0.5);
    float detail = noise(position, 6, 0.5);
    height = 1.4 * base + mountain * (0.5 + 0.5 * detail);
    height *= 1.0 - smoothstep(0.8, 0.9, abs(normalize(position).y));
    height = 0.0;
}")
}

pub fn default_colorator() -> String {
String::from("const lowp vec3 icoVerts[12] = vec3[](
    vec3(-0.59807, -0.25358,  0.76027),
    vec3(-0.52296,  0.73967,  0.42355),
    vec3( 0.52296, -0.73967, -0.42355),
    vec3( 0.59807,  0.25358, -0.76027),
    vec3( 0.98339, -0.01299,  0.18103),
    vec3( 0.29056,  0.28743,  0.91267),
    vec3(-0.29056, -0.28743, -0.91267),
    vec3(-0.98339,  0.01299, -0.18103),
    vec3(-0.33291,  0.71875, -0.61038),
    vec3( 0.45443,  0.88837,  0.06556),
    vec3(-0.45443, -0.88837, -0.06556),
    vec3( 0.33291, -0.71875,  0.61038)
);

const lowp vec3 icoMats1[20] = vec3[](
    vec3(-1.09276,  0.51477,  0.59140),
    vec3(-0.72726, -0.59140,  0.88028),
    vec3(-0.18322, -0.88028, -0.04735),
    vec3(-0.21249,  0.04735, -0.90954),
    vec3(-0.77461,  0.90954, -0.51477),
    vec3( 0.72726,  0.43081, -1.04087),
    vec3( 1.09276, -0.08397, -0.16059),
    vec3( 0.77461, -0.99351,  0.43081),
    vec3( 0.21249, -1.04087, -0.08397),
    vec3( 0.18322, -0.16059, -0.99351),
    vec3( 0.21249,  0.90954, -0.04735),
    vec3( 0.18322,  0.04735,  0.88028),
    vec3( 0.72726, -0.88028,  0.59140),
    vec3( 1.09276, -0.59140, -0.51477),
    vec3( 0.77461,  0.51477, -0.90954),
    vec3(-0.21249,  0.08397,  1.04087),
    vec3(-0.18322,  0.99351,  0.16059),
    vec3(-0.72726,  1.04087, -0.43081),
    vec3(-1.09276,  0.16059,  0.08397),
    vec3(-0.77461, -0.43081,  0.99351)
);

const lowp vec3 icoMats2[20] = vec3[](
    vec3(-0.16625,  0.81245, -1.00775),
    vec3(-0.78907,  1.00775,  0.18963),
    vec3(-0.67187, -0.18963,  1.12494),
    vec3( 0.02339, -1.12494,  0.50562),
    vec3( 0.33588, -0.50562, -0.81245),
    vec3( 0.78907, -0.29804,  0.52008),
    vec3( 0.16625, -1.11049,  0.70971),
    vec3(-0.33588, -0.60487, -0.29804),
    vec3(-0.02339,  0.52008, -1.11049),
    vec3( 0.67187,  0.70971, -0.60487),
    vec3(-0.02339, -0.50562,  1.12494),
    vec3( 0.67187, -1.12494,  0.18963),
    vec3( 0.78907, -0.18963, -1.00775),
    vec3( 0.16625,  1.00775, -0.81245),
    vec3(-0.33588,  0.81245,  0.50562),
    vec3( 0.02339,  1.11049, -0.52008),
    vec3(-0.67187,  0.60487, -0.70971),
    vec3(-0.78907, -0.52008,  0.29804),
    vec3(-0.16625, -0.70971,  1.11049),
    vec3( 0.33588,  0.29804,  0.60487)
);

const lowp vec3 icoMats3[20] = vec3[](
    vec3( 0.40025,  0.67594,  0.12909),
    vec3( 0.48003, -0.12909,  0.75572),
    vec3( 0.94709, -0.75572,  0.33797),
    vec3( 1.15597, -0.33797, -0.54685),
    vec3( 0.81800,  0.54685, -0.67594),
    vec3(-0.48003,  1.05240,  0.16758),
    vec3(-0.40025,  0.37646,  0.92331),
    vec3(-0.81800, -0.17039,  1.05240),
    vec3(-1.15597,  0.16758,  0.37646),
    vec3(-0.94709,  0.92331, -0.17039),
    vec3(-1.15597,  0.54685,  0.33797),
    vec3(-0.94709, -0.33797,  0.75572),
    vec3(-0.48003, -0.75572,  0.12909),
    vec3(-0.40025, -0.12909, -0.67594),
    vec3(-0.81800,  0.67594, -0.54685),
    vec3( 1.15597, -0.37646, -0.16758),
    vec3( 0.94709,  0.17039, -0.92331),
    vec3( 0.48003, -0.16758, -1.05240),
    vec3( 0.40025, -0.92331, -0.37646),
    vec3( 0.81800, -1.05240,  0.17039)
);

const highp vec3 icoNorms[20] = vec3[](
    vec3( 0.01065, -0.28730,  0.95778),
    vec3(-0.34836,  0.32447,  0.87941),
    vec3(-0.88274,  0.20935,  0.42064),
    vec3(-0.85400, -0.47356,  0.21547),
    vec3(-0.30185, -0.78051,  0.54744),
    vec3( 0.09313,  0.80348,  0.58800),
    vec3( 0.67403, -0.18638,  0.71481),
    vec3( 0.16839, -0.98441,  0.05087),
    vec3(-0.72500, -0.48776, -0.48627),
    vec3(-0.77152,  0.61721, -0.15430),
    vec3( 0.85400,  0.47356, -0.21547),
    vec3( 0.88274, -0.20935, -0.42064),
    vec3( 0.34836, -0.32447, -0.87941),
    vec3(-0.01065,  0.28730, -0.95778),
    vec3( 0.30185,  0.78051, -0.54744),
    vec3( 0.72500,  0.48776,  0.48627),
    vec3( 0.77152, -0.61721,  0.15430),
    vec3(-0.09313, -0.80348, -0.58800),
    vec3(-0.67403,  0.18638, -0.71481),
    vec3(-0.16839,  0.98441, -0.05087)
);

vec3 normUnit(vec3 v) {
    vec3 L = abs(v);
    return v / (L.x + L.y + L.z);
}

float maxElem(vec3 v) {
    return max(v.x, max(v.y, v.z));
}

const float DROPOFF = 0.2;

// output:
vec3 tc1;
vec3 tc2;
vec3 tc3;

vec3 lultex(vec2 uv) {
    uv *= 1.0;
    float r = fract(0.5 * (floor(uv.x) + floor(uv.y)));
    float bw = step(r, 0.4);
    return vec3(fract(uv), 0.5) * bw;
}

vec2 projectIntoUvSpace(vec3 position, vec3 normal) {
    float x1 = normal.x;
    float y1 = normal.y;
    float z1 = normal.z;
    float x2 = y1 + z1;
    float y2 = x1 + z1;
    float z2 = (-x1 * x2 - y1 * y2) / z1;

    vec3 dir1 = normalize(vec3(x2, y2, z2));
    vec3 dir2 = cross(dir1, normal);

    // project onto plane
    vec3 onPlane = position - normal * dot(position, normal);

    float u = dot(onPlane, dir1);
    float v = dot(onPlane, dir2);
    return vec2(u, v);
}

vec3 color(vec3 normal, vec3 position)
{
    vec3 n = normalize(normal);

    float d1 = 0.0;
    float d2 = 0.0;
    float dp = 0.0;
    int i1 = 0;
    int i2 = 0;
    int ip = 0;

    // find highest and second-highest scoring hexagon
    for (int i = 0; i < 20; ++i) {
        float d = dot(n, icoNorms[i]);
        if (d > d1) {
            d2 = d1;
            i2 = i1;
            d1 = d;
            i1 = i;
        } else if (d > d2) {
            d2 = d;
            i2 = i;
        }
    }

    // find highest-scoring pentagon
    for (int i = 0; i < 12; ++i) {
        float d = dot(n, icoVerts[i]);
        if (d > dp) {
            dp = d;
            ip = i;
        }
    }

    // normals of this triangle/hexagon, neighbor triangle/hexagon, and pentagon
    vec3 thisHexNorm = icoNorms[i1];
    vec3 neighborHexNorm = icoNorms[i2];
    vec3 pentNorm = icoVerts[ip];

    // barycentric coordinates of N in this and the neighbor hexagon/triangle UVW space
    vec3 thisUvw = normUnit(mat3(icoMats1[i1], icoMats2[i1], icoMats3[i1]) * n);
    vec3 neighborUvw = normUnit(mat3(icoMats1[i2], icoMats2[i2], icoMats3[i2]) * n);

    // relative distance to the neighbor hexagon border, 0 is the border, 1 is this hexagon's center
    float neighborDist = 1.4142 * 3.0 * maxElem(-neighborUvw);

    // UV spaces for the three adjacent surfaces
    vec2 thisUv = projectIntoUvSpace(position, thisHexNorm);
    vec2 neighborUv = projectIntoUvSpace(position, neighborHexNorm);
    vec2 pentUv = projectIntoUvSpace(position, pentNorm);

    if (all(lessThan(thisUvw, vec3(2.0 / 3.0)))) {
        // relative distance to the pentagon border, 0 is in the border, 1 is this hexagon's center
        float pentDist = 2.0 - 3.0 * maxElem(thisUvw);

        if (neighborDist > DROPOFF && pentDist > DROPOFF) {
            tc1 = vec3(thisUv, 1.0);
            tc2 = vec3(0.0);
            tc3 = vec3(0.0);
        }
        else {
            float fNeighbor = pow(1.0 - min(neighborDist / DROPOFF, 1.0), 2.0);
            float fPentagon = pow(1.0 - min(pentDist / DROPOFF, 1.0), 2.0);
            float sum = 1.0 + fNeighbor + fPentagon;

            tc1 = vec3(thisUv, 1.0 / sum);
            tc2 = vec3(neighborUv, fNeighbor / sum);
            tc3 = vec3(pentUv, fPentagon / sum);
        }
    }
    else {
        // relative distance to the pentagon border, 0 is in the border, 1 is this hexagon's center
        float mainDist = 3.0 * maxElem(thisUvw) - 2.0;

        if (mainDist > DROPOFF) {
            tc1 = vec3(pentUv, 1.0);
            tc2 = vec3(0.0);
            tc3 = vec3(0.0);
        }
        else {
            float fNeighbor = pow(1.0 - min(neighborDist / DROPOFF, 1.0), 2.0);
            float fMain = pow(1.0 - min(mainDist / DROPOFF, 1.0), 2.0);
            float sum = 1.0 + fMain * (1.0 + fNeighbor);

            tc1 = vec3(pentUv, 1.0 / sum);
            tc2 = vec3(thisUv, fMain / sum);
            tc3 = vec3(neighborUv, fMain * fNeighbor  / sum);
        }
    }

    vec3 ret = vec3(0.0);
    ret += lultex(tc1.xy) * tc1.z;
    ret += lultex(tc2.xy) * tc2.z;
    ret += lultex(tc3.xy) * tc3.z;
    return ret;
}")
}

impl Renderer {
    fn create_planet(&mut self, generator: &str, channels: &Channels, update_errors: bool) -> bool {
        let conf = super::Configuration {
            size: self.planet_depth as _,
            radius: self.planet_radius,
            detail: (255.0 * self.vertex_detail) as _,
            generator: generator.to_string(),
            channels: channels.clone(),
        };

        let planet = tree::Planet::new(&conf);

        match planet {
            Ok(mut planet) => {
                // start data generation for the first levels
                let culler = culling::Culler::new(&self.camera.mvp((2, 1)));
                planet.update_quad_tree(&self.camera.eye(), &culler, 3, self.hide_backside);
                planet.start_data_generation(30);

                // Clear errors
                self.planet = Some(planet);
                self.generator = generator.to_string();
                self.channels = channels.clone();
                if update_errors {
                    self.errors_generator = None;
                    self.errors_channels = None;
                }

                true
            },
            Err(errs) => {
                if update_errors {
                    self.errors_generator = errs.0;
                    self.errors_channels = errs.1;
                }
                false
            },
        }
    }

    pub fn new() -> Self {
        let colorator = default_colorator();
        let channels = Channels::new(&Vec::new());

        let mut ret = Renderer {
            camera: FlyCamera::new(100.0),
            program: None,
            default_program: create_render_program(&colorator, &channels, &Vec::new()),

            planet: None,
            planet_depth: 6,
            planet_radius: 100.0,

            wireframe: false,
            reduce_poly_count: true,
            no_update_plates: false,
            hide_backside: false,
            vertex_detail: 0.5,

            rendered_plates: 0,
            rendered_triangles: 0,

            fbo: None,

            generator: default_generator(),
            channels,
            colorator,
            textures: Vec::new(),

            errors_generator: None,
            errors_channels: None,
            errors_colorator: None,
        };

        ret.create_planet(&default_generator(), &Channels::new(&Vec::new()), false);
        ret
    }

    pub fn fbo(&self) -> Option<&tinygl::OffscreenBuffer> {
        self.fbo.as_ref()
    }

    pub fn camera(&mut self) -> &mut FlyCamera {
        &mut self.camera
    }

    pub fn errors_generator(&self) -> Option<&String> {
        self.errors_generator.as_ref()
    }

    pub fn errors_colorator(&self) -> Option<&String> {
        self.errors_colorator.as_ref()
    }

    pub fn errors_channels(&self) -> Option<&String> {
        self.errors_channels.as_ref()
    }

    pub fn rendered_triangles(&self) -> usize {
        self.rendered_triangles
    }

    pub fn rendered_plates(&self) -> usize {
        self.rendered_plates
    }

    pub fn vertex_detail(&self) -> f32 {
        self.vertex_detail
    }

    pub fn set_vertex_detail(&mut self, vertex_detail: f32) {
        if self.vertex_detail != vertex_detail {
            self.vertex_detail = vertex_detail;
            self.planet.as_mut().unwrap().set_detail((255.0 * self.vertex_detail) as _);
        }
    }

    pub fn get_surface_height(&self, position: &Vector3<f32>) -> f32 {
        self.planet.as_ref().unwrap().get_surface_height(position)
    }

    pub fn depth(&self) -> i32 {
        self.planet_depth
    }

    pub fn set_depth(&mut self, depth: i32) {
        self.planet_depth = depth;
        let gen = self.generator.clone();
        let chan = self.channels.clone();
        self.create_planet(&gen, &chan, false);
    }

    pub fn radius(&self) -> f32 {
        self.planet_radius
    }

    pub fn set_radius(&mut self, radius: f32) {
        self.planet_radius = radius;
        let gen = self.generator.clone();
        let chan = self.channels.clone();
        self.create_planet(&gen, &chan, false);
        self.camera.scale_with_planet(radius);
    }

    pub fn set_colorator(&mut self, colorator: &str) -> bool {
        let new_program = create_render_program(colorator, &self.channels, &self.textures);
        let ret = new_program.valid();

        if ret {
            self.colorator = colorator.to_string();
            self.errors_colorator = None;
            self.program = Some(new_program);
        } else {
            self.errors_colorator = Some(new_program.fragment_log());
        }
        ret
    }

    pub fn set_generator_and_channels(&mut self, generator: &str, channels: &Channels) -> bool {
        let ret = self.create_planet(generator, channels, true);
        if ret {
            self.generator = generator.to_string();
            self.channels = channels.clone();
        }
        ret
    }

    pub fn set_generator(&mut self, generator: &str) -> bool {
        let chan = self.channels.clone();
        let ret = self.create_planet(generator, &chan, true);
        if ret {
            self.generator = generator.to_string();
        }
        ret
    }

    fn recreate_proram(&mut self) {
        // need to check if our colorator still fits with all those new channels and textures coming in
        let new_program = create_render_program(&self.colorator, &self.channels, &self.textures);

        if new_program.valid() {
            self.errors_colorator = None;
            self.program = Some(new_program);
        } else {
            self.errors_colorator = Some(new_program.fragment_log());
            self.program = None;
        }
    }

    pub fn set_channels(&mut self, channels: &Channels, generator: &str) -> bool {
        let ret = self.create_planet(generator, channels, true);

        if ret {
            self.generator = generator.to_string();
            self.channels = channels.clone();
            self.recreate_proram();
        }
        ret
    }

    pub fn clear_textures(&mut self) {
        self.textures.clear();
        self.recreate_proram();
    }

    pub fn add_texture(&mut self, name: &str, texture: tinygl::Texture) {
        self.textures.push((name.to_string(), texture));
        self.recreate_proram();
    }

    pub fn rename_texture(&mut self, index: usize, new_name: &str) {
        if index < self.textures.len() {
            self.textures[index].0 = new_name.to_string();
            self.recreate_proram();
        }
    }

    pub fn remove_texture(&mut self, index: usize) {
        if index < self.textures.len() {
            self.textures.remove(index);
            self.recreate_proram();
        }
    }

    pub fn textures(&self) -> &Vec<(String, tinygl::Texture)> {
        &self.textures
    }

    pub fn render(&mut self, windowsize: (u32, u32)){
        //
        // Setup view/projection matrices
        //
        let mvp = self.camera.mvp(windowsize);
        let culler = culling::Culler::new(&mvp);

        //
        // Update Quad-Trees
        //
        let planet = self.planet.as_mut().unwrap();
        planet.collect_render_data();
        if !self.no_update_plates {
            planet.update_quad_tree(&self.camera.eye(), &culler, 14, self.hide_backside);
        }
        planet.update_priorities();
        planet.start_data_generation(3);

        //
        // Update and bind Render Target
        //
        if self.fbo.as_ref().map(|fbo| fbo.size() != windowsize).unwrap_or(true) {
            let mut fbo = tinygl::OffscreenBuffer::new((windowsize.0 as _, windowsize.1 as _));
            fbo.add("colorWf", gl::RGBA8, gl::RGBA, gl::UNSIGNED_BYTE);
            fbo.add("normal", gl::RGBA8, gl::RGBA, gl::UNSIGNED_BYTE);
            fbo.add("position", gl::RGBA32F, gl::RGBA, gl::FLOAT);
            fbo.add_depth_renderbuffer();
            self.fbo = Some(fbo);
        }
        self.fbo.as_ref().unwrap().bind();

        unsafe {
            gl::ClearColor(0.0, 0.0, 0.0, 0.0);
            gl::Clear(gl::COLOR_BUFFER_BIT | gl::DEPTH_BUFFER_BIT);
            gl::Enable(gl::CULL_FACE);
            gl::Enable(gl::DEPTH_TEST);
            gl::Enable(gl::BLEND);
            gl::DepthFunc(gl::LEQUAL);
        }

        //
        // Render sphere
        //
        let program = match self.program.as_mut() {
            Some(prog) => prog,
            None => &mut self.default_program
        };
        program.bind();
        program.uniform("mvp", tinygl::Uniform::Mat4(mvp));
        program.uniform("Far", tinygl::Uniform::Float(self.camera.far()));
        program.uniform("eye", tinygl::Uniform::Vec3(self.camera.eye()));
        program.uniform("radius", tinygl::Uniform::Float(self.planet_radius));
        program.uniform("normals", tinygl::Uniform::Signed(self.textures.len() as _));
        program.uniform("heights", tinygl::Uniform::Signed((self.textures.len() + 1) as _));
        program.vertex_attrib_buffer("plateCoords", planet.plate_coords(), 2, gl::UNSIGNED_SHORT, true, 4, 0);

        // Bind Textures
        for tex in self.textures.iter().enumerate() {
            (tex.1).1.bind_at(tex.0 as _);
            program.uniform(&(tex.1).0, tinygl::Uniform::Signed(tex.0 as _));
        }

        self.rendered_triangles = 0;
        let rendered_plates = planet.rendered_plates();

        for plate in &rendered_plates {
            plate.borrow().bind_render_data(program, self.textures.len());

            // Render Triangles
            self.rendered_triangles += if self.reduce_poly_count {
                plate.borrow().indices().draw_all(gl::TRIANGLES)
            } else {
                planet.triangle_indices().draw_all(gl::TRIANGLES)
            } / 3;

            // Maybe Render Wireframe
            if self.wireframe {
                program.uniform("wf", tinygl::Uniform::Float(1.0));
                if self.reduce_poly_count {
                    plate.borrow().wireframe().draw_all(gl::LINES);
                } else {
                    planet.wireframe_indices().draw_all(gl::LINES);
                }
                program.uniform("wf", tinygl::Uniform::Float(0.0));
            }
        };

        program.disable_all_vertex_attribs();

        self.rendered_plates = rendered_plates.len();

        unsafe { gl::BindFramebuffer(gl::FRAMEBUFFER, 0) }
    }

    pub fn render_for(&self, program: &tinygl::Program, eye: Vector3<f32>, mvp: Matrix4<f32>) {
        let planet = self.planet.as_ref().unwrap();
        let plates = planet.rendered_plates_for_camera(eye, mvp, 1.0);

        program.uniform("radius", tinygl::Uniform::Float(self.planet_radius));

        for plate in &plates {
            plate.borrow().bind_pos_height_buffer(program);
            plate.borrow().indices().draw_all(gl::TRIANGLES);
        };
    }
}
