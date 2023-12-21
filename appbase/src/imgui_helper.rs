pub fn staticwindow<'ui, F: FnOnce()> (
    ui: &'ui imgui::Ui,
    name: &str,
    pos: (f32, f32),
    size: (f32, f32),
    font_scale: f32,
    color: (f32, f32, f32, f32),
    f: F)
{
    // ui.with_color_var(imgui::ImGuiCol::WindowBg, color, || {
        ui.window(name)
            .position([pos.0, pos.1], imgui::Condition::Always)
            .size([size.0, size.1], imgui::Condition::Always)
            .title_bar(false)
            .movable(false)
            .resizable(false)
            .collapsible(false)
            .build(f);
    // });
}
