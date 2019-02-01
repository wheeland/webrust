extern crate gl;
extern crate tinygl;
extern crate sdl2;
extern crate imgui;
extern crate cgmath;
extern crate lru_cache;
extern crate appbase;
extern crate array2d;

extern crate serde;
extern crate serde_json;
#[macro_use] extern crate serde_derive;

use appbase::webrunner;
use appbase::imgui_renderer;

mod earth;
mod culling;
mod guiutil;
mod fileloading_web;
mod atmosphere;

use std::collections::HashMap;
use std::io::Read;
use sdl2::keyboard::Keycode;
use sdl2::mouse::MouseButton;
use sdl2::event::{Event};
use imgui::*;
use appbase::fpswidget::FpsWidget;
use cgmath::SquareMatrix;

struct MyApp {
    windowsize: (u32, u32),

    keyboard: HashMap<Keycode, bool>,
    current_mouse_press: Option<(i32, i32)>,
    savegame: Option<(String, String)>,

    fps: FpsWidget,

    edit_generator: guiutil::ShaderEditData,
    edit_colorator: guiutil::ShaderEditData,
    edit_js: guiutil::ShaderEditData,
    select_channels: Vec<(String, i32)>,

    renderer: earth::renderer::Renderer,
    atmosphere: atmosphere::Atmosphere,
    postprocess: tinygl::Program,

    fsquad: tinygl::shapes::FullscreenQuad,
}

#[derive(Serialize)]
#[derive(Deserialize)]
struct Serialized {
    generator: String,
    colorator: String,
    select_channels: Vec<(String, i32)>,
}

impl MyApp {
    fn pressed(&self, key: Keycode) -> bool {
        *self.keyboard.get(&key).unwrap_or(&false)
    }

    fn channels_changed(&mut self) {
        let new_channels = earth::Channels::new(&self.select_channels);

        if self.renderer.set_channels(&new_channels, &self.edit_generator.to_str()) {
            // also update the colorator, because it might have been already adapted to the new channels
            if self.renderer.set_colorator(&self.edit_colorator.to_str()) {
                self.edit_colorator.works();
            }

            self.edit_generator.works();
        }
    }

    fn save_state(&self) -> String {
        serde_json::to_string(&Serialized {
            generator: self.edit_generator.to_str().to_string(),
            colorator: self.edit_colorator.to_str().to_string(),
            select_channels: self.select_channels.clone(),
        }).unwrap()
    }

    fn restore_state(&mut self, serialized: &str) {
        let deser = serde_json::from_str::<Serialized>(serialized);

        if let Ok(deser) = deser {
            self.edit_generator.set_source(&deser.generator);
            self.edit_colorator.set_source(&deser.colorator);
            self.select_channels = deser.select_channels.clone();

            self.edit_generator.works();
            self.edit_colorator.works();

            let new_channels = earth::Channels::new(&self.select_channels);
            self.renderer.set_generator_and_channels(&self.edit_generator.to_str(), &new_channels);
            self.renderer.set_colorator(&self.edit_colorator.to_str());
        }
    }
}

impl webrunner::WebApp for MyApp {
    fn new(windowsize: (u32, u32)) -> Self {
        let mut savegame = None;

        // check for loading savegame files
        #[cfg(not(target_os = "emscripten"))]
            {
                // check for default savegame in 'planet.json'
                savegame = std::fs::read_to_string("planet.json")
                    .ok().map(|data| (String::from("planet.json"), data));

                // check if a savegame was passed through the command line
                for arg in std::env::args().enumerate() {
                    if arg.0 > 0 {
                        if let Ok(s) = std::fs::read_to_string(&arg.1) {
                            savegame = Some((arg.1, s));
                        }
                    }
                }
            }
        fileloading_web::start_upload("state");

        MyApp {
            windowsize,
            keyboard: HashMap::new(),
            savegame,
            current_mouse_press: None,
            fps: FpsWidget::new(150),
            edit_generator: guiutil::ShaderEditData::new("Generator", &earth::renderer::default_generator()),
            edit_colorator: guiutil::ShaderEditData::new("Kolorator", &earth::renderer::default_colorator()),
            edit_js: guiutil::ShaderEditData::new("JavaScript executor", "var elem = document.getElementById('state');"),
            select_channels: Vec::new(),
            renderer: earth::renderer::Renderer::new(),
            atmosphere: atmosphere::Atmosphere::new(),
            postprocess: tinygl::Program::new_versioned("
                in vec2 vertex;
                out vec2 clipPos;
                void main() {
                    clipPos = vertex;
                    gl_Position = vec4(vertex, 0.0, 1.0);
                }
                ", &(String::from("
                uniform vec3 eyePosition;
                uniform vec3 sunDirection;
                uniform mat4 inverseViewProjectionMatrix;
                uniform sampler2D planetColor;
                uniform sampler2D planetNormal;
                uniform sampler2D planetPosition;
                ")
                + &atmosphere::Atmosphere::shader_source() +
                "
                in vec2 clipPos;
                out vec4 outColor;

                void main() {
                    vec3 normalFromTex = texture(planetNormal, vec2(0.5) + 0.5 * clipPos).rgb;

                    // calculate eye direction in that pixel
                    vec4 globalPosV4 = inverseViewProjectionMatrix * vec4(clipPos, 0.0, 1.0);
                    vec3 globalPos = globalPosV4.xyz / globalPosV4.w;
                    vec3 eyeDir = normalize(globalPos - eyePosition);

                    // get intersection points of view ray with atmosphere
                    float t0, t1;
                    bool hasAtmo = raySphereIntersect(eyePosition, eyeDir, atmosphereRadius, t0, t1);

                    vec3 color = vec3(0.0);

                    if (length(normalFromTex) > 0.0) {
                        vec3 normal = vec3(-1.0) + 2.0 * normalFromTex;
                        vec3 pColor = texture(planetColor, vec2(0.5) + 0.5 * clipPos).rgb;
                        vec3 pPos = texture(planetPosition, vec2(0.5) + 0.5 * clipPos).rgb;

                        // add shadow to planet color
                        float shadow = max(dot(normal, sunDirection), 0.0);
                        pColor *= shadow;

                        // calc atmospheric depth along view ray
                        float dist = length(pPos - eyePosition);
                        color = computeIncidentLight(eyePosition, eyeDir, max(t0, 0.0), dist, sunDirection, pColor);
                    }
                    else {
                        if (hasAtmo && t1 > 0.0) {
                            color = computeIncidentLight(eyePosition, eyeDir, max(t0, 0.0), t1, sunDirection, vec3(0.0));
                        }
                    }

                    outColor = vec4(color, 1.0);
                }
                "), 300),
            fsquad: tinygl::shapes::FullscreenQuad::new(),
        }
    }

    fn resize(&mut self, size: (u32, u32)) {
        self.windowsize = size;
    }

    fn render(&mut self, dt: f32) {
        self.fps.push(dt);

        //
        // check for uploads
        //
        let state_data = fileloading_web::get_result("state");
        if let Some(state_data) = state_data {
            let serialized = unsafe { String::from_utf8_unchecked(state_data.1) };
            self.savegame = Some((state_data.0.clone(), serialized));
        }

        // advance camera
        let cdx = (if self.pressed(Keycode::A) {0.0} else {1.0}) + (if self.pressed(Keycode::D) {0.0} else {-1.0});
        let cdy = (if self.pressed(Keycode::S) {0.0} else {1.0}) + (if self.pressed(Keycode::W) {0.0} else {-1.0});
        let cdz = (if self.pressed(Keycode::LShift) {0.0} else {1.0}) + (if self.pressed(Keycode::Space) {0.0} else {-1.0});
        self.renderer.camera().translate(&(cgmath::Vector3::new(cdx, cdz, cdy) * dt));
        let mvp = self.renderer.camera().mvp(self.windowsize);
        let eye = self.renderer.camera().eye();

        // render planet into FBO
        self.renderer.render(self.windowsize);
        let fbo = self.renderer.fbo().unwrap();

        self.postprocess.bind();
        self.postprocess.uniform("eyePosition", tinygl::Uniform::Vec3(eye));
        self.postprocess.uniform("inverseViewProjectionMatrix", tinygl::Uniform::Mat4(mvp.invert().unwrap()));
        self.postprocess.uniform("sunDirection", tinygl::Uniform::Vec3(cgmath::Vector3::new(1.0, 0.0, 0.0)));
        self.postprocess.uniform("planetColor", tinygl::Uniform::Signed(0));
        self.postprocess.uniform("planetNormal", tinygl::Uniform::Signed(1));
        self.postprocess.uniform("planetPosition", tinygl::Uniform::Signed(2));
        self.postprocess.uniform("planetRadius", tinygl::Uniform::Float(self.renderer.radius()));
        self.atmosphere.prepare_shader(&self.postprocess, self.renderer.radius(), 3);

        unsafe {
            gl::Viewport(0, 0, self.windowsize.0 as _, self.windowsize.1 as _);
            gl::Disable(gl::CULL_FACE);
            gl::Disable(gl::DEPTH_TEST);
            gl::Disable(gl::BLEND);
        }

        fbo.texture("position").unwrap().bind_at(2);
        fbo.texture("normal").unwrap().bind_at(1);
        fbo.texture("colorWf").unwrap().bind_at(0);

        self.fsquad.render(&self.postprocess, "vertex");
    }

    fn do_ui(&mut self, ui: &imgui::Ui, keymod: sdl2::keyboard::Mod) {
        self.fps.render(ui, (0.0, 0.0), (200.0, 80.0));

        ui.window(im_str!("renderstats"))
            .flags(ImGuiWindowFlags::NoResize | ImGuiWindowFlags::NoMove | ImGuiWindowFlags::NoTitleBar | ImGuiWindowFlags::NoSavedSettings | ImGuiWindowFlags::NoScrollbar)
            .size((150.0, 300.0), ImGuiCond::Always)
            .position((0.0, 80.0), ImGuiCond::Always)
            .build(|| {
                ui.text(format!("Plates: {}", self.renderer.rendered_plates()));
                ui.text(format!("Triangles: {}", guiutil::format_number( self.renderer.rendered_triangles() as _)));
                ui.separator();

                ui.checkbox(im_str!("Wireframe"), &mut self.renderer.wireframe);
                ui.checkbox(im_str!("No Update"), &mut self.renderer.no_update_plates);
                ui.checkbox(im_str!("Cull Backside"), &mut self.renderer.hide_backside);

                let slideropt = |text, id, value, min, max| -> f32 {
                    let mut value = value;
                    ui.text(text);
                    ui.push_item_width(-1.0);
                    ui.slider_float(im_str!("##{}", id), &mut value, min, max).build();
                    ui.pop_item_width();
                    value
                };

                let detail = slideropt("Vertex Detail:", "vertexdetail", self.renderer.vertex_detail(), 0.0, 1.0);
                self.renderer.set_vertex_detail(detail);

                //
                // Atmoshpere Settings
                //
                let atmopt = |label, id, max, mut h, mut beta: [f32;3]| {
                    ui.text(label);
                    ui.color_edit(im_str!("#{}color", id), &mut beta)
                        .inputs(false)
                        .label(false)
                        .build();
                    ui.same_line(40.0);
                    ui.push_item_width(-1.0);
                    ui.slider_float(im_str!("#{}slider", id), &mut h, 0.0, max).build();
                    ui.pop_item_width();
                    (h, beta)
                };
                let raleigh = atmopt("Atmosphere Raleigh", "raleigh", 50.0, 1000.0 * self.atmosphere.hr(), self.atmosphere.beta_r().into());
                let mie = atmopt("Atmosphere Mie", "mie", 5.0, 1000.0 * self.atmosphere.hm(), self.atmosphere.beta_m().into());
                self.atmosphere.set_hr(raleigh.0 / 1000.0);
                self.atmosphere.set_hm(mie.0 / 1000.0);
                self.atmosphere.set_beta_r(raleigh.1);
                self.atmosphere.set_beta_m(mie.1);

                if ui.button(im_str!("Load Shite"), (100.0, 20.0)) {
                    fileloading_web::start_upload("state");
                    fileloading_web::download("shit.txt", "Why Hello thar!");
                }
            });

        let planet_opt_win_size = (260.0, 300.0 + 24.0 * self.select_channels.len() as f32);

        ui.window(im_str!("Planet Options"))
            .flags(ImGuiWindowFlags::NoResize | ImGuiWindowFlags::NoMove | ImGuiWindowFlags::NoSavedSettings | ImGuiWindowFlags::NoScrollbar)
            .size(planet_opt_win_size, ImGuiCond::Always)
            .position((self.windowsize.0 as f32 - planet_opt_win_size.0, 0.0), ImGuiCond::Always)
            .build(|| {
                //
                // Slider for Plate Size and Planet Radius
                //
                let mut depth = self.renderer.depth();
                if ui.slider_int(im_str!("Plate Size##platesizeslider"), &mut depth, 3, 7).build() {
                    self.renderer.set_depth(depth);
                }

                let mut radius = self.renderer.radius();
                if ui.slider_float(im_str!("Radius##radiusslider"), &mut radius, 0.2, 10.0).build() {
                    self.renderer.set_radius(radius);
                }

                ui.spacing(); ui.separator();

                //
                // Button for Saving / Restoring
                //
                ui.spacing(); ui.spacing(); ui.same_line(planet_opt_win_size.0 / 2.0 - 80.0);
                if ui.button(im_str!("Save##planet"), (160.0, 20.0)) {
                    fileloading_web::download("planet.json", &self.save_state());
                }
                ui.spacing(); ui.spacing(); ui.same_line(planet_opt_win_size.0 / 2.0 - 80.0);
                if let Some(savegame) = self.savegame.take() {
                    if ui.button(im_str!("Load: {}##planet", savegame.0), (160.0, 20.0)) {
                        self.restore_state(&savegame.1);
                    } else {
                        self.savegame = Some(savegame);
                    }
                } else {
                    ui.button(im_str!("Load##planet"), (160.0, 20.0));
                }
                ui.spacing(); ui.separator();

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
                    ui.push_item_width(-1.0);
                    let mut entry = ImString::with_capacity(16);
                    entry.push_str(&(chan.1).0);
                    if ui.input_text(im_str!("##channame{}", chan.0), &mut entry).build() {
                        changed = true;
                    }
                    (chan.1).0 = entry.to_str().to_string();
                    ui.pop_item_width();
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
            });

        #[cfg(target_os = "emscripten")]
            {
//                if !self.edit_js.is_open() { self.edit_js.toggle()  }
                if self.edit_js.render(ui, None, (250.0, 250.0), (600.0, 400.0), keymod) {
                    webrunner::run_javascript(&self.edit_js.to_str());
                }
            }

        if self.edit_generator.render(ui, self.renderer.errors_generator(), (250.0, 0.0), (600.0, 400.0), keymod) {
            self.channels_changed();
            if self.renderer.set_generator(&self.edit_generator.to_str()) {
                self.edit_generator.works();
            }
        }

        if self.edit_colorator.render(ui, self.renderer.errors_colorator(), (250.0, 250.0), (600.0, 400.0), keymod) {
            if self.renderer.set_colorator(&self.edit_colorator.to_str()) {
                self.edit_colorator.works();
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
