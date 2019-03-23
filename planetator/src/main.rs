extern crate gl;
extern crate tinygl;
extern crate util3d;
extern crate sdl2;
extern crate imgui;
extern crate imgui_sys;
extern crate cgmath;
extern crate lru_cache;
extern crate appbase;
extern crate array2d;
#[cfg(target_os = "emscripten")] extern crate emscripten_util;

extern crate serde;
extern crate bincode;
#[macro_use] extern crate serde_derive;

use appbase::webrunner;
#[cfg(target_os = "emscripten")] use emscripten_util::fileload;
#[cfg(target_os = "emscripten")] use emscripten_util::imgdecode;

mod earth;
mod guiutil;
mod atmosphere;
mod shadowmap;
mod savegames;

use std::collections::HashMap;
use sdl2::keyboard::Keycode;
use sdl2::mouse::MouseButton;
use sdl2::event::{Event};
use imgui::*;
use appbase::fpswidget::FpsWidget;
use cgmath::prelude::*;

const HTML_INPUT_PLANET: &str = "input_loadsavegame";
const HTML_INPUT_TEXTURE: &str = "input_texupload";

struct MyApp {
    windowsize: (u32, u32),
    errors: Vec<String>,

    keyboard: HashMap<Keycode, bool>,
    current_mouse_press: Option<(i32, i32)>,

    fps: FpsWidget,

    edit_generator: guiutil::ShaderEditData,
    edit_colorator: guiutil::ShaderEditData,
    edit_js: guiutil::ShaderEditData,
    select_channels: Vec<(String, i32)>,
    texuploads: Vec<i32>,
    active_textures: Vec<(String, (i32, i32), Vec<u8>)>,

    sun_speed: f32,
    sun_lon: f32,
    sun_lat: f32,

    shadows: shadowmap::ShadowMap,

    renderer: earth::renderer::Renderer,
    postprocess: tinygl::Program,

    fsquad: tinygl::shapes::FullscreenQuad,
}

impl MyApp {
    fn pressed(&self, key: Keycode) -> bool {
        *self.keyboard.get(&key).unwrap_or(&false)
    }

    fn build_channels(&self) -> HashMap<String, usize> {
        let mut ret = HashMap::new();
        for chan in &self.select_channels {
            ret.insert(chan.0.clone(), (chan.1 + 1) as usize);
        }
        ret
    }

    fn channels_changed(&mut self) {
        let chans = self.build_channels();
        if self.renderer.set_channels(&chans, &self.edit_generator.to_str()) {
            // also update the colorator, because it might have been already adapted to the new channels
            if self.renderer.set_colorator(&self.edit_colorator.to_str()) {
                self.edit_colorator.works();
            }

            self.edit_generator.works();
        }
    }

    fn save_state(&self) -> Vec<u8> {
        bincode::serialize(&savegames::Savegame::Version0 {
            generator: self.edit_generator.to_str().to_string(),
            colorator: self.edit_colorator.to_str().to_string(),
            select_channels: self.select_channels.clone(),
            active_textures: self.active_textures.clone(),
        }).unwrap()
    }

    fn restore_state(&mut self, serialized: &Vec<u8>) {
        let deser = bincode::deserialize::<savegames::Savegame>(serialized);

        if let Ok(deser) = deser {
            match deser {
                savegames::Savegame::Version0{generator, colorator, select_channels, active_textures} => {
                    self.edit_generator.set_source(&generator);
                    self.edit_colorator.set_source(&colorator);
                    self.select_channels = select_channels.clone();
                    self.renderer.clear_textures();
                    for tex in &active_textures {
                        self.renderer.add_texture(&tex.0, tinygl::Texture::from_data_2d(&tex.2, tex.1));
                    }
                    self.active_textures = active_textures;
                }
                _ => {
                    self.errors.push(String::from("Couldn't parse planet data"));
                    return;
                }
            }

            self.edit_generator.works();
            self.edit_colorator.works();

            let new_chans = self.build_channels();
            self.renderer.set_generator_and_channels(&self.edit_generator.to_str(), &new_chans);
            self.renderer.set_colorator(&self.edit_colorator.to_str());
        } else {
            self.errors.push(String::from("Couldn't deserialize planet data"));
        }
    }
}

fn create_postprocess_shader() -> tinygl::Program {
    tinygl::Program::new_versioned("
        in vec2 vertex;
        out vec2 clipPos;
        void main() {
            clipPos = vertex;
            gl_Position = vec4(vertex, 0.0, 1.0);
        }
        ", &(String::from("
        uniform float planetRadius;
        uniform vec3 eyePosition;
        uniform vec3 sunDirection;
        uniform mat4 inverseViewProjectionMatrix;
        uniform sampler2D planetColor;
        uniform sampler2D planetNormal;
        uniform sampler2D planetPosition;

        ")
        + &shadowmap::ShadowMap::glsl()
        + &atmosphere::shader_source().replace("#version 300 es", "") +
        "
        in vec2 clipPos;
        out vec4 outColor;

        uniform vec3 white_point;
        uniform float exposure;
        uniform vec2 sun_size;
        const vec3 kGroundAlbedo = vec3(0.0, 0.0, 0.04);

        vec3 clampVecLen(vec3 vec, float minLen) {
            float vecLen = length(vec);
            if (vecLen < minLen)
                vec *= minLen / vecLen;
            return vec;
        }

        void main() {
            vec4 normalFromTex = texture(planetNormal, vec2(0.5) + 0.5 * clipPos);

            // calculate eye direction in that pixel
            vec4 globalPosV4 = inverseViewProjectionMatrix * vec4(clipPos, 0.0, 1.0);
            vec3 globalPos = globalPosV4.xyz / globalPosV4.w;
            vec3 eyeDir = normalize(globalPos - eyePosition);

            vec3 color = vec3(0.0);

            if (length(normalFromTex) > 0.0) {
                //
                // Load position/normal/color from planet rendering textures
                //
                vec3 normal = vec3(-1.0) + 2.0 * normalFromTex.xyz;
                float wireframe = normalFromTex.w;
                vec3 pColor = texture(planetColor, vec2(0.5) + 0.5 * clipPos).rgb;
                vec3 pPos = texture(planetPosition, vec2(0.5) + 0.5 * clipPos).rgb;
                float dist = length(pPos - eyePosition);
                float dotSun = dot(normal, sunDirection);

                //
                // Find out if we are shadowed by the terrain, and interpolate between last and curr sun position
                //
                vec3 shadowMapDebugColor;
                float shadow = getShadow(pPos, dotSun, dist, shadowMapDebugColor);

                // visibility of the sky and sun, based on shadows cast by the terrain
                float sunVisibility = 0.5 * shadow;
                float skyVisibility = 1.0;

                vec3 fakedEyePos = clampVecLen(eyePosition / planetRadius, 1.01);
                vec3 fakedTerrainPos = clampVecLen(pPos / planetRadius, 1.01);

                //
                // Compute the radiance reflected by the ground.
                //
                vec3 sky_irradiance;
                vec3 sun_irradiance = GetSunAndSkyIrradiance(pPos / planetRadius, normalize(pPos), sunDirection, sky_irradiance);
                vec3 ground_radiance = pColor * (1.0 / PI) * (
                    sun_irradiance * sunVisibility +
                    sky_irradiance * skyVisibility);

                // float shadow_length =
                //     max(0.0, min(shadow_out, distance_to_intersection) - shadow_in) *
                //     lightshaft_fadein_hack;
                float shadow_length = 0.0;

                vec3 transmittance;

                vec3 in_scatter = GetSkyRadianceToPoint(
                    fakedEyePos,
                    normalize(pPos),
                    // TODO: fake the position to make sure that we are not looking 'up', i.e.
                    // that we always look below the horizon line.
                    // we can optimize this further a bit, because we might not have to reduce it all the way down to 1.0
                    // but only so far as to match the horizon
                    // TODO: also make sure that eyePosition is never < radius
                    shadow_length, sunDirection, transmittance
                );
                ground_radiance = ground_radiance * transmittance + in_scatter;

                color = pow(vec3(1.0) - exp(-ground_radiance / white_point * exposure), vec3(1.0 / 2.2));

                // draw wireframes on top?
                float brightness = dot(vec3(0.2126, 0.7152, 0.0722), color);
                color = mix(color, vec3(step(brightness, 0.4)), wireframe);
            }
            else {
                // Compute the radiance of the sky.
                float shadow_length = 0.0;
                vec3 transmittance;
                vec3 radiance = GetSkyRadiance(
                    eyePosition / planetRadius, eyeDir, shadow_length, sunDirection, transmittance
                );

                // If the view ray intersects the Sun, add the Sun radiance.
                if (dot(eyeDir, sunDirection) > sun_size.y) {
                    radiance = radiance + transmittance * GetSolarRadiance();
                }
                color = pow(vec3(1.0) - exp(-radiance / white_point * exposure), vec3(1.0 / 2.2));
            }

            outColor = vec4(color, 1.0);
        }
        "),
    300)
}

impl webrunner::WebApp for MyApp {
    fn new(windowsize: (u32, u32)) -> Self {
        // check for loading savegame files
        #[cfg(target_os = "emscripten")] fileload::start_upload(HTML_INPUT_PLANET);
        #[cfg(target_os = "emscripten")] fileload::start_upload(HTML_INPUT_TEXTURE);

        MyApp {
            windowsize,
            errors: Vec::new(),
            keyboard: HashMap::new(),
            current_mouse_press: None,
            fps: FpsWidget::new(150),
            edit_generator: guiutil::ShaderEditData::new("Generator", &earth::renderer::default_generator(), (250.0, 250.0), (600.0, 400.0)),
            edit_colorator: guiutil::ShaderEditData::new("Kolorator", &earth::renderer::default_colorator(), (250.0, 250.0), (600.0, 400.0)),
            edit_js: guiutil::ShaderEditData::new("JavaScript executor", "var elem = document.getElementById('state');", (250.0, 250.0), (600.0, 400.0)),
            select_channels: Vec::new(),
            texuploads: Vec::new(),
            active_textures: Vec::new(),
            sun_speed: 0.0,
            sun_lon: 0.0,
            sun_lat: 0.0,
            renderer: earth::renderer::Renderer::new(),
            shadows: shadowmap::ShadowMap::new(100.0),
            postprocess: create_postprocess_shader(),
            fsquad: tinygl::shapes::FullscreenQuad::new(),
        }
    }

    fn resize(&mut self, size: (u32, u32)) {
        self.windowsize = size;
    }

    fn render(&mut self, dt: f32) {
        self.fps.push(dt);
        let radius = self.renderer.radius();

        //
        // advance camera
        //
        let cdx = (if self.pressed(Keycode::A) {0.0} else {1.0}) + (if self.pressed(Keycode::D) {0.0} else {-1.0});
        let cdy = (if self.pressed(Keycode::S) {0.0} else {1.0}) + (if self.pressed(Keycode::W) {0.0} else {-1.0});
        let cdz = (if self.pressed(Keycode::LShift) {0.0} else {1.0}) + (if self.pressed(Keycode::Space) {0.0} else {-1.0});
        let speed = if self.pressed(Keycode::LCtrl) { 0.2 * dt } else { dt };
        self.renderer.camera().translate(&(cgmath::Vector3::new(cdx, cdz, cdy) * speed));
        let mvp = self.renderer.camera().mvp(self.windowsize);
        let eye = self.renderer.camera().eye();
        let look = self.renderer.camera().look();

        let camspeed = 2.0;
        let cam_height = self.renderer.get_surface_height(&eye);
        self.renderer.camera().set_move_speed(camspeed * cam_height.max(0.01));

        //
        // render planet into FBO
        //
        self.renderer.render(self.windowsize);

        //
        // Move Sun
        //
        self.sun_lon += dt * self.sun_speed;
        if self.sun_lon > 360.0 { self.sun_lon -= 360.0 }
        if self.sun_lon < 0.0 { self.sun_lon += 360.0 }
        let sun_lon = self.sun_lon * 3.14159 / 180.0;
        let sun_lat = self.sun_lat * 3.14159 / 180.0;
        let sun_direction = cgmath::Vector3::new(sun_lon.sin(), sun_lat.sin(), sun_lon.cos()).normalize();

        //
        // Update Shadow Depth Cascades
        //
        self.shadows.set_radius(radius);
        self.shadows.push_sun_direction(sun_direction);
        let to_render = self.shadows.prepare_render(eye, look);
        self.renderer.render_for(self.shadows.program(), to_render.1, to_render.0);
        self.shadows.finish_render();

        //
        // Setup Post-processing shader
        //
        self.postprocess.bind();
        self.shadows.prepare_postprocess(&self.postprocess, 4);
        self.postprocess.uniform("eyePosition", tinygl::Uniform::Vec3(eye));
        self.postprocess.uniform("inverseViewProjectionMatrix", tinygl::Uniform::Mat4(mvp.invert().unwrap()));
        self.postprocess.uniform("planetColor", tinygl::Uniform::Signed(0));
        self.postprocess.uniform("planetNormal", tinygl::Uniform::Signed(1));
        self.postprocess.uniform("planetPosition", tinygl::Uniform::Signed(2));
        self.postprocess.uniform("planetRadius", tinygl::Uniform::Float(radius));
        atmosphere::prepare_shader(self.postprocess.handle().unwrap(), 4 + self.shadows.num_textures());

        unsafe {
            gl::Viewport(0, 0, self.windowsize.0 as _, self.windowsize.1 as _);
            gl::Disable(gl::DEPTH_TEST);
            gl::Disable(gl::BLEND);
        }

        self.renderer.out_position().bind_at(2);
        self.renderer.out_normal().bind_at(1);
        self.renderer.out_color().bind_at(0);

        self.fsquad.render(&self.postprocess, "vertex");
    }

    fn do_ui(&mut self, ui: &imgui::Ui, keymod: sdl2::keyboard::Mod) {
        self.fps.render(ui, (0.0, 0.0), (200.0, 80.0));

        ui.window(im_str!("renderstats"))
            .flags(ImGuiWindowFlags::NoMove | ImGuiWindowFlags::NoTitleBar | ImGuiWindowFlags::NoSavedSettings)
            .size((200.0, 300.0), ImGuiCond::Appearing)
            .position((0.0, 80.0), ImGuiCond::Always)
            .constraints((200.0, 200.0), (1000.0, 1000.0))
            .build(|| {
                // Triangulation settings
                if ui.collapsing_header(im_str!("Triangulation")).default_open(false).build() {
                    ui.text(format!("Plates: {}", self.renderer.rendered_plates()));
                    ui.text(format!("Triangles: {}", guiutil::format_number( self.renderer.rendered_triangles() as _)));
                    ui.separator();

                    ui.checkbox(im_str!("Wireframe"), &mut self.renderer.wireframe);
                    ui.checkbox(im_str!("No Update"), &mut self.renderer.no_update_plates);
                    ui.checkbox(im_str!("Cull Backside"), &mut self.renderer.hide_backside);

                    let detail = guiutil::slider_float(ui, "Vertex Detail:", self.renderer.vertex_detail(), (0.0, 1.0), 1.0);
                    self.renderer.set_vertex_detail(detail);
                }

                // Atmosphere settings
                if ui.collapsing_header(im_str!("Atmosphere")).default_open(false).build() {
                    atmosphere::set_shader_radius(guiutil::slider_float(ui, "Shader Radius", atmosphere::shader_radius(), (1.0, 1.2), 1.0));
                    atmosphere::set_generator_radius(guiutil::slider_float(ui, "Generator Radius", atmosphere::generator_radius(), (1.0, 1.2), 1.0));
                    atmosphere::set_raleigh_scattering(guiutil::slider_float(ui, "Raleigh Scattering", atmosphere::raleigh_scattering(), (0.1, 10.0), 2.0));
                    atmosphere::set_raleigh_height(guiutil::slider_float(ui, "Raleigh Height", atmosphere::raleigh_height(), (0.0, 10.0), 2.0));
                    atmosphere::set_mie_scattering(guiutil::slider_float(ui, "Mie Scattering", atmosphere::mie_scattering(), (0.1, 10.0), 2.0));
                    atmosphere::set_mie_height(guiutil::slider_float(ui, "Mie Height", atmosphere::mie_height(), (0.0, 10.0), 2.0));
                    let mut half_precision = atmosphere::half_precision();
                    ui.checkbox(im_str!("Half-Precision"), &mut half_precision);
                    atmosphere::set_half_precision(half_precision);

                    if atmosphere::is_dirty() {
                        atmosphere::recreate();
                        self.postprocess = create_postprocess_shader();
                    }
                }

                // Sun Settings
                if ui.collapsing_header(im_str!("Sun")).default_open(false).build() {
                    self.sun_speed = guiutil::slider_float(ui, "Rotation:", self.sun_speed, (-90.0, 90.0), 2.0);
                    self.sun_lon = guiutil::slider_float(ui, "Longitude:", self.sun_lon, (0.0, 360.0), 1.0);
                    self.sun_lat = guiutil::slider_float(ui, "Latitude:", self.sun_lat, (-45.0, 45.0), 1.0);
                }

                // shadow map settings
                if ui.collapsing_header(im_str!("Shadow Mapping")).default_open(false).build() {
                    self.shadows.options(ui);
                }
            });

        let planet_opt_win_size = (260.0, 390.0 + 24.0 * self.select_channels.len() as f32);

        ui.window(im_str!("Planet Options"))
            .flags(ImGuiWindowFlags::NoResize | ImGuiWindowFlags::NoMove | ImGuiWindowFlags::NoSavedSettings | ImGuiWindowFlags::NoScrollbar)
            .size(planet_opt_win_size, ImGuiCond::Always)
            .position((self.windowsize.0 as f32 - planet_opt_win_size.0, 0.0), ImGuiCond::Always)
            .build(|| {
                //
                // Slider for Plate Size and Planet Radius
                //
                let mut depth = self.renderer.depth();
                let platesz = 2i32.pow(self.renderer.depth() as u32);
                if ui.slider_int(im_str!("Plate Size##platesizeslider"), &mut depth, 3, 7)
                        .display_format(im_str!("%.0f ({}x{})", platesz, platesz))
                        .build() {
                    self.renderer.set_depth(depth);
                }

                let mut radius = self.renderer.radius();
                if ui.slider_float(im_str!("Radius##radiusslider"), &mut radius, 50.0, 1000.0).build() {
                    self.renderer.set_radius(radius);
                }

                ui.spacing(); ui.separator();

                //
                // Button for Saving / Restoring
                //
                ui.spacing(); ui.spacing(); ui.same_line(planet_opt_win_size.0 / 2.0 - 80.0);
                if ui.button(im_str!("Save##planet"), (160.0, 20.0)) {
                    #[cfg(target_os = "emscripten")] fileload::download("planet.json", &self.save_state());
                }
                ui.spacing(); ui.spacing(); ui.same_line(planet_opt_win_size.0 / 2.0 - 80.0);
                let curr_pos = ui.get_cursor_screen_pos();
                #[cfg(target_os = "emscripten")] emscripten_util::set_overlay_position(HTML_INPUT_PLANET, curr_pos, (160.0, 20.0));
                ui.button(im_str!("Load##planet"), (160.0, 20.0));
                ui.spacing(); ui.separator();

                // check for uploads
                #[cfg(target_os = "emscripten")] {
                    if let Some(state_data) = fileload::get_result(HTML_INPUT_PLANET) {
                        self.restore_state(&state_data.1);
                    }
                }

                //
                // Button to show/hide Shader Generator Windows
                //
                ui.spacing(); ui.spacing(); ui.same_line(planet_opt_win_size.0 / 2.0 - 80.0);
                self.edit_generator.toggle_button(ui, (160.0, 20.0));
                ui.spacing(); ui.spacing(); ui.same_line(planet_opt_win_size.0 / 2.0 - 80.0);
                self.edit_colorator.toggle_button(ui, (160.0, 20.0));
                ui.spacing(); ui.separator();

                //
                // Buttons to add/remove channels
                //
                ui.spacing(); ui.spacing(); ui.same_line(planet_opt_win_size.0 / 2.0 - 80.0);
                if ui.button(im_str!("Add Channel"), (120.0, 20.0)) {
                    let len = self.select_channels.len();
                    self.select_channels.push((format!("channel{}", len).to_string(), 0));
                    self.channels_changed();
                }
                ui.spacing();

                //
                // List of channels
                //
                ui.columns(3, im_str!("Channels"), true);
                ui.set_column_width(0, 125.0);
                ui.set_column_width(1, 90.0);
                ui.set_column_width(2, 25.0);
                ui.separator();
                ui.text("Name"); ui.next_column();
                ui.text("Size"); ui.next_column();
                ui.next_column();
                ui.separator();

                let mut remove = None;
                let mut changed = false;
                for chan in self.select_channels.iter_mut().enumerate() {
                    if guiutil::textinput(ui, &format!("##channame{}", chan.0), &mut (chan.1).0, 16, true) {
                        changed = true;
                    }
                    ui.next_column();

                    let items = vec!(im_str!("1"), im_str!("2"), im_str!("3"), im_str!("4"));
                    ui.push_item_width(-1.0);
                    if ui.combo(im_str!("##chantype{}", chan.0), &mut (chan.1).1, &items[..], 4) {
                        changed = true;
                    }
                    ui.pop_item_width();
                    ui.next_column();

                    if ui.button(im_str!("X##chandelete{}", chan.0), (20.0, 20.0)) {
                        remove = Some(chan.0);
                    }
                    ui.next_column();
                }

                if changed {
                    self.channels_changed();
                }

                if let Some(remove) = remove {
                    self.select_channels.remove(remove);
                    self.channels_changed();
                }
                ui.columns(1, im_str!("Channels"), true);
                ui.separator();

                //
                // Add new texture button
                //
                ui.spacing(); ui.spacing(); ui.same_line(planet_opt_win_size.0 / 2.0 - 80.0);
                #[cfg(target_os = "emscripten")] emscripten_util::set_overlay_position(HTML_INPUT_TEXTURE, ui.get_cursor_screen_pos(), (160.0, 20.0));
                ui.button(im_str!("Add Texture"), (120.0, 20.0));
                // start new uploads?
                #[cfg(target_os = "emscripten")] {
                    if let Some(tex_data) =  fileload::get_result(HTML_INPUT_TEXTURE) {
                        self.texuploads.push(imgdecode::start(tex_data.1));
                    }
                    // start new image parsing?
                    if let Some(texupload) = self.texuploads.pop() {
                        if let Some(texdata) = imgdecode::get(texupload) {
                            let texname = String::from("newTexture");
                            self.renderer.add_texture(&texname, tinygl::Texture::from_data_2d(&texdata.1, texdata.0));
                            self.active_textures.push((texname, texdata.0, texdata.1));
                        } else {
                            self.texuploads.push(texupload);
                        }
                    }
                }

                //
                // List of textures
                //
                ui.spacing();
                ui.columns(3, im_str!("Textures"), true);
                // 1: image, 2: name, nextline: size, 3: X
                let maximgsz = 100.0;
                ui.set_column_width(0, 110.0);
                ui.set_column_width(1, 105.0);
                ui.set_column_width(2, 25.0);
                ui.separator();

                let mut change = None;
                {
                    for tex in self.renderer.textures().iter().enumerate() {
                        let sz = (tex.1).1.size().unwrap();
                        let maxsz = sz.0.max(sz.1) as f32;
                        let imgsz = (maximgsz * sz.0 as f32 / maxsz, maximgsz * sz.1 as f32 / maxsz);

                        ui.image((tex.1).1.handle() as _, imgsz);
                        ui.next_column();

                        let ofs = (0.5 * (imgsz.1 - 50.0)).max(0.0);
                        ui.dummy((1.0, ofs));
                        let mut texname = (tex.1).0.clone();
                        if guiutil::textinput(ui, &format!("##texname{}", tex.0), &mut texname, 16, true) {
                            change = Some((tex.0, Some(texname)));
                        }
                        ui.dummy((1.0, 2.0));
                        let szstr = format!("{}x{}", sz.0, sz.1);
                        ui.text(szstr);
                        ui.next_column();

                        let ofs = (0.5 * (imgsz.1 - 15.0)).max(0.0);
                        ui.dummy((1.0, ofs));
                        if ui.button(im_str!("X##texdelete{}", tex.0), (20.0, 20.0)) {
                            change = Some((tex.0, None));
                        }
                        ui.next_column();
                    }
                }
                if let Some(change) = change {
                    if let Some(new_name) = change.1 {
                        self.active_textures[change.0].0 = new_name.clone();
                        self.renderer.rename_texture(change.0 as _, &new_name);
                    } else {
                        self.active_textures.remove(change.0 as _);
                        self.renderer.remove_texture(change.0 as _);
                    }
                }
            });

        #[cfg(target_os = "emscripten")]
            {
//                if !self.edit_js.is_open() { self.edit_js.toggle()  }
                if self.edit_js.render(ui, None, keymod) {
                    emscripten_util::run_javascript(&self.edit_js.to_str());
                }
            }

        if self.edit_generator.render(ui, self.renderer.errors_generator(), keymod) {
            self.channels_changed();
            if self.renderer.set_generator(&self.edit_generator.to_str()) {
                self.edit_generator.works();
            }
        }

        if self.edit_colorator.render(ui, self.renderer.errors_colorator(), keymod) {
            if self.renderer.set_colorator(&self.edit_colorator.to_str()) {
                self.edit_colorator.works();
            }
        }

        //
        // show Errors Pop-ups
        //
        let err = self.errors.first().map(|strref| strref.to_string());
        if let Some(err) = err {
            if guiutil::error_popup(ui, &err, self.windowsize) {
                self.errors.remove(0);
            }
        }
    }

    fn event(&mut self, event: &Event) {
        match event {
            Event::MouseButtonDown{mouse_btn, x, y, ..} => {
                if *mouse_btn == MouseButton::Left {
                    self.current_mouse_press = Some((*x, *y));
                }
            },
            Event::MouseButtonUp{mouse_btn, ..} => {
                if *mouse_btn == MouseButton::Left {
                    self.current_mouse_press = None;
                }
            },
            Event::MouseMotion{x, y, ..} => {
                self.current_mouse_press = self.current_mouse_press.map(|old| {
                    let sz = self.windowsize.0.min(self.windowsize.1) as f32;
                    self.renderer.camera().pan((*x - old.0) as f32 / sz, (old.1 - *y) as f32 / sz);
                    (*x, *y)
                });
            }
            Event::KeyDown{keycode, .. } => {
                if let Some(keycode) = keycode {
                    self.keyboard.insert(*keycode, true);
                }
            },
            Event::KeyUp{keycode, .. } => {
                if let Some(keycode) = keycode {
                    self.keyboard.insert(*keycode, false);
                }
            },
            _ => {}
        }
    }
}

fn main() {
    webrunner::AppRunner::<MyApp>::start("foo bar");
}
