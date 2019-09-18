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
    program_plates: Option<tinygl::Program>,
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

    // errors to be picked up by y'all
    errors_generator: Option<String>,
    errors_colorator: Option<String>,
}

fn create_water_program(channels: &Channels) -> tinygl::Program {
    let vert = include_str!("../shaders/render_water.vert");
    let frag = include_str!("../shaders/render_water.frag");
    let frag = frag.replace("$CHANNEL_OUTPUTS", &channels.glsl_texture_declarations());
    let frag = frag.replace("$CHANNEL_TEXTURES", &channels.glsl_output_declarations(2));
    let frag = frag.replace("$CHANNEL_ASSIGNMENTS", &channels.glsl_assignments("tc"));
    tinygl::Program::new_versioned(vert, &frag, 300)
}

fn create_plates_program(channels: &Channels) -> tinygl::Program {
    let vert = include_str!("../shaders/render_plates.vert");
    let frag = include_str!("../shaders/render_plates.frag");
    let frag = frag.replace("$CHANNEL_OUTPUTS", &channels.glsl_texture_declarations());
    let frag = frag.replace("$CHANNEL_TEXTURES", &channels.glsl_output_declarations(2));
    let frag = frag.replace("$CHANNEL_ASSIGNMENTS", &channels.glsl_assignments("plateTc"));
    tinygl::Program::new_versioned(vert, &frag, 300)
}

fn create_color_program(colorator: &str, channels: &Channels, textures: &Vec<(String, tinygl::Texture)>) -> tinygl::Program {
    let vert = include_str!("../shaders/render_color.vert");

    // TODO: proper texture filtering + borders for channel textures

    let texture_function_definitions = textures.iter().fold(String::new(), |acc, tex| {
        let texname = String::from("_texture_") + &tex.0;
        let src = include_str!("../shaders/render_color_texture.glsl");
        let src = src.replace("$TEXNAME", &texname);
        let src = src.replace("$FUNCNAME", &tex.0);
        src
    });

    let channel_variables = channels.glsl_base_declarations().iter().fold(
        String::new(), |acc, chan| acc + chan + ";\n"
    );

    let icosahedron = include_str!("../shaders/render_color_icosahedron.glsl");

    let frag = include_str!("../shaders/render_color.frag");
    let frag = frag.replace("$CHANNEL_TEXTURES", &channels.glsl_texture_declarations());
    let frag = frag.replace("$CHANNEL_VARIABLES", &channel_variables);
    let frag = frag.replace("$CHANNEL_ASSIGNMENTS", &channels.glsl_assignments("tc_screen"));
    let frag = frag.replace("$NOISE", noise::ShaderNoise::definitions());
    let frag = frag.replace("$ICOSAHEDRON", icosahedron);
    let frag = frag.replace("$TEXTURE_FUNCTIONS", &texture_function_definitions);
    let frag = frag.replace("$COLORATOR", colorator);

    tinygl::Program::new(vert, &frag)
}

pub fn default_generator() -> & 'static str {
    include_str!("../shaders/default_generator.glsl")
}

pub fn default_colorator() -> & 'static str {
    include_str!("../shaders/default_colorator.glsl")
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
        let colorator = default_colorator().to_string();
        let channels = Channels::new();

        let mut ret = Renderer {
            camera: FlyCamera::new(100.0),
            program_plates: Some(create_plates_program(&channels)),
            program_color: None,
            program_color_default: create_color_program(&colorator, &channels, &Vec::new()),
            program_water: create_water_program(&channels),

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

            generator: default_generator().to_string(),
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
        let water = position.magnitude() - (self.planet_radius + self.water_height());
        self.planet.as_ref().unwrap().get_surface_height(position).min(water)
    }

    pub fn get_camera_surface_height(&self) -> f32 {
        let eye = self.camera.eye();
        self.get_surface_height(&eye)
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
            self.program_plates = Some(create_plates_program(&self.channels));
            self.program_water = create_water_program(&self.channels);
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
            self.program_plates = Some(create_plates_program(&self.channels));
            self.program_water = create_water_program(&self.channels);
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
            fbo.add("normalWf", gl::RGBA32F, gl::RGBA, gl::FLOAT);
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
        let program_plates = self.program_plates.as_ref().unwrap();
        program_plates.bind();
        program_plates.uniform("mvp", tinygl::Uniform::Mat4(mvp));
        program_plates.uniform("farPlane", tinygl::Uniform::Float(self.camera.far()));
        program_plates.uniform("radius", tinygl::Uniform::Float(self.planet_radius));
        program_plates.uniform("tex_normals", tinygl::Uniform::Signed(0));
        program_plates.uniform("tex_heights", tinygl::Uniform::Signed(1));
        program_plates.vertex_attrib_buffer("plateCoords", planet.plate_coords(), 2, gl::UNSIGNED_SHORT, true, 4, 0);

        self.rendered_triangles = 0;
        let rendered_plates = planet.rendered_plates();

        //
        // Render scene into position/normal FBO
        //
        for plate_ref in &rendered_plates {
            let mut plate = plate_ref.borrow();
            plate.bind_render_data(program_plates, 0);

            // Render Triangles
            self.rendered_triangles += if self.reduce_poly_count {
                plate.indices().draw_all(gl::TRIANGLES)
            } else {
                planet.triangle_indices().draw_all(gl::TRIANGLES)
            } / 3;

            // Maybe Render Wireframe
            if self.wireframe {
                program_plates.uniform("wf", tinygl::Uniform::Float(1.0));
                if self.reduce_poly_count {
                    plate.indices().draw(gl::LINES, plate.wireframe_count() as _, 0);
                } else {
                    planet.triangle_indices().draw_all(gl::LINES);
                }
                program_plates.uniform("wf", tinygl::Uniform::Float(0.0));
            }
        };

        program_plates.disable_all_vertex_attribs();
        self.rendered_plates = rendered_plates.len();

        //
        // Render water on top of terrain
        //
        let water_plates = planet.rendered_water_plates(&culler, self.water_height);
        self.program_water.bind();
        self.program_water.uniform("mvp", tinygl::Uniform::Mat4(mvp));
        self.program_water.uniform("farPlane", tinygl::Uniform::Float(self.camera.far()));
        self.program_water.uniform("radius", tinygl::Uniform::Float(self.planet_radius));
        self.program_water.uniform("waterHeight", tinygl::Uniform::Float(self.water_height));
        self.program_water.uniform("normals", tinygl::Uniform::Signed(0));
        self.program_water.uniform("heights", tinygl::Uniform::Signed(1));
        self.program_water.vertex_attrib_buffer("texCoords", planet.plate_coords(), 2, gl::UNSIGNED_SHORT, true, 4, 0);
        self.program_water.vertex_attrib_buffer("isRibbon", self.water_plate_factory.ribbons(), 1, gl::UNSIGNED_BYTE, true, 1, 0);
        let water_idx_count = self.water_plate_factory.indices().count() as _;
        self.water_plate_factory.indices().bind();
        for water_plate in water_plates {
            water_plate.borrow().bind_render_data(&self.program_water, 0);
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
