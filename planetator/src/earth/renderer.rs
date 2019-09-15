use std::collections::HashMap;
use cgmath::prelude::*;
use cgmath::*;
use util3d::culling;

use super::channels::Channels;
use util3d::flycamera::FlyCamera;
use super::tree;
use super::water::WaterPlateFactory;
use util3d::noise;

/// Maintains a planetary 6-quad-tree and renders it into an FBO
///
/// The Renderer is told about
/// # changes ot the camera position
/// # the channels that should be additionally generated for each plate
/// # the GLSL generator function that generates elevation and channel data
/// # radius of the planet
/// # size of the generated plates (in power of two)
/// # the LOD detail threshold that controls whether quad-trees are refined or not
/// # a few flags to debug rendering
///
/// Based on this information the Renderer
/// # maintains the quad-tree according to camera position and LOD detail threshold
/// # makes sure that new render data is generated and stale data is thrown away
/// # maintains OpenGL textures (matching the screen-size) for position, normal and color, as
///   rendered from the supplied camera position, using the colorator GLSL function
/// # offers different methods of rendering the planet and accessing the results
///
pub struct Renderer {
    camera: FlyCamera,
    program_scene: Option<tinygl::Program>,
    program_water: tinygl::Program,
    program_color: Option<tinygl::Program>,
    program_color_default: tinygl::Program,

    planet: Option<tree::Planet>,
    plate_depth: u32,
    texture_delta: u32,
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

    fbo_scene: Option<tinygl::FrameBufferObject>,
    fbo_color: Option<tinygl::FrameBufferObject>,
    fsquad: tinygl::shapes::FullscreenQuad,
    water_plate_factory: WaterPlateFactory,
    water_depth: u32,
    water_height: f32,
    water_wavetime: f32,

    // errors to be picked up by y'all
    errors_generator: Option<String>,
    errors_colorator: Option<String>,
}

fn create_water_program() -> tinygl::Program {
    tinygl::Program::new_versioned("
        uniform mat4 mvp;
        uniform float radius;
        uniform float waterHeight;
        uniform float waterTime;
        uniform float farPlane;
        in vec4 sphereCoordsBorder;
        in vec2 texCoords;
        out vec3 pos;
        out vec2 tc;

        void main()
        {
            tc = texCoords;
            float ribbon = 0.01 * sphereCoordsBorder.w * 0.01;
            pos = sphereCoordsBorder.xyz * (radius + waterHeight - ribbon);

            vec4 cpos = mvp * vec4(pos, 1.0);
            // float C = 1.0;
            // cpos.z = (2.0 * log(C * wpos.w + 1.0) / log(C * farPlane + 1.0) - 1.0) * wpos.w;

            gl_Position = cpos;
        }",
        "
        layout(location = 0) out vec4 outNormalWf;
        layout(location = 1) out vec4 outPositionHeight;
        uniform float radius;
        uniform float waterHeight;
        uniform sampler2D heights;
        uniform sampler2D normals;
        in vec3 pos;
        in vec2 tc;
        void main()
        {
            float terrainHeight = texture(heights, tc).r;
            vec3 terrainNormal = texture(normals, tc).xyz;
            outNormalWf = vec4(vec3(0.5) + 0.5 * normalize(pos), 0.0);
            outPositionHeight = vec4(pos, terrainHeight);
        }
    ",
    300)
}

fn create_scene_program(channels: &Channels) -> tinygl::Program {
    tinygl::Program::new_versioned("
        uniform mat4 mvp;
        uniform float radius;
        uniform float wf;
        uniform float farPlane;
        in vec4 posHeight;
        in vec2 plateCoords;
        out vec2 plateTc;
        out vec3 pos;

        void main()
        {
            plateTc = plateCoords;
            pos = posHeight.xyz * (posHeight.w + radius + 0.0001 * wf);

            vec4 cpos = mvp * vec4(pos, 1.0);
            // float C = 1.0;
            // cpos.z = (2.0 * log(C * wpos.w + 1.0) / log(C * farPlane + 1.0) - 1.0) * wpos.w;

            gl_Position = cpos;
        }",
        &(String::from("
        uniform float wf;
        uniform float radius;
        uniform sampler2D tex_normals;
        uniform sampler2D tex_heights;
        in vec2 plateTc;
        in vec3 pos;
        layout(location = 0) out vec4 outNormalWf;
        layout(location = 1) out vec4 outPositionHeight;
        ") + &channels.glsl_texture_declarations() + "
        " + &channels.glsl_output_declarations(2) + "

        void main()
        {
            vec3 normalFromTex = texture(tex_normals, plateTc).xyz;
            float height = texture(tex_heights, plateTc).r;
            outNormalWf = vec4(normalFromTex, wf);
            outPositionHeight = vec4(normalize(pos) * (radius + height), height);
            " + &channels.glsl_assignments("plateTc") + "
        }"), 300)
}

fn create_color_program(colorator: &str, channels: &Channels, textures: &Vec<(String, tinygl::Texture)>) -> tinygl::Program {
    let vert_source = "
        in vec2 vertex;
        out vec2 tc_screen;
        void main() {
            tc_screen = vec2(0.5) + 0.5 * vertex;
            gl_Position = vec4(vertex, 0.0, 1.0);
        }";

    let texture_function_definitions = textures
        .iter()
        .fold(String::new(), |acc, tex| {
            let texname = String::from("_texture_") + &tex.0;
            acc + "uniform sampler2D " + &texname + ";\n" +
            "vec3 " + &tex.0 + "(float scale, float dropoff) {\n" +
                // build actual scale factors for this and the two neighbor tiles, based on dropoff factor
                "vec3 adjustedDists = clamp(uvDists / dropoff, vec3(0.0), vec3(1.0));\n" +
                "vec2 factors = pow(vec2(1.0) - adjustedDists.xy, vec2(2.0));" +
                "if (adjustedDists.z > 0.0) factors.y *= factors.x;\n" +

                // calculate some arbitrary, but stable gradient, so that texture filtering doesn't screw up
                "vec2 globalUV = (scenePosition.xy + scenePosition.yz + scenePosition.zx) * scale;\n" +
                "vec2 dUVdx = dFdx(globalUV);\n" +
                "vec2 dUVdy = dFdy(globalUV);\n" +

                // calculate how much the prime value is ahead of the others
                "float primeness = smoothstep(1.2, 1.6, 1.0 / max(0.001, max(factors.x, factors.y)));\n" +

                "vec3 ret = textureGrad(" + &texname + ", uv1 * scale, dUVdx, dUVdy).rgb;\n" +
                "if (primeness > 0.0) {\n" +
                    // improve on inigo quilez' algorithm a bit by stretching and invsmoothstep()ing the index
                    "float r = -0.1 + 1.2 * noise(uv1 * scale);\n" +
                    "r = (r + (r - (r * r * (3.0 - 2.0 * r))));\n;" +
                    "float index = 4.0 * clamp(r, 0.0, 1.0);\n;" +
                    "float i = floor(index);\n" +
                    "float f = fract(index);\n" +
                    "vec2 off1 = sin(vec2(3.0,7.0)*(i+0.0));\n" +
                    "vec2 off2 = sin(vec2(3.0,7.0)*(i+1.0));\n" +
                    "vec3 jumble1 = textureGrad(" + &texname + ", uv1 * scale + off1, dUVdx, dUVdy).rgb;\n" +
                    "vec3 jumble2 = textureGrad(" + &texname + ", uv1 * scale + off2, dUVdx, dUVdy).rgb;\n" +
                    "vec3 jumble = mix(jumble1, jumble2, smoothstep(0.1, 1.0, f));\n" +
                    "ret = mix(ret, jumble, primeness);\n" +
                "}\n" +
                "if (factors.x > 0.0) ret += textureGrad(" + &texname + ", uv23.xy * scale, dUVdx, dUVdy).rgb * factors.x;\n" +
                "if (factors.y > 0.0) ret += textureGrad(" + &texname + ", uv23.zw * scale, dUVdx, dUVdy).rgb * factors.y;\n" +
                "return ret / (1.0 + factors.x + factors.y);\n}\n"
        });

    let channel_variables = channels.glsl_base_declarations().iter().fold(
        String::new(), |acc, chan| acc + chan + ";\n"
    );

    let frag_source = String::from("
        uniform sampler2D scene_normal;
        uniform sampler2D scene_position;
        uniform float waterHeight;
        in vec2 tc_screen;
        layout(location = 0) out vec4 outColorReflectivity;

        vec2 uv1;
        vec4 uv23;
        vec3 uvDists;

        vec3 sceneNormal;
        vec3 scenePosition;

        ") + &channels.glsl_texture_declarations() + "
        " + &channel_variables + "
        " + &noise::ShaderNoise::declarations() + "
        " + &texture_function_definitions + "
        \n#line 1\n" + colorator + "

        " + &noise::ShaderNoise::definitions() + "
        " + &super::icosahedron_defs::DEFS + "

        vec3 _normUnit(vec3 v) {
            vec3 L = abs(v);
            return v / (L.x + L.y + L.z);
        }

        float _maxElem(vec3 v) {
            return max(v.x, max(v.y, v.z));
        }

        vec2 _projectIntoUvSpace(vec3 position, vec3 normal) {
            float x2 = normal.y + normal.z;
            float y2 = normal.x + normal.z;
            float z2 = (-normal.x * x2 - normal.y * y2) / normal.z;

            vec3 dir1 = normalize(vec3(x2, y2, z2));
            vec3 dir2 = cross(dir1, normal);

            // project onto plane
            vec3 onPlane = position - normal * dot(position, normal);

            float u = dot(onPlane, dir1);
            float v = dot(onPlane, dir2);
            return vec2(u, v);
        }

        void _generateUvMaps(vec3 n, vec3 position)
        {
            float d1 = 0.0;
            float d2 = 0.0;
            float dp = 0.0;
            int i1 = 0;
            int i2 = 0;
            int ip = 0;

            //
            // find highest and second-highest scoring hexagon
            //
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

            //
            // find highest-scoring pentagon
            //
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
            vec3 thisUvw = _normUnit(mat3(icoMats1[i1], icoMats2[i1], icoMats3[i1]) * n);
            vec3 neighborUvw = _normUnit(mat3(icoMats1[i2], icoMats2[i2], icoMats3[i2]) * n);

            // relative distance to the neighbor hexagon border, 0 is the border, 1 is this hexagon's center
            float neighborDist = 1.4142 * 3.0 * _maxElem(-neighborUvw);

            // UV spaces for the three adjacent surfaces
            vec2 thisUv = _projectIntoUvSpace(position, thisHexNorm);
            vec2 neighborUv = _projectIntoUvSpace(position, neighborHexNorm);
            vec2 pentUv = _projectIntoUvSpace(position, pentNorm);

            if (all(lessThan(thisUvw, vec3(2.0 / 3.0)))) {
                // relative distance to the pentagon border, 0 is in the border, 1 is this hexagon's center
                float pentDist = 2.0 - 3.0 * _maxElem(thisUvw);

                float fNeighbor = clamp(0.0, neighborDist, 1.0);
                float fPentagon = clamp(0.0, pentDist, 1.0);

                uv1 = thisUv;
                uv23 = vec4(neighborUv, pentUv);
                uvDists = vec3(fNeighbor, fPentagon, 0.0);
            }
            else {
                // relative distance to the pentagon border, 0 is in the border, 1 is this hexagon's center
                float mainDist = 3.0 * _maxElem(thisUvw) - 2.0;

                float fNeighbor = clamp(0.0, neighborDist, 1.0);
                float fMain = clamp(0.0, mainDist, 1.0);

                uv1 = pentUv;
                uv23 = vec4(thisUv, neighborUv);
                uvDists = vec3(fMain, fNeighbor, 1.0);
            }
        }

        void main()
        {
            vec3 normalFromTex = texture(scene_normal, tc_screen).xyz;

            // open sky is encoded as (0,0,0) normal
            if (normalFromTex == vec3(0.0)) {
                outColorReflectivity = vec4(0.0);
                return;
            }

            vec4 scenePosTex = texture(scene_position, tc_screen);

            sceneNormal = vec3(-1.0) + 2.0 * normalFromTex;
            scenePosition = scenePosTex.xyz;

            // water can have any color, so long as it's black.
            if (scenePosTex.w <= waterHeight) {
                outColorReflectivity = vec4(0.0, 0.0, 0.0, 1.0);
                return;
            }

            _generateUvMaps(sceneNormal, scenePosition);

        " + &channels.glsl_assignments("tc_screen") + "
            vec3 col = color(sceneNormal, scenePosition, scenePosTex.w);
            outColorReflectivity = vec4(col, 0.0);
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
    //height = 0.0;
}")
}

pub fn default_colorator() -> String {
String::from("vec3 color(vec3 normal, vec3 position, float height)
{
    return vec3(1.0);
}")
}

impl Renderer {
    fn create_planet(&mut self, generator: &str, channels: Channels, update_errors: bool) -> bool {
        let planet = tree::Planet::new(self.plate_depth as _, self.texture_delta, self.planet_radius, generator, &channels);

        match planet {
            Ok(mut planet) => {
                // start data generation for the first levels
                let culler = culling::Culler::new(&self.camera.mvp((2, 1), false));
                planet.set_detail((255.0 * self.vertex_detail) as _);
                planet.update_quad_tree(&self.camera.eye(), &culler, 3, self.hide_backside);
                planet.start_data_generation(30);

                // Clear errors
                self.planet = Some(planet);
                self.generator = generator.to_string();
                self.channels = channels;
                if update_errors {
                    self.errors_generator = None;
                }

                true
            },
            Err(errs) => {
                if update_errors {
                    self.errors_generator = Some(errs);
                }
                false
            },
        }
    }

    pub fn new() -> Self {
        let colorator = default_colorator();
        let channels = Channels::new();

        let mut ret = Renderer {
            camera: FlyCamera::new(100.0),
            program_scene: Some(create_scene_program(&channels)),
            program_color: None,
            program_color_default: create_color_program(&colorator, &channels, &Vec::new()),
            program_water: create_water_program(),

            planet: None,
            plate_depth: 6,
            texture_delta: 0,
            planet_radius: 100.0,

            wireframe: false,
            reduce_poly_count: true,
            no_update_plates: false,
            hide_backside: false,
            vertex_detail: 0.5,

            rendered_plates: 0,
            rendered_triangles: 0,

            fbo_scene: None,
            fbo_color: None,
            fsquad: tinygl::shapes::FullscreenQuad::new(),
            water_plate_factory: WaterPlateFactory::new(6, 6, 0),
            water_height: 1.0,
            water_depth: 6,
            water_wavetime: 0.0,

            generator: default_generator(),
            channels,
            colorator,
            textures: Vec::new(),

            errors_generator: None,
            errors_colorator: None,
        };

        ret.create_planet(&default_generator(), Channels::new(), false);
        ret
    }

    pub fn out_position(&self) -> &tinygl::Texture {
        self.fbo_scene.as_ref().unwrap().texture("positionHeight").as_ref().unwrap()
    }

    pub fn out_normal(&self) -> &tinygl::Texture {
        self.fbo_scene.as_ref().unwrap().texture("normalWf").as_ref().unwrap()
    }

    pub fn out_color(&self) -> &tinygl::Texture {
        self.fbo_color.as_ref().unwrap().texture("color").as_ref().unwrap()
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

    pub fn plate_depth(&self) -> u32 {
        self.plate_depth
    }

    pub fn set_plate_depth(&mut self, depth: u32) {
        self.plate_depth = depth;
        self.water_plate_factory = WaterPlateFactory::new(self.water_depth, self.plate_depth, self.texture_delta);
        let gen = self.generator.clone();
        let chan = self.channels.clone();
        self.create_planet(&gen, chan, false);
    }

    pub fn water_depth(&self) -> u32 {
        self.water_depth
    }

    pub fn set_water_depth(&mut self, depth: u32) {
        self.water_depth = depth;
        self.water_plate_factory = WaterPlateFactory::new(self.water_depth, self.plate_depth, self.texture_delta);
    }

    pub fn texture_delta(&self) -> u32 {
        self.texture_delta
    }

    pub fn set_texture_delta(&mut self, delta: u32) {
        self.texture_delta = delta;
        self.water_plate_factory = WaterPlateFactory::new(self.water_depth, self.plate_depth, self.texture_delta);
        let gen = self.generator.clone();
        let chan = self.channels.clone();
        self.create_planet(&gen, chan, false);
    }

    pub fn radius(&self) -> f32 {
        self.planet_radius
    }

    pub fn set_radius(&mut self, radius: f32) {
        self.planet_radius = radius;
        let gen = self.generator.clone();
        let chan = self.channels.clone();
        self.create_planet(&gen, chan, false);
        self.camera.scale_with_planet(radius);
    }

    pub fn water_height(&self) -> f32 {
        self.water_height
    }

    pub fn set_water_height(&mut self, height: f32) {
        self.water_height = height;
    }

    pub fn set_colorator(&mut self, colorator: &str) -> bool {
        let new_program = create_color_program(colorator, &self.channels, &self.textures);
        let ret = new_program.valid();

        if ret {
            self.colorator = colorator.to_string();
            self.errors_colorator = None;
            self.program_color = Some(new_program);
        } else {
            self.errors_colorator = Some(new_program.fragment_log());
        }
        ret
    }

    pub fn set_generator_and_channels(&mut self, generator: &str, channels: &HashMap<String, usize>) -> bool {
        let ret = self.create_planet(generator, Channels::from(channels), true);
        if ret {
            self.generator = generator.to_string();
            self.channels = Channels::from(channels);
            self.program_scene = Some(create_scene_program(&self.channels));
            self.fbo_scene = None;  // need to re-create channels FBOs
        }
        ret
    }

    pub fn set_generator(&mut self, generator: &str) -> bool {
        let chans = self.channels.clone();
        let ret = self.create_planet(generator, chans, true);
        if ret {
            self.generator = generator.to_string();
        }
        ret
    }

    fn recreate_program(&mut self) {
        // need to check if our colorator still fits with all those new channels and textures coming in
        let new_program = create_color_program(&self.colorator, &self.channels, &self.textures);

        if new_program.valid() {
            self.errors_colorator = None;
            self.program_color = Some(new_program);
        } else {
            self.errors_colorator = Some(new_program.fragment_log());
            self.program_color = None;
        }
    }

    pub fn set_channels(&mut self, channels: &HashMap<String, usize>, generator: &str) -> bool {
        let ret = self.create_planet(generator, Channels::from(channels), true);

        if ret {
            self.generator = generator.to_string();
            self.channels = Channels::from(channels);
            self.recreate_program();
            self.program_scene = Some(create_scene_program(&self.channels));
            self.fbo_scene = None;  // need to re-create channels FBOs
        }
        ret
    }

    pub fn clear_textures(&mut self) {
        self.textures.clear();
        self.recreate_program();
    }

    pub fn add_texture(&mut self, name: &str, texture: tinygl::Texture) {
        self.textures.push((name.to_string(), texture));
        self.recreate_program();
    }

    pub fn rename_texture(&mut self, index: usize, new_name: &str) {
        if index < self.textures.len() {
            self.textures[index].0 = new_name.to_string();
            self.recreate_program();
        }
    }

    pub fn remove_texture(&mut self, index: usize) {
        if index < self.textures.len() {
            self.textures.remove(index);
            self.recreate_program();
        }
    }

    pub fn textures(&self) -> &Vec<(String, tinygl::Texture)> {
        &self.textures
    }

    /// Renders the planet into the internal FBO
    ///
    /// The results can be accessed through `out_position()`, `out_normal()`, and `out_color()`.
    pub fn render(&mut self, windowsize: (u32, u32), dt: f32) {
        //
        // Setup view/projection matrices
        //
        let mvp = self.camera.mvp(windowsize, true);
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
        // Update and bind Scene Render Target
        //
        if self.fbo_scene.as_ref().map(|fbo| fbo.size() != windowsize).unwrap_or(true) {
            let mut fbo = tinygl::FrameBufferObject::new((windowsize.0 as _, windowsize.1 as _));
            fbo.add("normalWf", gl::RGBA, gl::RGBA, gl::UNSIGNED_BYTE);
            fbo.add("positionHeight", gl::RGBA32F, gl::RGBA, gl::FLOAT);
            // TODO: avoid duplication
            for chan in self.channels.iter() {
                let int_fmt = match chan.1 {
                    1 => (gl::R8, gl::RED),
                    2 => (gl::RG8, gl::RG),
                    3 => (gl::RGB8, gl::RGB),
                    4 => (gl::RGBA8, gl::RGBA),
                    _ => { panic!("Does not compute"); },
                };
                fbo.add(&chan.0, int_fmt.0, int_fmt.1, gl::UNSIGNED_BYTE);
            }
            fbo.add_depth_renderbuffer();
            self.fbo_scene = Some(fbo);
        }
        self.fbo_scene.as_ref().unwrap().bind();

        unsafe {
            gl::ClearColor(0.0, 0.0, 0.0, 0.0);
            gl::Clear(gl::COLOR_BUFFER_BIT | gl::DEPTH_BUFFER_BIT);
            gl::Enable(gl::CULL_FACE);
            gl::Enable(gl::DEPTH_TEST);
            gl::Disable(gl::BLEND);
            gl::DepthFunc(gl::LEQUAL);
        }

        //
        // Prepare scene program
        //
        let program_scene = self.program_scene.as_ref().unwrap();
        program_scene.bind();
        program_scene.uniform("mvp", tinygl::Uniform::Mat4(mvp));
        program_scene.uniform("farPlane", tinygl::Uniform::Float(self.camera.far()));
        program_scene.uniform("radius", tinygl::Uniform::Float(self.planet_radius));
        program_scene.uniform("tex_normals", tinygl::Uniform::Signed(0));
        program_scene.uniform("tex_heights", tinygl::Uniform::Signed(1));
        program_scene.vertex_attrib_buffer("plateCoords", planet.plate_coords(), 2, gl::UNSIGNED_SHORT, true, 4, 0);

        self.rendered_triangles = 0;
        let rendered_plates = planet.rendered_plates();

        //
        // Render scene into position/normal FBO
        //
        for plate_ref in &rendered_plates {
            let mut plate = plate_ref.borrow();
            plate.bind_render_data(program_scene, 0);

            // Render Triangles
            self.rendered_triangles += if self.reduce_poly_count {
                plate.indices().draw_all(gl::TRIANGLES)
            } else {
                planet.triangle_indices().draw_all(gl::TRIANGLES)
            } / 3;

            // Maybe Render Wireframe
            if self.wireframe {
                program_scene.uniform("wf", tinygl::Uniform::Float(1.0));
                if self.reduce_poly_count {
                    plate.indices().draw(gl::LINES, plate.wireframe_count() as _, 0);
                } else {
                    planet.triangle_indices().draw_all(gl::LINES);
                }
                program_scene.uniform("wf", tinygl::Uniform::Float(0.0));
            }
        };

        program_scene.disable_all_vertex_attribs();
        self.rendered_plates = rendered_plates.len();

        //
        // Render water on top of terrain
        //
        let water_plates = planet.rendered_water_plates(&culler, self.water_height);
        self.water_wavetime = (self.water_wavetime + dt).fract();
        self.program_water.bind();
        self.program_water.uniform("mvp", tinygl::Uniform::Mat4(mvp));
        self.program_water.uniform("farPlane", tinygl::Uniform::Float(self.camera.far()));
        self.program_water.uniform("radius", tinygl::Uniform::Float(self.planet_radius));
        self.program_water.uniform("waterHeight", tinygl::Uniform::Float(self.water_height));
        self.program_water.uniform("waterTime", tinygl::Uniform::Float(self.water_wavetime));
        self.program_water.uniform("heights", tinygl::Uniform::Signed(0));
        self.program_water.uniform("normals", tinygl::Uniform::Signed(1));
        self.program_water.vertex_attrib_buffer("texCoords", self.water_plate_factory.tex_coords(), 2, gl::UNSIGNED_SHORT, true, 4, 0);
        unsafe { gl::ActiveTexture(gl::TEXTURE0); }
        let water_idx_count = self.water_plate_factory.indices().count() as _;
        self.water_plate_factory.indices().bind();
        for water_plate in water_plates {
            let mut water_plate = water_plate.borrow_mut();
            water_plate.bind_height_texture(0);
            water_plate.bind_normal_texture(1);
            let water_buffer = water_plate.get_water_buffer(&self.water_plate_factory);
            self.program_water.vertex_attrib_buffer("sphereCoordsBorder", &water_buffer, 4, gl::FLOAT, false, 16, 0);
            unsafe { gl::DrawElements(gl::TRIANGLES, water_idx_count, gl::UNSIGNED_SHORT, std::ptr::null()); }
        }
        self.program_water.disable_all_vertex_attribs();

        //
        // Update and bind Color Render Target
        //
        if self.fbo_color.as_ref().map(|fbo| fbo.size() != windowsize).unwrap_or(true) {
            let mut fbo = tinygl::FrameBufferObject::new((windowsize.0 as _, windowsize.1 as _));
            fbo.add("color", gl::RGBA, gl::RGBA, gl::UNSIGNED_BYTE);
            self.fbo_color = Some(fbo);
        }
        self.fbo_color.as_ref().unwrap().bind();

        let program_color = match self.program_color.as_mut() {
            Some(prog) => prog,
            None => &mut self.program_color_default
        };

        // bind scene textures
        program_color.bind();
        program_color.uniform("waterHeight", tinygl::Uniform::Float(self.water_height));
        program_color.uniform("scene_normal", tinygl::Uniform::Signed(0));
        program_color.uniform("scene_position", tinygl::Uniform::Signed(1));
        self.fbo_scene.as_ref().unwrap().texture("normalWf").unwrap().bind_at(0);
        self.fbo_scene.as_ref().unwrap().texture("positionHeight").unwrap().bind_at(1);

        // Bind Textures
        for tex in self.textures.iter().enumerate() {
            let idx = tex.0 + 2;
            (tex.1).1.bind_at(idx as _);
            program_color.uniform(&format!("_texture_{}", (tex.1).0), tinygl::Uniform::Signed(idx as _));
        }

        // Bind channel textures
        // TODO: avoid duplication
        for chan in self.channels.iter().enumerate() {
            let idx = 2 + self.textures.len() + chan.0;
            self.fbo_scene.as_ref().unwrap().texture((chan.1).0).unwrap().bind_at(idx as _);
            program_color.uniform(&format!("_channel_texture_{}", (chan.1).0), tinygl::Uniform::Signed(idx as _));
        }

        self.fsquad.render(program_color, "vertex");
        program_color.disable_all_vertex_attribs();

        unsafe { gl::BindFramebuffer(gl::FRAMEBUFFER, 0); }
    }

    /// Simply renders the planet for the given camera and using the given program
    pub fn render_for(&self, program: &tinygl::Program, eye: Vector3<f32>, mvp: Matrix4<f32>) {
        let planet = self.planet.as_ref().unwrap();
        let plates = planet.rendered_plates_for_camera(eye, mvp, 1.0);

        program.uniform("radius", tinygl::Uniform::Float(self.planet_radius));

        for plate in &plates {
            program.vertex_attrib_buffer("posHeight", plate.borrow().get_pos_height_buffer(), 4, gl::FLOAT, false, 16, 0);
            plate.borrow().indices().draw_all(gl::TRIANGLES);
        };
    }
}
