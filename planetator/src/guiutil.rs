use imgui::*;
use std::ffi::{CStr, CString};

pub fn format_number(n: i32) -> String {
    if n < 1000000 {
        format!("{0:.3}", 0.001 * n as f32)
    } else {
        format!("{0:.3}.{1:03}", 0.000001 * n as f32, (n % 1000))
    }
}

struct ShaderEditCallbackData {
    keymod: sdl2::keyboard::Mod,
    pos_line: usize,
    pos_char: usize,
    spaces_leading: usize,
    spaces_curr: usize,
    insert: Option<String>,
}

pub struct ShaderEditData {
    window_title: String,
    window_open: bool,
    window_active: bool,

    working_source: String,             // code that is actually used
    source: ImString,                   // code in the window
    callback_data: ShaderEditCallbackData,
}

impl ShaderEditData {
    pub fn new(window_title: &str, src: &str) -> Self {
        let mut source = ImString::with_capacity(64 * 1024);
        source.push_str(src);
        ShaderEditData {
            window_title: window_title.to_string(),
            window_open: false,
            window_active: false,
            source,
            working_source: src.to_string(),
            callback_data: ShaderEditCallbackData {
                keymod: sdl2::keyboard::Mod::empty(),
                pos_line: 0,
                pos_char: 0,
                spaces_leading: 0,
                spaces_curr: 0,
                insert: None,
            }
        }
    }

    pub fn set_source(&mut self, source: &str) {
        self.source.clear();
        self.source.push_str(source);
    }

    pub fn reset(&mut self) {
        self.source.clear();
        self.source.push_str(&self.working_source);
    }

    pub fn is_open(&self) -> bool {
        self.window_open
    }

    pub fn toggle(&mut self) {
        self.window_open = !self.window_open;
    }

    pub fn to_str(&self) -> String {
        self.source.to_str().to_string()
    }

    pub fn get_working(&self) -> String {
        self.working_source.clone()
    }

    pub fn works(&mut self) {
        self.working_source = self.to_str();
    }

    pub fn toggle_button(&mut self, ui: &imgui::Ui, size: (f32, f32)) {
        if ui.button(im_str!("{} {}##edit_window",
                             if self.window_open { "Close" } else {"Open" },
                             self.window_title),
                     size
        ) {
            self.toggle();
        }
    }

    pub fn render(&mut self,
              ui: &imgui::Ui,
              errors: Option<&String>,
              position: (f32, f32),
              size: (f32, f32),
              keymod: sdl2::keyboard::Mod
    ) -> bool {
        if !self.window_open {
            return false;
        }

        let is_esc_pressed = ui.imgui().is_key_pressed(sdl2::keyboard::Scancode::Escape as usize);
        let is_ctrl_pressed = keymod.intersects(sdl2::keyboard::Mod::RCTRLMOD | keymod & sdl2::keyboard::Mod::LCTRLMOD);
        let is_ctrl_return_pressed = ui.imgui().is_key_pressed(sdl2::keyboard::Scancode::Return as usize) && is_ctrl_pressed;

        let mut accepted = self.window_active && is_ctrl_return_pressed;
        let mut window_open = self.window_open;
        let mut do_close_window=  self.window_active && is_esc_pressed;

        // if ESC was pressed, don't execute any of this to prevent source code loss by evil imgui!
        if do_close_window {
            self.window_open = false;
            return false
        }

        self.callback_data.keymod = keymod;

        ui.window(im_str!("{}", self.window_title))
            .flags(ImGuiWindowFlags::NoCollapse | ImGuiWindowFlags::NoSavedSettings | ImGuiWindowFlags::NoScrollbar)
            .size(size, ImGuiCond::Appearing)
            .opened(&mut window_open)
            .position(position, ImGuiCond::Appearing)
            .build(|| {
                let has_error = errors.is_some();
                let errorsz = if has_error { 150.0 } else { 0.0 };
                let winsz = ui.get_window_size();

                // if Ctrl+Return was pressed, the focus was taken away from this item, but this is not what we want, so GIVE IT BACK ALREADY!
                if accepted {
                    unsafe { imgui::sys::igSetKeyboardFocusHere(0); }
                }

                ui.input_text_multiline(im_str!("##shadertextinput"), &mut self.source, (winsz.0 - 20.0, winsz.1 - 50.0 - errorsz))
                    .callback_completion(true)
                    .callback_char_filter(true)
                    .callback_always(true)
                    .callback(Some(callback), &mut self.callback_data as _)
                    .build();
                self.window_active = ui.is_item_active();

                if has_error {
                    let mut error_str = ImString::new(errors.unwrap().to_string());
                    ui.input_text_multiline(im_str!("##shadertexterror"), &mut error_str, (winsz.0 - 20.0, errorsz))
                        .flags(ImGuiInputTextFlags::ReadOnly)
                        .build();
                }

                ui.new_line();
                ui.same_line(50.0);
                if ui.button(im_str!("Apply (Ctrl+Return)"), (140.0, 20.0)) {
                    accepted = true;
                }

                ui.same_line(winsz.0 / 2.0 - 50.0);
                if ui.button(im_str!("Reset##shaderedit"), (100.0, 20.0)) {
                    self.reset();
                }

                ui.same_line(winsz.0 - 150.0);
                if ui.button(im_str!("Close (Esc)"), (140.0, 20.0)) {
                    do_close_window = true;
                }
            });

        self.window_open = window_open && !do_close_window;
        accepted
    }
}

extern "C" fn callback(data: *mut imgui::sys::ImGuiTextEditCallbackData) -> std::os::raw::c_int {
    let mut ret = 0;

    unsafe {
        let cbdata = &*data;
        let shaderdata = &mut *(cbdata.user_data as *mut ShaderEditCallbackData);

        // tab -> insert 1-4 spaces
        // return -> insert \n and N spaces
        // backspace -> maybe delete N spaces

        //
        // if we have access to the buffer, find out current line/cursor + number of leading spaces
        //
        if cbdata.buf != 0 as _ {
            let buf = CStr::from_ptr(cbdata.buf);
            let mut lineno = 0;
            let mut cursor = 0;
            let mut spaces_leading = 0;
            let mut spaces_curr = 0;
            let mut spaces_still = true;
            for ch in buf.to_bytes().iter().enumerate() {
                let idx = ch.0;
                let ch = *ch.1;

                if idx == cbdata.cursor_pos as usize {
                    shaderdata.pos_char = cursor;
                    shaderdata.pos_line = lineno;
                    shaderdata.spaces_curr = spaces_curr;
                }

                let is_space = ch == ' ' as u8;
                if is_space {
                    if spaces_still {
                        spaces_leading += 1;
                        if lineno == shaderdata.pos_line {
                            shaderdata.spaces_leading = spaces_leading;
                        }
                    }
                    spaces_curr += 1;
                }
                else {
                    spaces_still = false;
                    spaces_curr = 0;
                }
                if ch == '\n' as u8 {
                    cursor = 0;
                    lineno += 1;
                    spaces_still = true;
                    spaces_leading = 0;
                } else {
                    cursor += 1;
                }
            }
            if cbdata.cursor_pos == buf.to_bytes().len() as i32 {
                shaderdata.pos_char = cursor;
                shaderdata.pos_line = lineno;
                shaderdata.spaces_curr = spaces_curr;
                shaderdata.spaces_leading = spaces_leading;
            }
        }

        shaderdata.spaces_leading = shaderdata.spaces_leading.min(shaderdata.pos_char);

//        println!("line {}, pos {}: lead={}, curr={}", shaderdata.pos_line, shaderdata.pos_char, shaderdata.spaces_leading, shaderdata.spaces_curr);

        // Tab?
        if cbdata.event_flag.contains(imgui::ImGuiInputTextFlags::CallbackCompletion) {
            let shift = shaderdata.keymod.intersects(sdl2::keyboard::Mod::LSHIFTMOD | sdl2::keyboard::Mod::RSHIFTMOD);

            // Shift+Tab -> remove spaces before cursor
            if shift && shaderdata.spaces_curr > 0 {
                let remove: std::os::raw::c_int = shaderdata.spaces_curr.min((shaderdata.pos_char + 3) % 4 + 1) as _;
                imgui::sys::ImGuiTextEditCallbackData_DeleteChars(data, cbdata.cursor_pos - remove, remove as _);
            }
            // Shift -> insert 1-4 spaces
            if !shift {
                let spaces = 4 - shaderdata.pos_char % 4;
                let insert = &CString::new(String::from(" ").repeat(spaces).as_str().as_bytes()).unwrap();
                imgui::sys::ImGuiTextEditCallbackData_InsertChars(data, cbdata.cursor_pos, insert.as_ptr(), insert.as_ptr().offset(spaces as _));
            }
        }
        // Return?
        else if cbdata.event_flag.contains(imgui::ImGuiInputTextFlags::CallbackCharFilter) {
            if cbdata.event_char == '\n' as imgui::sys::ImWchar {
                shaderdata.insert = Some(String::from("\n") + String::from(" ").repeat(shaderdata.spaces_leading).as_str());
                ret = 1;
            }
        }
        // Have to insert new-line leading spaces from last iteration? (Jesus....)
        else if let Some(insert) = shaderdata.insert.take() {
            let cstr = &CString::new(insert.as_bytes()).unwrap();
            imgui::sys::ImGuiTextEditCallbackData_InsertChars(data, cbdata.cursor_pos, cstr.as_ptr(), cstr.as_ptr().offset(insert.len() as _));
        }
    }

    ret
}
