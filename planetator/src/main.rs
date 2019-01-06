extern crate gl;
extern crate sdl2;
extern crate imgui;
extern crate cgmath;
extern crate lru_cache;
extern crate webhelper;

extern crate serde;
extern crate serde_json;
#[macro_use] extern crate serde_derive;

use webhelper::util;
use webhelper::tinygl;
use webhelper::webrunner;
use webhelper::imgui_renderer;

mod earth;
mod culling;
mod guiutil;
mod fileloading_web;

use std::collections::HashMap;
use std::io::Read;
use sdl2::keyboard::Keycode;
use sdl2::mouse::MouseButton;
use sdl2::event::{Event};
use imgui::*;
use util::ValueHistory;

struct MyApp {
    windowsize: (u32, u32),

    keyboard: HashMap<Keycode, bool>,
    current_mouse_press: Option<(i32, i32)>,
    savegame: Option<(String, String)>,

    frame_times: ValueHistory,
    generate_times: ValueHistory,

    edit_generator: guiutil::ShaderEditData,
    edit_colorator: guiutil::ShaderEditData,
    edit_js: guiutil::ShaderEditData,
    select_channels: Vec<(String, i32)>,

    renderer: earth::renderer::Renderer,
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
            frame_times: ValueHistory::new(150),
            generate_times: ValueHistory::new(25),
            edit_generator: guiutil::ShaderEditData::new("Generator", &earth::renderer::default_generator()),
            edit_colorator: guiutil::ShaderEditData::new("Kolorator", &earth::renderer::default_colorator()),
            edit_js: guiutil::ShaderEditData::new("JavaScript executor", "var elem = document.getElementById('state');"),
            select_channels: Vec::new(),
            renderer: earth::renderer::Renderer::new(),
        }
    }

    fn resize(&mut self, size: (u32, u32)) {
        self.windowsize = size;
    }

    fn render(&mut self, dt: f32) {
        self.frame_times.push(dt * 1000.0);

        //
        // check for uploads
        //
        let state_data = fileloading_web::get_result("state");
        if let Some(state_data) = state_data {
            let serialized = unsafe { String::from_utf8_unchecked(state_data.1) };
            self.savegame = Some((state_data.0.clone(), serialized));
        }

        // advance camera
        let cdx = (if self.pressed(Keycode::Left) {0.0} else {1.0}) + (if self.pressed(Keycode::Right) {0.0} else {-1.0});
        let cdy = (if self.pressed(Keycode::Down) {0.0} else {1.0}) + (if self.pressed(Keycode::Up) {0.0} else {-1.0});
        self.renderer.camera().translate(&(cgmath::Vector3::new(cdx, 0.0, cdy) * dt));

        // render planet
        self.renderer.render(self.windowsize);
    }

    fn do_ui(&mut self, ui: &imgui::Ui, keymod: sdl2::keyboard::Mod) {
        guiutil::fps_widget(ui, &self.frame_times, (0.0, 0.0), (200.0, 80.0));

        ui.window(im_str!("renderstats"))
            .flags(ImGuiWindowFlags::NoResize | ImGuiWindowFlags::NoMove | ImGuiWindowFlags::NoTitleBar | ImGuiWindowFlags::NoSavedSettings | ImGuiWindowFlags::NoScrollbar)
            .size((150.0, 190.0), ImGuiCond::Always)
            .position((0.0, 80.0), ImGuiCond::Always)
            .build(|| {
                ui.text(format!("Plates: {}", self.renderer.rendered_plates()));
                ui.text(format!("Triangles: {}", guiutil::format_number( self.renderer.rendered_triangles() as _)));
                ui.separator();

                ui.checkbox(im_str!("Wireframe"), &mut self.renderer.wireframe);
                ui.checkbox(im_str!("No Update"), &mut self.renderer.no_update_plates);
                ui.checkbox(im_str!("Cull Backside"), &mut self.renderer.hide_backside);

                ui.text("Vertex Detail:");
                ui.push_item_width(-1.0);
                let mut detail = self.renderer.vertex_detail();
                if ui.slider_float(im_str!("##pcd"), &mut detail, 0.0, 1.0).build() {
                    self.renderer.set_vertex_detail(detail);
                }
                ui.pop_item_width();

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
