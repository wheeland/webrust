use imgui::*;
use std::ffi::{CStr, CString};

pub fn format_number(n: i32) -> String {
    if n < 1000000 {
        format!("{0:.3}", 0.001 * n as f32)
    } else {
        format!("{0:.3}.{1:03}", 0.000001 * n as f32, (n % 1000))
    }
}

pub fn slider_exp2int(ui: &imgui::Ui, id: &str, value: i32, minmax: (i32, i32)) -> i32 {
    let mut value = value;
    let pow2 = 2i32.pow(value as _);

    ui.text(id);
    let item_width = ui.push_item_width(-1.0);
    ui.slider_config(format!("##{}slider", id), minmax.0, minmax.1)
            .display_format(format!("%.0f ({}x{})", pow2, pow2))
            .build(&mut value);
    item_width.end();
    value
}

pub fn slider_int(ui: &imgui::Ui, id: &str, value: i32, minmax: (i32, i32)) -> i32 {
    let mut value = value;

    ui.text(id);
    let item_width = ui.push_item_width(-1.0);
    ui.slider(format!("##{}slider", id), minmax.0, minmax.1, &mut value);
    value
}

pub fn slider_float(ui: &imgui::Ui, text: &str, value: f32, minmax: (f32, f32), power: f32) -> f32 {
    let mut value = value;
    ui.text(text);
    let item_width = ui.push_item_width(-1.0);
    ui.slider(format!("##{}slider", text), minmax.0, minmax.1, &mut value);
    value
}

pub fn textinput(ui: &imgui::Ui, id: &str, value: &mut String, capacity: usize, fullwidth: bool) -> bool {
    let mut entry = String::with_capacity(capacity);
    entry.push_str(&value);

    let item_width = if fullwidth { Some(ui.push_item_width(-1.0)) } else { None };
    let ret = ui.input_text(format!("{}", id), &mut entry).build();
    if ret {
        *value = entry;
    }
    ret
}

pub fn error_popup(ui: &imgui::Ui, message: &str, windowsize: (u32, u32)) -> bool {
    let sx = 240.0;
    let sy = 120.0;

    let mut string = message.to_string();
    let mut ret = false;

    ui.window(format!("##errorwindow"))
        .title_bar(false)
        .resizable(false)
        .movable(false)
        .save_settings(false)
        .scroll_bar(false)
        .size([sx, sy], Condition::Always)
        .position([0.5 * (windowsize.0 as f32 - sx), 0.5 * (windowsize.1 as f32 - sy)], Condition::Always)
        .build(|| {
            ui.set_cursor_pos([10.0, 10.0]);
            ui.input_text_multiline(format!("##errorwindowtext"), &mut string, [sx - 20.0, sy - 50.0])
                .read_only(true)
                .build();
            ui.set_cursor_pos([0.5 * sx - 50.0, sy - 30.0]);
            ret = ui.button_with_size(format!("Okay.."), [100.0, 20.0]);
        });

    ret
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
    window_fade: f32,

    position: (f32, f32),
    size: (f32, f32),

    working_source: String,             // code that is actually used
    source: String,                   // code in the window
    callback_data: ShaderEditCallbackData,
}

impl ShaderEditData {
    pub fn new(
        window_title: &str,
        src: &str,
        position: (f32, f32),
        size: (f32, f32)
    ) -> Self {
        let mut source = String::with_capacity(64 * 1024);
        source.push_str(src);
        ShaderEditData {
            window_title: window_title.to_string(),
            window_open: false,
            window_active: false,
            window_fade: 0.0,
            position,
            size,
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
        self.window_fade = 1.0;
    }

    pub fn to_str(&self) -> String {
        self.source.clone()
    }

    pub fn get_working(&self) -> String {
        self.working_source.clone()
    }

    pub fn works(&mut self) {
        self.working_source = self.to_str();
    }

    pub fn toggle_button(&mut self, ui: &imgui::Ui, size: (f32, f32)) {
        let action = if self.window_open { "Close" } else {"Open" };
        let label = format!("{} {}##edit_window", action, self.window_title);
        if ui.button_with_size(label, [size.0, size.1]) {
            self.toggle();
        }
    }

    pub fn render(&mut self,
              ui: &imgui::Ui,
              errors: Option<&String>,
              keymod: sdl2::keyboard::Mod
    ) -> bool {
        if !self.window_open && self.window_fade <= 0.0 {
            return false;
        }

        let is_esc_pressed = ui.is_key_pressed(Key::Escape);
        let is_ctrl_pressed = keymod.intersects(sdl2::keyboard::Mod::RCTRLMOD | keymod & sdl2::keyboard::Mod::LCTRLMOD);
        let is_ctrl_return_pressed = ui.is_key_pressed(Key::Enter) && is_ctrl_pressed;

        let mut accepted = self.window_active && is_ctrl_return_pressed;
        let mut window_open = self.window_open;
        let mut do_close_window = self.window_active && is_esc_pressed;

        // if ESC was pressed, take away focus to prevent source code loss by evil imgui!
        if do_close_window {
            self.window_fade = 1.0;
            self.window_open = false;
        }

        // fade in/out the window when opened/closed
        self.window_fade = (self.window_fade - 0.1).max(0.0);
        let alpha = if self.window_open { 1.0 - self.window_fade } else { self.window_fade };
        let ofs = if self.window_open { -50.0 * self.window_fade } else { 50.0 * (1.0 - self.window_fade) };

        self.callback_data.keymod = keymod;

        // ui.set_next_window_pos(self.position);

        let style = ui.push_style_var(imgui::StyleVar::Alpha(alpha * alpha));
        let mut window = ui.window(format!("{}", self.window_title))
            .collapsible(false)
            .save_settings(false)
            .scroll_bar(false)
            .movable(true)
            .position([self.position.0, self.position.1], Condition::Always);
            
        if !self.window_open {
            window = window.no_inputs();
        }

        window
            .size(
                [self.size.0 + ofs, self.size.1 + ofs],
                Condition::Always
            )
            .position(
                [self.position.0 - 0.5 * ofs, self.position.1 - 0.5 * ofs],
                if self.window_fade <= 0.0 { Condition::Appearing } else { Condition::Always }
            )
            .opened(&mut window_open)
            .build(|| {
                let has_error = errors.is_some();
                let errorsz = if has_error { 150.0 } else { 0.0 };
                let winsz = ui.window_size();

                // if Ctrl+Return was pressed, the focus was taken away from this item, but this is not what we want, so GIVE IT BACK ALREADY!
                if accepted {
                    unsafe { imgui::sys::igSetKeyboardFocusHere(0); }
                }

                ui.input_text_multiline(format!("##shadertextinput"), &mut self.source, [winsz[0] - 20.0, winsz[1] - 50.0 - errorsz])
                    // TODO:
                    // port C-style callback to new shiny imgui-rs trait
                    .build();
                self.window_active = ui.is_item_active();

                if has_error {
                    let mut error_str = errors.unwrap_or(&String::new()).clone();
                    ui.input_text_multiline(format!("##shadertexterror"), &mut error_str, [winsz[0] - 20.0, errorsz])
                        .flags(InputTextFlags::READ_ONLY)
                        .build();
                }

                ui.new_line();
                ui.same_line_with_pos(50.0);
                if ui.button_with_size(format!("Apply (Ctrl+Return)"), [140.0, 20.0]) {
                    accepted = true;
                }

                ui.same_line_with_pos(winsz[0] / 2.0 - 50.0);
                if ui.button_with_size(format!("Reset##shaderedit"), [100.0, 20.0]) {
                    self.reset();
                }

                ui.same_line_with_pos(winsz[0] - 150.0);
                if ui.button_with_size(format!("Close (Esc)"), [140.0, 20.0]) {
                    do_close_window = true;
                }

                if self.window_fade <= 0.0 {
                    let size = ui.window_size();
                    let position = ui.window_pos();
                    self.size = (size[0], size[1]);
                    self.position = (position[0], position[1]);
                }
            });

        self.window_open = window_open && !do_close_window;
        accepted
    }
}

extern "C" fn callback(data: *mut imgui::sys::ImGuiInputTextCallbackData) -> std::os::raw::c_int {
    let mut ret = 0;

    unsafe {
        let cbdata = &*data;
        let shaderdata = &mut *(cbdata.UserData as *mut ShaderEditCallbackData);

        // tab -> insert 1-4 spaces
        // return -> insert \n and N spaces
        // backspace -> maybe delete N spaces

        //
        // if we have access to the buffer, find out current line/cursor + number of leading spaces
        //
        if cbdata.Buf != 0 as _ {
            let buf = CStr::from_ptr(cbdata.Buf);
            let mut lineno = 0;
            let mut cursor = 0;
            let mut spaces_leading = 0;
            let mut spaces_curr = 0;
            let mut spaces_still = true;
            for ch in buf.to_bytes().iter().enumerate() {
                let idx = ch.0;
                let ch = *ch.1;

                if idx == cbdata.CursorPos as usize {
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
            if cbdata.CursorPos == buf.to_bytes().len() as i32 {
                shaderdata.pos_char = cursor;
                shaderdata.pos_line = lineno;
                shaderdata.spaces_curr = spaces_curr;
                shaderdata.spaces_leading = spaces_leading;
            }
        }

        shaderdata.spaces_leading = shaderdata.spaces_leading.min(shaderdata.pos_char);

//        println!("line {}, pos {}: lead={}, curr={}", shaderdata.pos_line, shaderdata.pos_char, shaderdata.spaces_leading, shaderdata.spaces_curr);

        // Tab?
        if cbdata.EventFlag & imgui::sys::ImGuiInputTextFlags_CallbackCompletion as i32 != 0 {
            let shift = shaderdata.keymod.intersects(sdl2::keyboard::Mod::LSHIFTMOD | sdl2::keyboard::Mod::RSHIFTMOD);

            // Shift+Tab -> remove spaces before cursor
            if shift && shaderdata.spaces_curr > 0 {
                let remove: std::os::raw::c_int = shaderdata.spaces_curr.min((shaderdata.pos_char + 3) % 4 + 1) as _;
                imgui::sys::ImGuiInputTextCallbackData_DeleteChars(data, cbdata.CursorPos - remove, remove as _);
            }
            // Shift -> insert 1-4 spaces
            if !shift {
                let spaces = 4 - shaderdata.pos_char % 4;
                let insert = &CString::new(String::from(" ").repeat(spaces).as_str().as_bytes()).unwrap();
                imgui::sys::ImGuiInputTextCallbackData_InsertChars(data, cbdata.CursorPos, insert.as_ptr(), insert.as_ptr().offset(spaces as _));
            }
        }
        // Return?
        else if cbdata.EventFlag & imgui::sys::ImGuiInputTextFlags_CallbackCharFilter as i32 != 0 {
            if cbdata.EventChar == '\n' as imgui::sys::ImWchar {
                shaderdata.insert = Some(String::from("\n") + String::from(" ").repeat(shaderdata.spaces_leading).as_str());
                ret = 1;
            }
        }
        // Have to insert new-line leading spaces from last iteration? (Jesus....)
        else if let Some(insert) = shaderdata.insert.take() {
            let cstr = &CString::new(insert.as_bytes()).unwrap();
            imgui::sys::ImGuiInputTextCallbackData_InsertChars(data, cbdata.CursorPos, cstr.as_ptr(), cstr.as_ptr().offset(insert.len() as _));
        }
    }

    ret
}
