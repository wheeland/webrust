extern crate sdl2;
extern crate gl;

#[cfg(target_os = "emscripten")]
extern crate emscripten_sys;

use std::rc::Rc;
use std::os::raw::{c_void};

use super::imgui_renderer::Renderer;

pub trait WebApp {
    fn new(size: (u32, u32)) -> Self;
    fn resize(&mut self, size: (u32, u32));
    fn render(&mut self, dt: f32);
    fn event(&mut self, event: &sdl2::event::Event);
    fn do_ui(&mut self, ui: &imgui::Ui, keymod: sdl2::keyboard::Mod);
}

pub struct AppRunner<T> {
    window: sdl2::video::Window,
    gl_ctx: sdl2::video::GLContext,

    events: sdl2::EventPump,
    keymod: sdl2::keyboard::Mod,
    last_frame: Option<std::time::SystemTime>,
    private: Option<T>,

    imgui: imgui::ImGui,
    imgui_sdl2: imgui_sdl2::ImguiSdl2,
    imgui_renderer: Renderer
}

pub fn run_javascript(code: &str) {
    #[cfg(target_os = "emscripten")]
        {
            let code = code.replace("\n", " ");
            let code = code.replace("\"", "\\\"");
            let code = String::from("try { eval(\"") + &code + "\"); } catch (error) { console.log('Error: ' + error); }";
            let code = std::ffi::CString::new(code).unwrap();

            unsafe { emscripten_sys::emscripten_run_script_string(code.as_ptr()); }
        }
}

#[cfg(target_os = "emscripten")]
unsafe extern fn loop_wrapper<T: WebApp>(arg: *mut c_void) {
    let runner = arg as *mut AppRunner<T>;
    (*runner).frame();
}

impl<T: WebApp> AppRunner<T> {
    pub fn start(name: &str) {
        let ctx = sdl2::init().unwrap();
        let video_ctx = ctx.video().unwrap();

        #[cfg(not(target_os = "emscripten"))]
        {
            video_ctx.gl_attr().set_context_profile(sdl2::video::GLProfile::GLES);
            video_ctx.gl_attr().set_context_version(3, 0);
        }

        let window = video_ctx.window(name, 1280, 800)
            .position_centered()
            .opengl()
            .resizable()
            .build()
            .unwrap();

        let gl_ctx = window.gl_create_context().ok().expect("No OpenGL context found");
        gl::load_with(|s| video_ctx.gl_get_proc_address(s) as *const c_void);

        let events = ctx.event_pump().unwrap();

        let mut imgui = imgui::ImGui::init();
        imgui.set_ini_filename(None);
        let imgui_sdl2 = imgui_sdl2::ImguiSdl2::new(&mut imgui);
        let imgui_renderer = Renderer::new(&mut imgui);

        let mut runner = Rc::new(AppRunner::<T> {
            gl_ctx,
            window,
            events,
            keymod: sdl2::keyboard::Mod::empty(),
            last_frame: None,
            private: None,
            imgui,
            imgui_sdl2,
            imgui_renderer
        });

        #[cfg(target_os = "emscripten")]
        unsafe {
            emscripten_sys::emscripten_set_main_loop_arg(Some(loop_wrapper::<T>), Rc::into_raw(runner) as *mut c_void, 0, 1);
        }

        #[cfg(not(target_os = "emscripten"))]
        loop {
            Rc::get_mut(&mut runner).unwrap().frame();
        };
    }

    fn frame(&mut self) {
        let now = std::time::SystemTime::now();

        // first init?
        let dt = match self.last_frame.take() {
            None => {
                self.private = Some(T::new(self.window.size()));
                self.private.as_mut().unwrap().resize(self.window.size());
                0.0
            },
            Some(last) => {
                match now.duration_since(last).ok() {
                    None => { 0.0 },
                    Some(dur) =>{
                        let secs = dur.as_secs() as f32;
                        let nanos = (dur.subsec_nanos() as f32) / 1000000000.0;
                        secs + nanos
                    }
                }
            }
        };

        self.last_frame = Some(now);
        let private = self.private.as_mut().unwrap();

        for event in self.events.poll_iter() {
            // Update keymod
            if let sdl2::event::Event::KeyDown{keymod,..} = &event {
                self.keymod = *keymod;
            }
            if let sdl2::event::Event::KeyUp{keymod,..} = &event {
                self.keymod = *keymod;
            }

            match event {
                sdl2::event::Event::Quit{..} => {
                    std::process::exit(1);
                }
                sdl2::event::Event::Window{win_event,..} => {
                    if let sdl2::event::WindowEvent::Resized(w, h) = win_event {
                        private.resize((w as u32, h as u32));
                    }
                }
                _ => {
                    if !self.imgui_sdl2.ignore_event(&event) {
                        private.event(&event);
                    }
                    self.imgui_sdl2.handle_event(&mut self.imgui, &event);
                }
            }
        }

        // Render app
        private.render(dt);

        // Render imgui UI
        let ui = self.imgui_sdl2.frame(&self.window, &mut self.imgui, &self.events, dt);
        private.do_ui(&ui, self.keymod);
        self.imgui_renderer.render(ui);

        self.window.gl_swap_window();
    }
}
