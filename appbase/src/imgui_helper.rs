pub fn staticwindow<'ui, 'p, F: FnOnce(), C: Into<imgui::ImVec4>+Copy>(
    ui: &'ui imgui::Ui,
    name: &'p imgui::ImStr,
    pos: (f32, f32),
    size: (f32, f32),
    font_scale: f32,
    color: C,
    f: F)
{
    ui.with_color_var(imgui::ImGuiCol::WindowBg, color, || {
        ui.window(name)
            .position(pos, imgui::ImGuiCond::Always)
            .size(size, imgui::ImGuiCond::Always)
            .title_bar(false)
            .movable(false)
            .resizable(false)
            .collapsible(false)
            .font_scale(font_scale)
            .build(f);
    });
}
